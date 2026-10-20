//! The reader: body text, link targets shown and opened only on click, "open original".

// reason: helpers outside `#[test]` fns may panic; clippy.toml only exempts `#[test]` bodies.
#![allow(clippy::unwrap_used, clippy::panic)]

use crate::support::{apply, gui, inbox_of, runtime, session, synced_state, wait_for_notice};
use app_core::{BodyState, Command, CommandKind, CommandName, CommandReceipt};
use app_gui::attachments::{PartKey, PreviewData};
use app_gui::images::ImageKey;
use app_gui::{Background, Bridge, External, Purpose, Reply};
use egui::accesskit::Role;
use egui_kittest::kittest::{By, Queryable};
use serde_json::json;

#[test]
fn bodies_show_their_links_which_open_only_on_click() {
    let state = synced_state();
    let (inbox, threads) = inbox_of(&state);
    let message = state.threads[&threads[0]].messages[0];
    let mut h = gui(state);
    apply(&mut h, Command::OpenMailbox { id: inbox });
    apply(&mut h, Command::OpenThread { id: threads[0] });
    // The session would attach the body text to the background get_thread; do it by hand.
    h.state_mut().out.clear();
    h.state_mut().replies.push(Reply {
        purpose: Purpose::Background(Background::ReaderBodies(threads[0])),
        name: CommandName::GetThread,
        result: Ok(Ok(CommandReceipt {
            ok: true,
            name: "get_thread".into(),
            kind: Some(CommandKind::Query),
            affected: Vec::new(),
            data: json!({ "messages": [{
                "id": message,
                "untrusted_content": { "body": {
                    "text": "Lunch at [noon][1]?\n\n[1]: https://example.org/menu",
                    "text_source": "html",
                    "attachments": [{ "filename": "menu.pdf", "content_type": "application/pdf",
                                      "size": 2048, "inline": false }],
                }},
            }]}),
            undo_label: None,
        })),
    });
    h.run();
    h.get_by_label_contains("Lunch at [noon][1]?");
    h.get_by_label("LINKS (1)");
    h.get_by_label_contains("menu.pdf");
    assert!(
        h.state_mut().shell.take_external().is_empty(),
        "nothing opens by itself"
    );
    h.get_by_label("↗ https://example.org/menu").click();
    h.run();
    assert_eq!(
        h.state_mut().shell.take_external(),
        [External::OpenUrl("https://example.org/menu".into())]
    );
}

#[test]
fn html_bodies_are_drawn_from_their_document() {
    let state = synced_state();
    let (inbox, threads) = inbox_of(&state);
    let message = state.threads[&threads[0]].messages[0];
    let mut h = gui(state);
    apply(&mut h, Command::OpenMailbox { id: inbox });
    apply(&mut h, Command::OpenThread { id: threads[0] });
    h.state_mut().out.clear();
    h.state_mut().replies.push(Reply {
        purpose: Purpose::Background(Background::ReaderBodies(threads[0])),
        name: CommandName::GetThread,
        result: Ok(Ok(CommandReceipt {
            ok: true,
            name: "get_thread".into(),
            kind: Some(CommandKind::Query),
            affected: Vec::new(),
            data: json!({ "messages": [{
                "id": message,
                "untrusted_content": { "body": {
                    "text": "LUNCH\n\nAt [noon][1]?\n\n[1]: https://example.org/menu",
                    "text_source": "html",
                    "attachments": [],
                    "document": { "blocks": [
                        { "type": "heading", "level": 1, "spans": [{ "text": "Lunch" }] },
                        { "type": "paragraph", "spans": [
                            { "text": "At " },
                            { "text": "noon", "link": "https://example.org/menu" },
                            { "text": "?" },
                        ]},
                        { "type": "list", "ordered": true, "start": 1, "items": [
                            [{ "type": "paragraph", "spans": [{ "text": "Soup" }] }],
                        ]},
                        { "type": "table", "rows": [[
                            { "header": true, "spans": [{ "text": "Dish" }] },
                            { "spans": [{ "text": "Price" }] },
                        ]]},
                        { "type": "fold", "kind": "signature", "blocks": [
                            { "type": "paragraph", "spans": [{ "text": "-- Chef" }] },
                        ]},
                    ]},
                }},
            }]}),
            undo_label: None,
        })),
    });
    h.run();
    // The blocks, not the text rendering.
    assert!(h.query_by_label_contains("[noon][1]").is_none());
    for label in ["Lunch", "At ", "Soup", "Dish", "Price"] {
        h.get_by_label(label);
    }
    assert!(
        h.query_by_label("-- Chef").is_none(),
        "the signature is folded"
    );
    h.get_by_label("··· Show signature").click();
    h.run();
    h.get_by_label("-- Chef");
    // The link's text opens its target on click; the full target is still listed below.
    h.get_by_label("LINKS (1)");
    assert!(h.state_mut().shell.take_external().is_empty());
    h.get_by_label("noon").click();
    h.run();
    assert_eq!(
        h.state_mut().shell.take_external(),
        [External::OpenUrl("https://example.org/menu".into())]
    );
}

