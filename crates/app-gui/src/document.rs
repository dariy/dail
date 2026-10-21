//! Draws an HTML body's [`Document`] in the reader: paragraphs, headings, lists, quotes,
//! preformatted text, rules and data tables, with bold, italic, code, strike-through and links.
//!
//! Everything here is email content: it is only displayed. A link opens in the browser only
//! when clicked, and its full target is shown on hover (the text may say something else); the
//! reader also lists every target below the body. Quoted text and signatures ([`Block::Fold`])
//! stay folded until the user opens them ([`Folds`]).

use app_core::MessageId;
use app_core::document::{Block, Cell, Document, FoldKind, Span};
use egui::{RichText, Stroke, Ui};

use crate::images::{ImageKey, Images, Slot};
use crate::reader::{Folds, fold_toggle};
use crate::shell::External;
use crate::theme::Tokens;
use crate::ui;

/// What drawing a message's body reads and writes besides the body.
pub struct DocIo<'a> {
    /// The message, whose folds are opened in [`Folds`].
    pub message: MessageId,
    /// Which folds the user opened.
    pub folds: &'a mut Folds,
    /// The body's blob and the image cache, when the sender's inline images are shown;
    /// otherwise their alt text is.
    pub images: Option<(&'a str, &'a mut Images)>,
    /// Link clicks become [`External::OpenUrl`].
    pub externals: &'a mut Vec<External>,
    /// Search words to mark (`app_front::highlight`), while a search is open.
    pub marks: &'a [String],
}

/// Draws `document`.
pub fn show(ui: &mut Ui, t: &Tokens, document: &Document, io: DocIo<'_>) {
    let mut out = Out { t, io, tables: 0 };
    ui.vertical(|ui| out.blocks(ui, &document.blocks));
}

struct Out<'a> {
    t: &'a Tokens,
    io: DocIo<'a>,
    /// Tables drawn so far: each grid needs its own ID.
    tables: usize,
}

