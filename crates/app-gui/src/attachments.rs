//! A received message's attachments in the reader: name, size and type, *Save…* for one and
//! *Save all…* for every one. The folder comes from the native folder dialog (`rfd`, on its own
//! thread); the save itself is `save_attachments`, whose progress the state shows
//! (`status.saves`). Images and text have *Preview* (decoded off the UI thread on request,
//! [`Previews`]); a PDF has *Open*, a private temporary copy in the browser's viewer. Previews
//! and opening are presentation, like *open original*: not commands.

use std::collections::{HashMap, HashSet};
use std::sync::mpsc;

use app_core::{AppState, Command, MessageId, SaveState};
use app_front::body::Body;
use app_front::format;
use app_theme::Icon;
use egui::TextureHandle;
use egui::{RichText, Ui};

use crate::bridge::Request;
use crate::shell::External;
use crate::theme::Tokens;
use crate::ui;

/// Asks the user for a folder (blocking; runs on a helper thread).
pub type FolderPicker = fn() -> Option<String>;

fn pick_folder() -> Option<String> {
    rfd::FileDialog::new()
        .set_title("Save attachments into")
        .pick_folder()
        .and_then(|path| path.to_str().map(str::to_owned))
}

/// The open folder dialog, if any, and what it is for.
pub struct Saver {
    picker: FolderPicker,
    pending: Option<(MessageId, Vec<usize>, mpsc::Receiver<Option<String>>)>,
}

impl Default for Saver {
    fn default() -> Self {
        Self {
            picker: pick_folder,
            pending: None,
        }
    }
}

impl Saver {
    /// A saver that asks `picker` instead of the native dialog (tests).
    pub fn with_picker(picker: FolderPicker) -> Self {
        Self {
            picker,
            pending: None,
        }
    }

    fn ask(&mut self, ctx: &egui::Context, message: MessageId, parts: Vec<usize>) {
        if self.pending.is_some() {
            return;
        }
        let (tx, rx) = mpsc::channel();
        let (ctx, picker) = (ctx.clone(), self.picker);
        let spawned = std::thread::Builder::new()
            .name("dail-folder-dialog".to_owned())
            .spawn(move || {
                let _ = tx.send(picker());
                ctx.request_repaint();
            });
        match spawned {
            Ok(_) => self.pending = Some((message, parts, rx)),
            Err(err) => tracing::warn!(error = %err, "could not open the folder dialog"),
        }
    }

    /// Turns a chosen folder into `save_attachments`.
    pub fn receive(&mut self, queue: &mut Vec<Request>) {
        let Some((_, _, rx)) = &self.pending else {
            return;
        };
        match rx.try_recv() {
            Ok(dir) => {
                if let (Some(dir), Some((message, parts, _))) = (dir, self.pending.take()) {
                    queue.push(Request::user(Command::SaveAttachments {
                        message,
                        parts,
                        dir,
                    }));
                }
                self.pending = None;
            }
            Err(mpsc::TryRecvError::Empty) => {}
            Err(mpsc::TryRecvError::Disconnected) => self.pending = None,
        }
    }
}

/// The most text a preview shows.
const MAX_PREVIEW_TEXT: usize = 64 * 1024;

/// An attachment of a stored body: the blob and the MIME part number.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PartKey {
    /// The body's blob ID (`BodyState::Fetched`).
    pub blob: String,
    /// The MIME part number.
    pub part: usize,
}

/// A loaded preview, before it becomes a texture.
#[derive(Debug, Clone, PartialEq)]
pub enum PreviewData {
    /// A decoded image.
    Image(egui::ColorImage),
    /// The start of a text attachment.
    Text(String),
}

#[derive(Clone)]
enum Preview {
    Loading,
    Image(TextureHandle),
    Text(String),
    Failed(String),
}

/// The previews the user opened, and the ones loaded.
#[derive(Default)]
pub struct Previews {
    open: HashSet<(MessageId, usize)>,
    loaded: HashMap<PartKey, Preview>,
    wanted: Vec<PartKey>,
}

impl Previews {
    /// The previews to load, each asked for once (`External::LoadPreview`).
    pub fn take_wanted(&mut self) -> Vec<PartKey> {
        std::mem::take(&mut self.wanted)
    }

    /// Takes a loaded preview (or why there is none).
    pub fn accept(&mut self, ctx: &egui::Context, key: PartKey, data: Result<PreviewData, String>) {
        let preview = match data {
            Ok(PreviewData::Image(image)) => Preview::Image(ctx.load_texture(
                format!("part:{}:{}", key.blob, key.part),
                image,
                egui::TextureOptions::LINEAR,
            )),
            Ok(PreviewData::Text(text)) => Preview::Text(text),
            Err(error) => Preview::Failed(error),
        };
        self.loaded.insert(key, preview);
    }

