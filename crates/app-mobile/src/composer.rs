//! The composer's buffers (`ARCHITECTURE.md §7.5`), the TUI's pattern without a terminal: the
//! typed subject and body stay here and go back as one `update_draft` when typing pauses for a
//! second, when the field loses the focus, or before any other action — never per keystroke,
//! which would flood the undo history. A change made elsewhere (an agent's) reloads a field
//! unless it has unsaved typing. To, Cc and Bcc take addresses (`Name <a@b>` or `a@b`) at a
//! comma, a semicolon, a line break or when the field loses the focus; invalid text stays.
//!
//! Every text value carries a version: the host reloads its field only when the version
//! changes, so a frame never overwrites what the IME is composing.

use app_core::listing::{self, Contact};
use app_core::{Addr, AppState, Command, Draft, DraftId};
use app_front::compose;
use app_front::text::is_unsafe;

use crate::event::Field;
use crate::frame::TextValue;
use crate::request::Request;

/// Milliseconds without typing after which the buffers are written back.
pub const IDLE_MS: i64 = 1_000;

/// Contact suggestions shown under a recipient field.
pub const SUGGESTIONS: usize = 3;

/// One editable buffer.
#[derive(Debug, Default, Clone)]
struct Buffer {
    text: String,
    /// The draft's value when the buffer was loaded or written back.
    loaded: String,
    dirty: bool,
    version: u64,
}

/// The composer's UI-local state.
#[derive(Debug, Default)]
pub struct Buffers {
    draft: Option<DraftId>,
    subject: Buffer,
    body: Buffer,
    /// Typed text of To, Cc and Bcc not yet taken as addresses.
    inputs: [Buffer; 3],
    edited_at: i64,
    next_version: u64,
    /// Whether the quote's header is shown.
    pub quote_open: bool,
    /// Everyone in the cached mail, best first, as of a state revision.
    contacts: Option<(u64, Vec<Contact>)>,
}

/// Untrusted text for an editor: unsafe characters become U+FFFD; tabs and line breaks stay.
pub fn scrub(text: &str) -> String {
    text.chars()
        .map(|c| {
            if !matches!(c, '\t' | '\n') && is_unsafe(c) {
                '\u{fffd}'
            } else {
                c
            }
        })
        .collect()
}

const fn recipients(field: Field) -> Option<compose::Field> {
    match field {
        Field::To => Some(compose::Field::To),
        Field::Cc => Some(compose::Field::Cc),
        Field::Bcc => Some(compose::Field::Bcc),
        Field::Subject | Field::Body | Field::Query => None,
    }
}

impl Buffers {
    fn bump(&mut self) -> u64 {
        self.next_version += 1;
        self.next_version
    }

