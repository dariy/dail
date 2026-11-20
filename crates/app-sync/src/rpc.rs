//! Newline-delimited JSON-RPC 2.0 over any byte stream: the framing shared by the profile socket
//! ([`crate::ipc`]) and the dail server's TLS connections ([`crate::serve`], [`crate::remote`]).
//!
//! One JSON value per line. A client sends requests and reads the answers in order
//! ([`RpcClient::call`]); after a request that starts a stream (`watch`), the connection carries
//! only notifications (`{"jsonrpc":"2.0","method":…,"params":…}`, [`Notifications`]).

use std::sync::Arc;

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tokio::io::{AsyncBufReadExt as _, AsyncRead, AsyncWrite, AsyncWriteExt as _, BufReader};
use tokio::sync::Mutex;

/// JSON-RPC error codes.
pub(crate) const INVALID_REQUEST: i64 = -32600;
pub(crate) const METHOD_NOT_FOUND: i64 = -32601;
pub(crate) const INVALID_PARAMS: i64 = -32602;
pub(crate) const INTERNAL_ERROR: i64 = -32603;

/// A boxed read half.
pub(crate) type BoxRead = Box<dyn AsyncRead + Unpin + Send>;
/// A boxed write half.
pub(crate) type BoxWrite = Box<dyn AsyncWrite + Unpin + Send>;

/// Splits `stream` into boxed halves.
pub(crate) fn split<S>(stream: S) -> (BoxRead, BoxWrite)
where
    S: AsyncRead + AsyncWrite + Unpin + Send + 'static,
{
    let (read, write) = tokio::io::split(stream);
    (Box::new(read), Box::new(write))
}

/// A request line.
#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct Request {
    pub(crate) jsonrpc: String,
    pub(crate) id: Value,
    pub(crate) method: String,
    #[serde(default)]
    pub(crate) params: Value,
}

/// Reads lines of at most a given length. Cancel-safe: a line read in part when the future is
/// dropped is completed by the next call.
pub(crate) struct LineReader<R> {
    inner: BufReader<R>,
    line: Vec<u8>,
}

impl<R: AsyncRead + Unpin> LineReader<R> {
    pub(crate) fn new(read: R) -> Self {
        Self {
            inner: BufReader::new(read),
            line: Vec::new(),
        }
    }

    /// The next line without its `\n` or `\r\n`; `None` at the end of the stream (a last line
    /// without `\n` is still returned). A line longer than `limit` bytes, or not UTF-8, is an
    /// `InvalidData` error, after which the connection should be closed: the reader is no longer
    /// at the start of a line.
    pub(crate) async fn next_line(&mut self, limit: usize) -> std::io::Result<Option<String>> {
        loop {
            let available = self.inner.fill_buf().await?;
            if available.is_empty() {
                if self.line.is_empty() {
                    return Ok(None);
                }
                break;
            }
            let (take, done) = match available.iter().position(|b| *b == b'\n') {
                Some(at) => (at + 1, true),
                None => (available.len(), false),
            };
            if self.line.len().saturating_add(take) > limit.saturating_add(1) {
                self.line.clear();
                self.inner.consume(take);
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    format!("a line is longer than {limit} bytes"),
                ));
            }
            self.line
                .extend_from_slice(available.get(..take).unwrap_or_default());
            self.inner.consume(take);
            if done {
                break;
            }
        }
        let mut line = std::mem::take(&mut self.line);
        if line.last() == Some(&b'\n') {
            line.pop();
            if line.last() == Some(&b'\r') {
                line.pop();
            }
        }
        if line.len() > limit {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("a line is longer than {limit} bytes"),
            ));
        }
        String::from_utf8(line)
            .map(Some)
            .map_err(|err| std::io::Error::new(std::io::ErrorKind::InvalidData, err))
    }
}

/// Writes `value` as one line.
pub(crate) async fn write_line<W: AsyncWrite + Unpin + ?Sized>(
    send: &mut W,
    value: &Value,
) -> std::io::Result<()> {
    let mut text = value.to_string();
    text.push('\n');
    send.write_all(text.as_bytes()).await?;
    send.flush().await
}

/// A successful answer.
pub(crate) fn answer(id: Value, result: Value) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "result": result })
}

/// An error answer.
pub(crate) fn error(id: Value, code: i64, message: &str) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "error": { "code": code, "message": message } })
}

/// A notification.
pub(crate) fn notification(method: &str, params: Value) -> Value {
    json!({ "jsonrpc": "2.0", "method": method, "params": params })
}

