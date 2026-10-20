//! The composer: the draft in `view.composer`, edited in place (`ARCHITECTURE.md §7.4`).
//!
//! Like the GUI's, it keeps the typed subject and body in local buffers and writes them back
//! with one `update_draft` when typing pauses for a second, when the cursor leaves the field, or
//! before a key command — not per keystroke, which would flood the undo history. An agent's edit
//! reloads the fields unless there is unsaved typing. To, Cc and Bcc take addresses
//! (`Name <a@b>` or `a@b`) on Enter, a comma or leaving the field; invalid text stays in the
//! field. The body is a `ratatui-textarea`; the quote is shown read-only below it.

use app_core::{AppState, Command, Draft, DraftId};
use app_front::compose::{self, Field};
use app_front::format;
use ratatui::Frame;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui_textarea::TextArea;

use crate::request::Request;
use crate::text::{clean, is_unsafe, pad, wrap};
use crate::view::{bold, dim, reverse};

/// Milliseconds without typing after which the buffers are written back.
pub const IDLE_MS: i64 = 1_000;

/// Where the cursor is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Spot {
    /// A recipient field.
    Recipients(Field),
    /// The subject.
    Subject,
    /// The body.
    Body,
}

impl Spot {
    const ORDER: [Self; 5] = [
        Self::Recipients(Field::To),
        Self::Recipients(Field::Cc),
        Self::Recipients(Field::Bcc),
        Self::Subject,
        Self::Body,
    ];

    fn step(self, by: isize) -> Self {
        let at = Self::ORDER.iter().position(|s| *s == self).unwrap_or(0);
        let len = Self::ORDER.len();
        let next =
            (isize::try_from(at).unwrap_or(0) + by).rem_euclid(isize::try_from(len).unwrap_or(1));
        Self::ORDER
            .get(usize::try_from(next).unwrap_or(0))
            .copied()
            .unwrap_or(Self::Body)
    }
}

/// Untrusted text for the editor: unsafe characters become U+FFFD, tabs stay (the editor
/// expands them).
fn scrub(text: &str) -> String {
    text.chars()
        .map(|c| {
            if c != '\t' && is_unsafe(c) {
                '\u{fffd}'
            } else {
                c
            }
        })
        .collect()
}

fn single_line(text: &str) -> TextArea<'static> {
    let mut area = TextArea::new(vec![scrub(text)]);
    area.move_cursor(ratatui_textarea::CursorMove::End);
    area.set_cursor_line_style(Style::default());
    area
}

/// The composer's UI-local state.
pub struct ComposerView {
    /// The language of the shared texts.
    pub(crate) lang: app_front::i18n::Lang,
    draft: Option<DraftId>,
    /// The draft's subject and body when the buffers were loaded or written.
    loaded: (String, String),
    spot: Spot,
    subject: TextArea<'static>,
    body: TextArea<'static>,
    inputs: [TextArea<'static>; 3],
    dirty: bool,
    edited_at: i64,
    /// Everyone in the cached mail, best first, as of a state revision (Tab completion).
    contacts: Option<(u64, Vec<app_core::listing::Contact>)>,
}

impl Default for ComposerView {
    fn default() -> Self {
        Self {
            lang: app_front::i18n::Lang::default(),
            draft: None,
            loaded: (String::new(), String::new()),
            spot: Spot::Body,
            subject: single_line(""),
            body: TextArea::default(),
            inputs: [single_line(""), single_line(""), single_line("")],
            dirty: false,
            edited_at: 0,
            contacts: None,
        }
    }
}

impl std::fmt::Debug for ComposerView {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ComposerView")
            .field("draft", &self.draft)
            .field("spot", &self.spot)
            .field("dirty", &self.dirty)
            .finish_non_exhaustive()
    }
}

fn text_of(area: &TextArea<'_>) -> String {
    area.lines().join("\n")
}

impl ComposerView {
    fn load(&mut self, draft: &Draft) {
        let fresh = self.draft != Some(draft.id);
        self.draft = Some(draft.id);
        self.loaded = (draft.subject.clone(), draft.body_text.clone());
        self.subject = single_line(&draft.subject);
        let lines: Vec<String> = draft.body_text.split('\n').map(scrub).collect();
        self.body = TextArea::new(lines);
        self.body.set_cursor_line_style(Style::default());
        self.body.set_placeholder_text("Write your message…");
        self.dirty = false;
        if fresh {
            self.inputs = [single_line(""), single_line(""), single_line("")];
            // A new message starts at To, a reply in the body.
            self.spot = if draft.to.is_empty() && draft.reply_to.is_none() {
                Spot::Recipients(Field::To)
            } else {
                Spot::Body
            };
        }
    }

