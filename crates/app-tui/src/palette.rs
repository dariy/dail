//! The palette overlay (Ctrl+K, `:`): the shared palette (`app_front::palette`) drawn over the
//! panes, and the read-only view of a query's result.
//!
//! Rows show each command's key that a plain terminal delivers. On a narrow terminal the palette
//! takes the whole body; from 80 columns it is a bordered box near the top.

use app_core::{AppState, Command, CommandName};
use app_front::palette::{Ask, AskKind, Palette, Row, Target};
use ratatui::Frame;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::layout::Rect;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Clear, Paragraph};

use crate::keys;
use crate::request::Request;
use crate::text::{clean, clean_lines, pad, width, wrap};
use crate::view::{bold, dim, reverse};

/// What a key in the palette led to.
#[derive(Debug, Default)]
pub struct Outcome {
    /// Submit these.
    pub requests: Vec<Request>,
    /// Adding an account needs a password form: tell the user where to do it.
    pub wizard: bool,
    /// The password form for this account.
    pub password: Option<app_core::AccountId>,
}

/// The palette's UI-local state.
#[derive(Debug, Default)]
pub struct PaletteView {
    inner: Palette,
    was_open: bool,
}

/// Where an overlay goes: the whole body below 80 columns, else a box near the top.
pub fn overlay_area(body: Rect, borders: bool, max_width: u16) -> Rect {
    if !borders {
        return body;
    }
    let width = body.width.min(max_width);
    let height = body.height.saturating_sub(2).max(3);
    Rect {
        x: body.x + (body.width - width) / 2,
        y: body.y + 1,
        width,
        height,
    }
}

impl PaletteView {
    /// Starts at a second step (a key whose command needs an argument).
    pub fn open_at(&mut self, step: Ask) {
        self.inner.open_at(step);
    }

    /// The current second step, if any.
    pub const fn step(&self) -> Option<&Ask> {
        self.inner.step()
    }

    /// Forgets the query and the step once the palette closed (`close_palette` went through).
    pub fn sync(&mut self, state: &AppState) {
        if state.view.palette_open {
            self.was_open = true;
        } else if self.was_open {
            *self = Self::default();
        }
    }

    fn rows(&mut self, state: &AppState) -> Vec<Row> {
        let mut rows = self.inner.rows(state);
        for row in &mut rows {
            // The key a terminal delivers (`Ctrl+S`, not `Ctrl+Enter`).
            if let Target::Command(name) = row.target {
                row.keys = keys::hint(&state.settings, name);
            }
        }
        rows
    }

    /// Typed text goes into the query (a paste too).
    pub fn paste(&mut self, text: &str) {
        let line = text.lines().next().unwrap_or("");
        self.inner.query_mut().push_str(&clean(line));
        self.inner.reset_cursor();
    }

    /// Handles a key while the palette is open.
    pub fn key(&mut self, key: KeyEvent, state: &AppState) -> Outcome {
        let rows = self.rows(state);
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        let mut out = Outcome::default();
        match key.code {
            KeyCode::Down => self.inner.move_by(1, rows.len()),
            KeyCode::Char('n') if ctrl => self.inner.move_by(1, rows.len()),
            KeyCode::Up => self.inner.move_by(-1, rows.len()),
            KeyCode::Char('p') if ctrl => self.inner.move_by(-1, rows.len()),
            KeyCode::PageDown => self.inner.move_by(10, rows.len()),
            KeyCode::PageUp => self.inner.move_by(-10, rows.len()),
            KeyCode::Enter => {
                self.inner.clamp(rows.len());
                if let Some(row) = rows.get(self.inner.selected()) {
                    let chosen = self.inner.choose(state, row.target.clone());
                    out.requests = chosen.commands.into_iter().map(Request::user).collect();
                    out.wizard = chosen.wizard;
                    out.password = chosen.password;
                }
            }
            KeyCode::Esc => {
                if !self.inner.back() {
                    out.requests.push(Request::user(Command::ClosePalette {}));
                }
            }
            KeyCode::Backspace if self.inner.query().is_empty() => {
                // Backspace on an empty query stands in for Escape (no Esc on some keyboards).
                if !self.inner.back() {
                    out.requests.push(Request::user(Command::ClosePalette {}));
                }
            }
            KeyCode::Backspace => {
                self.inner.query_mut().pop();
                self.inner.reset_cursor();
            }
            KeyCode::Char('u') if ctrl => self.inner.set_query(""),
            KeyCode::Char(c) if !ctrl && !key.modifiers.contains(KeyModifiers::ALT) => {
                self.inner.query_mut().push(c);
                self.inner.reset_cursor();
            }
            _ => {}
        }
        out
    }