impl Out<'_> {
    fn blocks(&mut self, ui: &mut Ui, blocks: &[Block]) {
        for (i, block) in blocks.iter().enumerate() {
            if i > 0 {
                ui.add_space(self.t.m.space.sm);
            }
            self.block(ui, block);
        }
    }

    fn block(&mut self, ui: &mut Ui, block: &Block) {
        match block {
            Block::Paragraph { spans } => self.spans(ui, spans, self.t.type_scale.body_lg.size),
            Block::Heading { level, spans } => {
                // h1 `headline_lg`, h2 `headline_md`, lower headings `body_md` at 600.
                let scale = &self.t.type_scale;
                let size = match level {
                    1 => scale.headline_lg.size,
                    2 => scale.headline_md.size,
                    _ => scale.body_md.size,
                };
                let bold: Vec<Span> = spans
                    .iter()
                    .map(|s| Span {
                        bold: true,
                        ..s.clone()
                    })
                    .collect();
                self.spans(ui, &bold, size);
            }
            Block::List {
                ordered,
                start,
                items,
            } => {
                for (n, item) in items.iter().enumerate() {
                    let marker = if *ordered {
                        let n = u64::from(*start).saturating_add(n as u64);
                        format!("{n}.")
                    } else {
                        "•".to_owned()
                    };
                    ui.horizontal_top(|ui| {
                        let body = &self.t.type_scale.body_lg;
                        let (rect, _) = ui.allocate_exact_size(
                            egui::vec2(self.t.m.space.xl, body.line_height),
                            egui::Sense::hover(),
                        );
                        ui.painter().text(
                            rect.right_top() + egui::vec2(-self.t.m.space.sm, 0.0),
                            egui::Align2::RIGHT_TOP,
                            marker,
                            ui::font(body),
                            self.t.text_secondary,
                        );
                        ui.vertical(|ui| self.blocks(ui, item));
                    });
                }
            }
            Block::Quote { blocks } => {
                let frame = egui::Frame::new()
                    .inner_margin(egui::Margin {
                        left: (self.t.m.space.md + self.t.m.stroke.indicator) as i8,
                        ..egui::Margin::ZERO
                    })
                    .show(ui, |ui| self.blocks(ui, blocks));
                let rect = frame.response.rect;
                ui.painter().vline(
                    rect.left() + self.t.m.stroke.indicator / 2.0,
                    rect.y_range(),
                    Stroke::new(self.t.m.stroke.indicator, self.t.border_strong),
                );
            }
            Block::Preformatted { text } => {
                // `label_md` on `raised`, radius and padding `md`.
                ui::card(self.t).show(ui, |ui| {
                    ui.add(
                        egui::Label::new(ui::text(&self.t.type_scale.label_md, text, self.t.text))
                            .wrap(),
                    );
                });
            }
            Block::Table { rows } => self.table(ui, rows),
            Block::Rule => ui::divider(ui, self.t),
            Block::Fold { kind, blocks } => {
                let message = self.io.message;
                let open = self.io.folds.is_open(message, *kind);
                if fold_toggle(ui, self.t, *kind, open) {
                    self.io.folds.toggle(message, *kind);
                }
                if open {
                    match kind {
                        FoldKind::Quote => self.block(
                            ui,
                            &Block::Quote {
                                blocks: blocks.clone(),
                            },
                        ),
                        FoldKind::Signature => self.blocks(ui, blocks),
                    }
                }
            }
        }
    }

    fn table(&mut self, ui: &mut Ui, rows: &[Vec<Cell>]) {
        self.tables += 1;
        let columns = rows.iter().map(Vec::len).max().unwrap_or(1).max(1);
        let gap = self.t.m.space.md;
        let width = (ui.available_width() - gap * (columns as f32 - 1.0)) / columns as f32;
        egui::Grid::new(("document_table", self.tables))
            .num_columns(columns)
            .spacing([gap, self.t.m.space.xs + 2.0])
            .striped(true)
            .show(ui, |ui| {
                for row in rows {
                    for cell in row {
                        ui.vertical(|ui| {
                            ui.set_max_width(width.max(self.t.m.controls.avatar_lg));
                            self.spans(ui, &cell.spans, self.t.type_scale.body_md.size);
                        });
                    }
                    ui.end_row();
                }
            });
    }

    /// Text that wraps like a paragraph; each span is its own label, so that links can be
    /// clicked.
    fn spans(&mut self, ui: &mut Ui, spans: &[Span], size: f32) {
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing.x = 0.0;
            for span in spans {
                for (i, line) in span.text.split('\n').enumerate() {
                    if i > 0 {
                        ui.end_row();
                    }
                    if !line.is_empty() {
                        self.span(ui, span, line, size);
                    }
                }
            }
        });
    }

    /// The texture of an inline image span, when images are shown and it is loaded.
    fn texture(&mut self, span: &Span) -> Option<egui::TextureHandle> {
        let cid = span.image.as_ref()?;
        let (blob, images) = self.io.images.as_mut()?;
        let key = ImageKey {
            blob: (*blob).to_owned(),
            cid: cid.clone(),
        };
        match images.get(&key) {
            Slot::Ready(texture) => Some(texture),
            Slot::Loading | Slot::Failed(_) => None,
        }
    }

    fn span(&mut self, ui: &mut Ui, span: &Span, text: &str, size: f32) {
        if span.image.is_some() || self.io.marks.is_empty() {
            self.piece(ui, span, text, false, size);
            return;
        }
        for (piece, marked) in app_front::highlight::pieces(text, self.io.marks) {
            self.piece(ui, span, piece, marked, size);
        }
    }

    /// One piece of a span's text, on a muted background if it is a search word.
    fn piece(&mut self, ui: &mut Ui, span: &Span, text: &str, marked: bool, size: f32) {
        if let Some(texture) = self.texture(span) {
            let width = texture.size_vec2().x.min(ui.available_width());
            let image = egui::Image::new(&texture)
                .max_width(width)
                .alt_text(span.text.clone());
            let response = match &span.link {
                Some(_) => ui.add(image.sense(egui::Sense::click())),
                None => ui.add(image),
            };
            self.link(response, span);
            return;
        }
        let mut rich = RichText::new(text).size(size).color(self.t.text);
        if span.bold {
            rich = rich.strong().family(crate::theme::semibold());
        }
        if span.italic {
            rich = rich.italics();
        }
        if span.code {
            // Inline code: `label_sm`'s face on `selected`.
            rich = rich
                .font(ui::mono(size - self.t.m.space.xs / 2.0))
                .background_color(self.t.selected);
        }
        if span.strike {
            rich = rich.strikethrough();
        }
        if marked {
            rich = rich
                .background_color(self.t.emphasis)
                .color(self.t.text_strong);
        }
        if span.link.is_none() {
            ui.label(rich);
            return;
        }
        // Links in `text_strong`, underlined.
        let rich = rich.color(self.t.text_strong).underline();
        let response = ui.add(egui::Label::new(rich).sense(egui::Sense::click()));
        self.link(response, span);
    }

    /// A span's link: the target on hover, opened on click.
    fn link(&mut self, response: egui::Response, span: &Span) {
        let Some(url) = &span.link else {
            return;
        };
        let response = response
            .on_hover_cursor(egui::CursorIcon::PointingHand)
            .on_hover_text(format!("Open in your browser:\n{url}"));
        if response.clicked() {
            self.io.externals.push(External::OpenUrl(url.clone()));
        }
    }
}
