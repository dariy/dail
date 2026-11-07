//! The reader: the open thread, oldest message first. A header per message (from, to, cc, date),
//! the text, the attachments and *Links (N)* with their full targets. Bodies come from the shared
//! body tracking (`app_front::body`). Everything is email content: cleaned, wrapped, never
//! interpreted. There is no browser and no "open original": over SSH they would open on the host.
//! A collapsed message (`collapse_messages`; by default the read ones but the newest) is one line.
//!
//! The message cursor (MISC-44, `n`/`p`) picks one message: it is marked `▶` with its actions
//! (`app_front::message_actions`) and their keys under it, and those keys act on it alone. Like
//! scrolling it is presentation, kept per open thread; before the first message it goes away.

use crate::text::{f, s};
use app_core::{AppState, BodyState, Envelope, MailboxRole, MessageId, ThreadId, listing};
use app_front::body::{Bodies, Body, links};
use app_front::i18n::Key;
use app_front::{fold, format, format::Zone, mailbox::mailbox_name};
use ratatui::style::Style;
use ratatui::text::{Line, Span};

use crate::text::{clean, clean_lines, wrap};
use crate::view::{bold, dim};

/// The reader's scroll position, kept per open thread.
#[derive(Debug, Default)]
pub struct Reader {
    thread: Option<ThreadId>,
    scroll: usize,
    /// Rows of the last frame (for paging) and the wrapped length.
    height: usize,
    len: usize,
    /// The message cursor, and whether the next frame scrolls to it.
    cursor: Option<MessageId>,
    follow: bool,
}

impl Reader {
    /// Scrolls by `rows` (negative: up), within the text.
    pub fn scroll_by(&mut self, rows: isize) {
        self.scroll = self
            .scroll
            .saturating_add_signed(rows)
            .min(self.len.saturating_sub(self.height));
    }

    /// One screen minus a line of overlap.
    pub fn page(&self) -> isize {
        isize::try_from(self.height.saturating_sub(1).max(1)).unwrap_or(1)
    }

    /// Forgets the scroll position and the cursor when another thread opened.
    fn sync(&mut self, state: &AppState) {
        if self.thread != state.view.thread {
            self.thread = state.view.thread;
            self.scroll = 0;
            self.cursor = None;
            self.follow = false;
        }
    }

    /// The message under the cursor, if it is still in the open thread.
    pub fn cursor(&self, state: &AppState) -> Option<MessageId> {
        let thread = state.threads.get(&state.view.thread?)?;
        self.cursor
            .filter(|_| self.thread == Some(thread.id))
            .filter(|m| thread.messages.contains(m))
    }

    /// Moves the cursor `step` messages (`n`: 1, `p`: -1); without a cursor `n` starts at the
    /// first message and `p` at the last. Before the first message the cursor goes away. Returns
    /// whether a thread is open.
    pub fn move_cursor(&mut self, state: &AppState, step: isize) -> bool {
        self.sync(state);
        let Some(thread) = state.view.thread.and_then(|id| state.threads.get(&id)) else {
            return false;
        };
        let messages = &thread.messages;
        let at = self
            .cursor(state)
            .and_then(|c| messages.iter().position(|m| *m == c));
        let next = match at {
            None if step > 0 => Some(0),
            None => messages.len().checked_sub(1),
            Some(at) => at
                .checked_add_signed(step)
                .map(|n| n.min(messages.len().saturating_sub(1))),
        };
        self.cursor = next.and_then(|n| messages.get(n).copied());
        self.follow = self.cursor.is_some();
        true
    }

    /// The visible lines of the open thread for a pane of `width`×`height`.
    pub fn lines(
        &mut self,
        state: &AppState,
        bodies: &Bodies,
        now: (i64, Zone),
        width: u16,
        height: u16,
    ) -> Vec<Line<'static>> {
        self.sync(state);
        let cursor = self.cursor(state);
        let (all, row) = wrapped(state, bodies, now, usize::from(width), cursor);
        self.height = usize::from(height);
        self.len = all.len();
        if std::mem::take(&mut self.follow)
            && let Some(row) = row
        {
            self.scroll = row;
        }
        self.scroll = self.scroll.min(self.len.saturating_sub(self.height));
        all.into_iter()
            .skip(self.scroll)
            .take(self.height)
            .collect()
    }
}

/// Every line of the open thread, wrapped to `width`, and the row where the cursor's message
/// starts.
fn wrapped(
    state: &AppState,
    bodies: &Bodies,
    (now_ms, zone): (i64, Zone),
    width: usize,
    cursor: Option<MessageId>,
) -> (Vec<Line<'static>>, Option<usize>) {
    let Some(thread) = state.view.thread.and_then(|id| state.threads.get(&id)) else {
        return (
            vec![Line::from(Span::styled(s(Key::NoConversationTui), dim()))],
            None,
        );
    };
    let mut marked = None;
    let mut out: Vec<(String, Style)> = Vec::new();
    let subject = match thread.subject.trim() {
        "" => s(Key::NoSubject).to_owned(),
        s => clean(s),
    };
    out.push((subject, bold()));
    for message in thread
        .messages
        .iter()
        .filter_map(|id| state.messages.get(id))
    {
        out.push((String::new(), Style::default()));
        let start = out.len();
        if listing::is_expanded(state, thread, message) {
            message_lines(
                state,
                message,
                bodies.get(message.id),
                (now_ms, zone),
                &mut out,
            );
        } else {
            out.push((
                collapsed_line(message, bodies.get(message.id), now_ms),
                dim(),
            ));
        }
        if cursor == Some(message.id) {
            marked = Some(start);
            if let Some((text, style)) = out.get_mut(start) {
                *text = format!("▶ {}", text.trim_start_matches("▸ "));
                *style = bold();
            }
            out.insert(start + 1, (actions_line(state, message.id), bold()));
        }
    }
    let mut row = None;
    let mut lines = Vec::new();
    for (index, (text, style)) in out.into_iter().enumerate() {
        if marked == Some(index) {
            row = Some(lines.len());
        }
        lines.extend(
            wrap(&text, width)
                .into_iter()
                .map(|piece| Line::from(Span::styled(piece, style))),
        );
    }
    (lines, row)
}

