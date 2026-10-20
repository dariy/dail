//! `Bridge`: the GUI's connection to the [`Session`] it owns, over `app_sync::Bridge`.
//!
//! The UI thread never awaits. At the end of a frame it hands its queued commands to
//! [`Bridge::submit`]; the shared bridge submits them in order with `Origin::Gui` and sends each
//! result back, to be picked up by the next frame ([`Bridge::replies`]). Every state change and
//! every reply wakes the UI through `egui::Context::request_repaint`. Opening the original of a
//! message in the browser and loading a message's inline images are the GUI's own, so they live
//! here.

use std::sync::{Arc, mpsc as std_mpsc};

use app_core::{AppState, Command, CommandName, Origin};
use app_sync::{Session, SessionClient, SubmitResult, SyncError};
use tokio::runtime::Handle;
use tokio::task::JoinHandle;

use crate::attachments::{PartKey, PreviewData};
use crate::images::ImageKey;

/// A loaded inline image, or why it couldn't be shown.
pub type LoadedImage = (ImageKey, Result<egui::ColorImage, String>);
/// A loaded attachment preview, or why there is none.
pub type LoadedPreview = (PartKey, Result<PreviewData, String>);

/// Why the GUI submitted a command; decides what happens with the result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Purpose {
    /// A user action: rejections and undo labels are shown as toasts.
    User,
    /// A background query of the UI (e.g. message bodies for the reader). Only the result's
    /// consumer looks at it.
    Background(Background),
}

/// Background queries the UI makes on its own.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Background {
    /// `get_thread` with bodies for the reader.
    ReaderBodies(app_core::ThreadId),
    /// The account wizard's `add_account` (its reply carries the new account's ID).
    WizardAdd,
    /// The account wizard's `discover_servers` (the servers arrive in `status.discovery`).
    WizardLookup,
    /// The password form's `set_password` (the test's result arrives in the account's status).
    WizardPassword,
}

/// A command and why it was submitted.
#[derive(Debug, Clone, PartialEq)]
pub struct Request {
    /// Why it was submitted.
    pub purpose: Purpose,
    /// The command.
    pub cmd: Command,
}

impl Request {
    /// A user action.
    pub const fn user(cmd: Command) -> Self {
        Self {
            purpose: Purpose::User,
            cmd,
        }
    }
}

/// The result of one [`Request`].
#[derive(Debug)]
pub struct Reply {
    /// Why the command was submitted.
    pub purpose: Purpose,
    /// Which command it was.
    pub name: CommandName,
    /// The engine's receipt or rejection, or the runtime failure.
    pub result: Result<SubmitResult, SyncError>,
}

/// The UI side of the connection.
pub struct Bridge {
    inner: app_sync::Bridge<Purpose>,
    runtime: Handle,
    session: SessionClient,
    notices: (std_mpsc::Sender<String>, std_mpsc::Receiver<String>),
    images: (
        std_mpsc::Sender<LoadedImage>,
        std_mpsc::Receiver<LoadedImage>,
    ),
    previews: (
        std_mpsc::Sender<LoadedPreview>,
        std_mpsc::Receiver<LoadedPreview>,
    ),
    repaint: egui::Context,
}

impl Bridge {
    /// Moves `session` onto a task of `runtime`. The task ends (and shuts the session down,
    /// saving unsaved drafts) once the bridge is closed or dropped; await the returned handle
    /// to wait for that.
    pub fn start(
        runtime: &Handle,
        session: Session,
        repaint: egui::Context,
    ) -> (Self, JoinHandle<()>) {
        let client = session.client();
        let wake = repaint.clone();
        let (inner, task) = app_sync::Bridge::own(
            runtime,
            session,
            Origin::Gui,
            Arc::new(move || wake.request_repaint()),
        );
        (
            Self {
                inner,
                runtime: runtime.clone(),
                session: client,
                notices: std_mpsc::channel(),
                images: std_mpsc::channel(),
                previews: std_mpsc::channel(),
                repaint,
            },
            task,
        )
    }

    /// The current state. Cheap: a shared snapshot, unchanged for the rest of the frame.
    pub fn state(&self) -> Arc<AppState> {
        self.inner.state()
    }

    /// Queues a command for the session. Ignored once the bridge is closed.
    pub fn submit(&self, request: Request) {
        self.inner.submit(request.purpose, request.cmd);
    }

    /// Writes the sanitized page of a stored message body (`blob`) to a private temporary file
    /// and opens it in the system browser. Failures arrive as [`Bridge::notices`].
    pub fn open_original(&self, blob: String) {
        let session = self.session.clone();
        let notices = self.notices.0.clone();
        let repaint = self.repaint.clone();
        self.runtime.spawn(async move {
            if let Err(notice) = open_original(&session, &blob).await {
                let _ = notices.send(notice);
                repaint.request_repaint();
            }
        });
    }

