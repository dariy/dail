//! The sidebar (UI-5): the brand row, *Compose*, the mailboxes of each account, *Add account* and
//! *Settings* at the bottom; collapsed, a rail of icons. It draws with the chrome group.

use app_core::{
    AccountId, AppState, Command, CommandName, FolderKey, Mailbox, MailboxRole, Pane, listing,
};
use app_front::mailbox::FolderNode;
use app_front::mailbox::mailbox_name;
use app_theme::Icon;
use egui::{Align, Layout, Sense, Ui, Vec2};

use crate::bridge::Request;
use crate::theme::Tokens;
use crate::ui;

/// What the sidebar asked for that is not a command.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct SidebarActions {
    /// Open the account wizard.
    pub add_account: bool,
    /// Open the settings page.
    pub settings: bool,
}

/// The account the sidebar's shortcuts act on: the open mailbox's, else the first.
pub fn current_account(state: &AppState) -> Option<AccountId> {
    state
        .view
        .mailbox
        .and_then(|m| state.mailboxes.get(&m))
        .map(|m| m.account)
        .or_else(|| state.accounts.keys().next().copied())
}

/// `create_draft` for the current account.
fn compose(state: &AppState) -> Option<Command> {
    current_account(state).map(|account| Command::CreateDraft {
        account,
        to: Vec::new(),
        cc: Vec::new(),
        bcc: Vec::new(),
        subject: String::new(),
        body: String::new(),
    })
}

/// The `g` key of a role mailbox, when the keymap has it (`open_mailbox`'s keys).
fn role_keys(state: &AppState, role: Option<MailboxRole>) -> Option<String> {
    let keys = match role? {
        MailboxRole::Inbox => "g i",
        MailboxRole::Sent => "g s",
        MailboxRole::Drafts => "g d",
        MailboxRole::Archive => "g a",
        MailboxRole::Trash => "g t",
        MailboxRole::Junk | MailboxRole::All | MailboxRole::Flagged => return None,
    };
    app_front::keymap::keys_for(&state.settings, CommandName::OpenMailbox)
        .into_iter()
        .find(|k| k == keys)
}

