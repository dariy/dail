//! The reader: the open thread, oldest message first. A header per message (from, to, cc, date),
//! the text, the attachments and *Links (N)* with their full targets. Bodies come from the shared
//! body tracking (`app_front::body`). Everything is email content: cleaned, wrapped, never
//! interpreted. There is no browser and no "open original": over SSH they would open on the host.
//! A collapsed message (`collapse_messages`; by default the read ones but the newest) is one line.

use app_core::{AppState, BodyState, Envelope, MailboxRole, ThreadId, listing};
use app_front::body::{Bodies, Body, links};
use app_front::{fold, format, mailbox::mailbox_name};
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

    /// The visible lines of the open thread for a pane of `width`×`height`.
    pub fn lines(
        &mut self,
        state: &AppState,
        bodies: &Bodies,
        now_ms: i64,
        width: u16,
        height: u16,
    ) -> Vec<Line<'static>> {
        if self.thread != state.view.thread {
            self.thread = state.view.thread;
            self.scroll = 0;
        }
        let all = wrapped(state, bodies, now_ms, usize::from(width));
        self.height = usize::from(height);
        self.len = all.len();
        self.scroll = self.scroll.min(self.len.saturating_sub(self.height));
        all.into_iter()
            .skip(self.scroll)
            .take(self.height)
            .collect()
    }
}

/// Every line of the open thread, wrapped to `width`.
fn wrapped(state: &AppState, bodies: &Bodies, now_ms: i64, width: usize) -> Vec<Line<'static>> {
    let Some(thread) = state.view.thread.and_then(|id| state.threads.get(&id)) else {
        return vec![Line::from(Span::styled(
            "No conversation open. Enter opens the selected one.",
            dim(),
        ))];
    };
    let mut out: Vec<(String, Style)> = Vec::new();
    let subject = match thread.subject.trim() {
        "" => "(no subject)".to_owned(),
        s => clean(s),
    };
    out.push((subject, bold()));
    for message in thread
        .messages
        .iter()
        .filter_map(|id| state.messages.get(id))
    {
        out.push((String::new(), Style::default()));
        if listing::is_expanded(state, thread, message) {
            message_lines(state, message, bodies.get(message.id), now_ms, &mut out);
        } else {
            out.push((
                collapsed_line(message, bodies.get(message.id), now_ms),
                dim(),
            ));
        }
    }
    out.into_iter()
        .flat_map(|(text, style)| {
            wrap(&text, width)
                .into_iter()
                .map(move |piece| Line::from(Span::styled(piece, style)))
        })
        .collect()
}

/// `▸ From · age · the start of the text` (at most 60 characters of it).
fn collapsed_line(message: &Envelope, body: Option<&Body>, now_ms: i64) -> String {
    let from = message
        .from
        .first()
        .map_or_else(|| "(no sender)".to_owned(), format::full);
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
    now_ms: i64,
    out: &mut Vec<(String, Style)>,
) {
    let from = message
        .from
        .first()
        .map_or_else(|| "(no sender)".to_owned(), |a| clean(&format::full(a)));
    out.push((format!("From: {from}"), bold()));
    for (label, addrs) in [("To", &message.to), ("Cc", &message.cc)] {
        if !addrs.is_empty() {
            let list: Vec<String> = addrs.iter().map(|a| clean(&format::full(a))).collect();
            out.push((format!("{label}: {}", list.join(", ")), dim()));
        }
    }
    out.push((
        format!(
            "Date: {} ({})",
            format::long_date(message.date_ms),
            format::age(message.date_ms, now_ms)
        ),
        dim(),
    ));
    if let Some(mailbox) = state.mailboxes.get(&message.location())
        && mailbox.role != Some(MailboxRole::Inbox)
    {
        out.push((format!("In: {}", clean(&mailbox_name(mailbox))), dim()));
    }
    out.push((String::new(), Style::default()));
    match (&message.body, body) {
        (_, Some(body)) => body_lines(body, out),
        (BodyState::Failed { error }, None) => out.push((
            format!("The body could not be downloaded: {}", clean(error)),
            bold(),
        )),
        _ => out.push(("Loading…".to_owned(), dim())),
    }
}

fn body_lines(body: &Body, out: &mut Vec<(String, Style)>) {
    if body.text.trim().is_empty() {
        out.push(("(no text)".to_owned(), dim()));
    } else {
        let lines = clean_lines(body.text.trim_end());
        out.extend(lines.into_iter().map(|l| (l, Style::default())));
    }
    if !body.attachments.is_empty() {
        out.push((String::new(), Style::default()));
        out.push((format!("Attachments ({})", body.attachments.len()), bold()));
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
        out.push((format!("Links ({})", links.len()), bold()));
        // The full target, wrapped rather than cut: what you see is where it goes.
        out.extend(
            links
                .into_iter()
                .map(|url| (format!("  {}", clean(&url)), dim())),
        );
    }
}
