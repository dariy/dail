//! Mailbox labels and the folder tree (UX-2).

use app_core::{
    AccountId, AppState, FocusTarget, FolderKey, Mailbox, MailboxId, MailboxRole, listing,
};

/// A mailbox's label: its role's name, else the last path segment.
pub fn mailbox_name(mailbox: &Mailbox) -> String {
    if let Some(role) = mailbox.role {
        return role_title(role).to_owned();
    }
    mailbox
        .path
        .rsplit(mailbox.separator())
        .next()
        .unwrap_or(&mailbox.path)
        .to_owned()
}

/// The display name of a role.
pub const fn role_title(role: MailboxRole) -> &'static str {
    match role {
        MailboxRole::Inbox => "Inbox",
        MailboxRole::Sent => "Sent",
        MailboxRole::Drafts => "Drafts",
        MailboxRole::Trash => "Trash",
        MailboxRole::Archive => "Archive",
        MailboxRole::Junk => "Junk",
        MailboxRole::All => "All Mail",
        MailboxRole::Flagged => "Starred",
    }
}

/// One row of an account's folder tree (UX-2).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FolderNode {
    /// The folder.
    pub key: FolderKey,
    /// What the row shows: the role's name, else the last path segment.
    pub label: String,
    /// Indentation level, `0` at the top.
    pub depth: usize,
    /// The cached mailbox; `None` for a parent the server does not let us open (`\Noselect`)
    /// or never listed. Such a node can be expanded but not opened.
    pub mailbox: Option<MailboxId>,
    /// The mailbox's role, if any.
    pub role: Option<MailboxRole>,
    /// The folders below, sorted.
    pub children: Vec<FolderNode>,
    /// Unread messages in this folder.
    pub unread: u32,
    /// Unread messages in every folder below (shown faintly when collapsed).
    pub unread_below: u32,
}

impl FolderNode {
    fn new(key: FolderKey, label: String, depth: usize) -> Self {
        Self {
            key,
            label,
            depth,
            mailbox: None,
            role: None,
            children: Vec::new(),
            unread: 0,
            unread_below: 0,
        }
    }

    /// Whether the node has folders below it.
    pub fn is_parent(&self) -> bool {
        !self.children.is_empty()
    }

    fn finish(&mut self, depth: usize) {
        self.depth = depth;
        self.children.sort_by_cached_key(|c| {
            (
                c.role.map_or(usize::MAX, MailboxRole::rank),
                c.label.to_lowercase(),
                c.label.clone(),
            )
        });
        for child in &mut self.children {
            child.finish(depth + 1);
        }
        self.unread_below = self
            .children
            .iter()
            .map(|c| c.unread.saturating_add(c.unread_below))
            .fold(0, u32::saturating_add);
    }
}

/// An account's folders as a tree: the role folders first, flat, in role order (as
/// `listing::mailboxes`); then the other folders nested by their paths, sorted
/// case-insensitively. Missing parents become nodes without a mailbox. The children of the
/// inbox move to the top level (many servers put every folder below `INBOX`, as a namespace);
/// the children of the other role folders stay below them.
pub fn tree(state: &AppState, account: AccountId) -> Vec<FolderNode> {
    let mut roots: Vec<FolderNode> = Vec::new();
    let mut users: Vec<FolderNode> = Vec::new();
    let mailboxes = listing::mailboxes(state, Some(account));
    // The role folders by path, so that their children can hang below them.
    let inbox: Option<&str> = mailboxes
        .iter()
        .find(|m| m.role == Some(MailboxRole::Inbox))
        .map(|m| m.path.as_str());
    for mailbox in &mailboxes {
        let mut node = FolderNode::new(mailbox.key(), mailbox_name(mailbox), 0);
        node.mailbox = Some(mailbox.id);
        node.role = mailbox.role;
        node.unread = mailbox.unread;
        if mailbox.role.is_some() {
            roots.push(node);
            continue;
        }
        // The segments from the top, without the inbox's namespace prefix.
        let sep = mailbox.separator();
        let mut prefix = String::new();
        let mut parents: Vec<(String, String)> = Vec::new();
        let segments: Vec<&str> = mailbox.path.split(sep).collect();
        for segment in &segments[..segments.len().saturating_sub(1)] {
            if !prefix.is_empty() {
                prefix.push_str(sep);
            }
            prefix.push_str(segment);
            if segment.is_empty() || Some(prefix.as_str()) == inbox && parents.is_empty() {
                continue;
            }
            parents.push((prefix.clone(), (*segment).to_owned()));
        }
        // A role folder above it takes it in; else it goes into the user tree.
        let role_parent = parents
            .iter()
            .rposition(|(path, _)| roots.iter().any(|r| r.key.path == *path));
        let (list, parents) = match role_parent {
            Some(at) => {
                let path = parents[at].0.clone();
                let Some(root) = roots.iter_mut().find(|r| r.key.path == path) else {
                    continue;
                };
                (&mut root.children, parents[at + 1..].to_vec())
            }
            None => (&mut users, parents),
        };
        insert(list, account, &parents, node);
    }
    // Real folders listed before their own parents were inserted as synthetic nodes: merge.
    for node in &mut roots {
        node.finish(0);
    }
    for node in &mut users {
        node.finish(0);
    }
    users.sort_by_cached_key(|c| (c.label.to_lowercase(), c.label.clone()));
    roots.extend(users);
    roots
}

