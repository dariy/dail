//! The audit log: every input of a session as a replay record (`ARCHITECTURE.md §6`), one JSON
//! line each, in `logs/replay-YYYYMMDD.jsonl` (UTC day of the input). Passwords and message text
//! are redacted ([`ReplayRecord::redacted_line`]); `dail replay <log>` replays a file.
//!
//! The engine loop hands records over without waiting; a writer task appends them in order.
//! A write failure is logged and the record dropped: auditing never stops the session.

use std::path::{Path, PathBuf};

use app_core::ReplayRecord;
use app_core::date::civil_from_days;
use tokio::io::AsyncWriteExt;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

/// Where a session's audit log goes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuditLog {
    dir: PathBuf,
}

impl AuditLog {
    /// Writes into `dir` (created if missing, user-only on Unix), e.g. the profile's `logs/`.
    pub fn new(dir: impl Into<PathBuf>) -> Self {
        Self { dir: dir.into() }
    }

    /// The directory.
    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// The file for the UTC day of `t_ms`: `replay-YYYYMMDD.jsonl`.
    pub fn file_for(&self, t_ms: i64) -> PathBuf {
        let (year, month, day) = civil_from_days(t_ms.div_euclid(86_400_000));
        self.dir
            .join(format!("replay-{year:04}{month:02}{day:02}.jsonl"))
    }

    /// Starts the writer task. Must be called inside a tokio runtime.
    pub(crate) fn start(self, opened_ms: i64) -> AuditWriter {
        let (tx, mut rx) = mpsc::unbounded_channel::<Line>();
        let task = tokio::spawn(async move {
            if let Err(err) = ensure_dir(&self.dir).await {
                tracing::warn!(dir = %self.dir.display(), error = %err, "audit log disabled");
                return;
            }
            let mut first = Some(opened_ms);
            while let Some(line) = rx.recv().await {
                let path = self.file_for(line.t);
                let mut text = String::new();
                if let Some(at) = first.take() {
                    text.push_str(&format!("# session opened at {at}\n"));
                }
                text.push_str(&line.json);
                text.push('\n');
                if let Err(err) = append(&path, text.as_bytes()).await {
                    tracing::warn!(file = %path.display(), error = %err, "audit write failed");
                }
            }
        });
        AuditWriter { tx, task }
    }
}

struct Line {
    t: i64,
    json: String,
}

/// The engine loop's end of the log.
pub(crate) struct AuditWriter {
    tx: mpsc::UnboundedSender<Line>,
    task: JoinHandle<()>,
}

impl AuditWriter {
    /// Queues one record (redacted).
    pub(crate) fn record(&self, record: &ReplayRecord) {
        let line = Line {
            t: record.t,
            json: record.redacted_line(),
        };
        if self.tx.send(line).is_err() {
            tracing::warn!("audit writer stopped; record dropped");
        }
    }

    /// Waits until every queued record is written.
    pub(crate) async fn finish(self) {
        drop(self.tx);
        if let Err(err) = self.task.await {
            tracing::warn!(error = %err, "audit writer failed");
        }
    }
}

async fn ensure_dir(dir: &Path) -> std::io::Result<()> {
    let mut builder = tokio::fs::DirBuilder::new();
    builder.recursive(true);
    #[cfg(unix)]
    builder.mode(0o700);
    builder.create(dir).await
}

async fn append(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let mut options = tokio::fs::OpenOptions::new();
    options.create(true).append(true);
    #[cfg(unix)]
    options.mode(0o600);
    let mut file = options.open(path).await?;
    file.write_all(bytes).await?;
    file.flush().await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn files_are_named_after_the_utc_day() {
        let log = AuditLog::new("/x");
        assert_eq!(log.file_for(0), Path::new("/x/replay-19700101.jsonl"));
        assert_eq!(
            log.file_for(1_700_000_000_000),
            Path::new("/x/replay-20231114.jsonl")
        );
        assert_eq!(
            log.file_for(951_782_400_000),
            Path::new("/x/replay-20000229.jsonl")
        );
        assert_eq!(log.file_for(-1), Path::new("/x/replay-19691231.jsonl"));
    }
}