/// Why a call failed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CallError {
    /// The connection is closed (or failed).
    Closed,
    /// The peer answered something that is not a valid answer to the call.
    Protocol(String),
    /// The peer answered with a JSON-RPC error.
    Refused {
        /// The JSON-RPC error code.
        code: i64,
        /// Its message.
        message: String,
    },
}

/// The client side of a connection: requests answered in order, one call at a time. Cheap to
/// clone; clones share the connection.
#[derive(Clone)]
pub struct RpcClient {
    inner: Arc<Mutex<RpcConn>>,
}

struct RpcConn {
    lines: LineReader<BoxRead>,
    send: BoxWrite,
    next_id: u64,
}

impl std::fmt::Debug for RpcClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RpcClient").finish_non_exhaustive()
    }
}

impl RpcClient {
    /// A client over `stream`.
    pub fn new<S>(stream: S) -> Self
    where
        S: AsyncRead + AsyncWrite + Unpin + Send + 'static,
    {
        let (read, send) = split(stream);
        Self {
            inner: Arc::new(Mutex::new(RpcConn {
                lines: LineReader::new(read),
                send,
                next_id: 1,
            })),
        }
    }

    /// Sends one request and waits for its answer's `result`.
    pub async fn call(&self, method: &str, params: Value) -> Result<Value, CallError> {
        let mut conn = self.inner.lock().await;
        let id = conn.next_id;
        conn.next_id += 1;
        let request = json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params });
        write_line(&mut conn.send, &request)
            .await
            .map_err(|_| CallError::Closed)?;
        let line = conn
            .lines
            .next_line(usize::MAX)
            .await
            .map_err(|_| CallError::Closed)?
            .ok_or(CallError::Closed)?;
        let mut response: Value = serde_json::from_str(&line)
            .map_err(|err| CallError::Protocol(format!("bad response: {err}")))?;
        if response["id"] != json!(id) {
            return Err(CallError::Protocol("response out of order".to_owned()));
        }
        if let Some(error) = response.get("error") {
            return Err(CallError::Refused {
                code: error["code"].as_i64().unwrap_or(INTERNAL_ERROR),
                message: error["message"]
                    .as_str()
                    .unwrap_or("unknown error")
                    .to_owned(),
            });
        }
        Ok(response["result"].take())
    }

    /// Calls `method`, which starts a stream, and turns the connection into its
    /// [`Notifications`]. Fails if a clone of this client is still alive.
    pub async fn into_notifications(
        self,
        method: &str,
        params: Value,
    ) -> Result<(Value, Notifications), CallError> {
        let result = self.call(method, params).await?;
        let conn = Arc::try_unwrap(self.inner)
            .map_err(|_| CallError::Protocol("the connection is shared".to_owned()))?
            .into_inner();
        Ok((
            result,
            Notifications {
                lines: conn.lines,
                _send: conn.send,
            },
        ))
    }
}

/// The notifications of a connection after a streaming call. Dropping it closes the connection.
pub struct Notifications {
    lines: LineReader<BoxRead>,
    /// Kept open: closing it would end the stream.
    _send: BoxWrite,
}

impl std::fmt::Debug for Notifications {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Notifications").finish_non_exhaustive()
    }
}

impl Notifications {
    /// The next notification's method and params; `None` once the connection is closed. Lines
    /// that are not notifications are skipped. Cancel-safe.
    pub async fn next(&mut self) -> Option<(String, Value)> {
        loop {
            let line = self.lines.next_line(usize::MAX).await.ok()??;
            let Ok(mut value) = serde_json::from_str::<Value>(&line) else {
                continue;
            };
            if let Some(method) = value["method"].as_str() {
                let method = method.to_owned();
                return Some((method, value["params"].take()));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn lines_are_split_and_bounded() {
        let (mut a, b) = tokio::io::duplex(64);
        let mut reader = LineReader::new(b);
        a.write_all(b"one\r\ntwo\nfour").await.unwrap();
        drop(a);
        assert_eq!(reader.next_line(8).await.unwrap().as_deref(), Some("one"));
        assert_eq!(reader.next_line(8).await.unwrap().as_deref(), Some("two"));
        assert_eq!(reader.next_line(8).await.unwrap().as_deref(), Some("four"));
        assert_eq!(reader.next_line(8).await.unwrap(), None);
        let (mut a, b) = tokio::io::duplex(64);
        let mut reader = LineReader::new(b);
        a.write_all(b"three-is-too-long\n").await.unwrap();
        assert!(reader.next_line(8).await.is_err());
    }
}