/// Puts `node` below the chain of `parents` (path, label) in `list`, creating missing parents.
fn insert(
    list: &mut Vec<FolderNode>,
    account: AccountId,
    parents: &[(String, String)],
    node: FolderNode,
) {
    let Some(((path, label), rest)) = parents.split_first() else {
        match list.iter_mut().find(|n| n.key == node.key) {
            // A synthetic node made for its children: it is a real mailbox after all.
            Some(existing) => {
                existing.mailbox = node.mailbox;
                existing.role = node.role;
                existing.unread = node.unread;
                existing.label = node.label;
            }
            None => list.push(node),
        }
        return;
    };
    let at = match list.iter().position(|n| n.key.path == *path) {
        Some(at) => at,
        None => {
            let key = FolderKey {
                account,
                path: path.clone(),
            };
            list.push(FolderNode::new(key, label.clone(), 0));
            list.len() - 1
        }
    };
    insert(&mut list[at].children, account, rest, node);
}

/// The rows of `nodes` in display order (depth first), skipping the children of the nodes
/// for which `collapsed` is true.
pub fn rows<'a>(
    nodes: &'a [FolderNode],
    collapsed: &dyn Fn(&FolderKey) -> bool,
) -> Vec<&'a FolderNode> {
    let mut out = Vec::new();
    fn walk<'a>(
        nodes: &'a [FolderNode],
        collapsed: &dyn Fn(&FolderKey) -> bool,
        out: &mut Vec<&'a FolderNode>,
    ) {
        for node in nodes {
            out.push(node);
            if !collapsed(&node.key) {
                walk(&node.children, collapsed, out);
            }
        }
    }
    walk(nodes, collapsed, &mut out);
    out
}

/// The sidebar rows of `account` as `view.collapsed` leaves them: none if the account is
/// collapsed, else the tree without the children of collapsed folders.
pub fn visible_rows<'a>(
    state: &AppState,
    account: AccountId,
    nodes: &'a [FolderNode],
) -> Vec<&'a FolderNode> {
    let collapsed = &state.view.collapsed;
    if collapsed.contains(&FolderKey {
        account,
        path: String::new(),
    }) {
        return Vec::new();
    }
    rows(nodes, &|key| collapsed.contains(key))
}

/// Whether the account's folders are collapsed (`collapse_folders` with an empty path).
pub fn account_collapsed(state: &AppState, account: AccountId) -> bool {
    state.view.collapsed.contains(&FolderKey {
        account,
        path: String::new(),
    })
}

/// Whether the sidebar shows `account` (focus mode hides the others, UX-4).
pub fn shows_account(state: &AppState, account: AccountId) -> bool {
    match state.view.focused {
        None => true,
        Some(FocusTarget::Account(id)) => id == account,
        Some(FocusTarget::Mailbox(id)) => state
            .mailboxes
            .get(&id)
            .is_some_and(|m| m.account == account),
    }
}

/// The folder tree the sidebar shows for `account`: [`tree`], or in focus mode on a mailbox,
/// only that mailbox with the folders below it (at the top level).
pub fn sidebar_tree(state: &AppState, account: AccountId) -> Vec<FolderNode> {
    if !shows_account(state, account) {
        return Vec::new();
    }
    let nodes = tree(state, account);
    let Some(FocusTarget::Mailbox(id)) = state.view.focused else {
        return nodes;
    };
    let found = rows(&nodes, &|_| false)
        .into_iter()
        .find(|n| n.mailbox == Some(id))
        .cloned();
    found.map_or_else(Vec::new, |mut node| {
        node.finish(0);
        vec![node]
    })
}