    /// Keeps the buffers in step with `state`: loads another draft, or an edit made elsewhere
    /// (an agent's) unless there is unsaved typing.
    pub fn sync(&mut self, state: &AppState) {
        let Some(draft) = state.view.composer.and_then(|id| state.drafts.get(&id)) else {
            self.draft = None;
            return;
        };
        let changed = draft.subject != self.loaded.0 || draft.body_text != self.loaded.1;
        if self.draft != Some(draft.id) || (!self.dirty && changed) {
            self.load(draft);
        }
    }

    /// The body as typed.
    pub fn body_text(&self) -> String {
        text_of(&self.body)
    }

    /// Whether typed text waits to be written back.
    pub const fn is_dirty(&self) -> bool {
        self.dirty
    }

    /// The `update_draft` for typed subject and body, if they changed.
    pub fn flush(&mut self, state: &AppState) -> Option<Request> {
        let draft = self.draft.and_then(|id| state.drafts.get(&id))?;
        if !self.dirty {
            return None;
        }
        self.dirty = false;
        let subject = text_of(&self.subject);
        let body = text_of(&self.body);
        self.loaded = (subject.clone(), body.clone());
        let subject = (subject != draft.subject).then_some(subject);
        let body = (body != draft.body_text).then_some(body);
        (subject.is_some() || body.is_some()).then(|| {
            Request::user(Command::UpdateDraft {
                id: draft.id,
                to: None,
                cc: None,
                bcc: None,
                subject,
                body,
            })
        })
    }

    /// The write-back once typing paused for [`IDLE_MS`].
    pub fn idle(&mut self, state: &AppState, now_ms: i64) -> Option<Request> {
        (self.dirty && now_ms - self.edited_at >= IDLE_MS)
            .then(|| self.flush(state))
            .flatten()
    }

    /// Commits the text typed into a recipient field, if any; invalid text stays.
    fn commit(&mut self, draft: &Draft, field: Field) -> Option<Request> {
        let input = self.inputs.get_mut(field as usize)?;
        let typed = text_of(input);
        if typed.trim().is_empty() {
            return None;
        }
        let added = compose::add_recipients(field.of(draft), &typed);
        *input = single_line(&added.rejected);
        added
            .list
            .map(|list| Request::user(field.update(draft, list)))
    }

    /// The contact Tab would add to `field`: the best one matching its typed text that the
    /// draft does not have yet.
    fn completion(
        &mut self,
        state: &AppState,
        draft: &Draft,
        field: Field,
    ) -> Option<app_core::Addr> {
        let typed = text_of(self.inputs.get(field as usize)?)
            .trim()
            .to_lowercase();
        if typed.is_empty() {
            return None;
        }
        let revision = state.revision.get();
        if self.contacts.as_ref().is_none_or(|(at, _)| *at != revision) {
            let all = app_core::listing::contacts(state, "", usize::MAX);
            self.contacts = Some((revision, all));
        }
        let current = field.of(draft);
        self.contacts
            .as_ref()?
            .1
            .iter()
            .filter(|c| c.matches(&typed))
            .find(|c| {
                !current
                    .iter()
                    .any(|a| a.email.eq_ignore_ascii_case(&c.email))
            })
            .map(app_core::listing::Contact::addr)
    }

    /// Moves the cursor to `spot`, committing what the field it leaves holds.
    fn go(&mut self, state: &AppState, spot: Spot) -> Vec<Request> {
        let mut out = Vec::new();
        if let Some(draft) = self.draft.and_then(|id| state.drafts.get(&id)) {
            if let Spot::Recipients(field) = self.spot
                && let Some(request) = self.commit(draft, field)
            {
                out.push(request);
            }
            if matches!(self.spot, Spot::Subject | Spot::Body)
                && let Some(request) = self.flush(state)
            {
                out.push(request);
            }
        }
        self.spot = spot;
        out
    }

