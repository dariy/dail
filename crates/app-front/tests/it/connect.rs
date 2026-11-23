//! *Connect to a dail server* (SRV-6): `app_front::connect::ConnectFlow` over a real engine,
//! with the runtime's events fed by hand.

// reason: helpers outside `#[test]` fns may panic; clippy.toml only exempts `#[test]` bodies.
#![allow(clippy::unwrap_used, clippy::panic)]

use app_core::{AppState, Command, CommandReceipt, Ctx, Engine, Event, Origin};
use app_front::connect::{
    Availability, ConnectFlow, DEFAULT_PORT, GRACE_MS, Retry, Stage, availability, parse_server,
};
use app_front::i18n::Key;

use crate::support::{NOW, synced_state};

const FP: &str = "5d41402abc4b2a76b9719d911017c592ae2f5e3c8a1b7d0f4e6c2a9b8d7f1e03";
const OTHER: &str = "0000402abc4b2a76b9719d911017c592ae2f5e3c8a1b7d0f4e6c2a9b8d7f1e03";

/// The engine standing in for the session.
struct Rig {
    engine: Engine,
    seed: u128,
}

impl Rig {
    fn new(state: AppState) -> Self {
        Self {
            engine: Engine::new(state),
            seed: 100,
        }
    }

    fn ctx(&mut self, origin: Origin) -> Ctx {
        self.seed += 1;
        Ctx {
            now_ms: NOW,
            id_seed: self.seed << 64,
            origin,
        }
    }

    /// Dispatches `cmd` as the GUI and returns the reply as the front ends hand it to the flow.
    fn dispatch(&mut self, cmd: Command) -> Result<CommandReceipt, String> {
        let ctx = self.ctx(Origin::Gui);
        self.engine
            .dispatch(cmd, &ctx)
            .map(|o| o.receipt)
            .map_err(|e| app_front::i18n::message(&e.to_string()))
    }

    fn ingest(&mut self, event: Event) {
        let ctx = self.ctx(Origin::Runtime);
        self.engine.ingest(event, &ctx).unwrap();
    }

    fn state(&self) -> &AppState {
        self.engine.state()
    }

    /// Submits what the flow asked for and hands it the reply.
    fn run(&mut self, flow: &mut ConnectFlow, cmd: Command) {
        let reply = self.dispatch(cmd);
        flow.accept(reply.as_ref().map_err(Clone::clone), NOW);
        flow.sync(self.state(), NOW);
    }
}

fn offered(host: &str, fingerprint: &str) -> Event {
    Event::ServerOffered {
        host: host.into(),
        port: DEFAULT_PORT,
        fingerprint: fingerprint.into(),
    }
}

fn link(fp: &str) -> String {
    format!("dail://pair?v=1&h=home.example&p=7443&c=abcd2345&fp={fp}&n=Laptop")
}

#[test]
fn the_happy_path_pairs_and_pins_the_offered_certificate() {
    let mut rig = Rig::new(AppState::default());
    assert_eq!(availability(rig.state()), Availability::Open);
    let mut flow = ConnectFlow::default();
    assert_eq!(flow.stage(), &Stage::Address);
    flow.address = " home.example ".into();
    assert_eq!(flow.address_problem(), None);
    let cmd = flow.submit_address(rig.state()).unwrap();
    assert_eq!(
        cmd,
        Command::ConnectServer {
            host: "home.example".into(),
            port: DEFAULT_PORT
        }
    );
    rig.run(&mut flow, cmd);
    assert_eq!(flow.stage(), &Stage::Probing);
    assert!(flow.is_waiting());
    assert_eq!(flow.server(), "home.example:7443");
    rig.ingest(offered("home.example", FP));
    flow.sync(rig.state(), NOW);
    assert_eq!(
        flow.stage(),
        &Stage::Compare {
            fingerprint: FP.into(),
            matches_link: false
        }
    );
    // Groups of four, eight a line, as `dail serve` and *Pair a device…* show it.
    assert_eq!(
        app_front::connect::fingerprint_lines(FP),
        [
            "5d41 402a bc4b 2a76 b971 9d91 1017 c592",
            "ae2f 5e3c 8a1b 7d0f 4e6c 2a9b 8d7f 1e03"
        ]
    );
    // No code yet: nothing to submit.
    assert!(flow.code_problem().is_some());
    assert!(flow.submit_code(rig.state()).is_none());
    flow.code = "abcd2345".into();
    assert!(
        !format!("{flow:?}").contains("abcd2345"),
        "Debug hides the code"
    );
    let cmd = flow.submit_code(rig.state()).unwrap();
    let Command::TrustServer { fingerprint, code } = &cmd else {
        panic!("{cmd:?}")
    };
    assert_eq!((fingerprint.as_str(), code.expose()), (FP, "ABCD2345"));
    assert!(flow.code.is_empty(), "the code left the field");
    rig.run(&mut flow, cmd);
    assert_eq!(flow.stage(), &Stage::Pairing);
    rig.ingest(Event::ServerPaired {
        fingerprint: FP.into(),
    });
    flow.sync(rig.state(), NOW);
    assert_eq!(flow.stage(), &Stage::Paired);
    assert_eq!(availability(rig.state()), Availability::Linked);
    let saved = serde_json::to_string(rig.state()).unwrap();
    assert!(
        !saved.contains("ABCD2345"),
        "the code never reaches the state"
    );
    // Nothing to undo once pinned.
    assert_eq!(flow.cancel(rig.state()), None);
}