    fn toggle(&mut self, message: MessageId, part: usize) {
        if !self.open.remove(&(message, part)) {
            self.open.insert((message, part));
        }
    }

    fn get(&mut self, key: &PartKey) -> Preview {
        if let Some(preview) = self.loaded.get(key) {
            return preview.clone();
        }
        self.loaded.insert(key.clone(), Preview::Loading);
        self.wanted.push(key.clone());
        Preview::Loading
    }
}

/// Turns attachment bytes into a preview: images decoded ([`crate::images::decode`]), text as
/// UTF-8 (the first 64 KiB).
pub fn preview_data(content_type: &str, bytes: &[u8]) -> Result<PreviewData, String> {
    if content_type.starts_with("image/") {
        crate::images::decode(bytes).map(PreviewData::Image)
    } else if is_text(content_type) {
        // A character cut at the end becomes U+FFFD.
        let end = bytes.len().min(MAX_PREVIEW_TEXT);
        Ok(PreviewData::Text(
            String::from_utf8_lossy(&bytes[..end]).into_owned(),
        ))
    } else {
        Err(format!("no preview for {content_type}"))
    }
}

fn is_text(content_type: &str) -> bool {
    content_type.starts_with("text/") && content_type != "text/html"
}

fn previewable(content_type: &str) -> bool {
    matches!(
        content_type,
        "image/png" | "image/jpeg" | "image/gif" | "image/webp"
    ) || is_text(content_type)
}

/// What the attachments section changes besides the queue.
pub struct AttachIo<'a> {
    /// The folder dialog.
    pub saver: &'a mut Saver,
    /// Previews.
    pub previews: &'a mut Previews,
    /// *Open* requests.
    pub externals: &'a mut Vec<External>,
}

/// The attachments of `message` (stored as `blob` once downloaded), with their buttons, open
/// previews and the last save's outcome.
pub fn show(
    ui: &mut Ui,
    t: &Tokens,
    state: &AppState,
    (message, blob): (MessageId, Option<&str>),
    body: &Body,
    io: AttachIo<'_>,
) {
    let AttachIo {
        saver,
        previews,
        externals,
    } = io;
    if body.attachments.is_empty() {
        return;
    }
    ui.add_space(t.m.space.md);
    ui.horizontal(|ui| {
        let total: u64 = body.attachments.iter().map(|a| a.size).sum();
        let n = body.attachments.len();
        let files = if n == 1 {
            "1 file".to_owned()
        } else {
            format!("{n} files")
        };
        ui::caption(
            ui,
            t,
            &format!("Attachments ({files} · {})", format::size(total)),
        );
        if n > 1 {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if icon_save_button(ui, t, Icon::Download, "Save all", "Save all…") {
                    let parts = body.attachments.iter().map(|a| a.part).collect();
                    saver.ask(ui.ctx(), message, parts);
                }
            });
        }
    });
    ui.add_space(t.m.space.xs);
    for attachment in &body.attachments {
        ui::card(t).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = t.m.space.md;
                let icon = if attachment.content_type == "application/pdf" {
                    Icon::PictureAsPdf
                } else if attachment.content_type.starts_with("image/") {
                    Icon::Image
                } else {
                    Icon::Description
                };
                let tile = t.m.controls.avatar_lg;
                let (rect, _) =
                    ui.allocate_exact_size(egui::Vec2::splat(tile), egui::Sense::hover());
                ui.painter()
                    .rect_filled(rect, ui::radius(t.m.radius.md), t.emphasis);
                ui::paint_icon(
                    ui,
                    rect.center(),
                    icon,
                    t.m.controls.icon_lg,
                    t.text_strong,
                    false,
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if attachment.content_type == "application/pdf"
                        && let Some(blob) = blob
                        && ui::icon_button(ui, t, t.text_secondary, Icon::OpenInNew, "Open ↗")
                            .clicked()
                    {
                        externals.push(External::OpenAttachment(PartKey {
                            blob: blob.to_owned(),
                            part: attachment.part,
                        }));
                    }
                    if blob.is_some() && previewable(&attachment.content_type) {
                        let open = previews.open.contains(&(message, attachment.part));
                        let (icon, name) = if open {
                            (Icon::VisibilityOff, "Hide preview")
                        } else {
                            (Icon::Visibility, "Preview")
                        };
                        if ui::icon_button(ui, t, t.text_secondary, icon, name).clicked() {
                            previews.toggle(message, attachment.part);
                        }
                    }
                    if ui::icon_button(ui, t, t.text_secondary, Icon::Download, "Save…").clicked()
                    {
                        saver.ask(ui.ctx(), message, vec![attachment.part]);
                    }
                    ui.with_layout(egui::Layout::top_down(egui::Align::Min), |ui| {
                        ui.spacing_mut().item_spacing.y = t.m.space.xs / 2.0;
                        let name = ui.add(
                            egui::Label::new(ui::text_at(
                                &t.type_scale.body_md,
                                600,
                                attachment.name.clone(),
                                t.text_strong,
                            ))
                            .truncate(),
                        );
                        let detail = format!(
                            "{} · {}",
                            format::size(attachment.size),
                            attachment.content_type
                        );
                        ui.label(ui::text(
                            &t.type_scale.label_sm,
                            detail.clone(),
                            t.text_muted,
                        ));
                        // Screen readers hear the whole line, as before.
                        let spoken = format!(
                            "📎 {}  ·  {}  ·  {}",
                            attachment.name,
                            format::size(attachment.size),
                            attachment.content_type
                        );
                        name.widget_info(|| {
                            egui::WidgetInfo::labeled(egui::Role::Label, true, &spoken)
                        });
                    });
                });
            });
        });
        ui.add_space(t.m.space.xs + 2.0);
        if let Some(blob) = blob
            && previews.open.contains(&(message, attachment.part))
        {
            let key = PartKey {
                blob: blob.to_owned(),
                part: attachment.part,
            };
            preview(ui, t, &previews.get(&key), &key);
        }
    }
    let Some(save) = state.status.saves.iter().find(|s| s.message == message) else {
        return;
    };
    match &save.state {
        SaveState::Saving => {
            ui.label(ui::text(
                &t.type_scale.body_sm,
                format!("Saving into {}…", save.dir),
                t.text_muted,
            ));
        }
        SaveState::Saved { files } => {
            ui.label(ui::text(
                &t.type_scale.body_sm,
                saved_text(files),
                t.text_muted,
            ));
        }
        SaveState::Failed { files, error } => {
            ui.colored_label(
                ui.visuals().error_fg_color,
                format!("Could not save the attachments: {error}"),
            );
            if !files.is_empty() {
                ui.label(RichText::new(saved_text(files)).color(t.text_muted));
            }
        }
    }
}