/// Renders the expanded sidebar.
pub fn show(ui: &mut Ui, t: &Tokens, state: &AppState, queue: &mut Vec<Request>) -> SidebarActions {
    let mut actions = SidebarActions::default();
    // Brand row, as tall as the top bar.
    ui.allocate_ui_with_layout(
        Vec2::new(ui.available_width(), t.m.frame.top_bar),
        Layout::left_to_right(Align::Center),
        |ui| {
            ui.spacing_mut().item_spacing.x = t.m.space.sm;
            let tile = t.m.controls.icon_button;
            let (rect, _) = ui.allocate_exact_size(Vec2::splat(tile), Sense::hover());
            ui.painter()
                .rect_filled(rect, ui::radius(t.m.radius.md), t.inverse_bg);
            ui::paint_icon(
                ui,
                rect.center(),
                Icon::Mail,
                t.m.controls.icon_md,
                t.inverse_text,
                false,
            );
            ui.label(ui::text(&t.type_scale.headline_md, "dail", t.text_strong));
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
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
        },
    );
    ui.add_space(t.m.space.md);
    ui.add_enabled_ui(!state.accounts.is_empty(), |ui| {
        let width = ui.available_width();
        ui.scope(|ui| {
            // Full width: the padding centres the label.
            ui.spacing_mut().button_padding.x =
                (width / 2.0 - t.m.space.xl * 2.0).max(t.m.space.sm);
            if ui::primary(ui, t, "Compose", Some("C"), true)
                && let Some(cmd) = compose(state)
            {
                queue.push(Request::user(cmd));
            }
        });
    });
    ui.add_space(t.m.space.md);
    // Settings is pinned at the bottom; the mailboxes scroll above it.
    egui::Panel::bottom("sidebar-settings")
        .exact_size(t.m.rows.nav + 2.0 * t.m.space.sm + t.m.space.xs + t.m.stroke.hairline)
        .resizable(false)
        .frame(egui::Frame::new().inner_margin(egui::Margin {
            top: t.m.space.sm as i8,
            bottom: t.m.space.sm as i8,
            ..egui::Margin::ZERO
        }))
        .show_separator_line(false)
        .show(ui, |ui| {
            ui::divider(ui, t);
            ui.add_space(t.m.space.xs);
            let keys =
                app_front::keymap::KeyState::hint(&state.settings, CommandName::OpenSettings);
            let nav = ui::Nav {
                icon: Icon::Settings,
                label: "Settings",
                trailing: keys.map_or(ui::Trailing::None, ui::Trailing::Keys),
                open: state.view.settings.is_some(),
                focused: false,
                name: "Settings",
            };
            if ui::nav_row(ui, t, &nav).clicked() {
                actions.settings = true;
            }
        });
    egui::ScrollArea::vertical()
        .id_salt("sidebar")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            let focused = state.view.focus == Pane::Sidebar;
            if state.view.focused.is_some() {
                // Focus mode (UX-4): the way back to every folder.
                let response = ui::ghost(
                    ui,
                    t,
                    ui::icon_text(
                        Icon::ChevronLeft,
                        "Show all folders",
                        ui::font(&t.type_scale.body_md),
                        t.text_secondary,
                    ),
                );
                response.widget_info(|| {
                    egui::WidgetInfo::labeled(egui::Role::Button, true, "Show all folders")
                });
                if response.clicked() {
                    queue.push(Request::user(Command::ClearFocus {}));
                }
                ui.add_space(t.m.space.sm);
            } else if state.accounts.len() > 1 {
                let unread: u32 = state
                    .mailboxes
                    .values()
                    .filter(|m| m.role == Some(MailboxRole::Inbox))
                    .map(|m| m.unread)
                    .sum();
                let row = Row {
                    icon: Icon::AllInbox,
                    label: "All inboxes",
                    unread,
                    keys: None,
                    open: state.view.unified && state.view.search.is_none(),
                };
                if row.show(ui, t, focused).clicked() {
                    queue.push(Request::user(Command::OpenUnifiedInbox {}));
                }
                ui.add_space(t.m.space.md);
            }
            let current = current_account(state);
            for account in state.accounts.values() {
                let account_key = FolderKey {
                    account: account.id,
                    path: String::new(),
                };
                let account_folded = state.view.collapsed.contains(&account_key);
                ui.horizontal(|ui| {
                    if state.accounts.len() > 1 {
                        let tag = t.tag(state, account.id);
                        let (rect, _) = ui.allocate_exact_size(
                            Vec2::new(t.m.space.xs, t.m.space.md - 2.0),
                            Sense::hover(),
                        );
                        ui.painter()
                            .rect_filled(rect, ui::radius(t.m.radius.sm), tag);
                    }
                    let caption = ui::caption(ui, t, &account.display_name);
                    let name = format!("{} folders", account.display_name);
                    if chevron(ui, t, !account_folded, &name, caption.rect.height()).clicked() {
                        queue.push(Request::user(fold(account_key.clone(), !account_folded)));
                    }
                });
                ui.add_space(t.m.space.xs / 2.0);
                let nodes = app_front::mailbox::sidebar_tree(state, account.id);
                if account_folded {
                    // The open mailbox is never hidden: the account stands in for it.
                    let open = nodes_open(state, &nodes);
                    if let Some(node) = open {
                        let row = Row {
                            icon: crate::theme::role_icon(node.role),
                            label: &node.label,
                            unread: node.unread,
                            keys: None,
                            open: true,
                        };
                        row.show(ui, t, focused);
                    }
                }
                let gutter = nodes
                    .iter()
                    .any(|n| n.is_parent() || n.children.iter().any(FolderNode::is_parent))
                    .then_some(t.m.controls.icon_sm + t.m.space.xs);
                for node in app_front::mailbox::visible_rows(state, account.id, &nodes) {
                    let indent = t.m.space.md * node.depth as f32;
                    let folded = state.view.collapsed.contains(&node.key);
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 0.0;
                        ui.add_space(indent);
                        if let Some(gutter) = gutter {
                            if node.is_parent() {
                                let name = format!("{} subfolders", node.label);
                                if chevron(ui, t, !folded, &name, t.m.rows.nav).clicked() {
                                    queue.push(Request::user(fold(node.key.clone(), !folded)));
                                }
                            } else {
                                ui.add_space(gutter);
                            }
                        }
                        let hidden = if folded { node.unread_below } else { 0 };
                        let mailbox = node.mailbox.and_then(|id| state.mailboxes.get(&id));
                        match mailbox {
                            Some(mailbox) => {
                                let keys = (Some(account.id) == current)
                                    .then(|| role_keys(state, mailbox.role))
                                    .flatten();
                                if mailbox_row(ui, t, state, mailbox, node, keys, focused) {
                                    queue.push(Request::user(Command::OpenMailbox {
                                        id: mailbox.id,
                                    }));
                                }
                            }
                            // A parent folder that can't be opened: a click folds it.
                            None => {
                                let row = Row {
                                    icon: Icon::Folder,
                                    label: &node.label,
                                    unread: 0,
                                    keys: None,
                                    open: false,
                                };
                                if row.show_with(ui, t, focused, hidden).clicked() {
                                    queue.push(Request::user(fold(node.key.clone(), !folded)));
                                }
                            }
                        }
                    });
                }
                ui.add_space(t.m.space.md);
            }
            if state.accounts.is_empty() {
                ui.label(ui::text(
                    &t.type_scale.body_md,
                    "No accounts yet.",
                    t.text_secondary,
                ));
            }
            ui.add_space(t.m.space.xs);
            let job = ui::icon_text(
                Icon::Add,
                "Add account",
                ui::font(&t.type_scale.body_md),
                t.text_secondary,
            );
            let response = ui::ghost(ui, t, job);
            response.widget_info(|| {
                egui::WidgetInfo::labeled(egui::Role::Button, true, "+ Add account")
            });
            if response.clicked() {
                actions.add_account = true;
            }
        });
    actions
}

