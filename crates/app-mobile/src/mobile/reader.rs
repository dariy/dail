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
use crate::event::{External, SaveFile};
use crate::frame::{
    Attachment, Button, DocBlock, DocCell, DocSpan, ImagesBar, InlineImage, LinkSheet, Preview,
    PreviewKind, Reader, ReaderMessage,
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
    /// The body's blob, while the sender's pictures show.
    images: Option<&'a str>,
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
    /// alt text, or as itself while the sender's pictures show.
    fn spans(&mut self, spans: &[Span]) -> Vec<DocSpan> {
        let mut out = Vec::new();
        for span in spans {
            let link = span
                .link
                .as_ref()
                .map(|url| self.mobile.add(Act::Link(url.clone())));
            let image = span
                .image
                .as_ref()
                .zip(self.images)
                .map(|(cid, blob)| InlineImage {
                    blob: blob.to_owned(),
                    // A Content-ID with unsafe characters is cleaned like any text from mail
                    // (and then finds no part).
                    cid: clean(cid),
                });
            let style = DocSpan {
                text: String::new(),
                bold: span.bold,
                italic: span.italic || span.image.is_some(),
                code: span.code,
                strike: span.strike,
                hit: false,
                link,
                image,
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
            let blob = match &message.body {
                BodyState::Fetched { blob } => Some(blob.clone()),
                _ => None,
            };
            let sender = message
                .from
                .first()
                .map(|a| a.email.clone())
                .unwrap_or_default();
            let pictures = loaded
                .as_ref()
                .and_then(|b| b.document.as_ref())
                .is_some_and(|d| !d.images().is_empty());
            let shown = pictures && state.settings.shows_images_from(&sender);
            let (body, status) = match (expanded, &loaded, &message.body) {
                (false, _, _) => (None, None),
                (true, Some(loaded), _) => {
                    let mut doc = Doc {
                        mobile: self,
                        message: message.id,
                        words: &words,
                        images: blob.as_deref().filter(|_| shown),
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
            let images = (expanded && pictures && !sender.is_empty()).then(|| {
                let who = clean(&sender);
                let (text, label, cmd) = if shown {
                    (
                        "Showing the pictures in this message.",
                        format!("Hide images from {who}"),
                        Command::HideImagesFrom {
                            sender: sender.clone(),
                        },
                    )
                } else {
                    (
                        "Pictures in this message are hidden.",
                        format!("Show images from {who}"),
                        Command::ShowImagesFrom {
                            sender: sender.clone(),
                        },
                    )
                };
                ImagesBar {
                    shown,
                    text: text.to_owned(),
                    toggle: Button {
                        icon: Some(
                            if shown {
                                "visibility_off"
                            } else {
                                "visibility"
                            }
                            .to_owned(),
                        ),
                        label,
                        action: self.add(Act::Run(cmd)),
                    },
                }
            });
            let files = loaded
                .as_ref()
                .filter(|_| expanded)
                .map(|b| b.attachments.clone())
                .unwrap_or_default();
            let mut attachments = Vec::new();
            for file in &files {
                let save = SaveFile {
                    part: file.part,
                    name: file.name.clone(),
                    mime: file.content_type.clone(),
                };
                attachments.push(Attachment {
                    part: file.part,
                    name: clean(&file.name),
                    mime: clean(&file.content_type),
                    size: format::size(file.size),
                    open: self.add(Act::Preview(message.id, file.part)),
                    save: self.add(Act::Save(blob.clone(), vec![save])),
                });
            }
            let save_all = (files.len() > 1).then(|| {
                let all = files
                    .iter()
                    .map(|file| SaveFile {
                        part: file.part,
                        name: file.name.clone(),
                        mime: file.content_type.clone(),
                    })
                    .collect();
                self.button(
                    "download",
                    &format!("Save all ({})", files.len()),
                    Act::Save(blob.clone(), all),
                )
            });
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
                attachments,
                save_all,
                images,
                links,
            });
        }
        let close = self.button("close", "Close message", Act::Run(Command::CloseThread {}));
        Some(Reader {
            close,
            maximize: None,
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

/// Text a preview carries at most.
const MAX_PREVIEW_BYTES: usize = 256 * 1024;

/// A text attachment's bytes as the preview shows them: UTF-8 (invalid sequences replaced),
/// at most 256 KiB, every line cleaned (`app_front::text`). The host answers `blob`'s `text`
/// request with it.
pub fn preview_text(bytes: &[u8]) -> String {
    let cut = bytes.len() > MAX_PREVIEW_BYTES;
    let text = String::from_utf8_lossy(bytes.get(..MAX_PREVIEW_BYTES).unwrap_or(bytes));
    let mut out = clean_text(&text);
    if cut {
        out.push_str("\n…");
    }
    out
}

/// How an attachment of type `mime` is shown (HTML is not shown as its source, as on the
/// desktop).
fn preview_kind(mime: &str) -> PreviewKind {
    let mime = mime.trim().to_ascii_lowercase();
    if mime.starts_with("image/") {
        PreviewKind::Image
    } else if mime.starts_with("text/") && !mime.starts_with("text/html") {
        PreviewKind::Text
    } else {
        PreviewKind::None
    }
}

impl Mobile {
    /// The attachment shown in the app, while open (and still in the open thread).
    pub(super) fn preview_view(&mut self, state: &AppState) -> Option<Preview> {
        let (message, part) = self.preview?;
        let blob = match state.messages.get(&message).map(|m| &m.body) {
            Some(BodyState::Fetched { blob }) => blob.clone(),
            _ => return None,
        };
        let file = self
            .bodies
            .get(message)?
            .attachments
            .iter()
            .find(|a| a.part == part)?
            .clone();
        let kind = preview_kind(&file.content_type);
        let save = SaveFile {
            part,
            name: file.name.clone(),
            mime: file.content_type.clone(),
        };
        let save = self.button(
            "download",
            "Save",
            Act::Save(Some(blob.clone()), vec![save]),
        );
        let close = self.button("close", "Close", Act::Back);
        Some(Preview {
            name: clean(&file.name),
            detail: format!(
                "{} · {}",
                clean(&file.content_type),
                format::size(file.size)
            ),
            kind,
            blob,
            part,
            note: (kind == PreviewKind::None).then(|| {
                "dail does not show this kind of file. Save it to open it in another app."
                    .to_owned()
            }),
            save,
            close,
        })
    }

    /// *Save* or *Save all*: the host's pickers, with the names made safe by the host.
    pub(super) fn save(&mut self, blob: Option<String>, files: Vec<SaveFile>) {
        match blob {
            Some(blob) if !files.is_empty() => {
                self.externals.push(External::SaveFiles { blob, files });
            }
            _ => self.notify("The message is not downloaded yet."),
        }
    }
}