/// A reply to the reader's body request: one message whose document shows the inline image
/// `chart@x`.
fn image_body(thread: app_core::ThreadId, message: app_core::MessageId) -> Reply {
    Reply {
        purpose: Purpose::Background(Background::ReaderBodies(thread)),
        name: CommandName::GetThread,
        result: Ok(Ok(CommandReceipt {
            ok: true,
            name: "get_thread".into(),
            kind: Some(CommandKind::Query),
            affected: Vec::new(),
            data: json!({ "messages": [{
                "id": message,
                "untrusted_content": { "body": {
                    "text": "See [Chart]",
                    "text_source": "html",
                    "attachments": [],
                    "document": { "blocks": [{ "type": "paragraph", "spans": [
                        { "text": "See " },
                        { "text": "[Chart]", "italic": true, "image": "chart@x" },
                    ]}]},
                }},
            }]}),
            undo_label: None,
        })),
    }
}

#[test]
fn inline_images_show_only_for_allowed_senders() {
    let mut state = synced_state();
    let (inbox, threads) = inbox_of(&state);
    let message = state.threads[&threads[0]].messages[0];
    let blob = "cd".repeat(32);
    state.messages.get_mut(&message).unwrap().body = BodyState::Fetched { blob: blob.clone() };
    let sender = state.messages[&message].from[0].email.clone();
    let mut h = gui(state);
    apply(&mut h, Command::OpenMailbox { id: inbox });
    apply(&mut h, Command::OpenThread { id: threads[0] });
    h.state_mut().out.clear();
    h.state_mut().replies.push(image_body(threads[0], message));
    h.run();
    // Hidden: the alt text, and a way to show the sender's images. Nothing is loaded.
    h.get_by_label("[Chart]");
    h.get_by_label("Pictures in this message are hidden.");
    assert!(h.state_mut().shell.take_external().is_empty());
    h.get(
        By::new()
            .role(Role::Button)
            .label(format!("Show images from {sender}").as_str()),
    )
    .click();
    h.run();
    assert_eq!(
        h.state_mut().take(),
        [Command::ShowImagesFrom {
            sender: sender.clone()
        }]
    );

    // Shown: the part is asked for once, and drawn when it arrives.
    apply(
        &mut h,
        Command::ShowImagesFrom {
            sender: sender.clone(),
        },
    );
    h.run();
    let key = ImageKey {
        blob,
        cid: "chart@x".into(),
    };
    assert_eq!(
        h.state_mut().shell.take_external(),
        [External::LoadImage(key.clone())]
    );
    h.run();
    assert!(h.state_mut().shell.take_external().is_empty(), "asked once");
    let ctx = h.ctx.clone();
    let pixels = egui::ColorImage::filled([4, 2], egui::Color32::RED);
    h.state_mut().shell.accept_image(&ctx, key, Ok(pixels));
    h.run();
    h.get(By::new().role(Role::Image).label("[Chart]"));
    h.get(
        By::new()
            .role(Role::Button)
            .label(format!("Hide images from {sender}").as_str()),
    )
    .click();
    h.run();
    assert_eq!(h.state_mut().take(), [Command::HideImagesFrom { sender }]);
}

