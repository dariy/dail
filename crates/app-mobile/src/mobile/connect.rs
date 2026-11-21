//! *Connect to a dail server* on the phone (ADR 0017, SRV-7): the setup's other first choice,
//! which makes this profile a device of a dail server instead of adding a mail account. It draws
//! `app_front::connect::ConnectFlow` into the setup sheet, as the GUI's wizard and the TUI's
//! setup do; the rules (what is a valid server, a pasted `dail://pair` link read as untrusted
//! input, the fingerprint a link pins, when a failure counts) are the flow's.
//!
//! The stages: the server (or a pasted link) → *Connect* (`connect_server`) → the offered
//! fingerprint in groups of four, the code → *Pair* (`trust_server`) → paired; a mismatch with the
//! link's fingerprint, a failure (*Try again*, *Enter another code*). Given up before pairing,
//! the flow's `disconnect_server` removes the link it left unpinned. Its commands go out as
//! [`Purpose::Connect`] and their replies go back to the flow.
//!
//! [`UiEvent::PairLink`](crate::UiEvent::PairLink) is the entry point of a link from outside the
//! app (the deep link, the scanner): the flow reads it and contacts its server at once.

use app_core::AppState;
use app_front::connect::{Availability, ConnectFlow, Retry, Stage, availability};
use app_front::i18n::{self, Key};
use app_front::text::clean;

use super::{Act, Mobile};
use crate::event::Field;
use crate::frame::{Button, Fingerprint, InputKind, SetupField, SetupSheet, TextValue};
use crate::request::{Purpose, Reply, Request};

/// What a button of the flow does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ConnectAct {
    /// *Connect*: `connect_server` for the typed server.
    Connect,
    /// *Pair*: `trust_server` with the offered fingerprint and the code.
    Pair,
    /// *Back*: from the code to the server, from the server out of the flow.
    Back,
    /// *Cancel*: give the flow up.
    Cancel,
    /// *Try again*, *Enter another code*, or *Back* from a mismatch.
    Retry,
    /// *Close* once paired.
    Close,
}

/// The stage a button was drawn for: it runs only while the flow is still there.
pub(super) const fn tag(stage: &Stage) -> u8 {
    match stage {
        Stage::Address => 0,
        Stage::Probing => 1,
        Stage::Compare { .. } => 2,
        Stage::Mismatch { .. } => 3,
        Stage::Pairing => 4,
        Stage::Paired => 5,
        Stage::Failed { .. } => 6,
    }
}

/// The flow and the version of its fields' text.
#[derive(Debug, Default)]
pub(super) struct Connect {
    flow: ConnectFlow,
    /// Changes whenever the presenter replaces a field's text (a link that was read).
    version: u64,
}

impl Connect {
    /// Whether it waits for the server (the frame ticks, so a failure's grace time passes).
    pub(super) const fn is_waiting(&self) -> bool {
        self.flow.is_waiting()
    }

    /// A field's typed text. A paste into the server field (more than one character at once)
    /// that is a pairing link is read at once; one typed by hand is read on *Connect*.
    pub(super) fn text(&mut self, field: Field, text: String) {
        match field {
            Field::Server => {
                if text == self.flow.address {
                    return;
                }
                let before = self.flow.address.chars().count();
                self.flow.address = text;
                self.flow.address_edited();
                if self.flow.address.chars().count() > before + 1 {
                    let typed = (self.flow.address.clone(), self.flow.code.clone());
                    self.flow.read_address();
                    if (&self.flow.address, &self.flow.code) != (&typed.0, &typed.1) {
                        self.version += 1;
                    }
                }
            }
            Field::Code => self.flow.code = text,
            _ => {}
        }
    }

    /// The reply to one of the flow's commands.
    pub(super) fn accept(&mut self, reply: &Reply, now_ms: i64) {
        let result = match &reply.result {
            Ok(Ok(receipt)) => Ok(receipt),
            Ok(Err(err)) => Err(i18n::message(&err.to_string())),
            Err(err) => Err(err.clone()),
        };
        self.flow.accept(result, now_ms);
    }
}

fn connect_request(cmd: app_core::Command) -> Request {
    Request {
        purpose: Purpose::Connect,
        cmd,
    }
}

