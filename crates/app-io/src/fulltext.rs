//! The local full-text index of downloaded bodies (ADR 0005): `tantivy`, in `<profile>/index`
//! (or in memory for tests). Derived data like the blobs: it can be deleted and rebuilt.
//!
//! A document is one message: its ID (stored, to find and delete it) and the indexed text of its
//! subject, senders and body (`mime::parse_message`). Searching takes the words of a dail query
//! and finds the messages that contain every one of them in any of these fields; the words are
//! tokenized like the indexed text, never parsed as query syntax.

use std::path::Path;

use app_core::MessageId;
use parking_lot::Mutex;
use tantivy::collector::TopDocs;
use tantivy::query::{BooleanQuery, Occur, Query, TermQuery};
use tantivy::schema::{Field, IndexRecordOption, STORED, STRING, Schema, TEXT, Value};
use tantivy::{Index, IndexReader, IndexWriter, ReloadPolicy, TantivyDocument, Term};

use crate::IoError;

/// Memory the writer may buffer before it flushes a segment (tantivy's minimum is 15 MB).
const WRITER_BUDGET: usize = 20_000_000;

/// The index.
pub struct FullText {
    index: Index,
    reader: IndexReader,
    writer: Mutex<Writer>,
    id: Field,
    subject: Field,
    from: Field,
    body: Field,
}

struct Writer {
    writer: IndexWriter,
    /// Changes not committed yet (searches commit them first).
    pending: usize,
}

impl std::fmt::Debug for FullText {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FullText").finish_non_exhaustive()
    }
}

fn index_error(err: impl std::fmt::Display) -> IoError {
    IoError::Unavailable(format!("full-text index: {err}"))
}

fn schema() -> (Schema, [Field; 4]) {
    let mut builder = Schema::builder();
    let id = builder.add_text_field("message", STRING | STORED);
    let subject = builder.add_text_field("subject", TEXT);
    let from = builder.add_text_field("from", TEXT);
    let body = builder.add_text_field("body", TEXT);
    (builder.build(), [id, subject, from, body])
}

impl FullText {
    /// Opens the index in `dir`, creating it if it does not exist.
    pub fn open(dir: &Path) -> Result<Self, IoError> {
        std::fs::create_dir_all(dir).map_err(|err| IoError::fs(dir, err))?;
        let (schema, fields) = schema();
        let index = if dir.join("meta.json").exists() {
            Index::open_in_dir(dir).map_err(index_error)?
        } else {
            Index::create_in_dir(dir, schema).map_err(index_error)?
        };
        Self::with(index, fields)
    }

    /// Opens the index in `dir`; one that can't be opened (damaged, or from an incompatible
    /// version) is deleted and made again: it is derived data, rebuilt from the bodies.
    pub fn open_or_rebuild(dir: &Path) -> Result<Self, IoError> {
        match Self::open(dir) {
            Ok(index) => Ok(index),
            Err(err) => {
                tracing::warn!(error = %err, "full-text index unusable; rebuilding it");
                std::fs::remove_dir_all(dir).map_err(|e| IoError::fs(dir, e))?;
                Self::open(dir)
            }
        }
    }

    /// An index in memory (tests, the memory backend).
    pub fn in_memory() -> Result<Self, IoError> {
        let (schema, fields) = schema();
        Self::with(Index::create_in_ram(schema), fields)
    }

    fn with(index: Index, [id, subject, from, body]: [Field; 4]) -> Result<Self, IoError> {
        let writer = index
            .writer_with_num_threads(1, WRITER_BUDGET)
            .map_err(index_error)?;
        let reader = index
            .reader_builder()
            .reload_policy(ReloadPolicy::Manual)
            .try_into()
            .map_err(index_error)?;
        Ok(Self {
            index,
            reader,
            writer: Mutex::new(Writer { writer, pending: 0 }),
            id,
            subject,
            from,
            body,
        })
    }

    fn id_term(&self, message: MessageId) -> Term {
        Term::from_field_text(self.id, &message.to_string())
    }

    /// Indexes the raw message `raw` as `message` (replacing an earlier version).
    pub fn add(&self, message: MessageId, raw: &[u8]) -> Result<(), IoError> {
        let parsed = crate::mime::parse_message(raw);
        let mut doc = TantivyDocument::default();
        doc.add_text(self.id, message.to_string());
        doc.add_text(self.subject, &parsed.headers.subject);
        for addr in &parsed.headers.from {
            doc.add_text(self.from, addr.name.as_deref().unwrap_or_default());
            doc.add_text(self.from, &addr.email);
        }
        doc.add_text(self.body, &parsed.text);
        let mut writer = self.writer.lock();
        writer.writer.delete_term(self.id_term(message));
        writer.writer.add_document(doc).map_err(index_error)?;
        writer.pending += 1;
        Ok(())
    }

    /// Drops `messages` from the index.
    pub fn remove(&self, messages: &[MessageId]) {
        if messages.is_empty() {
            return;
        }
        let mut writer = self.writer.lock();
        for message in messages {
            writer.writer.delete_term(self.id_term(*message));
        }
        writer.pending += messages.len();
    }

    /// Makes the changes so far visible to searches.
    pub fn commit(&self) -> Result<(), IoError> {
        let mut writer = self.writer.lock();
        if writer.pending == 0 {
            return Ok(());
        }
        writer.writer.commit().map_err(index_error)?;
        writer.pending = 0;
        drop(writer);
        self.reader.reload().map_err(index_error)
    }

