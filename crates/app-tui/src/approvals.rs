//! Agents' send requests (`ARCHITECTURE.md §4.6`): a banner while `state.approvals` is not
//! empty, and the requests view, opened by *Show send requests* (`list_approvals`) in the palette.
//!
//! The view shows each request in full — the account, every recipient, the first lines of the
//! text, every attachment with its full path and its size or state — so the user sees what the
//! agent wrote before deciding. Approving asks for a confirmation first. The text is the agent's
//! and may quote email: it is cleaned and shown, never interpreted.

use app_core::{AppState, Command};
use app_front::approvals::{Card, cards};
use ratatui::Frame;
use ratatui::crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Clear, Paragraph};

use crate::request::Request;
use crate::text::{clean, clean_lines, wrap};
use crate::view::{bold, dim, reverse};

/// The banner above the panes, if an agent is waiting.
pub fn banner(state: &AppState) -> Option<Line<'static>> {
    let n = state.approvals.len();
    if n == 0 {
        return None;
    }
    let what = if n == 1 {
        "a message".to_owned()
    } else {
        format!("{n} messages")
    };
    Some(
        Line::from(vec![
            Span::styled(format!("! An agent wants to send {what}"), bold()),
            Span::raw(" — Ctrl+K, “Show send requests”"),
        ])
        .style(reverse()),
    )
}

/// What a key in the view led to.
#[derive(Debug, Default)]
pub struct Outcome {
    /// Submit these.
    pub requests: Vec<Request>,
    /// Close the view.
    pub close: bool,
    /// Tell the user something.
    pub notice: Option<String>,
}

/// The requests view's UI-local state.
#[derive(Debug, Default)]
pub struct ApprovalsView {
    cursor: usize,
    /// Asking "send it?" for the request under the cursor.
    confirming: bool,
    scroll: usize,
    height: usize,
    len: usize,
}

impl ApprovalsView {
    /// Handles a key while the view is open.
    pub fn key(&mut self, key: KeyEvent, state: &AppState) -> Outcome {
        let cards = cards(state);
        let mut out = Outcome::default();
        if cards.is_empty() {
            out.close = true;
            return out;
        }
        self.cursor = self.cursor.min(cards.len() - 1);
        let Some(card) = cards.get(self.cursor) else {
            return out;
        };
        if self.confirming {
            self.confirming = false;
            if key.code == KeyCode::Char('y') {
                out.requests
                    .push(Request::user(Command::ApproveRequest { id: card.id }));
            } else {
                out.notice = Some("Not sent.".to_owned());
            }
            return out;
        }
        match key.code {
            KeyCode::Char('j') | KeyCode::Down | KeyCode::Tab => {
                self.cursor = (self.cursor + 1).min(cards.len() - 1);
                self.scroll = 0;
            }
            KeyCode::Char('k') | KeyCode::Up | KeyCode::BackTab => {
                self.cursor = self.cursor.saturating_sub(1);
                self.scroll = 0;
            }
            KeyCode::Char(' ') | KeyCode::PageDown => self.scroll_by(self.page()),
            KeyCode::Char('b') | KeyCode::PageUp => self.scroll_by(-self.page()),
            KeyCode::Char('a') if card.ready => self.confirming = true,
            KeyCode::Char('a') => {
                out.notice = Some("Its attachments are still being checked.".to_owned());
            }
            KeyCode::Char('e') => {
                // The user's edits keep the request.
                out.requests
                    .push(Request::user(Command::OpenComposer { id: card.draft }));
                out.close = true;
            }
            KeyCode::Char('r') => {
                out.requests
                    .push(Request::user(Command::RejectRequest { id: card.id }));
            }
            KeyCode::Char('q') | KeyCode::Esc | KeyCode::Backspace => out.close = true,
            _ => {}
        }
        out
    }

    fn page(&self) -> isize {
        isize::try_from(self.height.saturating_sub(1).max(1)).unwrap_or(1)
    }

    fn scroll_by(&mut self, rows: isize) {
        self.scroll = self
            .scroll
            .saturating_add_signed(rows)
            .min(self.len.saturating_sub(self.height));
    }

    /// Draws the view over `area`.
    pub fn draw(&mut self, frame: &mut Frame, area: Rect, state: &AppState, borders: bool) {
        let cards = cards(state);
        self.cursor = self.cursor.min(cards.len().saturating_sub(1));
        frame.render_widget(Clear, area);
        let block = if borders {
            Block::bordered()
                .border_style(bold())
                .title(" Send requests ")
        } else {
            Block::new()
        };
        let inner = block.inner(area);
        frame.render_widget(block, area);
        let columns = usize::from(inner.width);
        let mut body: Vec<(String, Style)> = Vec::new();
        for (i, card) in cards.iter().enumerate() {
            card_lines(card, i == self.cursor, &mut body);
        }
        if cards.is_empty() {
            body.push(("No send requests.".to_owned(), dim()));
        }
        let wrapped: Vec<Line<'static>> = body
            .into_iter()
            .flat_map(|(text, style)| {
                wrap(&text, columns)
                    .into_iter()
                    .map(move |piece| Line::from(Span::styled(piece, style)))
            })
            .collect();
        let footer = match cards.get(self.cursor) {
            Some(card) if self.confirming => Line::from(Span::styled(
                format!(
                    "Send “{}” now? y sends (with the undo window), any other key doesn't",
                    clean(&card.subject)
                ),
                bold(),
            )),
            Some(card) if !card.ready => Line::from(Span::styled(
                "attachments are being checked · e edit · r reject · q close",
                dim(),
            )),
            Some(_) => Line::from(Span::styled(
                "a approve · e edit · r reject · j/k next · q close",
                dim(),
            )),
            None => Line::from(Span::styled("q close", dim())),
        };
        self.height = usize::from(inner.height).saturating_sub(1);
        self.len = wrapped.len();
        self.scroll = self.scroll.min(self.len.saturating_sub(self.height));
        let mut lines: Vec<Line<'static>> = wrapped
            .into_iter()
            .skip(self.scroll)
            .take(self.height)
            .collect();
        while lines.len() < self.height {
            lines.push(Line::from(""));
        }
        lines.push(footer);
        frame.render_widget(Paragraph::new(lines), inner);
    }
}

fn card_lines(card: &Card, selected: bool, out: &mut Vec<(String, Style)>) {
    let marker = if selected { "› " } else { "  " };
    let title = if selected { bold() } else { Style::default() };
    out.push((format!("{marker}{}", clean(&card.subject)), title));
    if !card.from.is_empty() {
        out.push((format!("  From: {}", clean(&card.from)), dim()));
    }
    for line in &card.recipients {
        out.push((format!("  {}", clean(line)), Style::default()));
    }
    for line in clean_lines(&card.excerpt) {
        out.push((format!("  │ {line}"), dim()));
    }
    for line in &card.attachments {
        out.push((format!("  @ {}", clean(line)), Style::default()));
    }
    out.push((String::new(), Style::default()));
}
