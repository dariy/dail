//! The composer: recipients, subject and body, written back per pause, field change or key
//! command; an agent's edit reloads the fields unless there is unsaved typing.

use app_core::{Addr, Command, Pane};
use ratatui::crossterm::event::{KeyCode, KeyModifiers};

use crate::support::{Term, inbox_of, synced_state};

fn bob() -> Addr {
    Addr {
        name: None,
        email: "bob@example.org".into(),
    }
}

/// A new message in the composer (from `c`), the cursor at To.
fn composing(width: u16, height: u16) -> (Term, app_core::DraftId) {
    let state = synced_state();
    let (inbox, _) = inbox_of(&state);
    let mut t = Term::new(state, width, height);
    t.apply(Command::OpenMailbox { id: inbox });
    t.out.clear();
    t.key(KeyCode::Char('c'));
    let sent = t.pump();
    let Some(Command::OpenComposer { id }) = sent.last().cloned() else {
        unreachable!("{sent:?}");
    };
    assert_eq!(t.engine.state().view.focus, Pane::Composer);
    (t, id)
}

fn update(
    id: app_core::DraftId,
    to: Option<Vec<Addr>>,
    subject: Option<&str>,
    body: Option<&str>,
) -> Command {
    Command::UpdateDraft {
        id,
        to,
        cc: None,
        bcc: None,
        subject: subject.map(str::to_owned),
        body: body.map(str::to_owned),
    }
}

#[test]
fn addresses_become_recipients_and_invalid_text_stays() {
    let (mut t, id) = composing(80, 24);
    t.typed("bob@example.org");
    t.key(KeyCode::Enter);
    assert_eq!(t.pump(), [update(id, Some(vec![bob()]), None, None)]);
    t.typed("not an address");
    t.key(KeyCode::Tab);
    assert!(t.pump().is_empty(), "nothing valid to add");
    t.key(KeyCode::BackTab);
    let screen = t.screen();
    assert!(
        screen.contains("To: bob@example.org, not an address"),
        "{screen}"
    );
    // Backspace on an empty field removes the last address.
    for _ in 0.."not an address".len() {
        t.key(KeyCode::Backspace);
    }
    t.key(KeyCode::Backspace);
    assert_eq!(t.pump(), [update(id, Some(Vec::new()), None, None)]);
}

#[test]
fn typing_goes_back_per_pause_field_change_or_key_command() {
    let (mut t, id) = composing(80, 24);
    t.typed("bob@example.org,");
    t.pump();
    // To → Cc → Bcc → Subject.
    t.key(KeyCode::Tab);
    t.key(KeyCode::Tab);
    t.key(KeyCode::Tab);
    t.typed("Lunch");
    assert!(t.take().is_empty(), "not per keystroke");
    t.key(KeyCode::Enter);
    assert_eq!(
        t.pump(),
        [update(id, None, Some("Lunch"), None)],
        "leaving the field writes it back"
    );
    t.typed("Noon?");
    t.advance(500);
    assert!(t.take().is_empty());
    t.advance(600);
    assert_eq!(
        t.pump(),
        [update(id, None, None, Some("Noon?"))],
        "a pause of a second writes it back"
    );
    t.typed(" Or one?");
    t.chord(KeyModifiers::CONTROL, KeyCode::Char('s'));
    assert_eq!(
        t.pump(),
        [
            update(id, None, None, Some("Noon? Or one?")),
            Command::SendDraft { id }
        ],
        "Ctrl+S sends what is on screen"
    );
}

#[test]
fn registry_chords_win_over_the_editors_shortcuts() {
    let (mut t, _) = composing(80, 24);
    t.key(KeyCode::Tab);
    t.key(KeyCode::Tab);
    t.key(KeyCode::Tab);
    t.key(KeyCode::Tab);
    t.typed("keep this line");
    // Ctrl+K deletes to the end of the line in the editor; here it opens the palette.
    t.chord(KeyModifiers::CONTROL, KeyCode::Home);
    t.chord(KeyModifiers::CONTROL, KeyCode::Char('k'));
    let sent = t.pump();
    assert_eq!(sent.last(), Some(&Command::OpenPalette {}), "{sent:?}");
    t.key(KeyCode::Esc);
    assert_eq!(t.pump(), [Command::ClosePalette {}]);
    assert!(t.screen().contains("keep this line"), "{}", t.screen());
    t.key(KeyCode::Esc);
    let sent = t.take();
    assert_eq!(sent.last(), Some(&Command::CloseComposer {}), "{sent:?}");
}

#[test]
fn an_agents_edit_reloads_unless_there_is_unsaved_typing() {
    let (mut t, id) = composing(80, 24);
    t.apply(update(id, None, Some("From the agent"), None));
    assert!(t.screen().contains("Subject: From the agent"));
    for _ in 0..3 {
        t.key(KeyCode::Tab);
    }
    t.typed("!");
    t.apply(update(id, None, Some("Second edit"), None));
    let screen = t.screen();
    assert!(screen.contains("Subject: From the agent!"), "{screen}");
}

#[test]
fn a_reply_shows_the_quote_read_only_below_the_body() {
    let state = synced_state();
    let (inbox, threads) = inbox_of(&state);
    let mut t = Term::new(state, 40, 20);
    t.apply(Command::OpenMailbox { id: inbox });
    t.apply(Command::OpenThread { id: threads[1] });
    t.out.clear();
    t.key(KeyCode::Char('r'));
    t.pump();
    t.typed("Thanks!");
    insta::assert_snapshot!(t.screen());
}

#[test]
fn ctrl_o_hands_the_body_to_the_editor_and_takes_it_back() {
    let (mut t, id) = composing(80, 24);
    for _ in 0..4 {
        t.key(KeyCode::Tab);
    }
    t.typed("draft text");
    t.chord(KeyModifiers::CONTROL, KeyCode::Char('o'));
    assert_eq!(
        t.pump(),
        [update(id, None, None, Some("draft text"))],
        "typed text first"
    );
    let Some(app_tui::External::EditBody { draft, text }) = t.tui.take_external() else {
        unreachable!("the editor was not asked for");
    };
    assert_eq!((draft, text.as_str()), (id, "draft text"));
    let state = t.engine.state().clone();
    let back = t.tui.edited(
        id,
        app_tui::EditOutcome::Text("edited\nin vi\n".into()),
        &state,
    );
    t.out.extend(back);
    assert_eq!(t.pump(), [update(id, None, None, Some("edited\nin vi"))]);
    assert!(
        t.screen().contains("in vi"),
        "the composer shows the editor's text"
    );
    let state = t.engine.state().clone();
    assert!(
        t.tui
            .edited(id, app_tui::EditOutcome::NoEditor, &state)
            .is_empty()
    );
    let screen = t.screen();
    assert!(screen.contains("Set $VISUAL or $EDITOR"), "{screen}");
}

#[test]
fn tab_completes_a_recipient_from_the_mail() {
    let (mut t, id) = composing(80, 24);
    t.typed("car");
    let screen = t.screen();
    assert!(
        screen.contains("Tab: Carol <carol@example.org>"),
        "{screen}"
    );
    t.key(KeyCode::Tab);
    let carol = Addr {
        name: Some("Carol".into()),
        email: "carol@example.org".into(),
    };
    assert_eq!(t.pump(), [update(id, Some(vec![carol]), None, None)]);
    // Without a match Tab moves on, as before.
    t.typed("zzz");
    t.key(KeyCode::Tab);
    assert!(t.pump().is_empty());
}
