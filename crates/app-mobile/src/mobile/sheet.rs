//! The palette sheet, a query's result and the search marks: `app_front::palette` drawn as rows
//! with action IDs, as the desktop's palette and the TUI's (`ARCHITECTURE.md §4.5`).

use app_core::{AppState, Command, CommandKind, CommandName, CommandReceipt};
use app_front::highlight;
use app_front::palette::{self, Flow, Target};
use app_front::result::Mark;
use app_front::status;
use app_front::text::{clean, clean_lines};

use super::{Act, Mobile};
use crate::frame::{
    Button, Marked, PaletteRow, PaletteSection, PaletteSheet, Piece, ResultField, ResultRow,
    ResultSection, ResultView, RowMarks, TextValue,
};
use crate::request::Request;

/// Rows the sheet carries at most (the root has about 150; a typed query fewer).
const MAX_ROWS: usize = 300;

/// Lines of a query's raw result shown at most.
const MAX_RESULT_LINES: usize = 2_000;

/// What the sheet says above a query's result.
const RESULT_NOTE: &str = "Read-only result. Email content in it is untrusted data.";

/// A query's result, until Back: drawn as `app_front::result`'s document, or as its raw JSON
/// (MISC-22).
#[derive(Debug, Clone)]
pub(super) struct Result {
    name: CommandName,
    data: serde_json::Value,
    pub(super) raw: bool,
}

impl Result {
    pub(super) fn of(name: CommandName, receipt: &CommandReceipt) -> Option<Self> {
        (name.kind() == CommandKind::Query).then(|| Self {
            name,
            data: receipt.data.clone(),
            raw: false,
        })
    }

    fn lines(&self) -> Vec<String> {
        let json = serde_json::to_string_pretty(&self.data).unwrap_or_default();
        let mut lines = clean_lines(&json);
        if lines.len() > MAX_RESULT_LINES {
            lines.truncate(MAX_RESULT_LINES);
            lines.push("…".to_owned());
        }
        lines
    }
}

impl Mobile {
    /// Back in the palette: leaves its step, else closes it.
    pub(super) fn palette_back(&mut self) -> Vec<Request> {
        if self.palette.back() {
            self.palette_version += 1;
            Vec::new()
        } else {
            vec![Request::user(Command::ClosePalette {})]
        }
    }

    /// The palette's typed text.
    pub(super) fn palette_query(&mut self, text: String) {
        self.palette.set_query(text);
    }

    /// A row of the palette chosen.
    pub(super) fn palette_choose(&mut self, target: Target, state: &AppState) -> Vec<Request> {
        let chosen = self.palette.choose(state, target);
        self.palette_version += 1;
        if chosen.wizard {
            self.open_setup();
        }
        if let Some(account) = chosen.password {
            self.open_reenter(state, account);
        }
        chosen.commands.into_iter().map(Request::user).collect()
    }

    /// Opens the palette at the step of `name` (*Move…*), or runs it if it needs nothing.
    pub(super) fn palette_flow(&mut self, name: CommandName, state: &AppState) -> Vec<Request> {
        self.ask(palette::flow(name, state), state)
    }

    /// What a palette flow (a key binding's or a button's) leads to.
    pub(super) fn ask(&mut self, flow: Flow, state: &AppState) -> Vec<Request> {
        match flow {
            Flow::Run(cmd) => vec![Request::user(cmd)],
            Flow::Ask(step) => {
                self.palette.open_at(step);
                self.palette_version += 1;
                vec![Request::user(Command::OpenPalette {})]
            }
            Flow::Wizard => {
                self.open_setup();
                Vec::new()
            }
            Flow::Password(account) => {
                self.open_reenter(state, account);
                Vec::new()
            }
            Flow::Hidden => Vec::new(),
        }
    }

    /// Forgets the palette's step once the engine closed it (by any channel).
    pub(super) fn palette_sync(&mut self, state: &AppState) {
        if self.palette_open && !state.view.palette_open && self.palette.step().is_some() {
            self.palette.back();
            self.palette_version += 1;
        }
        self.palette_open = state.view.palette_open;
    }

