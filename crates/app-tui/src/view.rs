//! Drawing: the header line, the panes of the [`layout`](crate::layout) and the status line.
//! Only the terminal's own colours: default foreground and background, bold, dim and reverse.
//! Email text goes through [`text::clean`](crate::text::clean).

use app_core::{AppState, ThreadSummary, listing};
use app_front::{format, mailbox::mailbox_name};
use ratatui::Frame;
use ratatui::layout::{Alignment, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Paragraph};

use crate::layout::{MIN_HEIGHT, MIN_WIDTH, Slot};
use crate::text::{clean, pad};

/// Bold.
pub fn bold() -> Style {
    Style::default().add_modifier(Modifier::BOLD)
}

/// Dim.
pub fn dim() -> Style {
    Style::default().add_modifier(Modifier::DIM)
}

/// Reverse video (the cursor).
pub fn reverse() -> Style {
    Style::default().add_modifier(Modifier::REVERSED)
}

/// The notice below 40×12.
pub fn too_small(frame: &mut Frame, area: Rect) {
    let text = vec![
        Line::from("Terminal too small"),
        Line::from(format!("{}×{} needed", MIN_WIDTH, MIN_HEIGHT)),
    ];
    frame.render_widget(Paragraph::new(text).alignment(Alignment::Center), area);
}

/// The title of a slot (in its border, or in the header line on one pane).
pub fn slot_title(slot: Slot, state: &AppState) -> String {
    match slot {
        Slot::Mailboxes => "Mailboxes".to_owned(),
        Slot::List => list_title(state),
        Slot::Reader => "Message".to_owned(),
        Slot::Composer => "Draft".to_owned(),
    }
}

/// The list's title: the search, or the open mailbox.
pub fn list_title(state: &AppState) -> String {
    if let Some(search) = &state.view.search {
        return format!("Search: {}", clean(search));
    }
    if state.view.unified {
        return "All inboxes".to_owned();
    }
    state
        .view
        .mailbox
        .and_then(|id| state.mailboxes.get(&id))
        .map_or_else(|| "Conversations".to_owned(), mailbox_name)
}

/// A pane's frame: a border with its title from 80 columns (bold when focused), nothing below.
pub fn pane_block(title: String, focused: bool, borders: bool) -> Block<'static> {
    if !borders {
        return Block::new();
    }
    let style = if focused { bold() } else { dim() };
    Block::bordered()
        .border_style(style)
        .title(Span::styled(format!(" {title} "), style))
}

/// The header line: `dail · <place> · <account>`.
pub fn header(state: &AppState, one_pane: Option<Slot>) -> Line<'static> {
    let mut spans = vec![Span::styled("dail", bold())];
    let place = one_pane.map_or_else(|| list_title(state), |slot| slot_title(slot, state));
    spans.push(Span::raw(format!(" · {place}")));
    if let Some(account) = state
        .view
        .mailbox
        .and_then(|id| state.mailboxes.get(&id))
        .and_then(|m| state.accounts.get(&m.account))
    {
        spans.push(Span::styled(format!(" · {}", account.email), dim()));
    }
    Line::from(spans)
}

/// The accounts and their mailboxes, each line with the mailbox it shows (for taps); the open
/// one is marked.
pub fn mailboxes(state: &AppState) -> Vec<(Line<'static>, Option<app_core::MailboxId>)> {
    let mut lines = Vec::new();
    for account in state.accounts.values() {
        let name = Line::from(Span::styled(clean(&account.display_name), bold()));
        lines.push((name, None));
        for mailbox in listing::mailboxes(state, Some(account.id)) {
            let open = state.view.mailbox == Some(mailbox.id) && state.view.search.is_none();
            let mut spans = vec![
                Span::raw(if open { "▸ " } else { "  " }),
                Span::raw(clean(&mailbox_name(mailbox))),
            ];
            if mailbox.unread > 0 {
                spans.push(Span::styled(format!(" {}", mailbox.unread), bold()));
            }
            let line = Line::from(spans);
            let line = if open { line.style(reverse()) } else { line };
            lines.push((line, Some(mailbox.id)));
        }
        lines.push((Line::from(""), None));
    }
    lines
}