#[test]
fn read_messages_collapse_and_quoted_text_folds() {
    // One conversation: Bob's read "Q3 numbers", then Carol's unread "Lunch?".
    let mut state = synced_state();
    let (inbox, threads) = inbox_of(&state);
    let (lunch, q3) = (
        state.threads[&threads[0]].messages[0],
        state.threads[&threads[1]].messages[0],
    );
    state.threads.shift_remove(&threads[1]);
    state.threads.get_mut(&threads[0]).unwrap().messages = vec![q3, lunch];
    state.messages.get_mut(&q3).unwrap().thread = threads[0];
    let mut h = gui(state);
    apply(&mut h, Command::OpenMailbox { id: inbox });
    apply(&mut h, Command::OpenThread { id: threads[0] });
    h.state_mut().out.clear();
    h.get_by_label("Expand message from Bob <bob@example.org>")
        .click();
    h.run();
    assert_eq!(
        h.state_mut().take(),
        [Command::ExpandMessages { ids: vec![q3] }]
    );
    h.get_by_label("Collapse message from Carol <carol@example.org>")
        .click();
    h.run();
    assert_eq!(
        h.state_mut().take(),
        [Command::CollapseMessages { ids: vec![lunch] }]
    );

    h.state_mut().replies.push(Reply {
        purpose: Purpose::Background(Background::ReaderBodies(threads[0])),
        name: CommandName::GetThread,
        result: Ok(Ok(CommandReceipt {
            ok: true,
            name: "get_thread".into(),
            kind: Some(CommandKind::Query),
            affected: Vec::new(),
            data: json!({ "messages": [{
                "id": lunch,
                "untrusted_content": { "body": {
                    "text": "Sure.\n\nOn Mon, Bob wrote:\n> Lunch?",
                    "text_source": "plain",
                    "attachments": [],
                }},
            }]}),
            undo_label: None,
        })),
    });
    h.run();
    h.get_by_label("Sure.");
    assert!(h.query_by_label_contains("> Lunch?").is_none(), "folded");
    h.get_by_label("··· Show quoted text").click();
    h.run();
    h.get_by_label("On Mon, Bob wrote:\n> Lunch?");
    h.get_by_label("Hide quoted text").click();
    h.run();
    assert!(h.query_by_label_contains("> Lunch?").is_none());
    assert!(h.state_mut().take().is_empty(), "folds are not commands");
}

#[test]
fn attachments_are_saved_into_a_chosen_folder() {
    let mut state = synced_state();
    let (inbox, threads) = inbox_of(&state);
    let message = state.threads[&threads[0]].messages[0];
    state.messages.get_mut(&message).unwrap().body = BodyState::Fetched {
        blob: "ef".repeat(32),
    };
    let mut h = gui(state);
    let shell = std::mem::take(&mut h.state_mut().shell);
    h.state_mut().shell = shell.with_folder_picker(|| Some("/home/me/Downloads".into()));
    apply(&mut h, Command::OpenMailbox { id: inbox });
    apply(&mut h, Command::OpenThread { id: threads[0] });
    h.state_mut().out.clear();
    h.state_mut().replies.push(Reply {
        purpose: Purpose::Background(Background::ReaderBodies(threads[0])),
        name: CommandName::GetThread,
        result: Ok(Ok(CommandReceipt {
            ok: true,
            name: "get_thread".into(),
            kind: Some(CommandKind::Query),
            affected: Vec::new(),
            data: json!({ "messages": [{
                "id": message,
                "untrusted_content": { "body": {
                    "text": "Menus attached.",
                    "text_source": "plain",
                    "attachments": [
                        { "part": 2, "filename": "menu.pdf", "content_type": "application/pdf",
                          "size": 2048, "inline": false },
                        { "part": 3, "filename": "wine.pdf", "content_type": "application/pdf",
                          "size": 1024, "inline": false },
                        { "part": 4, "filename": "logo.png", "content_type": "image/png",
                          "size": 10, "inline": true },
                    ],
                }},
            }]}),
            undo_label: None,
        })),
    });
    h.run();
    let saved = |h: &mut egui_kittest::Harness<'static, crate::support::Gui>| {
        // The folder dialog answers on its own thread.
        for _ in 0..200 {
            h.run();
            let sent = h.state_mut().take();
            if !sent.is_empty() {
                return sent;
            }
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        panic!("no save_attachments")
    };
    h.get_by_label("Save all…").click();
    assert_eq!(
        saved(&mut h),
        [Command::SaveAttachments {
            message,
            parts: vec![2, 3],
            dir: "/home/me/Downloads".into(),
        }]
    );
    h.get_all_by_label("Save…").next().unwrap().click();
    assert_eq!(
        saved(&mut h),
        [Command::SaveAttachments {
            message,
            parts: vec![2],
            dir: "/home/me/Downloads".into(),
        }]
    );
    // The outcome comes from the state.
    apply(
        &mut h,
        Command::SaveAttachments {
            message,
            parts: vec![2],
            dir: "/home/me/Downloads".into(),
        },
    );
    h.get_by_label("Saving into /home/me/Downloads…");
    crate::support::ingest(
        &mut h,
        app_core::Event::AttachmentsSaved {
            message,
            files: vec!["/home/me/Downloads/menu.pdf".into()],
        },
    );
    h.get_by_label("Saved as /home/me/Downloads/menu.pdf");
}