/// `collapse_folders` or `expand_folders` for one key.
fn fold(key: FolderKey, collapse: bool) -> Command {
    if collapse {
        Command::CollapseFolders { keys: vec![key] }
    } else {
        Command::ExpandFolders { keys: vec![key] }
    }
}

/// The node of the open mailbox, anywhere in the tree.
fn nodes_open<'a>(state: &AppState, nodes: &'a [FolderNode]) -> Option<&'a FolderNode> {
    let open = state.view.mailbox?;
    if state.view.search.is_some() || state.view.unified {
        return None;
    }
    app_front::mailbox::rows(nodes, &|_| false)
        .into_iter()
        .find(|n| n.mailbox == Some(open))
}

/// The fold chevron of a parent row or an account caption: `ExpandMore` while expanded,
/// `ChevronRight` while collapsed. Its accessible name says what a click does.
fn chevron(ui: &mut Ui, t: &Tokens, expanded: bool, what: &str, height: f32) -> egui::Response {
    let side = t.m.controls.icon_sm + t.m.space.xs;
    let (rect, response) = ui.allocate_exact_size(Vec2::new(side, height), Sense::click());
    let label = if expanded {
        format!("Collapse {what}")
    } else {
        format!("Expand {what}")
    };
    response.widget_info(|| {
        egui::WidgetInfo::selected(egui::Role::Button, true, expanded, label.as_str())
    });
    if response.hovered() {
        ui.painter()
            .rect_filled(rect, ui::radius(t.m.radius.sm), t.hover);
    }
    let icon = if expanded {
        Icon::ExpandMore
    } else {
        Icon::ChevronRight
    };
    ui::paint_icon(
        ui,
        rect.center(),
        icon,
        t.m.controls.icon_sm,
        t.text_muted,
        false,
    );
    response.on_hover_text(label)
}

/// One nav row: the name, then the unread count or else the key hint.
struct Row<'a> {
    icon: Icon,
    label: &'a str,
    unread: u32,
    keys: Option<String>,
    open: bool,
}