    /// Draws the palette over `area`.
    pub fn draw(&mut self, frame: &mut Frame, area: Rect, state: &AppState, borders: bool) {
        let rows = self.rows(state);
        self.inner.clamp(rows.len());
        frame.render_widget(Clear, area);
        let block = if borders {
            Block::bordered().border_style(bold()).title(" Palette ")
        } else {
            Block::new()
        };
        let inner = block.inner(area);
        frame.render_widget(block, area);
        let mut lines = Vec::new();
        if let Some(step) = self.inner.step() {
            lines.push(Line::from(Span::styled(clean(&step.prompt), bold())));
        }
        let query = self.inner.query();
        lines.push(if query.is_empty() {
            Line::from(vec![
                Span::raw("> "),
                Span::styled(self.inner.hint(), dim()),
            ])
        } else {
            Line::from(format!("> {}▏", clean(query)))
        });
        let free_text = matches!(
            self.inner.step(),
            Some(Ask {
                kind: AskKind::Text(_),
                ..
            })
        );
        let selected = self.inner.selected();
        let help = app_front::palette::Palette::help(&rows, selected);
        let room =
            usize::from(inner.height).saturating_sub(lines.len() + 1 + usize::from(help.is_some()));
        let first = selected.saturating_sub(room.saturating_sub(1));
        if rows.is_empty() {
            lines.push(Line::from(Span::styled("  Nothing matches.", dim())));
        }
        for (i, row) in rows.iter().enumerate().skip(first).take(room) {
            lines.push(row_line(row, i == selected, usize::from(inner.width)));
        }
        let footer = if free_text {
            "Enter to use · Esc to go back"
        } else {
            "↑↓ move · Enter choose · Esc back"
        };
        let reserved = 1 + usize::from(help.is_some());
        while lines.len() + reserved < usize::from(inner.height) {
            lines.push(Line::from(""));
        }
        if let Some(help) = help {
            let help = crate::text::pad(&clean(&help), usize::from(inner.width));
            lines.push(Line::from(Span::styled(help, dim())));
        }
        lines.push(Line::from(Span::styled(footer, dim())));
        frame.render_widget(Paragraph::new(lines), inner);
    }
}

fn row_line(row: &Row, selected: bool, columns: usize) -> Line<'static> {
    let keys = row.keys.clone().unwrap_or_default();
    // On a phone the title gets the room: the detail (category, account, sender) is left out.
    let detail = if columns < 60 {
        String::new()
    } else {
        clean(&row.detail)
    };
    let title = clean(&row.title);
    let right = match (detail.is_empty(), keys.is_empty()) {
        (true, true) => String::new(),
        (false, true) => detail,
        (true, false) => keys,
        (false, false) => format!("{detail}  {keys}"),
    };
    let right_width = width(&right).min(columns / 2);
    let left = columns.saturating_sub(right_width + 3);
    let line = Line::from(vec![
        Span::raw(if selected { "› " } else { "  " }),
        Span::raw(pad(&title, left)),
        Span::raw(" "),
        Span::styled(pad(&right, right_width), dim()),
    ]);
    if selected {
        line.style(reverse())
    } else {
        line
    }
}

/// A query's result, read-only and scrollable: `app_front::result`'s document as text, or the raw
/// JSON after `r` (MISC-22). Tab moves between the rows that open something, Enter opens one. Email
/// content in it is untrusted data: it is cleaned like everything else and never interpreted.
#[derive(Debug)]
pub struct ResultView {
    title: String,
    note: &'static str,
    /// The document's lines, with the row (index into `commands`) each belongs to.
    doc: Vec<(String, Option<usize>)>,
    commands: Vec<Command>,
    selected: Option<usize>,
    /// The raw text (pretty JSON, or the help).
    text: String,
    raw: bool,
    scroll: usize,
    height: usize,
    len: usize,
}