fn preview(ui: &mut Ui, t: &Tokens, preview: &Preview, key: &PartKey) {
    ui::card_base(t).show(ui, |ui| match preview {
        Preview::Loading => {
            ui.label(RichText::new("Loading the preview…").color(t.text_muted));
        }
        Preview::Image(texture) => {
            let width = texture.size_vec2().x.min(ui.available_width());
            ui.add(
                egui::Image::new(texture)
                    .max_width(width)
                    .alt_text("Preview"),
            );
        }
        Preview::Text(text) => {
            egui::ScrollArea::vertical()
                .id_salt(("preview", &key.blob, key.part))
                .max_height(t.m.overlays.palette_list_max)
                .show(ui, |ui| {
                    ui.add(egui::Label::new(ui::text(&t.type_scale.label_md, text, t.text)).wrap());
                });
        }
        Preview::Failed(error) => {
            ui.label(RichText::new(format!("No preview: {error}")).color(t.text_muted));
        }
    });
}

fn saved_text(files: &[String]) -> String {
    match files {
        [one] => format!("Saved as {one}"),
        _ => format!("Saved {} files: {}", files.len(), files.join(", ")),
    }
}

/// [`save_button`] with an icon; `name` is its accessible name.
fn icon_save_button(ui: &mut Ui, t: &Tokens, icon: Icon, label: &str, name: &str) -> bool {
    let job = ui::icon_text(
        icon,
        label,
        ui::font(&t.type_scale.body_sm),
        t.text_secondary,
    );
    let response = ui::ghost(ui, t, job);
    response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, name));
    response.clicked()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn previews_are_images_or_text() {
        assert!(matches!(
            preview_data("text/csv", b"a,b\n1,2"),
            Ok(PreviewData::Text(t)) if t == "a,b\n1,2"
        ));
        let long = "é".repeat(MAX_PREVIEW_TEXT);
        let Ok(PreviewData::Text(cut)) = preview_data("text/plain", long.as_bytes()) else {
            unreachable!()
        };
        assert!(cut.len() <= MAX_PREVIEW_TEXT + 3);
        assert!(preview_data("image/png", b"not a png").is_err());
        assert!(preview_data("application/pdf", b"%PDF").is_err());
        assert!(!previewable("text/html"), "markup is not shown as text");
        assert!(previewable("image/webp") && !previewable("image/svg+xml"));
    }
}
