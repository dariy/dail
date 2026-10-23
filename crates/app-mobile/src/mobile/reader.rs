//! The reader: the open thread's messages, collapsed and expanded, with HTML bodies drawn from
//! `app_core::document` and plain text with its folds (`app_front::fold`); the toolbar, the
//! previous and next conversation, *Links (N)*, and the sheet that shows a link's full target
//! before it opens.
//!
//! Everything here is email content: only displayed, never interpreted. Every string goes
//! through `app_front::text`.

use app_core::document::{Block, FoldKind, Span};
use app_core::{AppState, BodyState, Command, CommandName, MessageId, listing};
use app_front::body::{self, Body};
use app_front::text::{clean, clean_lines};
use app_front::{fold, format, highlight, palette};

use super::{Act, Mobile};
use crate::event::External;
use crate::frame::{
    Attachment, Button, DocBlock, DocCell, DocSpan, LinkSheet, Reader, ReaderMessage,
};
use crate::request::Request;

/// One line of untrusted text per line, joined again: line breaks stay, nothing else unsafe.
fn clean_text(text: &str) -> String {
    clean_lines(text).join("\n")
}

/// What a fold says.
const fn fold_label(kind: FoldKind) -> &'static str {
    match kind {
        FoldKind::Quote => "Quoted text",
        FoldKind::Signature => "Signature",
    }
}

/// Builds a message's blocks, adding the action IDs of its links and folds.
struct Doc<'a> {
    mobile: &'a mut Mobile,
    message: MessageId,
    words: &'a [String],
}

impl Doc<'_> {
    fn blocks(&mut self, blocks: &[Block]) -> Vec<DocBlock> {
        blocks.iter().map(|b| self.block(b)).collect()
    }

    fn block(&mut self, block: &Block) -> DocBlock {
        match block {
            Block::Paragraph { spans } => DocBlock::Paragraph {
                spans: self.spans(spans),
            },
            Block::Heading { level, spans } => DocBlock::Heading {
                level: *level,
                spans: self.spans(spans),
            },
            Block::List {
                ordered,
                start,
                items,
            } => DocBlock::List {
                ordered: *ordered,
                start: *start,
                items: items.iter().map(|item| self.blocks(item)).collect(),
            },
            Block::Quote { blocks } => DocBlock::Quote {
                blocks: self.blocks(blocks),
            },
            Block::Preformatted { text } => DocBlock::Preformatted {
                lines: clean_lines(text),
            },
            Block::Table { rows } => DocBlock::Table {
                rows: rows
                    .iter()
                    .map(|row| {
                        row.iter()
                            .map(|cell| DocCell {
                                header: cell.header,
                                spans: self.spans(&cell.spans),
                            })
                            .collect()
                    })
                    .collect(),
            },
            Block::Rule => DocBlock::Rule,
            Block::Fold { kind, blocks } => {
                let message = self.message;
                self.fold(*kind, |doc| doc.blocks(blocks), message)
            }
        }
    }

    /// A fold, its blocks built only while it is open.
    fn fold(
        &mut self,
        kind: FoldKind,
        inner: impl FnOnce(&mut Self) -> Vec<DocBlock>,
        message: MessageId,
    ) -> DocBlock {
        let expanded = self.mobile.folds.contains(&(message, kind));
        let toggle = self.mobile.add(Act::Fold(message, kind));
        DocBlock::Fold {
            label: fold_label(kind).to_owned(),
            expanded,
            toggle,
            blocks: if expanded { inner(self) } else { Vec::new() },
        }
    }

    /// Spans with their link actions, cut at the search words. An inline image shows as its
    /// alt text here (the pictures of `show_images_from` come with the attachments).
    fn spans(&mut self, spans: &[Span]) -> Vec<DocSpan> {
        let mut out = Vec::new();
        for span in spans {
            let link = span
                .link
                .as_ref()
                .map(|url| self.mobile.add(Act::Link(url.clone())));
            let style = DocSpan {
                text: String::new(),
                bold: span.bold,
                italic: span.italic || span.image.is_some(),
                code: span.code,
                strike: span.strike,
                hit: false,
                link,
            };
            out.extend(self.marked(&span.text, &style));
        }
        out
    }

    /// `text` in `style`, cut where the search words are.
    fn marked(&self, text: &str, style: &DocSpan) -> Vec<DocSpan> {
        highlight::pieces(text, self.words)
            .into_iter()
            .filter(|(piece, _)| !piece.is_empty())
            .map(|(piece, hit)| DocSpan {
                text: clean_text(piece),
                hit,
                ..style.clone()
            })
            .collect()
    }

    /// Plain text: its paragraphs, then the signature and the quote folded.
    fn plain(&mut self, text: &str) -> Vec<DocBlock> {
        let folded = fold::split(text);
        let mut out = self.paragraphs(folded.main);
        for (kind, part) in [
            (FoldKind::Signature, folded.signature),
            (FoldKind::Quote, folded.quote),
        ] {
            if let Some(part) = part {
                let message = self.message;
                out.push(self.fold(kind, |doc| doc.paragraphs(part), message));
            }
        }
        if out.is_empty() {
            out.push(DocBlock::Paragraph {
                spans: vec![DocSpan {
                    text: "(no text)".to_owned(),
                    italic: true,
                    ..DocSpan::default()
                }],
            });
        }
        out
    }

    /// Text split at blank lines into paragraphs.
    fn paragraphs(&self, text: &str) -> Vec<DocBlock> {
        let mut out = Vec::new();
        let mut current: Vec<&str> = Vec::new();
        let flush = |current: &mut Vec<&str>, out: &mut Vec<DocBlock>| {
            if !current.is_empty() {
                let text = current.join("\n");
                out.push(DocBlock::Paragraph {
                    spans: self.marked(&text, &DocSpan::default()),
                });
                current.clear();
            }
        };
        for line in text.lines() {
            if line.trim().is_empty() {
                flush(&mut current, &mut out);
            } else {
                current.push(line.trim_end());
            }
        }
        flush(&mut current, &mut out);
        out
    }
}

