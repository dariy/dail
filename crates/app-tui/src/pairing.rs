//! The *Pair a device…* dialog (ADR 0017, SRV-QR1): draws [`PairDialog`] and feeds it keys. The
//! flow (role and name, `start_pairing`, the code, its expiry, the fingerprint and the QR code)
//! is `app_front::pairing`'s. The QR code is drawn in half-blocks, black on white whatever the
//! theme, so it scans on light and dark terminals alike; with `NO_COLOR` it is plain text whose
//! glyphs are the light modules (scannable on the common light-on-dark terminal).

use app_core::{AppState, DeviceRole};
use app_front::i18n::Key;
use app_front::pairing::{PairDialog, Stage, countdown, fingerprint_lines, forbidden_in_name};
use app_front::qr::{Ink, QUIET_ZONE, half_blocks};
use ratatui::Frame;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Clear, Paragraph, Wrap};

use crate::request::{Purpose, Request};
use crate::text::{clean, f, s};
use crate::view::{bold, dim, reverse};

/// What a key in the dialog did.
#[derive(Debug, Default)]
pub struct Outcome {
    /// Commands for the session.
    pub requests: Vec<Request>,
    /// The dialog closes.
    pub close: bool,
}

/// The dialog's UI state.
#[derive(Debug, Default)]
pub struct PairView {
    /// The shared state machine.
    pub dialog: PairDialog,
}

impl PairView {
    /// The dialog waiting for the reply of a `start_pairing` submitted elsewhere.
    pub fn submitted(role: DeviceRole, name: &str, state: &AppState) -> Self {
        Self {
            dialog: PairDialog::submitted(role, name, state),
        }
    }

    /// Handles a key.
    pub fn key(&mut self, key: KeyEvent, state: &AppState) -> Outcome {
        let mut out = Outcome::default();
        if key.code == KeyCode::Esc {
            out.close = true;
            return out;
        }
        match self.dialog.stage() {
            Stage::Form => match key.code {
                KeyCode::Tab | KeyCode::BackTab | KeyCode::Up | KeyCode::Down => {
                    self.dialog.role = match self.dialog.role {
                        DeviceRole::User => DeviceRole::Agent,
                        DeviceRole::Agent => DeviceRole::User,
                    };
                }
                KeyCode::Backspace => {
                    self.dialog.name.pop();
                }
                KeyCode::Enter => {
                    if let Some(cmd) = self.dialog.submit(state) {
                        out.requests.push(Request {
                            purpose: Purpose::Pairing,
                            cmd,
                        });
                    }
                }
                KeyCode::Char(c)
                    if !key.modifiers.contains(KeyModifiers::CONTROL) && !forbidden_in_name(c) =>
                {
                    self.dialog.name.push(c);
                }
                _ => {}
            },
            Stage::Opened(_) | Stage::Failed(_) => match key.code {
                KeyCode::Char('n') => self.dialog.again(),
                KeyCode::Char('q') | KeyCode::Enter => out.close = true,
                _ => {}
            },
            Stage::Waiting => {}
        }
        out
    }

    /// Pasted text goes into the name.
    pub fn paste(&mut self, text: &str) {
        if self.dialog.stage() == &Stage::Form {
            self.dialog
                .name
                .extend(text.chars().filter(|c| !forbidden_in_name(*c)));
        }
    }

    /// Draws the dialog over `area`.
    pub fn draw(&self, frame: &mut Frame, area: Rect, (now_ms, no_color): (i64, bool)) {
        frame.render_widget(Clear, area);
        let block = Block::bordered()
            .border_style(bold())
            .title(s(Key::PairADevice));
        let inner = block.inner(area);
        frame.render_widget(block, area);
        let mut lines: Vec<Line<'static>> = Vec::new();
        let footer = match self.dialog.stage() {
            Stage::Form => {
                self.form(&mut lines);
                Key::PairFormFooter
            }
            Stage::Waiting => {
                lines.push(Line::from(s(Key::PairingWaiting)));
                Key::PairFormFooter
            }
            Stage::Failed(error) => {
                lines.push(Line::from(Span::styled(
                    f(Key::PairingFailed, &[("error", &clean(error))]),
                    bold(),
                )));
                Key::PairOpenedFooter
            }
            Stage::Opened(opened) => {
                return self.draw_opened(frame, inner, opened, (now_ms, no_color));
            }
        };
        frame.render_widget(
            Paragraph::new(lines).wrap(Wrap { trim: false }),
            body(inner),
        );
        draw_footer(frame, inner, footer);
    }