#[test]
fn a_pasted_link_fills_the_form_and_its_fingerprint_must_match() {
    // The link's fingerprint is the one the server offers: compared, then paired with its code.
    let mut rig = Rig::new(AppState::default());
    let mut flow = ConnectFlow::default();
    flow.paste(&format!("{}\nignored", link(FP)));
    assert_eq!(flow.address, "home.example:7443");
    assert_eq!(flow.code, "ABCD2345");
    assert_eq!(flow.link_fingerprint(), Some(FP));
    assert!(flow.note().unwrap().contains("home.example:7443"));
    let cmd = flow.submit_address(rig.state()).unwrap();
    rig.run(&mut flow, cmd);
    rig.ingest(offered("home.example", FP));
    flow.sync(rig.state(), NOW);
    assert_eq!(
        flow.stage(),
        &Stage::Compare {
            fingerprint: FP.into(),
            matches_link: true
        }
    );
    assert!(matches!(
        flow.submit_code(rig.state()),
        Some(Command::TrustServer { .. })
    ));

    // Another fingerprint than the link's: refused, nothing trusted, the link's code dropped.
    let mut rig = Rig::new(AppState::default());
    let mut flow = ConnectFlow::default();
    flow.paste(&link(OTHER));
    let cmd = flow.submit_address(rig.state()).unwrap();
    rig.run(&mut flow, cmd);
    rig.ingest(offered("home.example", FP));
    flow.sync(rig.state(), NOW);
    assert_eq!(
        flow.stage(),
        &Stage::Mismatch {
            offered: FP.into(),
            expected: OTHER.into()
        }
    );
    assert_eq!(flow.submit_code(rig.state()), None, "never trusted");
    let server = rig.state().link.as_ref().unwrap().server.as_ref().unwrap();
    assert!(server.pairing.is_none() && server.fingerprint.is_none());
    flow.retry();
    assert_eq!(flow.stage(), &Stage::Address);
    assert!(flow.code.is_empty());
    assert_eq!(flow.link_fingerprint(), None);
    // Given up: the unpinned link goes, so the profile is a plain one again.
    let cmd = flow.cancel(rig.state()).unwrap();
    assert_eq!(cmd, Command::DisconnectServer { confirm: true });
    rig.dispatch(cmd).unwrap();
    assert!(rig.state().link.is_none());
}

#[test]
fn a_link_typed_into_the_field_is_read_on_connect() {
    let rig = Rig::new(AppState::default());
    let mut flow = ConnectFlow::default();
    flow.address = link(FP);
    assert_eq!(flow.address_problem(), None, "read on Connect");
    let cmd = flow.submit_address(rig.state()).unwrap();
    assert_eq!(
        cmd,
        Command::ConnectServer {
            host: "home.example".into(),
            port: 7443
        }
    );
    assert_eq!(flow.link_fingerprint(), Some(FP));
    // An edit by hand drops the link's fingerprint.
    flow.address_edited();
    assert_eq!(flow.link_fingerprint(), None);
}

#[test]
fn a_bad_link_is_refused_without_repeating_it() {
    let rig = Rig::new(AppState::default());
    let mut flow = ConnectFlow::default();
    flow.paste("dail://pair?v=1&h=home.example&p=7443&c=EVIL&fp=00&n=x");
    assert!(flow.address.is_empty(), "the field is emptied");
    assert!(flow.code.is_empty());
    let note = flow.note().unwrap();
    assert!(
        note.starts_with("Not a usable pairing link: ERR471"),
        "{note}"
    );
    assert!(!note.contains("EVIL"), "{note}");
    assert_eq!(flow.submit_address(rig.state()), None);
}