    /// Pasted text goes into the field under the cursor (one line in a single-line field).
    pub fn paste(&mut self, text: &str, now_ms: i64) {
        match self.spot {
            Spot::Body => {
                self.body.insert_str(scrub(&text.replace("\r\n", "\n")));
                self.touch(now_ms);
            }
            Spot::Subject => {
                self.subject
                    .insert_str(scrub(text.lines().next().unwrap_or("")));
                self.touch(now_ms);
            }
            Spot::Recipients(field) => {
                if let Some(input) = self.inputs.get_mut(field as usize) {
                    input.insert_str(scrub(&text.replace(['\n', '\r'], ", ")));
                }
            }
        }
    }

    const fn touch(&mut self, now_ms: i64) {
        self.dirty = true;
        self.edited_at = now_ms;
    }

    /// A key typed into the composer (the keymap has had its turn).
    pub fn key(&mut self, key: KeyEvent, state: &AppState, now_ms: i64) -> Vec<Request> {
        let Some(draft) = self.draft.and_then(|id| state.drafts.get(&id)) else {
            return Vec::new();
        };
        if draft.is_outgoing() {
            // Scheduled or leaving: read-only until the send is cancelled.
            return Vec::new();
        }
        if key.code == KeyCode::Tab
            && let Spot::Recipients(field) = self.spot
            && let Some(addr) = self.completion(state, draft, field)
        {
            // Tab completes the typed text to the best matching contact.
            if let Some(input) = self.inputs.get_mut(field as usize) {
                *input = single_line("");
            }
            let mut list = field.of(draft).to_vec();
            list.push(addr);
            return vec![Request::user(field.update(draft, list))];
        }
        match key.code {
            KeyCode::Tab => return self.go(state, self.spot.step(1)),
            KeyCode::BackTab => return self.go(state, self.spot.step(-1)),
            KeyCode::Up if self.spot != Spot::Body => return self.go(state, self.spot.step(-1)),
            KeyCode::Down if self.spot != Spot::Body => return self.go(state, self.spot.step(1)),
            KeyCode::Up if self.body.cursor().0 == 0 => return self.go(state, Spot::Subject),
            _ => {}
        }
        match self.spot {
            Spot::Recipients(field) => {
                let enter = key.code == KeyCode::Enter;
                let separator = matches!(key.code, KeyCode::Char(',' | ';'));
                if enter || separator {
                    let mut out: Vec<Request> = self.commit(draft, field).into_iter().collect();
                    let empty = self
                        .inputs
                        .get(field as usize)
                        .is_some_and(|i| text_of(i).is_empty());
                    if enter && empty && out.is_empty() {
                        // Enter on an empty field moves on.
                        out = self.go(state, self.spot.step(1));
                    }
                    return out;
                }
                if key.code == KeyCode::Backspace
                    && self
                        .inputs
                        .get(field as usize)
                        .is_some_and(|i| text_of(i).is_empty())
                {
                    // Backspace on an empty field removes the last address.
                    let mut list = field.of(draft).to_vec();
                    if list.pop().is_some() {
                        return vec![Request::user(field.update(draft, list))];
                    }
                    return Vec::new();
                }
                if let Some(input) = self.inputs.get_mut(field as usize) {
                    input.input(key);
                }
                Vec::new()
            }
            Spot::Subject => {
                if key.code == KeyCode::Enter {
                    return self.go(state, Spot::Body);
                }
                if self.subject.input(key) {
                    self.touch(now_ms);
                }
                Vec::new()
            }
            Spot::Body => {
                if self.body.input(key) {
                    self.touch(now_ms);
                }
                Vec::new()
            }
        }
    }