/// The link targets of a body: the document's, else those found in the text.
fn link_targets(body: &Body) -> Vec<String> {
    match body.document.as_ref().filter(|d| !d.blocks.is_empty()) {
        Some(document) => {
            let mut links: Vec<String> = document.links().into_iter().map(str::to_owned).collect();
            for url in body::links(&body.text) {
                if !links.contains(&url) {
                    links.push(url);
                }
            }
            links
        }
        None => body::links(&body.text),
    }
}

/// Whether a link target may be opened: `http`, `https` or `mailto` (the document keeps no
/// other kind, and the text scan finds no other kind; this holds the line).
fn openable(url: &str) -> bool {
    let lower = url.to_ascii_lowercase();
    ["https://", "http://", "mailto:"]
        .iter()
        .any(|scheme| lower.starts_with(scheme))
}

impl Mobile {
    pub(super) fn reader(&mut self, state: &AppState) -> Option<Reader> {
        let thread = state.view.thread.and_then(|id| state.threads.get(&id))?;
        let now = self.now_ms();
        let ids = vec![thread.id];
        let envelopes: Vec<_> = thread
            .messages
            .iter()
            .filter_map(|id| state.messages.get(id))
            .collect();
        let starred = envelopes.iter().any(|m| m.flags.flagged);
        let unread = envelopes.iter().any(|m| !m.flags.seen);
        let messages = Vec::new;
        let (star_icon, star_label, star) = if starred {
            (
                "star",
                "Unstar",
                Command::Unstar {
                    ids: ids.clone(),
                    messages: messages(),
                },
            )
        } else {
            (
                "star",
                "Star",
                Command::Star {
                    ids: ids.clone(),
                    messages: messages(),
                },
            )
        };
        let (read_icon, read_label, read) = if unread {
            (
                "mark_email_read",
                "Mark read",
                Command::MarkRead {
                    ids: ids.clone(),
                    messages: messages(),
                },
            )
        } else {
            (
                "mark_email_unread",
                "Mark unread",
                Command::MarkUnread {
                    ids: ids.clone(),
                    messages: messages(),
                },
            )
        };
        let mut actions = vec![
            self.button(
                "archive",
                "Archive",
                Act::Run(Command::ArchiveThreads {
                    ids: ids.clone(),
                    messages: messages(),
                }),
            ),
            self.button(
                "delete",
                "Trash",
                Act::Run(Command::TrashThreads {
                    ids: ids.clone(),
                    messages: messages(),
                }),
            ),
            self.button(star_icon, star_label, Act::Run(star)),
            self.button(read_icon, read_label, Act::Run(read)),
            self.button(
                "drive_file_move",
                "Move…",
                Act::Flow(CommandName::MoveThreads),
            ),
        ];
        if let Some(message) = palette::newest_message(state, thread.id) {
            actions.push(self.button(
                "reply",
                "Reply",
                Act::Run(Command::Reply {
                    message,
                    body: None,
                }),
            ));
        }
        let (previous, next, position) = self.neighbours(state, thread.id);
        let words = state
            .view
            .search
            .as_deref()
            .map(highlight::words)
            .unwrap_or_default();
        let mut out = Vec::new();
        for message in envelopes {
            let expanded = listing::is_expanded(state, thread, message);
            let toggle = self.add(Act::Run(if expanded {
                Command::CollapseMessages {
                    ids: vec![message.id],
                }
            } else {
                Command::ExpandMessages {
                    ids: vec![message.id],
                }
            }));
            let loaded = self.bodies.get(message.id).cloned();
            let snippet = loaded
                .as_ref()
                .map(|b| fold::split(&b.text).main)
                .and_then(|text| text.lines().find(|l| !l.trim().is_empty()))
                .or(message.snippet.as_deref())
                .unwrap_or_default()
                .trim()
                .chars()
                .take(120)
                .collect::<String>();
            let (body, status) = match (expanded, &loaded, &message.body) {
                (false, _, _) => (None, None),
                (true, Some(loaded), _) => {
                    let mut doc = Doc {
                        mobile: self,
                        message: message.id,
                        words: &words,
                    };
                    let blocks = match loaded.document.as_ref().filter(|d| !d.blocks.is_empty()) {
                        Some(document) => doc.blocks(&document.blocks),
                        None => doc.plain(loaded.text.trim_end()),
                    };
                    (Some(blocks), None)
                }
                (true, None, BodyState::Failed { error }) => (
                    None,
                    Some(format!(
                        "The body could not be downloaded: {}",
                        clean(error)
                    )),
                ),
                (true, None, _) => (None, Some("Loading…".to_owned())),
            };
            let links = match (&loaded, expanded) {
                (Some(loaded), true) => link_targets(loaded)
                    .into_iter()
                    .map(|url| Button {
                        icon: Some("link".to_owned()),
                        label: clean(&url),
                        action: self.add(Act::Link(url)),
                    })
                    .collect(),
                _ => Vec::new(),
            };
            let to: Vec<String> = message
                .to
                .iter()
                .chain(&message.cc)
                .map(|a| clean(&format::full(a)))
                .collect();
            out.push(ReaderMessage {
                message: message.id,
                from: message
                    .from
                    .first()
                    .map_or_else(|| "(no sender)".to_owned(), |a| clean(&format::full(a))),
                to: to.join(", "),
                date: format::short_date(message.date_ms, now),
                expanded,
                toggle,
                snippet: clean(&snippet),
                body,
                status,
                attachments: loaded
                    .as_ref()
                    .map(|b| {
                        b.attachments
                            .iter()
                            .map(|a| Attachment {
                                part: a.part,
                                name: clean(&a.name),
                                mime: clean(&a.content_type),
                                size: format::size(a.size),
                            })
                            .collect()
                    })
                    .unwrap_or_default(),
                links,
            });
        }
        Some(Reader {
            thread: thread.id,
            subject: match thread.subject.trim() {
                "" => "(no subject)".to_owned(),
                s => clean(s),
            },
            actions,
            previous,
            next,
            position,
            messages: out,
        })
    }