impl Row<'_> {
    /// Screen readers hear the name, the unread count and whether it is the open one.
    fn show(self, ui: &mut Ui, t: &Tokens, focused: bool) -> egui::Response {
        self.show_with(ui, t, focused, 0)
    }

    /// `hidden`: the unread mail of the folders a collapsed row hides.
    fn show_with(self, ui: &mut Ui, t: &Tokens, focused: bool, hidden: u32) -> egui::Response {
        let name = app_front::a11y::nav_row(self.label, self.unread);
        let trailing = match (self.unread, self.keys) {
            (n, _) if n > 0 => ui::Trailing::Count(n),
            _ if hidden > 0 => ui::Trailing::Faint(hidden),
            (_, Some(keys)) => ui::Trailing::Keys(keys),
            _ => ui::Trailing::None,
        };
        ui::nav_row(
            ui,
            t,
            &ui::Nav {
                icon: self.icon,
                label: self.label,
                trailing,
                open: self.open,
                focused,
                name: &name,
            },
        )
    }
}

fn mailbox_row(
    ui: &mut Ui,
    t: &Tokens,
    state: &AppState,
    mailbox: &Mailbox,
    node: &FolderNode,
    keys: Option<String>,
    focused: bool,
) -> bool {
    let hidden = if state.view.collapsed.contains(&node.key) {
        node.unread_below
    } else {
        0
    };
    let row = Row {
        icon: crate::theme::role_icon(mailbox.role),
        label: &node.label,
        unread: mailbox.unread,
        keys,
        open: state.view.mailbox == Some(mailbox.id)
            && state.view.search.is_none()
            && !state.view.unified,
    };
    let response = row.show_with(ui, t, focused, hidden);
    let backfilling = state.status.backfilling.contains(&mailbox.id);
    if backfilling && let Some(history) = listing::history(state, mailbox.id) {
        // Older mail is being fetched in the background: how far it got.
        let percent = u64::from(history.cached) * 100 / u64::from(history.wanted.max(1));
        let mut child = ui.new_child(
            egui::UiBuilder::new()
                .max_rect(response.rect.shrink2(Vec2::new(t.m.space.sm + 2.0, 0.0)))
                .layout(Layout::right_to_left(Align::Center)),
        );
        if mailbox.unread > 0 {
            child.add_space(t.m.space.xl + t.m.space.xs);
        }
        child
            .label(ui::text(
                &t.type_scale.label_sm,
                format!("↓{percent}%"),
                t.text_muted,
            ))
            .on_hover_text(format!(
                "Fetching older mail: {} of {} cached",
                history.cached, history.wanted
            ));
    }
    response.clicked()
}

/// Renders the collapsed rail: expand, compose, the role mailboxes of the current account and
/// settings, as icon buttons named by their tooltips.
pub fn rail(ui: &mut Ui, t: &Tokens, state: &AppState, queue: &mut Vec<Request>) -> SidebarActions {
    let mut actions = SidebarActions::default();
    ui.vertical_centered(|ui| {
        ui.add_space((t.m.frame.top_bar - t.m.controls.icon_button) / 2.0);
        ui.spacing_mut().item_spacing.y = t.m.space.xs;
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
        ui.add_space(t.m.space.sm);
        ui.add_enabled_ui(!state.accounts.is_empty(), |ui| {
            if ui::icon_button(ui, t, t.text_strong, Icon::Edit, "Compose (C)").clicked()
                && let Some(cmd) = compose(state)
            {
                queue.push(Request::user(cmd));
            }
        });
        ui.add_space(t.m.space.sm);
        if let Some(account) = current_account(state) {
            for mailbox in listing::mailboxes(state, Some(account)) {
                if mailbox.role.is_none() {
                    continue;
                }
                let open = state.view.mailbox == Some(mailbox.id) && state.view.search.is_none();
                let color = if open {
                    t.text_strong
                } else {
                    t.text_secondary
                };
                let icon = crate::theme::role_icon(mailbox.role);
                if ui::icon_button(ui, t, color, icon, &mailbox_name(mailbox)).clicked() {
                    queue.push(Request::user(Command::OpenMailbox { id: mailbox.id }));
                }
            }
        }
        ui.add_space(t.m.space.sm);
        if ui::icon_button(ui, t, t.text_secondary, Icon::Settings, "Settings").clicked() {
            actions.settings = true;
        }
    });
    actions
}