    /// Whether `message` is indexed (as of the last commit).
    pub fn contains(&self, message: MessageId) -> bool {
        let query = TermQuery::new(self.id_term(message), IndexRecordOption::Basic);
        self.reader
            .searcher()
            .search(&query, &tantivy::collector::Count)
            .is_ok_and(|n| n > 0)
    }

    /// The messages that contain every word of `phrases` (in the subject, the senders or the
    /// body), best first, at most `limit`. Commits pending changes first.
    pub fn search(&self, phrases: &[String], limit: usize) -> Result<Vec<MessageId>, IoError> {
        self.commit()?;
        let words = self.words(phrases)?;
        if words.is_empty() || limit == 0 {
            return Ok(Vec::new());
        }
        let clauses: Vec<(Occur, Box<dyn Query>)> = words
            .iter()
            .map(|word| {
                let any_field: Vec<(Occur, Box<dyn Query>)> = [self.subject, self.from, self.body]
                    .into_iter()
                    .map(|field| {
                        let term = Term::from_field_text(field, word);
                        let query: Box<dyn Query> =
                            Box::new(TermQuery::new(term, IndexRecordOption::WithFreqs));
                        (Occur::Should, query)
                    })
                    .collect();
                let query: Box<dyn Query> = Box::new(BooleanQuery::new(any_field));
                (Occur::Must, query)
            })
            .collect();
        let query = BooleanQuery::new(clauses);
        let searcher = self.reader.searcher();
        let top = searcher
            .search(&query, &TopDocs::with_limit(limit).order_by_score())
            .map_err(index_error)?;
        let mut hits = Vec::with_capacity(top.len());
        for (_, address) in top {
            let doc: TantivyDocument = searcher.doc(address).map_err(index_error)?;
            let id = doc
                .get_first(self.id)
                .and_then(|value| value.as_str())
                .and_then(|text| text.parse::<uuid::Uuid>().ok());
            if let Some(id) = id {
                hits.push(MessageId::from_uuid(id));
            }
        }
        Ok(hits)
    }

    /// The words of `phrases`, tokenized like the body (lower case, punctuation split off).
    fn words(&self, phrases: &[String]) -> Result<Vec<String>, IoError> {
        let mut analyzer = self
            .index
            .tokenizer_for_field(self.body)
            .map_err(index_error)?;
        let mut words: Vec<String> = Vec::new();
        for phrase in phrases {
            let mut stream = analyzer.token_stream(phrase);
            while let Some(token) = stream.next() {
                if !words.contains(&token.text) {
                    words.push(token.text.clone());
                }
            }
        }
        Ok(words)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id(n: u128) -> MessageId {
        MessageId::from_uuid(uuid::Uuid::from_u128(n))
    }

    fn mail(from: &str, subject: &str, body: &str) -> Vec<u8> {
        format!("From: {from}\r\nSubject: {subject}\r\nContent-Type: text/plain\r\n\r\n{body}\r\n")
            .into_bytes()
    }

    #[test]
    fn bodies_are_found_by_every_word() {
        let index = FullText::in_memory().unwrap();
        index
            .add(
                id(1),
                &mail(
                    "Bob <bob@x.org>",
                    "Q3",
                    "The invoice for the Zürich office.",
                ),
            )
            .unwrap();
        index
            .add(
                id(2),
                &mail(
                    "Carol <carol@x.org>",
                    "Lunch",
                    "Invoice? No, lunch at noon.",
                ),
            )
            .unwrap();
        assert!(!index.contains(id(1)), "not committed yet");
        let search = |words: &[&str]| {
            let words: Vec<String> = words.iter().map(|w| (*w).to_owned()).collect();
            let mut hits = index.search(&words, 10).unwrap();
            hits.sort();
            hits
        };
        assert_eq!(search(&["invoice"]), [id(1), id(2)]);
        assert_eq!(
            search(&["INVOICE", "zürich"]),
            [id(1)],
            "every word, any case"
        );
        assert_eq!(search(&["carol"]), [id(2)], "senders");
        assert_eq!(
            search(&["office invoice"]),
            [id(1)],
            "a phrase is its words"
        );
        assert!(search(&["nothing"]).is_empty());
        assert!(search(&["\"(\" AND OR *"]).is_empty(), "no query syntax");
        assert!(index.contains(id(1)));

        index.remove(&[id(1)]);
        assert_eq!(search(&["invoice"]), [id(2)]);
        // Indexing again replaces the document.
        index
            .add(id(2), &mail("Carol <c@x>", "Lunch", "Pizza"))
            .unwrap();
        assert!(search(&["invoice"]).is_empty());
        assert_eq!(search(&["pizza"]), [id(2)]);
    }

    #[test]
    fn an_index_on_disk_survives_reopening() {
        let dir = std::env::temp_dir().join(format!("dail-fulltext-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        {
            let index = FullText::open(&dir).unwrap();
            index
                .add(id(7), &mail("a@x", "Hello", "kept on disk"))
                .unwrap();
            index.commit().unwrap();
        }
        let index = FullText::open(&dir).unwrap();
        assert_eq!(index.search(&["disk".into()], 5).unwrap(), [id(7)]);
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