    /// The opened code. The lines are laid out unwrapped (texts pre-wrapped to the width), so
    /// their count is known: when the QR code does not fit below the essentials (code, time
    /// left, address), the scan hint goes first, then the fingerprint; if it still does not
    /// fit, a line asks for a larger terminal. A QR code is drawn whole or not at all.
    fn draw_opened(
        &self,
        frame: &mut Frame,
        inner: Rect,
        opened: &app_front::pairing::Opened,
        (now_ms, no_color): (i64, bool),
    ) {
        let width = usize::from(inner.width).max(1);
        let height = usize::from(body(inner).height);
        let wrapped = |text: &str, style: Style| -> Vec<Line<'static>> {
            crate::text::wrap(text, width)
                .into_iter()
                .map(|l| Line::from(Span::styled(l, style)))
                .collect()
        };
        let role = match opened.role {
            DeviceRole::User => s(Key::DeviceOfYours),
            DeviceRole::Agent => s(Key::AnAgent),
        };
        let mut head = vec![Line::from(vec![
            Span::raw(format!("{}: ", s(Key::PairingCode))),
            Span::styled(opened.code.clone(), reverse()),
            Span::styled(format!("  {} · {role}", clean(&opened.name)), dim()),
        ])];
        let remaining = opened.remaining_ms(now_ms);
        head.extend(wrapped(
            &if remaining == 0 {
                s(Key::PairingExpired).to_owned()
            } else {
                f(Key::PairingExpiresIn, &[("time", &countdown(remaining))])
            },
            Style::new(),
        ));
        let mut fingerprint = Vec::new();
        if let Some(endpoint) = &opened.endpoint {
            let address = if endpoint.host.contains(':') {
                format!("[{}]:{}", endpoint.host, endpoint.port)
            } else {
                format!("{}:{}", endpoint.host, endpoint.port)
            };
            head.extend(wrapped(
                &f(Key::PairingAddress, &[("address", &address)]),
                Style::new(),
            ));
            fingerprint.extend(wrapped(s(Key::PairingFingerprint), dim()));
            for line in fingerprint_lines(&endpoint.fingerprint, 8) {
                fingerprint.extend(wrapped(&format!("  {line}"), Style::new()));
            }
        }
        let mut lines = head;
        match &opened.qr {
            Some(qr) => {
                let (ink, style) = if no_color {
                    (Ink::Light, Style::new())
                } else {
                    (Ink::Dark, Style::new().fg(Color::Black).bg(Color::White))
                };
                let rows = half_blocks(qr, QUIET_ZONE, ink);
                let qr_width = rows.first().map_or(0, |r| r.chars().count());
                let hint = wrapped(s(Key::PairingScanHint), dim());
                let fits =
                    |extra: usize| qr_width <= width && lines.len() + extra + rows.len() <= height;
                let (show_fp, show_hint) = if fits(fingerprint.len() + hint.len()) {
                    (true, true)
                } else if fits(fingerprint.len()) {
                    (true, false)
                } else {
                    (false, false)
                };
                if show_fp || fits(0) {
                    if show_fp {
                        lines.extend(fingerprint);
                    }
                    if show_hint {
                        lines.extend(hint);
                    }
                    lines.extend(
                        rows.into_iter()
                            .map(|row| Line::from(Span::styled(row, style))),
                    );
                } else {
                    lines.extend(fingerprint);
                    lines.extend(wrapped(s(Key::PairingEnlarge), bold()));
                }
            }
            None => {
                lines.extend(fingerprint);
                let why = match &opened.link_error {
                    Some(error) => f(Key::PairingNoLink, &[("error", &clean(error))]),
                    None => s(Key::PairingNoQr).to_owned(),
                };
                lines.extend(wrapped(&why, dim()));
            }
        }
        frame.render_widget(Paragraph::new(lines), body(inner));
        draw_footer(frame, inner, Key::PairOpenedFooter);
    }

    fn form(&self, lines: &mut Vec<Line<'static>>) {
        lines.push(Line::from(Span::styled(
            s(Key::PairWhichKindOfDevice),
            bold(),
        )));
        for (role, title, detail) in [
            (
                DeviceRole::User,
                Key::DeviceOfYours,
                Key::DeviceOfYoursDetail,
            ),
            (DeviceRole::Agent, Key::AnAgent, Key::AnAgentDetail),
        ] {
            let chosen = self.dialog.role == role;
            let mark = if chosen { "(•) " } else { "( ) " };
            lines.push(Line::from(vec![
                Span::styled(
                    format!("{mark}{}", s(title)),
                    if chosen { reverse() } else { Style::new() },
                ),
                Span::styled(format!("  {}", s(detail)), dim()),
            ]));
        }
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(s(Key::DeviceNameQuestion), bold())));
        let shown = if self.dialog.name.is_empty() {
            Span::styled(s(Key::DeviceNameHint).to_owned(), dim())
        } else {
            Span::raw(clean(&self.dialog.name))
        };
        lines.push(Line::from(vec![
            Span::raw("> "),
            shown,
            Span::styled("▏", dim()),
        ]));
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            format!("[ {} ]", s(Key::Pair)),
            if self.dialog.can_submit() {
                reverse()
            } else {
                dim()
            },
        )));
    }
}

/// The dialog's inside without its footer line.
const fn body(inner: Rect) -> Rect {
    Rect {
        height: inner.height.saturating_sub(1),
        ..inner
    }
}

fn draw_footer(frame: &mut Frame, inner: Rect, footer: Key) {
    let area = Rect {
        y: inner.bottom().saturating_sub(1),
        height: 1,
        ..inner
    };
    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(s(footer), dim()))),
        area,
    );
}