#[test]
fn a_wrong_code_fails_and_another_one_can_be_tried() {
    let mut rig = Rig::new(AppState::default());
    let mut flow = ConnectFlow::default();
    flow.address = "home.example".into();
    let cmd = flow.submit_address(rig.state()).unwrap();
    rig.run(&mut flow, cmd);
    rig.ingest(offered("home.example", FP));
    flow.sync(rig.state(), NOW);
    flow.code = "ABCD2345".into();
    let cmd = flow.submit_code(rig.state()).unwrap();
    rig.run(&mut flow, cmd);
    rig.ingest(Event::LinkFailed {
        error: "the server refused the pairing code".into(),
    });
    flow.sync(rig.state(), NOW);
    let Stage::Failed { error, retry } = flow.stage().clone() else {
        panic!("{:?}", flow.stage())
    };
    assert_eq!(retry, Retry::Code);
    assert_eq!(error, "Pairing failed: the server refused the pairing code");
    flow.retry();
    assert_eq!(
        flow.stage(),
        &Stage::Compare {
            fingerprint: FP.into(),
            matches_link: false
        }
    );
    assert!(flow.code.is_empty());

    // The second try fails the same way, so fast that no frame saw the attempt in progress:
    // the old error is only trusted once the grace period is over.
    flow.code = "EFGH2345".into();
    let cmd = flow.submit_code(rig.state()).unwrap();
    let reply = rig.dispatch(cmd);
    rig.ingest(Event::LinkFailed {
        error: "the server refused the pairing code".into(),
    });
    flow.accept(reply.as_ref().map_err(Clone::clone), NOW);
    flow.sync(rig.state(), NOW);
    assert_eq!(
        flow.stage(),
        &Stage::Pairing,
        "maybe a state older than the reply"
    );
    flow.sync(rig.state(), NOW + GRACE_MS);
    assert!(matches!(
        flow.stage(),
        Stage::Failed {
            retry: Retry::Code,
            ..
        }
    ));
}

#[test]
fn an_unreachable_server_can_be_tried_again() {
    let mut rig = Rig::new(AppState::default());
    let mut flow = ConnectFlow::default();
    flow.address = "[fd7a::1]:8443".into();
    let cmd = flow.submit_address(rig.state()).unwrap();
    assert_eq!(
        cmd,
        Command::ConnectServer {
            host: "[fd7a::1]".into(),
            port: 8443
        }
    );
    rig.run(&mut flow, cmd);
    rig.ingest(Event::LinkFailed {
        error: "connect failed: connection refused".into(),
    });
    flow.sync(rig.state(), NOW);
    let Stage::Failed { error, retry } = flow.stage().clone() else {
        panic!("{:?}", flow.stage())
    };
    assert_eq!(retry, Retry::Address);
    assert!(error.starts_with("Could not reach the server: "), "{error}");
    flow.retry();
    assert_eq!(flow.stage(), &Stage::Address);
    assert_eq!(flow.address, "[fd7a::1]:8443", "kept");
    assert_eq!(
        flow.cancel(rig.state()),
        Some(Command::DisconnectServer { confirm: true })
    );
}

#[test]
fn a_profile_with_accounts_cannot_connect() {
    let state = synced_state();
    assert_eq!(availability(&state), Availability::HasAccounts(1));
    let mut flow = ConnectFlow::default();
    flow.address = "home.example".into();
    assert_eq!(
        flow.submit_address(&state),
        None,
        "the option is not offered"
    );

    // Should another front end add an account meanwhile, the engine refuses with ERR013 and the
    // flow says what to do.
    let mut rig = Rig::new(state);
    let mut flow = ConnectFlow::default();
    flow.address = "home.example".into();
    let cmd = flow.submit_address(&AppState::default()).unwrap();
    let reply = rig.dispatch(cmd);
    flow.accept(reply.as_ref().map_err(Clone::clone), NOW);
    let Stage::Failed { error, retry } = flow.stage().clone() else {
        panic!("{:?}", flow.stage())
    };
    assert_eq!(retry, Retry::Address);
    assert!(error.starts_with("ERR013"), "{error}");
    assert!(error.contains("dail --profile <name>"), "{error}");
    assert!(rig.state().link.is_none());
}

#[test]
fn servers_parse_like_dail_remote_add() {
    let parse = |text: &str| parse_server(text).unwrap();
    assert_eq!(parse("home.example.org"), ("home.example.org".into(), 7443));
    assert_eq!(parse(" 100.64.0.7:8443 "), ("100.64.0.7".into(), 8443));
    assert_eq!(parse("[::1]:9000"), ("[::1]".into(), 9000));
    assert_eq!(parse("[fd7a::1]"), ("[fd7a::1]".into(), 7443));
    assert_eq!(parse("fd7a::1"), ("fd7a::1".into(), 7443));
    for (bad, key) in [
        ("", Key::ConnectExpectedHostPort),
        (":7443", Key::ConnectExpectedHostPort),
        ("[::1", Key::ConnectExpectedHostPort),
        ("[::1]x", Key::ConnectExpectedHostPort),
        ("host:0", Key::ConnectBadPort),
        ("host:http", Key::ConnectBadPort),
        ("host:99999", Key::ConnectBadPort),
        ("bad_host!:7443", Key::ConnectBadHost),
        ("-x.example", Key::ConnectBadHost),
        ("10.0.0.300", Key::ConnectBadHost),
    ] {
        assert_eq!(parse_server(bad), Err(key), "{bad}");
    }
    let mut flow = ConnectFlow::default();
    flow.address = "host:0".into();
    assert_eq!(
        flow.address_problem().as_deref(),
        Some("The port must be 1 to 65535.")
    );
    assert_eq!(flow.submit_address(&AppState::default()), None);
}