    /// Reads an inline image part from the store and decodes it on a blocking thread; the
    /// result arrives through [`Bridge::images`].
    pub fn load_image(&self, key: ImageKey) {
        let session = self.session.clone();
        let images = self.images.0.clone();
        let repaint = self.repaint.clone();
        self.runtime.spawn(async move {
            let image = load_image(&session, &key).await;
            let _ = images.send((key, image));
            repaint.request_repaint();
        });
    }

    /// Reads an attachment for its preview (decoding images on a blocking thread); the result
    /// arrives through [`Bridge::previews`].
    pub fn load_preview(&self, key: PartKey) {
        let session = self.session.clone();
        let previews = self.previews.0.clone();
        let repaint = self.repaint.clone();
        self.runtime.spawn(async move {
            let preview = load_preview(&session, &key).await;
            let _ = previews.send((key, preview));
            repaint.request_repaint();
        });
    }

    /// Attachment previews loaded since the last call.
    pub fn previews(&self) -> Vec<LoadedPreview> {
        self.previews.1.try_iter().collect()
    }

    /// Writes a private temporary copy of an attachment and opens it in the browser's viewer.
    /// Failures arrive as [`Bridge::notices`].
    pub fn open_attachment(&self, key: PartKey) {
        let session = self.session.clone();
        let notices = self.notices.0.clone();
        let repaint = self.repaint.clone();
        self.runtime.spawn(async move {
            if let Err(notice) = open_attachment(&session, &key).await {
                let _ = notices.send(notice);
                repaint.request_repaint();
            }
        });
    }

    /// Inline images loaded since the last call.
    pub fn images(&self) -> Vec<LoadedImage> {
        self.images.1.try_iter().collect()
    }

    /// Failures of background work (not of commands) since the last call, for the user.
    pub fn notices(&self) -> Vec<String> {
        self.notices.1.try_iter().collect()
    }

    /// Results that arrived since the last call, in submission order.
    pub fn replies(&self) -> Vec<Reply> {
        self.inner
            .replies()
            .into_iter()
            .map(|reply| Reply {
                purpose: reply.tag,
                name: reply.name,
                result: reply.result,
            })
            .collect()
    }

    /// Stops accepting commands; the session shuts down after the queued ones.
    pub fn close(&mut self) {
        self.inner.close();
    }
}

async fn load_image(session: &SessionClient, key: &ImageKey) -> Result<egui::ColorImage, String> {
    let part = match session.inline_part(&key.blob, &key.cid).await {
        Ok(Some(part)) => part,
        Ok(None) => return Err("the image is not in the message".to_owned()),
        Err(err) => return Err(err.to_string()),
    };
    if !part.content_type.starts_with("image/") {
        return Err(format!("{} is not an image", part.content_type));
    }
    tokio::task::spawn_blocking(move || crate::images::decode(&part.bytes))
        .await
        .map_err(|err| err.to_string())?
}

async fn load_preview(session: &SessionClient, key: &PartKey) -> Result<PreviewData, String> {
    let (_, part) = match session.attachment_part(&key.blob, key.part).await {
        Ok(Some(part)) => part,
        Ok(None) => return Err("the attachment is not in the local store".to_owned()),
        Err(err) => return Err(err.to_string()),
    };
    tokio::task::spawn_blocking(move || {
        crate::attachments::preview_data(&part.content_type, &part.bytes)
    })
    .await
    .map_err(|err| err.to_string())?
}

async fn open_in_browser(path: std::path::PathBuf) -> Result<(), String> {
    let target = path.display().to_string();
    let opened = tokio::task::spawn_blocking(move || webbrowser::open(&target)).await;
    match opened {
        Ok(Ok(())) => Ok(()),
        Ok(Err(err)) => Err(format!("Could not open the browser: {err}")),
        Err(err) => Err(format!("Could not open the browser: {err}")),
    }
}

async fn open_original(session: &SessionClient, blob: &str) -> Result<(), String> {
    let path = match session.open_original(blob).await {
        Ok(Some(path)) => path,
        Ok(None) => return Err("The message is not in the local store any more.".to_owned()),
        Err(err) => return Err(format!("Could not write the original: {err}")),
    };
    open_in_browser(path).await
}

async fn open_attachment(session: &SessionClient, key: &PartKey) -> Result<(), String> {
    let path = match session.open_attachment(&key.blob, key.part).await {
        Ok(Some(path)) => path,
        Ok(None) => return Err("The attachment is not in the local store any more.".to_owned()),
        Err(err) => return Err(format!("Could not write the attachment: {err}")),
    };
    open_in_browser(path).await
}