impl Mobile {
    /// Opens the flow at the server (the setup's *Connect to a dail server*).
    pub(super) fn open_connect(&mut self) {
        self.connect = Some(Connect::default());
    }

    /// A `dail://pair` link from outside the app: the flow with it read, its server contacted.
    pub(super) fn pair_link(&mut self, uri: &str, state: &AppState) -> Vec<Request> {
        let mut out = Vec::new();
        if availability(state) != Availability::Open {
            self.notify(i18n::s(Key::PairLinkRefused));
            return out;
        }
        // A flow already under way gives way to the link.
        if let Some(mut old) = self.connect.take()
            && let Some(cmd) = old.flow.cancel(state)
        {
            out.push(connect_request(cmd));
        }
        let mut connect = Connect::default();
        connect.flow.address = uri.trim().to_owned();
        if let Some(cmd) = connect.flow.submit_address(state) {
            out.push(connect_request(cmd));
        }
        connect.version += 1;
        self.connect = Some(connect);
        out
    }

    /// A button of the flow, drawn at the stage `tag`.
    pub(super) fn connect_act(
        &mut self,
        act: ConnectAct,
        at: u8,
        state: &AppState,
    ) -> Vec<Request> {
        let Some(connect) = self.connect.as_mut() else {
            return Vec::new();
        };
        if tag(connect.flow.stage()) != at {
            return Vec::new();
        }
        let mut out = Vec::new();
        match act {
            ConnectAct::Connect => {
                out.extend(connect.flow.submit_address(state).map(connect_request));
            }
            ConnectAct::Pair => {
                if let Some(cmd) = connect.flow.submit_code(state) {
                    // The code moved into the command: the field is emptied.
                    connect.version += 1;
                    out.push(connect_request(cmd));
                }
            }
            ConnectAct::Retry => {
                connect.flow.retry();
                connect.version += 1;
            }
            ConnectAct::Back | ConnectAct::Cancel | ConnectAct::Close => {
                return self.connect_back(state);
            }
        }
        out
    }

    /// Back (the sheet's or the system's): from the code to the server, else out of the flow
    /// (a link it left unpinned removed), back to the setup's first step if it came from there.
    /// Once paired the setup closes too.
    pub(super) fn connect_back(&mut self, state: &AppState) -> Vec<Request> {
        let Some(connect) = self.connect.as_mut() else {
            return Vec::new();
        };
        match connect.flow.stage() {
            Stage::Compare { .. } => {
                connect.flow.back();
                connect.version += 1;
                Vec::new()
            }
            Stage::Paired => {
                self.connect = None;
                self.setup = None;
                Vec::new()
            }
            _ => {
                let cmd = connect.flow.cancel(state);
                self.connect = None;
                cmd.map(connect_request).into_iter().collect()
            }
        }
    }

    fn connect_button(&mut self, label: &str, act: ConnectAct, at: u8) -> Button {
        Button {
            icon: None,
            label: label.to_owned(),
            action: self.add(Act::Connect(act, at)),
        }
    }

    /// The flow's sheet, while open (in the setup sheet's place).
    pub(super) fn connect_sheet(&mut self, state: &AppState) -> Option<SetupSheet> {
        let mut connect = self.connect.take()?;
        connect.flow.sync(state, self.now_ms());
        let sheet = self.draw_connect(&connect, state);
        self.connect = Some(connect);
        Some(sheet)
    }

