//! Drawing: the header line, the panes of the [`layout`](crate::layout) and the status line.
//! Only the terminal's own colours: default foreground and background, bold, dim and reverse.
//! Email text goes through [`text::clean`](crate::text::clean).

use crate::text::{f, s};
use app_core::{AppState, ThreadSummary};
use app_front::i18n::Key;
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
        Line::from(s(Key::TerminalTooSmall)),
        Line::from(format!("{}×{} needed", MIN_WIDTH, MIN_HEIGHT)),
    ];
    frame.render_widget(Paragraph::new(text).alignment(Alignment::Center), area);
}

/// The title of a slot (in its border, or in the header line on one pane).
pub fn slot_title(slot: Slot, state: &AppState) -> String {
    match slot {
        Slot::Mailboxes => s(Key::Mailboxes).to_owned(),
        Slot::List => list_title(state),
        Slot::Reader => s(Key::Message).to_owned(),
        Slot::Composer => s(Key::Draft).to_owned(),
    }
}

/// The list's title: the search, or the open mailbox.
pub fn list_title(state: &AppState) -> String {
    if let Some(search) = &state.view.search {
        return f(Key::SearchTitle, &[("query", &clean(search))]);
    }
    if state.view.unified {
        return s(Key::AllInboxes).to_owned();
    }
    let title = state
        .view
        .mailbox
        .and_then(|id| state.mailboxes.get(&id))
        .map_or_else(|| s(Key::Conversations).to_owned(), mailbox_name);
    let mut title = if state.view.focused.is_some() {
        f(Key::TitleFocus, &[("title", &title)])
    } else {
        title
    };
    let filter = state.view.filter;
    let only: Vec<&str> = [
        (filter.unread, s(Key::UnreadX)),
        (filter.starred, s(Key::StarredX)),
        (filter.attachments, s(Key::AttachmentsX)),
    ]
    .into_iter()
    .filter_map(|(on, name)| on.then_some(name))
    .collect();
    if !only.is_empty() {
        title = f(
            Key::TitleOnly,
            &[("title", &title), ("only", &only.join(", "))],
        );
    }
    title
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

/// The accounts and their folder trees (UX-2), each line with the mailbox it shows (for taps):
/// two spaces per level, `▾` on an expanded parent and `▸` on a collapsed one (with the
/// unread mail it hides, `+n`), the open one marked `›` and reversed.
pub fn mailboxes(state: &AppState) -> Vec<(Line<'static>, Option<app_core::MailboxId>)> {
    let mut lines = Vec::new();
    if state.view.focused.is_some() {
        // Focus mode (UX-4): `Exit focus` in the palette shows every folder again.
        lines.push((Line::from(Span::styled(s(Key::FocusFooter), dim())), None));
    }
    let favourites = app_front::mailbox::favourites(state);
    if !favourites.is_empty() {
        // UX-8: pinned folders first.
        lines.push((Line::from(Span::styled(s(Key::Favourites), bold())), None));
        for mailbox in favourites {
            let open = state.view.mailbox == Some(mailbox.id) && state.view.search.is_none();
            let mut spans = vec![
                Span::raw(if open { "› " } else { "  " }),
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
    for account in state.accounts.values() {
        if !app_front::mailbox::shows_account(state, account.id) {
            continue;
        }
        let folded = app_front::mailbox::account_collapsed(state, account.id);
        let marker = if folded { "▸ " } else { "" };
        let mut name = vec![
            Span::raw(marker),
            Span::styled(clean(&account.display_name), bold()),
        ];
        // UX-9: the account's sync state.
        match app_front::status::account_state(state, account.id) {
            app_front::status::AccountState::Idle => {}
            app_front::status::AccountState::Syncing => name.push(Span::raw(" ⟳")),
            app_front::status::AccountState::Failed(_) => name.push(Span::styled(" ⚠", bold())),
            app_front::status::AccountState::Offline { .. } => {
                name.push(Span::styled(s(Key::OfflineSuffix), dim()));
            }
        }
        let name = Line::from(name);
        lines.push((name, None));
        let nodes = app_front::mailbox::sidebar_tree(state, account.id);
        let shown = app_front::mailbox::visible_rows(state, account.id, &nodes);
        let open_hidden = folded
            .then(|| {
                app_front::mailbox::rows(&nodes, &|_| false)
                    .into_iter()
                    .find(|n| n.mailbox.is_some() && n.mailbox == state.view.mailbox)
            })
            .flatten();
        // The open mailbox is never hidden: a collapsed account still shows it.
        for node in shown.into_iter().chain(open_hidden) {
            let open = node.mailbox.is_some()
                && state.view.mailbox == node.mailbox
                && state.view.search.is_none();
            let collapsed = state.view.collapsed.contains(&node.key);
            let fold = match (node.is_parent(), collapsed) {
                (false, _) => "",
                (true, false) => "▾ ",
                (true, true) => "▸ ",
            };
            let label = if node.mailbox.is_none() {
                Span::styled(clean(&node.label), dim())
            } else {
                Span::raw(clean(&node.label))
            };
            let mut spans = vec![
                Span::raw(if open { "› " } else { "  " }),
                Span::raw("  ".repeat(node.depth)),
                Span::raw(fold),
                label,
            ];
            // A parent folder counts the unread mail below it too.
            if node.unread_total() > 0 {
                spans.push(Span::styled(format!(" {}", node.unread_total()), bold()));
            }
            let line = Line::from(spans);
            let line = if open { line.style(reverse()) } else { line };
            lines.push((line, node.mailbox));
        }
        lines.push((Line::from(""), None));
    }
    lines
}

/// One row of the thread list, `width` columns wide: checked `✓` (else the cursor `›`), unread
/// `●`, starred `★`, attachment `@`, sender, subject (after `▲` when pinned) and date. Unread
/// rows are bold, the cursor's row reversed.
pub fn thread_row(
    summary: &ThreadSummary<'_>,
    (now_ms, zone): (i64, format::Zone),
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
        .map_or_else(|| s(Key::NoSender).to_owned(), |a| clean(format::name(a)));
    let subject = match summary.thread.subject.trim() {
        "" => s(Key::NoSubject).to_owned(),
        s => clean(s),
    };
    // MISC-4: pinned threads (at the top) say so before the subject.
    let subject = if summary.pinned {
        format!("▲ {subject}")
    } else {
        subject
    };
    let date = format::short_date(summary.thread.last_date_ms, now_ms, zone);
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
/// changes, the latest undoable action with `z`, a waiting update, and the sync state — then `? help` if there is
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
        segments.push((f(Key::SendingCancel, &[("sending", sending)]), bold()));
    }
    if let Some(notice) = parts.notice {
        segments.push((clean(notice), bold()));
    }
    let problems = status::problems(state, parts.lang);
    if let Some(first) = problems.first() {
        let more = match problems.len() {
            1 => String::new(),
            n => f(Key::MoreDetails, &[("n", &(n - 1).to_string())]),
        };
        // The fix is in the palette (no buttons in a status line).
        let fix = match first.fix {
            Some(status::Fix::Password(_)) => s(Key::FixPalettePassword),
            Some(status::Fix::Retry(_)) => s(Key::FixPaletteSync),
            Some(status::Fix::Test(_)) => s(Key::FixPaletteTest),
            None => "",
        };
        segments.push((format!("{}{fix}{more}", clean(&first.text)), bold()));
    }
    if let Some((text, failed)) = status::pending_text(state, parts.lang) {
        let text = if failed {
            f(Key::RetryPalette, &[("text", &text)])
        } else {
            text
        };
        segments.push((text, if failed { bold() } else { Style::default() }));
    }
    // While a send counts down, `z` cancels it: the countdown says so.
    if let Some(label) = parts.undo.filter(|_| parts.sending.is_none()) {
        segments.push((
            f(Key::UndoLabel, &[("label", &clean(label))]),
            Style::default(),
        ));
    }
    // Wave 7 F1/F3: a waiting update, or the unanswered question about checks.
    if let Some(badge) = app_front::updates::badge(state) {
        segments.push((f(Key::BadgeSettings, &[("badge", &badge)]), bold()));
    } else if app_front::updates::ask_first_run(state) {
        segments.push((s(Key::UpdatesQuestion).to_owned(), Style::default()));
    }
    segments.push((status::sync_text(state, now_ms, parts.lang), dim()));
    if status::searching(state) {
        segments.push((s(Key::SearchingServer).to_owned(), dim()));
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
    let help = s(Key::HelpHint);
    if used + help.len() + 2 <= width {
        spans.push(Span::raw(" ".repeat(width - used - help.len())));
        spans.push(Span::styled(help, dim()));
    }
    Line::from(spans)
}
