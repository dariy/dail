//! `search` (Query) over the cached envelopes and the `set_search` / `clear_search` View commands.

// reason: helpers outside `#[test]` fns may panic; clippy.toml only exempts `#[test]` bodies.
#![allow(clippy::unwrap_used, clippy::panic)]

use crate::support::{Mail, T0, mail};
use app_core::{
    Addr, Command, CommandKind, CoreError, Ctx, Effect, EntityRef, Event, MailboxRole, Origin,
    ProviderPreset, RemoteEnvelope,
};
use serde_json::{Value, json};

/// The server searches among `effects` (without the full-text index search).
fn server_only(effects: Vec<Effect>) -> Vec<Effect> {
    effects
        .into_iter()
        .filter(|e| !matches!(e, Effect::SearchIndex { .. }))
        .collect()
}

/// INBOX: a "Plans" thread (UID 1 read, UID 2 unread reply from Alice with an attachment) and
/// "Lunch" (UID 3, starred, to Carol). Archive: "Invoice" (UID 1, cc Dave, a year earlier).
fn mailbox() -> Mail {
    let mut m = Mail::hostinger();
    let alice = Addr {
        name: Some("Alice Example".into()),
        email: "alice@example.com".into(),
    };
    let reply = RemoteEnvelope {
        from: vec![alice],
        has_attachments: true,
        ..mail(2, "b@x", Some("a@x"), "Re: Plans", false)
    };
    let mut lunch = mail(3, "c@x", None, "Lunch", false);
    lunch.flags.flagged = true;
    lunch.to = vec![Addr {
        name: None,
        email: "carol@example.net".into(),
    }];
    m.fetch(
        "INBOX",
        vec![mail(1, "a@x", None, "Plans", true), reply, lunch],
    );
    let mut invoice = mail(1, "d@x", None, "Invoice 2022", true);
    invoice.date_ms = T0 - 365 * 86_400_000;
    invoice.cc = vec![Addr {
        name: Some("Dave".into()),
        email: "dave@example.org".into(),
    }];
    m.fetch("Archive", vec![invoice]);
    m
}

fn search(m: &mut Mail, query: &str) -> Value {
    m.run(Command::Search {
        query: query.into(),
        account: None,
        offset: 0,
        limit: 50,
        server: false,
        order: app_core::SearchOrder::default(),
    })
    .unwrap()
    .receipt
    .data
}

/// The subjects of the threads found, in result order.
fn subjects(m: &mut Mail, query: &str) -> Vec<String> {
    search(m, query)["threads"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| {
            t["untrusted_content"]["subject"]
                .as_str()
                .unwrap()
                .to_owned()
        })
        .collect()
}

#[test]
fn search_returns_matching_threads_with_their_matching_messages() {
    let mut m = mailbox();
    let outcome = m
        .run(Command::Search {
            query: "PLANS".into(),
            account: None,
            offset: 0,
            limit: 50,
            server: false,
            order: app_core::SearchOrder::default(),
        })
        .unwrap();
    assert_eq!(outcome.receipt.kind, Some(CommandKind::Query));
    // Without the servers, only the local full-text index is asked.
    assert!(server_only(outcome.effects.clone()).is_empty());
    assert!(
        matches!(&outcome.effects[..], [Effect::SearchIndex { words, .. }] if words == &["PLANS"])
    );
    let data = outcome.receipt.data;
    assert_eq!(data["query"], "PLANS");
    assert_eq!(data["total"], 1);
    let thread = &data["threads"][0];
    let (one, two) = (m.message("INBOX", 1), m.message("INBOX", 2));
    assert_eq!(thread["id"], json!(m.thread_of(one)));
    assert_eq!(thread["matches"], json!([one, two]));
    assert_eq!(thread["unread"], 1);
    assert!(thread.get("in_mailbox").is_none());
    assert_eq!(thread["untrusted_content"]["subject"], "Plans");
    assert_eq!(m.engine.history().undo_len(), 1, "only add_account");
}