    /// The conversations above and below the open one in the list, and where it is.
    fn neighbours(
        &mut self,
        state: &AppState,
        thread: app_core::ThreadId,
    ) -> (Option<Button>, Option<Button>, Option<String>) {
        let rows = self.rows.get(state);
        let Some(at) = rows.iter().position(|r| *r == thread) else {
            return (None, None, None);
        };
        let total = rows.len();
        let previous = at.checked_sub(1).and_then(|i| rows.get(i)).copied();
        let next = rows.get(at + 1).copied();
        let previous = previous.map(|id| {
            self.button(
                "expand_less",
                "Previous conversation",
                Act::Run(Command::OpenThread { id }),
            )
        });
        let next = next.map(|id| {
            self.button(
                "expand_more",
                "Next conversation",
                Act::Run(Command::OpenThread { id }),
            )
        });
        (previous, next, Some(format!("{} of {total}", at + 1)))
    }

    /// The sheet of the tapped link, while open.
    pub(super) fn link_sheet(&mut self) -> Option<LinkSheet> {
        let url = self.link.clone()?;
        let open = self.button("open_in_new", "Open", Act::OpenLink(url.clone()));
        let copy = self.button("content_copy", "Copy", Act::CopyLink(url.clone()));
        let close = self.button("close", "Close", Act::Back);
        Some(LinkSheet {
            url: clean(&url),
            open,
            copy,
            close,
        })
    }

    /// *Open* on the link sheet: the browser for `http(s)`, a new draft for `mailto:`.
    pub(super) fn open_link(&mut self, url: &str, state: &AppState) -> Vec<Request> {
        self.link = None;
        if !openable(url) {
            return Vec::new();
        }
        if url
            .get(..7)
            .is_some_and(|s| s.eq_ignore_ascii_case("mailto:"))
        {
            return self.share(state, Some(url), &[], None, None, Vec::new());
        }
        self.externals.push(External::OpenUrl {
            url: url.to_owned(),
        });
        Vec::new()
    }

    /// *Copy* on the link sheet.
    pub(super) fn copy_link(&mut self, url: String) {
        self.link = None;
        self.externals.push(External::Copy { text: url });
        self.notify("Link copied.");
    }

    /// Opens or folds a message's quote or signature.
    pub(super) fn toggle_fold(&mut self, message: MessageId, kind: FoldKind) {
        if !self.folds.remove(&(message, kind)) {
            self.folds.insert((message, kind));
        }
    }
}