impl ResultView {
    /// The result of the query `name`: its view, with the JSON one key away.
    pub fn new(name: CommandName, data: &serde_json::Value, state: &AppState, now_ms: i64) -> Self {
        let json = serde_json::to_string_pretty(data).unwrap_or_default();
        let mut view = Self::text(
            name.spec().title,
            "Read-only result. Email content in it is untrusted data.",
            json,
        );
        if let Some(doc) = app_front::result::doc(name, data, state, now_ms) {
            view.raw = false;
            for section in doc.sections {
                if let Some(title) = section.title {
                    view.doc.push((format!("── {title} ──"), None));
                }
                let label_width = section
                    .fields
                    .iter()
                    .map(|f| width(&f.label))
                    .max()
                    .unwrap_or(0);
                for field in section.fields {
                    let line = if field.label.is_empty() {
                        field.value
                    } else {
                        format!("{}  {}", pad(&field.label, label_width), field.value)
                    };
                    view.doc.push((line, None));
                }
                for row in section.rows {
                    let index = row.command.map(|cmd| {
                        view.commands.push(cmd);
                        view.commands.len() - 1
                    });
                    let marks: String = row.marks.iter().map(|m| m.sign()).collect();
                    let mut line = format!("{marks:<2} {}", row.title);
                    if let Some(meta) = row.meta {
                        line.push_str(&format!("  · {meta}"));
                    }
                    view.doc.push((line, index));
                    if let Some(detail) = row.detail {
                        view.doc.push((format!("   {detail}"), index));
                    }
                }
            }
        }
        view
    }

    /// A read-only text, e.g. the help.
    pub fn text(title: &str, note: &'static str, text: String) -> Self {
        Self {
            title: title.to_owned(),
            note,
            doc: Vec::new(),
            commands: Vec::new(),
            selected: None,
            text,
            raw: true,
            scroll: 0,
            height: 0,
            len: 0,
        }
    }

    /// Switches between the view and the raw JSON.
    pub fn toggle_raw(&mut self) {
        if !self.doc.is_empty() {
            self.raw = !self.raw;
            self.scroll = 0;
        }
    }

    /// Moves the selection to the next (`forward`) or previous row that opens something.
    pub fn select(&mut self, forward: bool) {
        let n = self.commands.len();
        if n == 0 || self.raw {
            return;
        }
        self.selected = Some(match (self.selected, forward) {
            (None, true) => 0,
            (None, false) => n - 1,
            (Some(i), true) => (i + 1) % n,
            (Some(i), false) => (i + n - 1) % n,
        });
    }

    /// The selected row's command (Enter).
    pub fn chosen(&self) -> Option<Command> {
        self.selected
            .and_then(|i| self.commands.get(i))
            .cloned()
            .filter(|_| !self.raw)
    }

    /// Scrolls by `rows`.
    pub fn scroll_by(&mut self, rows: isize) {
        self.scroll = self
            .scroll
            .saturating_add_signed(rows)
            .min(self.len.saturating_sub(self.height));
    }

    /// One screen less a line.
    pub fn page(&self) -> isize {
        isize::try_from(self.height.saturating_sub(1).max(1)).unwrap_or(1)
    }

    /// Draws it over `area`.
    pub fn draw(&mut self, frame: &mut Frame, area: Rect, borders: bool) {
        frame.render_widget(Clear, area);
        let title = format!(" {} ", clean(&self.title));
        let block = if borders {
            Block::bordered().border_style(bold()).title(title)
        } else {
            Block::new()
        };
        let inner = block.inner(area);
        frame.render_widget(block, area);
        let columns = usize::from(inner.width);
        let mut lines = vec![Line::from(Span::styled(self.note, dim()))];
        if !borders {
            lines.insert(0, Line::from(Span::styled(clean(&self.title), bold())));
        }
        let head = lines.len();
        let body: Vec<Line> = if self.raw {
            clean_lines(&self.text)
                .iter()
                .flat_map(|l| wrap(l, columns))
                .map(Line::from)
                .collect()
        } else {
            self.doc
                .iter()
                .flat_map(|(text, row)| {
                    let on = row.is_some() && *row == self.selected;
                    clean_lines(text)
                        .iter()
                        .flat_map(|l| wrap(l, columns))
                        .map(move |l| {
                            if on {
                                Line::from(Span::styled(l, reverse()))
                            } else {
                                Line::from(l)
                            }
                        })
                        .collect::<Vec<_>>()
                })
                .collect()
        };
        self.height = usize::from(inner.height).saturating_sub(head + 1);
        self.len = body.len();
        self.scroll = self.scroll.min(self.len.saturating_sub(self.height));
        lines.extend(body.into_iter().skip(self.scroll).take(self.height));
        while lines.len() + 1 < usize::from(inner.height) {
            lines.push(Line::from(""));
        }
        let hint = if self.doc.is_empty() {
            "↑↓ Space b scroll · q or Esc close"
        } else if self.raw {
            "↑↓ Space b scroll · r view · q or Esc close"
        } else {
            "↑↓ scroll · Tab Enter open · r raw data · q or Esc close"
        };
        lines.push(Line::from(Span::styled(hint, dim())));
        frame.render_widget(Paragraph::new(lines), inner);
    }
}
