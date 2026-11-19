//! Sample threads for the Appearance preview (MISC-25): a few rows that follow the density and
//! text size settings as they change, without real mail. They live in an [`AppState`] of their
//! own, built through the engine from fixed envelopes, so front ends draw them with their real
//! list rows (`ThreadSummary::of`) and nothing of the user's mail shows in the settings. Their
//! texts are in the UI language.

use app_core::{
    Addr, AppState, Command, Ctx, Engine, Event, Flags, Origin, ProviderPreset, RemoteEnvelope,
    RemoteMailbox, ThreadId,
};

use crate::i18n::{Key, s};

/// The sample state: one account, its INBOX with three threads (an unread one with an
/// attachment, a starred one, a conversation of two), dated before `now_ms`, with `base`'s
/// settings.
pub fn state(base: &AppState, now_ms: i64) -> AppState {
    let mut engine = Engine::default();
    let ctx = |n: u128| Ctx {
        now_ms,
        id_seed: n << 64,
        origin: Origin::Cli,
    };
    let added = engine.dispatch(
        Command::AddAccount {
            preset: ProviderPreset::Hostinger,
            email: "you@example.com".into(),
            display_name: None,
            password: None,
            imap: None,
            smtp: None,
        },
        &ctx(1),
    );
    let account = engine.state().accounts.keys().next().copied();
    let (Ok(_), Some(account)) = (added, account) else {
        return AppState::default();
    };
    let listed = engine.ingest(
        Event::MailboxesListed {
            account,
            mailboxes: vec![RemoteMailbox {
                path: "INBOX".into(),
                delimiter: Some("/".into()),
                special_use: None,
                selectable: true,
            }],
        },
        &ctx(2),
    );
    let mailbox = engine.state().mailboxes.keys().next().copied();
    let (Ok(_), Some(mailbox)) = (listed, mailbox) else {
        return AppState::default();
    };
    let minute = 60_000;
    let envelopes = vec![
        envelope(
            1,
            "Ana Lima",
            Key::SampleSubject1,
            now_ms - 12 * minute,
            None,
        ),
        envelope(
            2,
            "Northwind Travel",
            Key::SampleSubject2,
            now_ms - 95 * minute,
            None,
        ),
        envelope(
            3,
            "Sam Okafor",
            Key::SampleSubject3,
            now_ms - 26 * 60 * minute,
            None,
        ),
        envelope(
            4,
            "Ana Lima",
            Key::SampleSubject3,
            now_ms - 25 * 60 * minute,
            Some(3),
        ),
    ];
    let fetched = engine.ingest(
        Event::EnvelopesFetched {
            account,
            mailbox,
            uid_validity: 1,
            uid_next: None,
            total: 4,
            unread: 1,
            envelopes,
        },
        &ctx(3),
    );
    if fetched.is_err() {
        return AppState::default();
    }
    let mut state = engine.state().clone();
    for message in state.messages.values_mut() {
        let snippet = match message.uid {
            1 => Key::SampleSnippet1,
            2 => Key::SampleSnippet2,
            _ => Key::SampleSnippet3,
        };
        message.snippet = Some(s(snippet).to_owned());
        match message.uid {
            1 => {
                message.flags.seen = false;
                message.has_attachments = true;
            }
            2 => message.flags.flagged = true,
            _ => {}
        }
    }
    state.settings = base.settings.clone();
    state
}

/// The sample threads, newest first: the rows to draw.
pub fn threads(state: &AppState) -> Vec<ThreadId> {
    let mut threads: Vec<_> = state.threads.values().collect();
    threads.sort_by_key(|t| std::cmp::Reverse(t.last_date_ms));
    threads.into_iter().map(|t| t.id).collect()
}

fn envelope(
    uid: u32,
    name: &str,
    subject: Key,
    date_ms: i64,
    reply_to: Option<u32>,
) -> RemoteEnvelope {
    let id = |uid: u32| format!("sample{uid}@example.com");
    RemoteEnvelope {
        uid,
        message_id_hdr: Some(id(uid)),
        in_reply_to: reply_to.map(id),
        references: reply_to.map(id).into_iter().collect(),
        from: vec![Addr {
            name: Some(name.to_owned()),
            email: format!("{}@example.com", name.to_lowercase().replace(' ', ".")),
        }],
        to: Vec::new(),
        cc: Vec::new(),
        subject: s(subject).to_owned(),
        date_ms,
        flags: Flags {
            seen: true,
            ..Flags::default()
        },
        size: 1,
        has_attachments: false,
        list_id: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn three_threads_newest_first_with_their_marks_and_the_users_settings() {
        let mut base = AppState::default();
        base.settings.list_density = 2;
        base.settings.text_scale = 125;
        let now = 1_700_000_000_000;
        let state = state(&base, now);
        assert_eq!(state.settings.list_density, 2);
        assert_eq!(state.settings.text_scale, 125);
        let rows = threads(&state);
        assert_eq!(rows.len(), 3);
        let summaries: Vec<_> = rows
            .iter()
            .map(|id| app_core::ThreadSummary::of(&state, &state.threads[id]))
            .collect();
        assert_eq!(summaries[0].unread, 1);
        assert!(summaries[0].has_attachments);
        assert!(summaries[1].flagged);
        assert_eq!(summaries[2].messages.len(), 2);
        assert!(
            state
                .messages
                .values()
                .all(|m| m.snippet.is_some() && m.date_ms < now)
        );
    }
}