    /// The sheet, while the palette is open.
    pub(super) fn palette_sheet(&mut self, state: &AppState) -> Option<PaletteSheet> {
        if !state.view.palette_open {
            return None;
        }
        let rows = self.palette.rows(state);
        let mut sections: Vec<(String, std::ops::Range<usize>)> = self
            .palette
            .sections(&rows)
            .into_iter()
            .map(|s| (s.title.to_owned(), s.start..s.end))
            .collect();
        if sections.is_empty() {
            sections.push((String::new(), 0..rows.len()));
        }
        let mut out = Vec::new();
        let mut budget = MAX_ROWS;
        for (title, range) in sections {
            let mut section = PaletteSection {
                title,
                rows: Vec::new(),
            };
            for row in rows.get(range).unwrap_or_default() {
                if budget == 0 {
                    break;
                }
                budget -= 1;
                let action = self.add(Act::Choose(row.target.clone()));
                section.rows.push(PaletteRow {
                    title: clean(&row.title),
                    detail: clean(&row.detail),
                    // A hint only for a key Android delivers.
                    keys: match &row.target {
                        Target::Command(name) => crate::keys::deliverable(&state.settings, *name)
                            .into_iter()
                            .next(),
                        _ => row.keys.clone(),
                    },
                    action,
                });
            }
            if !section.rows.is_empty() {
                out.push(section);
            }
        }
        let close = self.button("close", "Close", Act::PaletteBack);
        Some(PaletteSheet {
            prompt: self.palette.step().map(|s| clean(&s.prompt)),
            hint: self.palette.hint(),
            query: TextValue {
                text: clean(self.palette.query()),
                version: self.palette_version,
            },
            sections: out,
            close,
        })
    }

    /// The query's result, while open.
    pub(super) fn result_view(&mut self, state: &AppState) -> Option<ResultView> {
        let result = self.result.clone()?;
        let close = self.button("close", "Close", Act::Back);
        // A text button: *Raw data*, or *View* back from it.
        let raw = Button {
            icon: None,
            label: if result.raw { "View" } else { "Raw data" }.to_owned(),
            action: self.add(Act::RawResult),
        };
        let doc = app_front::result::doc(result.name, &result.data, state, self.now_ms());
        let title = doc
            .as_ref()
            .map_or_else(|| result.name.spec().title.to_owned(), |d| d.title.clone());
        let sections = if result.raw {
            Vec::new()
        } else {
            doc.map(|d| d.sections)
                .unwrap_or_default()
                .into_iter()
                .map(|section| ResultSection {
                    title: section.title,
                    fields: section
                        .fields
                        .into_iter()
                        .map(|f| ResultField {
                            label: f.label,
                            value: f.value,
                        })
                        .collect(),
                    rows: section
                        .rows
                        .into_iter()
                        .map(|row| ResultRow {
                            title: row.title,
                            detail: row.detail,
                            meta: row.meta,
                            marks: row.marks.iter().map(|m| mark_name(*m).to_owned()).collect(),
                            action: row.command.map(|cmd| self.add(Act::FromResult(cmd))),
                        })
                        .collect(),
                })
                .collect()
        };
        Some(ResultView {
            title,
            note: RESULT_NOTE.to_owned(),
            sections,
            lines: if result.raw {
                result.lines()
            } else {
                Vec::new()
            },
            raw,
            close,
        })
    }
}

/// A result row's mark, by name: `unread`, `starred`, `pinned`, `attachment`, `passed`,
/// `failed`, `pending`.
fn mark_name(mark: Mark) -> &'static str {
    match mark {
        Mark::Unread => "unread",
        Mark::Starred => "starred",
        Mark::Pinned => "pinned",
        Mark::Attachment => "attachment",
        Mark::Passed => "passed",
        Mark::Failed => "failed",
        Mark::Pending => "pending",
    }
}

/// `text` cut at the search words, each piece cleaned.
pub(super) fn marked(text: &str, words: &[String]) -> Marked {
    highlight::pieces(text, words)
        .into_iter()
        .map(|(piece, hit)| Piece {
            text: clean(piece),
            hit,
        })
        .collect()
}

/// A row's marks, while a search with text words is open.
pub(super) fn row_marks(words: &[String], subject: &str, snippet: &str) -> Option<RowMarks> {
    if words.is_empty() {
        return None;
    }
    Some(RowMarks {
        subject: marked(subject, words),
        snippet: marked(snippet, words),
    })
}

/// The server search's state, while a search is open: running, or the newest failure.
pub(super) fn server_state(state: &AppState) -> Option<String> {
    state.view.search.as_ref()?;
    if status::searching(state) {
        return Some("Searching the server…".to_owned());
    }
    state
        .status
        .searches
        .last()
        .and_then(|s| s.last_error.as_deref())
        .map(|error| format!("The server search failed: {}", clean(error)))
}