/// One row of the thread list, `width` columns wide: checked `✓` (else the cursor `›`), unread
/// `●`, starred `★`, attachment `@`, sender, subject and date. Unread rows are bold, the
/// cursor's row reversed.
pub fn thread_row(
    summary: &ThreadSummary<'_>,
    now_ms: i64,
    width: u16,
    (cursor, checked): (bool, bool),
) -> Line<'static> {
    let flags = format!(
        "{}{}{}{}",
        if checked {
            '✓'
        } else if cursor {
            '›'
        } else {
            ' '
        },
        if summary.unread > 0 { '●' } else { ' ' },
        if summary.flagged { '★' } else { ' ' },
        if summary.has_attachments { '@' } else { ' ' },
    );
    let width = usize::from(width).saturating_sub(5);
    let sender = summary
        .senders
        .first()
        .map_or_else(|| "(no sender)".to_owned(), |a| clean(format::name(a)));
    let subject = match summary.thread.subject.trim() {
        "" => "(no subject)".to_owned(),
        s => clean(s),
    };
    let date = format::short_date(summary.thread.last_date_ms, now_ms);
    let sender_width = (width / 4).clamp(6, 18);
    let rest = width.saturating_sub(sender_width + date.chars().count() + 3);
    let style = if summary.unread > 0 {
        bold()
    } else {
        Style::default()
    };
    let line = Line::from(vec![
        Span::raw(flags),
        Span::raw(" "),
        Span::styled(pad(&sender, sender_width), style),
        Span::raw(" "),
        Span::styled(pad(&subject, rest), style),
        Span::raw(" "),
        Span::styled(date, dim()),
    ]);
    if cursor { line.style(reverse()) } else { line }
}

/// The first row to show so that `cursor` stays visible in `height` rows of `len`, moving
/// `offset` as little as possible.
pub const fn scroll_to(offset: usize, cursor: Option<usize>, height: usize, len: usize) -> usize {
    let mut offset = offset;
    if let Some(cursor) = cursor {
        if cursor < offset {
            offset = cursor;
        } else if height > 0 && cursor >= offset + height {
            offset = cursor + 1 - height;
        }
    }
    let max = len.saturating_sub(height);
    if offset > max { max } else { offset }
}

/// What the status line shows besides the state's own status.
#[derive(Debug, Default)]
pub struct StatusParts<'a> {
    /// A question for the user; it takes the whole line.
    pub prompt: Option<&'a str>,
    /// `Sending in 7 s` while a send of this front-end is in its undo window.
    pub sending: Option<String>,
    /// A rejection or runtime failure, for a few seconds.
    pub notice: Option<&'a str>,
    /// The label of the latest undoable action.
    pub undo: Option<&'a str>,
    /// The language of the shared texts.
    pub lang: app_front::i18n::Lang,
}

/// The status line, `width` columns: the parts that fit, most important first — a send's
/// countdown, the latest notice, problems (store, keyring, login, sync), pending and failed
/// changes, the latest undoable action with `z`, and the sync state — then `? help` if there is
/// room.
pub fn status(state: &AppState, now_ms: i64, parts: &StatusParts<'_>, width: u16) -> Line<'static> {
    use app_front::status;
    if let Some(prompt) = parts.prompt {
        return Line::from(Span::styled(
            pad(&clean(prompt), usize::from(width)),
            reverse(),
        ));
    }
    let mut segments: Vec<(String, Style)> = Vec::new();
    if let Some(sending) = &parts.sending {
        segments.push((format!("{sending} · z cancels"), bold()));
    }
    if let Some(notice) = parts.notice {
        segments.push((clean(notice), bold()));
    }
    let problems = status::problems(state, parts.lang);
    if let Some(first) = problems.first() {
        let more = match problems.len() {
            1 => String::new(),
            n => format!(" (+{} more, ? for details)", n - 1),
        };
        // The fix is in the palette (no buttons in a status line).
        let fix = match first.fix {
            Some(status::Fix::Password(_)) => " (palette: Change account password)",
            Some(status::Fix::Retry(_)) => " (palette: Sync now)",
            Some(status::Fix::Test(_)) => " (palette: Test account connection)",
            None => "",
        };
        segments.push((format!("{}{fix}{more}", clean(&first.text)), bold()));
    }
    if let Some((text, failed)) = status::pending_text(state, parts.lang) {
        let text = if failed {
            format!("{text} (retry: palette)")
        } else {
            text
        };
        segments.push((text, if failed { bold() } else { Style::default() }));
    }
    // While a send counts down, `z` cancels it: the countdown says so.
    if let Some(label) = parts.undo.filter(|_| parts.sending.is_none()) {
        segments.push((format!("“{}” z undo", clean(label)), Style::default()));
    }
    segments.push((status::sync_text(state, now_ms, parts.lang), dim()));
    if status::searching(state) {
        segments.push(("searching the server…".to_owned(), dim()));
    }
    let width = usize::from(width);
    let mut used = 0;
    let mut spans = Vec::new();
    for (text, style) in segments {
        let w = crate::text::width(&text) + if spans.is_empty() { 0 } else { 3 };
        if used + w > width {
            if spans.is_empty() {
                // The most important part always shows, cut to the line.
                spans.push(Span::styled(pad(&text, width), style));
                used = width;
            }
            continue;
        }
        if !spans.is_empty() {
            spans.push(Span::styled(" · ", dim()));
        }
        used += w;
        spans.push(Span::styled(text, style));
    }
    let help = "? help";
    if used + help.len() + 2 <= width {
        spans.push(Span::raw(" ".repeat(width - used - help.len())));
        spans.push(Span::styled(help, dim()));
    }
    Line::from(spans)
}