    fn draw_connect(&mut self, connect: &Connect, state: &AppState) -> SetupSheet {
        let flow = &connect.flow;
        let stage = flow.stage().clone();
        let at = tag(&stage);
        let server = clean(&flow.server());
        let field = |field, label: Key, hint: &str, input, text: &str| SetupField {
            field,
            label: i18n::s(label).to_owned(),
            hint: hint.to_owned(),
            input,
            value: TextValue {
                text: text.to_owned(),
                version: connect.version,
            },
        };
        let fingerprint = |label: Key, fingerprint: &str| Fingerprint {
            label: i18n::s(label).to_owned(),
            lines: app_front::connect::fingerprint_lines(fingerprint),
        };
        let mut fields = Vec::new();
        let mut fingerprints = Vec::new();
        let mut note = None;
        let mut error = None;
        let mut actions = Vec::new();
        let mut busy = false;
        // From the setup's first step, Back goes there; from a link, the flow is all there is.
        let mut back = (i18n::s(Key::Cancel), ConnectAct::Cancel);
        let step = match &stage {
            Stage::Address => {
                fields.push(field(
                    Field::Server,
                    Key::ConnectServerLabel,
                    "home.example.org:7443",
                    InputKind::Server,
                    &flow.address,
                ));
                let mut text = i18n::s(Key::ConnectAddressHint).to_owned();
                if let Some(read) = flow.note() {
                    text = format!("{text}\n{}", clean(read));
                }
                note = Some(text);
                match flow.address_problem() {
                    None => actions.push(self.connect_button(
                        i18n::s(Key::Connect),
                        ConnectAct::Connect,
                        at,
                    )),
                    Some(problem) if !flow.address.trim().is_empty() => error = Some(problem),
                    Some(_) => {}
                }
                if self.setup.is_some() {
                    back = (i18n::s(Key::Back), ConnectAct::Back);
                }
                i18n::s(Key::ConnectServerLabel).to_owned()
            }
            Stage::Probing | Stage::Pairing => {
                busy = true;
                let key = if stage == Stage::Probing {
                    Key::ConnectProbing
                } else {
                    Key::ConnectPairing
                };
                note = Some(i18n::f(key, &[("server", &server)]));
                server.clone()
            }
            Stage::Compare {
                fingerprint: offered,
                matches_link,
            } => {
                fields.push(field(
                    Field::Code,
                    Key::PairingCode,
                    "ABCD2345",
                    InputKind::Code,
                    &flow.code,
                ));
                let mut text = i18n::f(Key::ConnectCompare, &[("server", &server)]);
                if *matches_link {
                    text = format!("{text}\n{}", i18n::s(Key::ConnectMatchesLink));
                }
                note = Some(format!("{text}\n{}", i18n::s(Key::ConnectCodeHint)));
                fingerprints.push(fingerprint(Key::PairingFingerprint, offered));
                match flow.code_problem() {
                    None => {
                        actions.push(self.connect_button(i18n::s(Key::Pair), ConnectAct::Pair, at));
                    }
                    Some(problem) if !flow.code.trim().is_empty() => error = Some(problem),
                    Some(_) => {}
                }
                back = (i18n::s(Key::Back), ConnectAct::Back);
                i18n::s(Key::PairingCode).to_owned()
            }
            Stage::Mismatch { offered, expected } => {
                error = Some(i18n::f(Key::ConnectMismatch, &[("server", &server)]));
                fingerprints.push(fingerprint(Key::PairingFingerprint, offered));
                fingerprints.push(fingerprint(Key::ConnectLinkFingerprint, expected));
                actions.push(self.connect_button(i18n::s(Key::Back), ConnectAct::Retry, at));
                i18n::s(Key::PairingFingerprint).to_owned()
            }
            Stage::Paired => {
                let mut text = i18n::f(Key::ConnectPaired, &[("server", &server)]);
                if let Some(summary) = app_front::server::summary(state, i18n::current()) {
                    text = format!("{text}\n{}", summary.text);
                }
                note = Some(text);
                actions.push(self.connect_button(i18n::s(Key::Close), ConnectAct::Close, at));
                back = (i18n::s(Key::Close), ConnectAct::Close);
                server.clone()
            }
            Stage::Failed { error: why, retry } => {
                error = Some(clean(why));
                let again = match retry {
                    Retry::Address => i18n::s(Key::TryAgain),
                    Retry::Code => i18n::s(Key::AnotherCode),
                };
                actions.push(self.connect_button(again, ConnectAct::Retry, at));
                server.clone()
            }
        };
        let (label, act) = back;
        let mut back = self.connect_button(label, act, at);
        if act == ConnectAct::Back {
            back.icon = Some("arrow_back".to_owned());
        }
        SetupSheet {
            title: i18n::s(Key::ConnectToServer).to_owned(),
            step,
            presets: Vec::new(),
            fields,
            note,
            error,
            fingerprints,
            busy,
            actions,
            back,
        }
    }
}