#[test]
fn every_field_filters() {
    let mut m = mailbox();
    assert_eq!(subjects(&mut m, "from:alice"), ["Plans"]);
    assert_eq!(subjects(&mut m, r#"from:"alice example""#), ["Plans"]);
    assert_eq!(subjects(&mut m, "to:carol"), ["Lunch"]);
    assert_eq!(subjects(&mut m, "cc:dave"), ["Invoice 2022"]);
    assert_eq!(subjects(&mut m, "subject:re:"), ["Plans"]);
    assert_eq!(subjects(&mut m, "is:unread"), ["Lunch", "Plans"]);
    assert_eq!(subjects(&mut m, "is:read"), ["Plans", "Invoice 2022"]);
    assert_eq!(subjects(&mut m, "is:starred"), ["Lunch"]);
    assert_eq!(subjects(&mut m, "has:attachment"), ["Plans"]);
    assert_eq!(subjects(&mut m, "in:inbox"), ["Lunch", "Plans"]);
    assert_eq!(subjects(&mut m, "in:ARCHIVE"), ["Invoice 2022"]);
    assert_eq!(subjects(&mut m, "in:nowhere"), Vec::<String>::new());
    // Mail is dated 2023-11-13; the invoice 2022-11-14.
    assert_eq!(subjects(&mut m, "before:2023-11-13"), ["Invoice 2022"]);
    assert_eq!(subjects(&mut m, "after:2023-11-13"), ["Lunch", "Plans"]);
    assert_eq!(subjects(&mut m, "after:2023-11-14"), Vec::<String>::new());
    // Text also matches addresses.
    assert_eq!(subjects(&mut m, "example.net"), ["Lunch"]);
}

#[test]
fn terms_combine_and_negate() {
    let mut m = mailbox();
    assert_eq!(subjects(&mut m, "in:inbox -is:starred"), ["Plans"]);
    assert_eq!(subjects(&mut m, "plans is:read"), ["Plans"]);
    let data = search(&mut m, "plans is:read");
    assert_eq!(
        data["threads"][0]["matches"],
        json!([m.message("INBOX", 1)])
    );
    assert_eq!(subjects(&mut m, "-bob"), ["Plans"]);
    assert_eq!(subjects(&mut m, "plans -plans"), Vec::<String>::new());
}

#[test]
fn search_follows_pending_moves_and_local_flags() {
    let mut m = mailbox();
    let lunch = m.thread_of(m.message("INBOX", 3));
    m.run(Command::ArchiveThreads {
        ids: vec![lunch],
        messages: Vec::new(),
    })
    .unwrap();
    m.run(Command::MarkRead {
        ids: vec![lunch],
        messages: Vec::new(),
    })
    .unwrap();
    assert_eq!(subjects(&mut m, "in:archive"), ["Lunch", "Invoice 2022"]);
    assert_eq!(subjects(&mut m, "is:unread"), ["Plans"]);
}

#[test]
fn search_pages_and_filters_by_account() {
    let mut m = mailbox();
    let data = m
        .run(Command::Search {
            query: "-zzz".into(),
            account: Some(m.account),
            offset: 1,
            limit: 1,
            server: false,
            order: app_core::SearchOrder::default(),
        })
        .unwrap();
    assert!(
        data.receipt
            .affected
            .contains(&EntityRef::Account(m.account))
    );
    let data = data.receipt.data;
    assert_eq!(data["total"], 3);
    assert_eq!(data["threads"].as_array().unwrap().len(), 1);
    assert_eq!(data["threads"][0]["untrusted_content"]["subject"], "Plans");

    // A second account's mail is left out when searching the first one.
    m.run(Command::AddAccount {
        preset: ProviderPreset::Gmail,
        email: "other@gmail.com".into(),
        display_name: None,
        password: None,
        imap: None,
        smtp: None,
    })
    .unwrap();
    let other = *m.engine.state().accounts.keys().last().unwrap();
    let data = m
        .run(Command::Search {
            query: "plans".into(),
            account: Some(other),
            offset: 0,
            limit: 50,
            server: false,
            order: app_core::SearchOrder::default(),
        })
        .unwrap()
        .receipt
        .data;
    assert_eq!(data["total"], 0);
}

#[test]
fn search_rejects_bad_arguments() {
    let mut m = mailbox();
    let run = |m: &mut Mail, query: &str, limit: u32, account| {
        m.run(Command::Search {
            query: query.into(),
            account,
            offset: 0,
            limit,
            server: false,
            order: app_core::SearchOrder::default(),
        })
        .unwrap_err()
    };
    let err = run(&mut m, "is:maybe", 50, None);
    assert!(
        matches!(err, CoreError::InvalidArgument { ref field, ref reason } if field == "query" && reason.contains("column 4")),
        "{err:?}"
    );
    let err = run(&mut m, "   ", 50, None);
    assert!(
        matches!(err, CoreError::InvalidArgument { ref reason, .. } if reason.contains("empty")),
        "{err:?}"
    );
    for limit in [0, 501] {
        let err = run(&mut m, "plans", limit, None);
        assert!(matches!(err, CoreError::InvalidArgument { ref field, .. } if field == "limit"));
    }
    let missing = app_core::AccountId::from_uuid(uuid::Uuid::from_u128(97));
    let err = run(&mut m, "plans", 50, Some(missing));
    assert!(matches!(err, CoreError::NotFound { ref entity, .. } if entity == "account"));
}

#[test]
fn set_search_shows_results_and_clears_the_selection() {
    let mut m = mailbox();
    let plans = m.thread_of(m.message("INBOX", 1));
    m.run(Command::OpenThread { id: plans }).unwrap();
    let outcome = m
        .run(Command::SetSearch {
            query: "  is:unread ".into(),
        })
        .unwrap();
    assert_eq!(outcome.receipt.kind, Some(CommandKind::View));
    assert!(outcome.receipt.affected.contains(&EntityRef::View));
    assert_eq!(outcome.receipt.data["search"], "is:unread");
    assert_eq!(outcome.receipt.data["total"], 2);
    let view = &m.engine.state().view;
    assert_eq!(view.search.as_deref(), Some("is:unread"));
    assert!(view.selected.is_empty());
    assert_eq!(view.thread, None);
    assert_eq!(m.engine.history().undo_len(), 1, "only add_account");
}

#[test]
fn set_search_rejects_bad_queries_without_changing_the_view() {
    let mut m = mailbox();
    m.run(Command::SetSearch {
        query: "plans".into(),
    })
    .unwrap();
    for bad in ["", "after:soon"] {
        let err = m.run(Command::SetSearch { query: bad.into() }).unwrap_err();
        assert!(matches!(err, CoreError::InvalidArgument { ref field, .. } if field == "query"));
    }
    assert_eq!(m.engine.state().view.search.as_deref(), Some("plans"));
}

#[test]
fn clear_search_returns_to_the_mailbox() {
    let mut m = mailbox();
    let inbox = m.mailbox("INBOX");
    m.run(Command::OpenMailbox { id: inbox }).unwrap();
    m.run(Command::SetSearch {
        query: "plans".into(),
    })
    .unwrap();
    let outcome = m.run(Command::ClearSearch {}).unwrap();
    assert_eq!(outcome.receipt.data, json!({ "cleared": "plans" }));
    let view = &m.engine.state().view;
    assert_eq!(view.search, None);
    assert_eq!(view.mailbox, Some(inbox));
    // Clearing again is harmless.
    let outcome = m.run(Command::ClearSearch {}).unwrap();
    assert_eq!(outcome.receipt.data, json!({ "cleared": null }));
}

#[test]
fn availability_follows_state() {
    let mut m = mailbox();
    let spec = |name: &str| app_core::CommandName::from_name(name).unwrap().spec();
    assert!((spec("search").available)(m.engine.state()));
    assert!(!(spec("clear_search").available)(m.engine.state()));
    m.run(Command::SetSearch {
        query: "plans".into(),
    })
    .unwrap();
    assert!((spec("clear_search").available)(m.engine.state()));
    assert!(!(spec("set_search").available)(
        &app_core::AppState::default()
    ));
}

// --- Server fallback ---------------------------------------------------------------------

fn server_search(m: &mut Mail, query: &str) -> app_core::Outcome {
    m.run(Command::Search {
        query: query.into(),
        account: None,
        offset: 0,
        limit: 50,
        server: true,
        order: app_core::SearchOrder::default(),
    })
    .unwrap()
}

/// `(path, search, query, gmail_raw, mailboxes)` of the `search_mailbox` effects.
fn searches(m: &Mail, effects: &[Effect]) -> Vec<(String, String, String, bool, u32)> {
    effects
        .iter()
        .filter_map(|effect| match effect {
            Effect::SearchMailbox {
                mailbox,
                path,
                search,
                query,
                gmail_raw,
                mailboxes,
                limit,
                ..
            } => {
                assert_eq!(*mailbox, m.mailbox(path));
                assert_eq!(*limit, 200);
                Some((
                    path.clone(),
                    search.clone(),
                    query.to_string(),
                    *gmail_raw,
                    *mailboxes,
                ))
            }
            _ => None,
        })
        .collect()
}

/// The `search_results` event answering `effect` with `envelopes`.
fn results(effect: &Effect, uid_validity: u32, envelopes: Vec<RemoteEnvelope>) -> Event {
    let Effect::SearchMailbox {
        account,
        mailbox,
        search,
        started_ms,
        mailboxes,
        ..
    } = effect.clone()
    else {
        panic!("not a search: {effect:?}");
    };
    Event::SearchResults {
        account,
        mailbox,
        uid_validity,
        search,
        started_ms,
        mailboxes,
        envelopes,
    }
}

/// An old Archive message whose subject and addresses do not contain "budget" (only its body
/// does, on the server).
fn old_budget_mail() -> RemoteEnvelope {
    let mut old = mail(40, "old@x", None, "Numbers", true);
    old.date_ms = T0 - 900 * 86_400_000;
    old
}

#[test]
fn search_asks_the_synced_mailboxes_of_each_account_without_changing_state() {
    let mut m = mailbox();
    let before = m.engine.state().clone();
    let outcome = server_search(&mut m, "budget -is:starred");
    // Trash and Sent were never synced; there is no `all` mailbox.
    assert_eq!(
        searches(&m, &outcome.effects),
        [
            (
                "INBOX".into(),
                "budget -is:starred".into(),
                "budget -is:starred".into(),
                false,
                2
            ),
            (
                "Archive".into(),
                "budget -is:starred".into(),
                "budget -is:starred".into(),
                false,
                2
            ),
        ]
    );
    assert_eq!(
        outcome.receipt.data["server"],
        json!([{ "account": m.account, "state": "started" }])
    );
    assert_eq!(*m.engine.state(), before, "a Query records nothing");

    // `in:` picks the mailboxes and is not sent to the server.
    let outcome = server_search(&mut m, "in:archive budget");
    assert_eq!(
        searches(&m, &outcome.effects),
        [(
            "Archive".into(),
            "in:archive budget".into(),
            "budget".into(),
            false,
            1
        )]
    );
    let outcome = server_search(&mut m, "-in:inbox budget");
    assert_eq!(searches(&m, &outcome.effects).len(), 1);
    // Nothing but mailbox terms: the cache already answers it.
    let outcome = server_search(&mut m, "in:inbox");
    assert!(outcome.effects.is_empty());
    assert_eq!(outcome.receipt.data["server"], json!([]));
}

#[test]
fn gmail_searches_all_mail_with_its_own_syntax() {
    let mut m = Mail::new(
        ProviderPreset::Gmail,
        &[
            ("INBOX", None),
            ("[Gmail]/All Mail", Some(MailboxRole::All)),
            ("[Gmail]/Trash", Some(MailboxRole::Trash)),
        ],
    );
    m.fetch("INBOX", vec![mail(1, "a@x", None, "Plans", false)]);
    m.fetch(
        "[Gmail]/All Mail",
        vec![mail(1, "a@x", None, "Plans", false)],
    );
    m.fetch("[Gmail]/Trash", vec![mail(1, "t@x", None, "Junk", false)]);
    let outcome = server_search(&mut m, "from:alice");
    assert_eq!(
        searches(&m, &outcome.effects),
        [(
            "[Gmail]/All Mail".into(),
            "from:alice".into(),
            "from:alice".into(),
            true,
            1
        )]
    );
    let outcome = server_search(&mut m, "in:trash from:alice");
    assert_eq!(searches(&m, &outcome.effects)[0].0, "[Gmail]/Trash");
}

#[test]
fn server_hits_are_merged_into_the_cache_and_found_again() {
    let mut m = mailbox();
    let outcome = m
        .run(Command::SetSearch {
            query: "budget".into(),
        })
        .unwrap();
    assert!(outcome.receipt.affected.contains(&EntityRef::Status));
    let effects = server_only(outcome.effects.clone());
    assert_eq!(effects.len(), 2);
    let entry = &m.engine.state().status.searches[0];
    assert_eq!((entry.query.as_str(), entry.pending), ("budget", 2));
    assert_eq!(
        outcome.receipt.data["server"],
        json!([{ "account": m.account, "state": "searching", "hits": 0, "error": null }])
    );

    // INBOX: the known UID 1 (its body mentions the budget); Archive: an old message.
    let inbox_hit = mail(1, "a@x", None, "Plans", true);
    let outcome = m.feed(results(&effects[0], 7, vec![inbox_hit]));
    assert_eq!(outcome.receipt.data["hits"], 1);
    assert_eq!(outcome.receipt.data["added"], 0);
    let outcome = m.feed(results(&effects[1], 7, vec![old_budget_mail()]));
    assert_eq!(outcome.receipt.data["added"], 1);
    assert!(
        outcome
            .effects
            .iter()
            .any(|e| matches!(e, Effect::PersistState { .. })),
        "the new message is persisted"
    );
    let old = m.message("Archive", 40);
    let entry = &m.engine.state().status.searches[0];
    assert_eq!(entry.pending, 0);
    assert_eq!(entry.hits, [m.message("INBOX", 1), old]);
    assert_eq!(m.engine.state().messages.len(), 5, "no duplicate of UID 1");

    // The cache now finds them for the same query, though neither subject says "budget";
    // non-text terms are still checked locally.
    assert_eq!(subjects(&mut m, "budget"), ["Plans", "Numbers"]);
    assert_eq!(
        subjects(&mut m, "BUDGET"),
        ["Plans", "Numbers"],
        "canonical key"
    );
    assert_eq!(subjects(&mut m, "plans"), ["Plans"], "another query");
    // The old message also shows in its mailbox like any synced one.
    let archive = m.mailbox("Archive");
    let listed = m
        .run(Command::ListThreads {
            mailbox: archive,
            offset: 0,
            limit: 50,
        })
        .unwrap();
    assert_eq!(listed.receipt.data["total"], 2);
}

#[test]
fn hits_still_obey_the_other_terms() {
    let mut m = mailbox();
    let effects = m
        .run(Command::SetSearch {
            query: "budget is:unread".into(),
        })
        .unwrap()
        .effects;
    m.feed(results(&effects[1], 7, vec![old_budget_mail()]));
    // The server said unread, but it is read in the cache (as fetched): the local state wins.
    assert_eq!(subjects(&mut m, "budget is:unread"), Vec::<String>::new());
}

#[test]
fn a_finished_server_search_is_reused_for_a_minute() {
    let mut m = mailbox();
    let at = |m: &mut Mail, ms: i64| {
        m.engine
            .dispatch(
                Command::Search {
                    query: "budget".into(),
                    account: None,
                    offset: 0,
                    limit: 50,
                    server: true,
                    order: app_core::SearchOrder::default(),
                },
                &Ctx {
                    now_ms: ms,
                    id_seed: 1 << 100,
                    origin: Origin::Mcp,
                },
            )
            .unwrap()
    };
    let effects = server_only(at(&mut m, T0 + 100_000).effects);
    assert_eq!(effects.len(), 2);
    // Results of a round `search` started create the entry.
    m.feed(results(&effects[0], 7, Vec::new()));
    let entry = &m.engine.state().status.searches[0];
    assert_eq!(entry.pending, 1, "one of two mailboxes reported");
    let outcome = at(&mut m, T0 + 200_000);
    assert!(server_only(outcome.effects).is_empty(), "still searching");
    assert_eq!(outcome.receipt.data["server"][0]["state"], "searching");
    m.feed(results(&effects[1], 7, Vec::new()));
    let updated = m.engine.state().status.searches[0].updated_ms;
    let outcome = at(&mut m, updated + 59_999);
    assert!(server_only(outcome.effects.clone()).is_empty());
    assert_eq!(outcome.receipt.data["server"][0]["state"], "done");
    let outcome = at(&mut m, updated + 60_000);
    assert_eq!(server_only(outcome.effects).len(), 2, "stale: asked again");
    // Opting out never asks.
    let outcome = m
        .run(Command::Search {
            query: "something else".into(),
            account: None,
            offset: 0,
            limit: 50,
            server: false,
            order: app_core::SearchOrder::default(),
        })
        .unwrap();
    assert!(server_only(outcome.effects).is_empty());
    assert!(outcome.receipt.data.get("server").is_none());
}

#[test]
fn search_failures_and_changed_mailboxes_are_recorded() {
    let mut m = mailbox();
    let effects = server_only(
        m.run(Command::SetSearch {
            query: "budget".into(),
        })
        .unwrap()
        .effects,
    );
    let failure = effects[0].failure_event("IMAP UID SEARCH in INBOX failed: BAD".into());
    assert!(matches!(failure, Event::SearchFailed { .. }));
    let outcome = m.feed(failure);
    assert_eq!(
        outcome.receipt.data["error"],
        "IMAP UID SEARCH in INBOX failed: BAD"
    );
    // Archive's UIDVALIDITY changed on the server: nothing is merged.
    let outcome = m.feed(results(&effects[1], 8, vec![old_budget_mail()]));
    assert_eq!(outcome.receipt.data["hits"], 0);
    let entry = &m.engine.state().status.searches[0];
    assert_eq!(entry.pending, 0);
    assert!(
        entry
            .last_error
            .as_deref()
            .unwrap()
            .contains("Archive changed")
    );
    assert_eq!(m.engine.state().messages.len(), 4);
    let data = m
        .run(Command::Search {
            query: "budget".into(),
            account: None,
            offset: 0,
            limit: 50,
            server: true,
            order: app_core::SearchOrder::default(),
        })
        .unwrap()
        .receipt
        .data;
    assert_eq!(data["server"][0]["state"], "done");
    assert!(
        data["server"][0]["error"]
            .as_str()
            .unwrap()
            .contains("changed")
    );
}

#[test]
fn only_the_latest_server_searches_are_kept() {
    let mut m = mailbox();
    for n in 0..10 {
        m.run(Command::SetSearch {
            query: format!("word{n}"),
        })
        .unwrap();
    }
    let kept: Vec<_> = m
        .engine
        .state()
        .status
        .searches
        .iter()
        .map(|s| s.query.clone())
        .collect();
    assert_eq!(
        kept,
        (2..10).map(|n| format!("word{n}")).collect::<Vec<_>>()
    );
    // Searching again moves a query to the end.
    m.run(Command::SetSearch {
        query: "word2".into(),
    })
    .unwrap();
    assert_eq!(
        m.engine.state().status.searches.last().unwrap().query,
        "word2"
    );
}

#[test]
fn a_restart_forgets_server_searches_but_keeps_their_hits_cached() {
    let mut m = mailbox();
    let effects = m
        .run(Command::SetSearch {
            query: "budget".into(),
        })
        .unwrap()
        .effects;
    m.feed(results(&effects[1], 7, vec![old_budget_mail()]));
    let restarted = app_core::Engine::new(m.engine.state().clone());
    assert!(restarted.state().status.searches.is_empty());
    assert!(
        restarted
            .state()
            .messages
            .values()
            .any(|e| e.subject == "Numbers")
    );
}

#[test]
fn full_text_hits_are_found_like_server_hits() {
    let mut m = mailbox();
    let plans = m.message("INBOX", 1);
    let lunch = m.message("INBOX", 3);
    let outcome = m
        .run(Command::SetSearch {
            query: "quarterly -is:unread".into(),
        })
        .unwrap();
    let Some(Effect::SearchIndex { search, words, .. }) = outcome
        .effects
        .iter()
        .find(|e| matches!(e, Effect::SearchIndex { .. }))
        .cloned()
    else {
        unreachable!()
    };
    assert_eq!(
        (search.as_str(), words),
        ("quarterly -is:unread", vec!["quarterly".to_owned()])
    );
    let outcome = m.feed(Event::IndexSearched {
        search: search.clone(),
        hits: vec![plans, lunch],
    });
    assert_eq!(outcome.receipt.data["hits"], 2);
    // Both bodies say "quarterly" (no header does); only the read one passes `-is:unread`.
    let found = m
        .run(Command::Search {
            query: "quarterly -is:unread".into(),
            account: None,
            offset: 0,
            limit: 50,
            server: false,
            order: app_core::SearchOrder::default(),
        })
        .unwrap();
    let ids: Vec<_> = found.receipt.data["threads"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|t| t["matches"].as_array().unwrap().clone())
        .collect();
    assert_eq!(ids, [json!(plans)]);
    m.feed(Event::IndexSearchFailed {
        search,
        error: "index damaged".into(),
    });
    let entry = &m.engine.state().status.index_searches[0];
    assert_eq!(entry.error.as_deref(), Some("index damaged"));
    assert!(entry.hits.is_empty());
}

#[test]
fn relevance_puts_the_best_full_text_hits_first() {
    let mut m = mailbox();
    let (plans, lunch) = (m.message("INBOX", 1), m.message("INBOX", 3));
    m.feed(Event::IndexSearched {
        search: "quarterly".into(),
        hits: vec![plans, lunch],
    });
    let order = |m: &mut Mail, order| {
        let found = m
            .run(Command::Search {
                query: "quarterly".into(),
                account: None,
                offset: 0,
                limit: 50,
                server: false,
                order,
            })
            .unwrap();
        found.receipt.data["threads"]
            .as_array()
            .unwrap()
            .iter()
            .map(|t| t["matches"][0].clone())
            .collect::<Vec<_>>()
    };
    assert_eq!(
        order(&mut m, app_core::SearchOrder::Newest),
        [json!(lunch), json!(plans)],
        "Lunch is newer"
    );
    assert_eq!(
        order(&mut m, app_core::SearchOrder::Relevance),
        [json!(plans), json!(lunch)],
        "the index ranked Plans higher"
    );
}