/// `  r Reply · a Reply all · …`: the cursor message's actions with the keys that run them.
fn actions_line(state: &AppState, message: MessageId) -> String {
    use app_front::message_actions::{MessageAction, message_actions};
    let items: Vec<String> = message_actions(state, message)
        .into_iter()
        .map(|item| {
            let key = match item.action {
                MessageAction::Edit => Some("Enter".to_owned()),
                // Unstar has no key of its own: the star key toggles.
                MessageAction::Unstar => {
                    crate::keys::hint(&state.settings, app_core::CommandName::Star)
                }
                action => crate::keys::hint(&state.settings, action.key_command()),
            };
            let label = s(item.action.label());
            key.map_or_else(|| label.to_owned(), |key| format!("{key} {label}"))
        })
        .collect();
    format!("  {}", items.join(" · "))
}

/// `▸ From · age · the start of the text` (at most 60 characters of it).
fn collapsed_line(message: &Envelope, body: Option<&Body>, now_ms: i64) -> String {
    let from = message
        .from
        .first()
        .map_or_else(|| s(Key::NoSender).to_owned(), format::full);
    let snippet = body
        .map(|b| fold::split(&b.text).main)
        .and_then(|text| text.lines().find(|l| !l.trim().is_empty()))
        .unwrap_or_default()
        .trim();
    let snippet: String = snippet.chars().take(60).collect();
    clean(&format!(
        "▸ {from} · {} · {snippet}",
        format::age(message.date_ms, now_ms)
    ))
}

fn message_lines(
    state: &AppState,
    message: &Envelope,
    body: Option<&Body>,
    (now_ms, zone): (i64, Zone),
    out: &mut Vec<(String, Style)>,
) {
    let from = message
        .from
        .first()
        .map_or_else(|| s(Key::NoSender).to_owned(), |a| clean(&format::full(a)));
    out.push((f(Key::FromHeader, &[("from", &from)]), bold()));
    for (label, addrs) in [(s(Key::ToLabel), &message.to), ("Cc", &message.cc)] {
        if !addrs.is_empty() {
            let list: Vec<String> = addrs.iter().map(|a| clean(&format::full(a))).collect();
            out.push((format!("{label}: {}", list.join(", ")), dim()));
        }
    }
    out.push((
        f(
            Key::DateLabel,
            &[
                ("date", &format::long_date(message.date_ms, zone)),
                ("age", &format::age(message.date_ms, now_ms)),
            ],
        ),
        dim(),
    ));
    if let Some(mailbox) = state.mailboxes.get(&message.location())
        && mailbox.role != Some(MailboxRole::Inbox)
    {
        out.push((
            f(
                Key::DateLine,
                &[("mailbox", &clean(&mailbox_name(mailbox)))],
            ),
            dim(),
        ));
    }
    out.push((String::new(), Style::default()));
    match (&message.body, body) {
        (_, Some(body)) => body_lines(body, out),
        (BodyState::Failed { error }, None) => {
            out.push((f(Key::BodyFailed, &[("error", &clean(error))]), bold()))
        }
        _ => out.push((s(Key::Loading).to_owned(), dim())),
    }
}

fn body_lines(body: &Body, out: &mut Vec<(String, Style)>) {
    if body.text.trim().is_empty() {
        out.push((s(Key::NoText).to_owned(), dim()));
    } else {
        let lines = clean_lines(body.text.trim_end());
        out.extend(lines.into_iter().map(|l| (l, Style::default())));
    }
    if !body.attachments.is_empty() {
        out.push((String::new(), Style::default()));
        out.push((
            f(
                Key::AttachmentsCount,
                &[("n", &body.attachments.len().to_string())],
            ),
            bold(),
        ));
        for a in &body.attachments {
            out.push((
                format!(
                    "  {} · {} · {}",
                    clean(&a.name),
                    format::size(a.size),
                    clean(&a.content_type)
                ),
                Style::default(),
            ));
        }
    }
    let links = links(&body.text);
    if !links.is_empty() {
        out.push((String::new(), Style::default()));
        out.push((
            f(Key::LinksCount, &[("n", &links.len().to_string())]),
            bold(),
        ));
        // The full target, wrapped rather than cut: what you see is where it goes.
        out.extend(
            links
                .into_iter()
                .map(|url| (format!("  {}", clean(&url)), dim())),
        );
    }
}