    /// Draws the composer into `area` (`focused`: the cursor is shown).
    pub fn draw(
        &mut self,
        frame: &mut Frame,
        area: Rect,
        state: &AppState,
        now_ms: i64,
        focused: bool,
    ) {
        let Some(draft) = self.draft.and_then(|id| state.drafts.get(&id)) else {
            return;
        };
        let columns = usize::from(area.width);
        let mut head: Vec<Line<'static>> = Vec::new();
        let status = compose::draft_status(state, draft, now_ms);
        head.push(Line::from(vec![
            Span::styled(compose::title(draft), bold()),
            Span::styled(format!(" · {status}"), dim()),
        ]));
        if let Some(error) = &draft.send_error {
            head.push(Line::from(Span::styled(
                format!("Not sent: {}", clean(error)),
                bold(),
            )));
            if let Some(hint) = app_front::status::send_hint(state, draft.account, error, self.lang)
            {
                for piece in crate::text::wrap(&hint, columns) {
                    head.push(Line::from(Span::styled(piece, dim())));
                }
            }
        }
        if let Some(error) = &draft.save_error {
            head.push(Line::from(Span::styled(
                format!("Draft save failed: {}", clean(error)),
                dim(),
            )));
        }
        let from = draft.from.as_ref().map_or_else(
            || {
                state
                    .accounts
                    .get(&draft.account)
                    .map_or_else(String::new, |a| a.email.clone())
            },
            |from| from.email.clone(),
        );
        head.push(Line::from(vec![
            Span::styled("From: ", dim()),
            Span::raw(clean(&from)),
        ]));
        for field in Field::ALL {
            let here = focused && self.spot == Spot::Recipients(field);
            let addrs = field.of(draft);
            if field != Field::To && addrs.is_empty() && !here {
                continue;
            }
            let list: Vec<String> = addrs.iter().map(|a| clean(&format::full(a))).collect();
            let typed = self
                .inputs
                .get(field as usize)
                .map(text_of)
                .unwrap_or_default();
            let mut text = format!("{}: {}", field.label(), list.join(", "));
            if here || !typed.is_empty() {
                if !list.is_empty() {
                    text.push_str(", ");
                }
                text.push_str(&clean(&typed));
                if here {
                    text.push('▏');
                }
            }
            let style = if here { bold() } else { Style::default() };
            head.extend(
                wrap(&text, columns)
                    .into_iter()
                    .map(|piece| Line::from(Span::styled(piece, style))),
            );
            if here && let Some(addr) = self.completion(state, draft, field) {
                let hint = format!("  Tab: {}", clean(&format::full(&addr)));
                head.push(Line::from(Span::styled(pad(&hint, columns), dim())));
            }
        }
        let subject_here = focused && self.spot == Spot::Subject;
        let mut subject = format!("Subject: {}", clean(&text_of(&self.subject)));
        if subject_here {
            subject.push('▏');
        }
        head.push(Line::from(Span::styled(
            pad(&subject, columns),
            if subject_here {
                bold()
            } else {
                Style::default()
            },
        )));
        head.push(Line::from(Span::styled("─".repeat(columns), dim())));
        let mut tail: Vec<Line<'static>> = Vec::new();
        if let Some(quote) = &draft.quote {
            for line in quote.header.lines() {
                for piece in wrap(&clean(line), columns) {
                    tail.push(Line::from(Span::styled(piece, dim())));
                }
            }
            tail.push(Line::from(Span::styled(
                "(the original is added below when sent)",
                dim(),
            )));
        }
        for attachment in &draft.attachments {
            let detail = match &attachment.state {
                app_core::AttachmentState::Checking => "checking…".to_owned(),
                app_core::AttachmentState::Ready { size, .. } => format::size(*size),
                app_core::AttachmentState::Failed { error } => format!("failed: {}", clean(error)),
            };
            tail.push(Line::from(format!(
                "@ {} ({detail})",
                clean(&attachment.path)
            )));
        }
        let footer = if draft.is_outgoing() {
            "read-only while it is sent · z cancels"
        } else {
            "Ctrl+S send · Ctrl+O $EDITOR · Ctrl+K “Attach file” · Esc close"
        };
        tail.push(Line::from(Span::styled(footer, dim())));
        let [head_area, body_area, tail_area] = Layout::vertical([
            Constraint::Length(u16::try_from(head.len()).unwrap_or(u16::MAX)),
            Constraint::Min(1),
            Constraint::Length(u16::try_from(tail.len()).unwrap_or(u16::MAX)),
        ])
        .areas(area);
        frame.render_widget(Paragraph::new(head), head_area);
        let body_here = focused && self.spot == Spot::Body && !draft.is_outgoing();
        self.body.set_cursor_style(if body_here {
            reverse()
        } else {
            Style::default()
        });
        frame.render_widget(&self.body, body_area);
        frame.render_widget(Paragraph::new(tail), tail_area);
    }
}

/// Whether `key` is plain typing for the editor (no Ctrl or Alt).
pub fn is_typing(key: &KeyEvent) -> bool {
    !key.modifiers
        .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT)
}