#[test]
fn text_and_image_attachments_preview_and_pdfs_open() {
    let mut state = synced_state();
    let (inbox, threads) = inbox_of(&state);
    let message = state.threads[&threads[0]].messages[0];
    let blob = "fa".repeat(32);
    state.messages.get_mut(&message).unwrap().body = BodyState::Fetched { blob: blob.clone() };
    let mut h = gui(state);
    apply(&mut h, Command::OpenMailbox { id: inbox });
    apply(&mut h, Command::OpenThread { id: threads[0] });
    h.state_mut().out.clear();
    h.state_mut().replies.push(Reply {
        purpose: Purpose::Background(Background::ReaderBodies(threads[0])),
        name: CommandName::GetThread,
        result: Ok(Ok(CommandReceipt {
            ok: true,
            name: "get_thread".into(),
            kind: Some(CommandKind::Query),
            affected: Vec::new(),
            data: json!({ "messages": [{
                "id": message,
                "untrusted_content": { "body": {
                    "text": "Menus attached.",
                    "text_source": "plain",
                    "attachments": [
                        { "part": 2, "filename": "menu.csv", "content_type": "text/csv",
                          "size": 20, "inline": false },
                        { "part": 3, "filename": "wine.pdf", "content_type": "application/pdf",
                          "size": 1024, "inline": false },
                        { "part": 4, "filename": "page.html", "content_type": "text/html",
                          "size": 99, "inline": false },
                    ],
                }},
            }]}),
            undo_label: None,
        })),
    });
    h.run();
    // One preview (the CSV; HTML is not previewed), one PDF to open.
    assert_eq!(h.get_all_by_label("Preview").count(), 1);
    assert!(
        h.state_mut().shell.take_external().is_empty(),
        "nothing loads by itself"
    );
    h.get_by_label("Preview").click();
    h.run();
    let csv = PartKey {
        blob: blob.clone(),
        part: 2,
    };
    assert_eq!(
        h.state_mut().shell.take_external(),
        [External::LoadPreview(csv.clone())]
    );
    h.get_by_label("Loading the preview…");
    let ctx = h.ctx.clone();
    h.state_mut().shell.accept_preview(
        &ctx,
        csv,
        Ok(PreviewData::Text("dish,price\nsoup,4".into())),
    );
    h.run();
    h.get_by_label("dish,price\nsoup,4");
    h.get_by_label("Hide preview").click();
    h.run();
    assert!(h.query_by_label("dish,price\nsoup,4").is_none());
    h.get_by_label("Open ↗").click();
    h.run();
    assert_eq!(
        h.state_mut().shell.take_external(),
        [External::OpenAttachment(PartKey { blob, part: 3 })]
    );
}

#[test]
fn open_original_is_offered_for_downloaded_bodies() {
    let mut state = synced_state();
    let (inbox, threads) = inbox_of(&state);
    let message = state.threads[&threads[0]].messages[0];
    let blob = "ab".repeat(32);
    state.messages.get_mut(&message).unwrap().body = BodyState::Fetched { blob: blob.clone() };
    let mut h = gui(state);
    apply(&mut h, Command::OpenMailbox { id: inbox });
    apply(&mut h, Command::OpenThread { id: threads[0] });
    h.get(By::new().role(Role::Button).label("Open original ↗"))
        .click();
    h.run();
    assert_eq!(
        h.state_mut().shell.take_external(),
        [External::OpenOriginal { blob }]
    );
}

#[test]
fn opening_a_missing_original_is_reported() {
    let rt = runtime();
    let (_backend, session) = rt.block_on(session());
    let (bridge, _task) = Bridge::start(rt.handle(), session, egui::Context::default());
    bridge.open_original("0".repeat(64));
    let notices = wait_for_notice(&bridge);
    assert_eq!(notices, ["The message is not in the local store any more."]);
}