    /// The open draft, if it is still there.
    pub fn draft<'a>(&self, state: &'a AppState) -> Option<&'a Draft> {
        self.draft.and_then(|id| state.drafts.get(&id))
    }

    /// Keeps the buffers in step with `state`: loads another draft, or reloads a field changed
    /// elsewhere unless it has unsaved typing.
    pub fn sync(&mut self, state: &AppState) {
        let Some(draft) = state.view.composer.and_then(|id| state.drafts.get(&id)) else {
            self.draft = None;
            return;
        };
        if self.draft != Some(draft.id) {
            self.draft = Some(draft.id);
            self.quote_open = false;
            for i in 0..3 {
                let version = self.bump();
                if let Some(input) = self.inputs.get_mut(i) {
                    *input = Buffer {
                        version,
                        ..Buffer::default()
                    };
                }
            }
            self.subject = Buffer::default();
            self.body = Buffer::default();
        }
        for (field, value) in [
            (Field::Subject, &draft.subject),
            (Field::Body, &draft.body_text),
        ] {
            let fresh = self.buffer(field).is_some_and(|b| b.version == 0);
            let changed = self
                .buffer(field)
                .is_some_and(|b| !b.dirty && b.loaded != *value);
            if fresh || changed {
                let version = self.bump();
                if let Some(buffer) = self.buffer_mut(field) {
                    *buffer = Buffer {
                        text: scrub(value),
                        loaded: value.clone(),
                        dirty: false,
                        version,
                    };
                }
            }
        }
    }

    fn buffer(&self, field: Field) -> Option<&Buffer> {
        match field {
            Field::Subject => Some(&self.subject),
            Field::Body => Some(&self.body),
            other => recipients(other).and_then(|f| self.inputs.get(f as usize)),
        }
    }

    fn buffer_mut(&mut self, field: Field) -> Option<&mut Buffer> {
        match field {
            Field::Subject => Some(&mut self.subject),
            Field::Body => Some(&mut self.body),
            other => recipients(other).and_then(|f| self.inputs.get_mut(f as usize)),
        }
    }

    /// A field's text and version for the frame.
    pub fn value(&self, field: Field) -> TextValue {
        self.buffer(field).map_or_else(
            || TextValue {
                text: String::new(),
                version: 0,
            },
            |b| TextValue {
                text: b.text.clone(),
                version: b.version,
            },
        )
    }

    /// Whether typed subject or body text waits to be written back.
    pub const fn is_dirty(&self) -> bool {
        self.subject.dirty || self.body.dirty
    }

    /// A field changed in the host. A recipient field takes the addresses before its last
    /// separator.
    pub fn text(&mut self, field: Field, text: String, state: &AppState, now: i64) -> Vec<Request> {
        let Some(draft) = self.draft(state) else {
            return Vec::new();
        };
        if draft.is_outgoing() {
            // Scheduled or leaving: read-only until the send is cancelled.
            return Vec::new();
        }
        if let Some(target) = recipients(field) {
            return match text.rfind([',', ';', '\n']) {
                Some(at) => {
                    let (head, tail) = text.split_at(at);
                    let tail = tail.get(1..).unwrap_or_default().trim_start().to_owned();
                    self.commit(state, target, head, tail)
                }
                None => {
                    if let Some(input) = self.inputs.get_mut(target as usize) {
                        input.text = text;
                    }
                    Vec::new()
                }
            };
        }
        if let Some(buffer) = self.buffer_mut(field)
            && buffer.text != text
        {
            buffer.text = text;
            buffer.dirty = true;
            self.edited_at = now;
        }
        Vec::new()
    }

    /// Takes the addresses in `typed`; what is not an address stays, followed by `rest`.
    fn commit(
        &mut self,
        state: &AppState,
        field: compose::Field,
        typed: &str,
        rest: String,
    ) -> Vec<Request> {
        let Some(draft) = self.draft(state) else {
            return Vec::new();
        };
        let added = compose::add_recipients(field.of(draft), typed);
        let request = added
            .list
            .map(|list| Request::user(field.update(draft, list)));
        let left = match (added.rejected.is_empty(), rest.is_empty()) {
            (true, _) => rest,
            (false, true) => added.rejected,
            (false, false) => format!("{}, {rest}", added.rejected),
        };
        let version = self.bump();
        if let Some(input) = self.inputs.get_mut(field as usize) {
            *input = Buffer {
                text: left,
                version,
                ..Buffer::default()
            };
        }
        request.into_iter().collect()
    }

    /// A field lost the focus: a recipient field takes its addresses, a text field is written
    /// back.
    pub fn blur(&mut self, field: Field, state: &AppState) -> Vec<Request> {
        match recipients(field) {
            Some(target) => {
                let typed = self
                    .inputs
                    .get(target as usize)
                    .map(|b| b.text.clone())
                    .unwrap_or_default();
                if typed.trim().is_empty() {
                    return Vec::new();
                }
                self.commit(state, target, &typed, String::new())
            }
            None => self.flush(state).into_iter().collect(),
        }
    }

    /// The `update_draft` for typed subject and body, if they changed.
    pub fn flush(&mut self, state: &AppState) -> Option<Request> {
        let draft = self.draft(state)?;
        if !self.is_dirty() {
            return None;
        }
        let take = |buffer: &mut Buffer, current: &str| {
            let dirty = std::mem::take(&mut buffer.dirty);
            buffer.loaded.clone_from(&buffer.text);
            (dirty && buffer.text != current).then(|| buffer.text.clone())
        };
        let subject = take(&mut self.subject, &draft.subject);
        let body = take(&mut self.body, &draft.body_text);
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
    pub fn idle(&mut self, state: &AppState, now: i64) -> Option<Request> {
        (self.is_dirty() && now - self.edited_at >= IDLE_MS)
            .then(|| self.flush(state))
            .flatten()
    }

    /// The best contacts matching the typed text of `field` that the draft does not have yet.
    pub fn suggestions(&mut self, state: &AppState, draft: &Draft, field: Field) -> Vec<Addr> {
        let Some(target) = recipients(field) else {
            return Vec::new();
        };
        let typed = self
            .inputs
            .get(target as usize)
            .map(|b| b.text.trim().to_lowercase())
            .unwrap_or_default();
        if typed.is_empty() {
            return Vec::new();
        }
        let revision = state.revision.get();
        if self.contacts.as_ref().is_none_or(|(at, _)| *at != revision) {
            self.contacts = Some((revision, listing::contacts(state, "", usize::MAX)));
        }
        let current = target.of(draft);
        self.contacts
            .as_ref()
            .map(|(_, all)| {
                all.iter()
                    .filter(|c| c.matches(&typed))
                    .filter(|c| {
                        !current
                            .iter()
                            .any(|a| a.email.eq_ignore_ascii_case(&c.email))
                    })
                    .take(SUGGESTIONS)
                    .map(Contact::addr)
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Adds a suggested address to `field` and empties its typed text.
    pub fn suggest(&mut self, field: Field, addr: Addr, state: &AppState) -> Vec<Request> {
        let Some(target) = recipients(field) else {
            return Vec::new();
        };
        let Some(draft) = self.draft(state) else {
            return Vec::new();
        };
        let mut list = target.of(draft).to_vec();
        if list
            .iter()
            .any(|a| a.email.eq_ignore_ascii_case(&addr.email))
        {
            return Vec::new();
        }
        list.push(addr);
        let request = Request::user(target.update(draft, list));
        let version = self.bump();
        if let Some(input) = self.inputs.get_mut(target as usize) {
            *input = Buffer {
                version,
                ..Buffer::default()
            };
        }
        vec![request]
    }
}
