//! The sidebar: accounts and their mailboxes, compose and account buttons.

use app_core::{AppState, Command, Mailbox, Pane, listing};
use app_front::mailbox::mailbox_name;
use app_theme::Icon;
use egui::{RichText, Sense, Ui, Vec2};

use crate::bridge::Request;
use crate::theme::Tokens;
use crate::ui;

/// What the sidebar asked for that is not a command.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct SidebarActions {
    /// Open the account wizard.
    pub add_account: bool,
}

/// Renders the expanded sidebar.
pub fn show(ui: &mut Ui, t: &Tokens, state: &AppState, queue: &mut Vec<Request>) -> SidebarActions {
    let mut actions = SidebarActions::default();
    ui.add_space(10.0);
    ui.horizontal(|ui| {
        ui::icon(ui, Icon::Mail, 20.0, t.text_strong, false);
        ui.label(
            RichText::new("dail")
                .size(17.0)
                .strong()
                .family(crate::theme::semibold())
                .color(t.text_strong),
        );
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui::icon_button(
                ui,
                t,
                t.text_secondary,
                Icon::DockToLeft,
                "Collapse sidebar (Ctrl+B)",
            )
            .clicked()
            {
                queue.push(Request::user(Command::ToggleSidebar {}));
            }
        });
    });
    ui.add_space(10.0);
    let first_account = state.accounts.keys().next().copied();
    ui.add_enabled_ui(first_account.is_some(), |ui| {
        ui.horizontal(|ui| {
            if ui::primary(ui, t, "Compose", Some("C"), true)
                && let Some(account) = first_account
            {
                queue.push(Request::user(Command::CreateDraft {
                    account: state
                        .view
                        .mailbox
                        .and_then(|m| state.mailboxes.get(&m))
                        .map_or(account, |m| m.account),
                    to: Vec::new(),
                    cc: Vec::new(),
                    bcc: Vec::new(),
                    subject: String::new(),
                    body: String::new(),
                }));
            }
        });
    });
    ui.add_space(12.0);
    egui::ScrollArea::vertical()
        .id_salt("sidebar")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            let focused = state.view.focus == Pane::Sidebar;
            if state.accounts.len() > 1 {
                let unread = state
                    .mailboxes
                    .values()
                    .filter(|m| m.role == Some(app_core::MailboxRole::Inbox))
                    .map(|m| m.unread)
                    .sum();
                let open = state.view.unified && state.view.search.is_none();
                let row = NavRow {
                    icon: Icon::AllInbox,
                    name: "All inboxes".to_owned(),
                    unread,
                    open,
                };
                if nav_row(ui, t, &row, focused).clicked() {
                    queue.push(Request::user(Command::OpenUnifiedInbox {}));
                }
                ui.add_space(12.0);
            }
            for account in state.accounts.values() {
                ui.horizontal(|ui| {
                    if state.accounts.len() > 1 {
                        let tag = t.tag(state, account.id);
                        let (rect, _) =
                            ui.allocate_exact_size(Vec2::new(4.0, 10.0), Sense::hover());
                        ui.painter().rect_filled(rect, 1.0, tag);
                    }
                    ui::caption(ui, t, &account.display_name);
                });
                ui.add_space(2.0);
                for mailbox in listing::mailboxes(state, Some(account.id)) {
                    if mailbox_row(ui, t, state, mailbox, focused) {
                        queue.push(Request::user(Command::OpenMailbox { id: mailbox.id }));
                    }
                }
                ui.add_space(12.0);
            }
            if state.accounts.is_empty() {
                ui.label(RichText::new("No accounts yet.").color(t.text_secondary));
            }
            ui.add_space(4.0);
            if ui
                .add(
                    egui::Button::new(RichText::new("+ Add account").color(t.text_secondary))
                        .frame(false),
                )
                .clicked()
            {
                actions.add_account = true;
            }
        });
    actions
}

/// One row of the sidebar.
struct NavRow {
    icon: Icon,
    name: String,
    unread: u32,
    open: bool,
}

fn mailbox_row(
    ui: &mut Ui,
    t: &Tokens,
    state: &AppState,
    mailbox: &Mailbox,
    focused: bool,
) -> bool {
    let row = NavRow {
        icon: crate::theme::role_icon(mailbox.role),
        name: mailbox_name(mailbox),
        unread: mailbox.unread,
        open: state.view.mailbox == Some(mailbox.id)
            && state.view.search.is_none()
            && !state.view.unified,
    };
    let response = nav_row(ui, t, &row, focused);
    let backfilling = state.status.backfilling.contains(&mailbox.id);
    if backfilling && let Some(history) = listing::history(state, mailbox.id) {
        // Older mail is being fetched in the background: how far it got.
        let percent = u64::from(history.cached) * 100 / u64::from(history.wanted.max(1));
        let mut child = ui.new_child(
            egui::UiBuilder::new()
                .max_rect(response.rect.shrink2(Vec2::new(10.0, 0.0)))
                .layout(egui::Layout::right_to_left(egui::Align::Center)),
        );
        if mailbox.unread > 0 {
            child.add_space(30.0);
        }
        child
            .label(
                RichText::new(format!("↓{percent}%"))
                    .font(ui::mono(10.0))
                    .color(t.text_muted),
            )
            .on_hover_text(format!(
                "Fetching older mail: {} of {} cached",
                history.cached, history.wanted
            ));
    }
    response.clicked()
}

fn nav_row(ui: &mut Ui, t: &Tokens, row: &NavRow, focused: bool) -> egui::Response {
    let open = row.open;
    let (rect, response) =
        ui.allocate_exact_size(Vec2::new(ui.available_width(), 30.0), Sense::click());
    // Screen readers: the name, the unread count and whether it is the open one.
    let name = if row.unread > 0 {
        format!("{}, {} unread", row.name, row.unread)
    } else {
        row.name.clone()
    };
    response.widget_info(|| egui::WidgetInfo::selected(egui::Role::Button, true, open, &name));
    if open {
        ui.painter().rect_filled(rect, 4.0, t.selected);
        if focused {
            ui.painter().rect_filled(
                egui::Rect::from_min_size(rect.min, Vec2::new(2.0, rect.height())),
                0.0,
                t.focus,
            );
        }
    } else if response.hovered() {
        ui.painter().rect_filled(rect, 4.0, t.hover);
    }
    let mut child = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(rect.shrink2(Vec2::new(10.0, 0.0)))
            .layout(egui::Layout::left_to_right(egui::Align::Center))
            .accessibility_parent(response.id),
    );
    ui::icon(
        &mut child,
        row.icon,
        16.0,
        if open {
            t.text_strong
        } else {
            t.text_secondary
        },
        open,
    );
    let name = RichText::new(&row.name).color(if open {
        t.text_strong
    } else {
        t.text_secondary
    });
    child.add(
        egui::Label::new(if open {
            name.strong().family(crate::theme::semibold())
        } else {
            name
        })
        .truncate()
        .selectable(false),
    );
    if row.unread > 0 {
        child.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui::count(ui, t, row.unread, open);
        });
    }
    response
}

/// Renders the collapsed rail.
pub fn rail(ui: &mut Ui, t: &Tokens, queue: &mut Vec<Request>) {
    ui.add_space(10.0);
    ui.vertical_centered(|ui| {
        if ui::icon_button(
            ui,
            t,
            t.text_secondary,
            Icon::DockToLeft,
            "Expand sidebar (Ctrl+B)",
        )
        .clicked()
        {
            queue.push(Request::user(Command::ToggleSidebar {}));
        }
    });
}
