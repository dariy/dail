//! `app_front::pairing`: the strict `dail://pair` parser, its round trip, where the QR code
//! points, and the *Pair a device…* dialog (SRV-QR1).

// reason: helpers outside `#[test]` fns may panic; clippy.toml only exempts `#[test]` bodies.
#![allow(clippy::unwrap_used, clippy::panic)]

use app_core::{
    AppState, Command, Ctx, DeviceRole, Engine, Event, LinkState, Origin, ServeInfo, ServerEndpoint,
};
use app_front::pairing::{
    MAX_URI_BYTES, PairDialog, PairingError, PairingUri, Stage, can_pair, countdown, endpoint,
    parse_uri,
};
use proptest::prelude::*;

const FP: &str = "5d41402abc4b2a76b9719d911017c592ae2f5e3c8a1b7d0f4e6c2a9b8d7f1e03";

fn good() -> String {
    format!("dail://pair?v=1&h=home.example&p=7443&c=ABCD2345&fp={FP}&n=Phone")
}

fn with(from: &str, to: &str) -> String {
    let uri = good();
    assert!(uri.contains(from), "{from}");
    uri.replacen(from, to, 1)
}

#[test]
fn a_good_uri_parses_and_normalises() {
    let uri = parse_uri(&good()).unwrap();
    assert_eq!(
        (
            uri.host(),
            uri.port(),
            uri.code(),
            uri.fingerprint(),
            uri.name()
        ),
        ("home.example", 7443, "ABCD2345", FP, "Phone")
    );
    assert_eq!(uri.to_uri(), good());
    // Case-insensitive code and fingerprint, upper-case host, any parameter order, encodings.
    let mixed = format!(
        "dail://pair?n=Ph%C3%B6ne%20X&fp={}&c=abcd2345&p=1&h=HOME.Example&v=1",
        FP.to_uppercase()
    );
    let uri = parse_uri(&mixed).unwrap();
    assert_eq!(uri.code(), "ABCD2345");
    assert_eq!(uri.fingerprint(), FP);
    assert_eq!(uri.host(), "home.example");
    assert_eq!(uri.name(), "Phöne X");
    assert!(uri.to_uri().contains("&n=Ph%C3%B6ne%20X"));
    // IP literals: IPv6 bare (canonical form), IPv4.
    let v6 = parse_uri(&with("h=home.example", "h=FD7A:115C:0:0::1")).unwrap();
    assert_eq!(v6.host(), "fd7a:115c::1");
    assert_eq!(parse_uri(&v6.to_uri()).unwrap(), v6);
    assert_eq!(
        parse_uri(&with("h=home.example", "h=100.64.0.7"))
            .unwrap()
            .host(),
        "100.64.0.7"
    );
    // Debug never shows the code.
    assert!(!format!("{v6:?}").contains("ABCD2345"));
}

#[test]
fn every_refusal() {
    use PairingError as E;
    let long = format!("{}{}", good(), "x".repeat(MAX_URI_BYTES));
    let cases: Vec<(String, PairingError)> = vec![
        (long, E::TooLong),
        (good().replace("dail://", "DAIL://"), E::NotPairing),
        (
            good().replace("dail://pair?", "dail://pairs?"),
            E::NotPairing,
        ),
        (good().replace("dail://pair?", "dail:pair?"), E::NotPairing),
        (
            good().replace("dail://pair?", "https://pair?"),
            E::NotPairing,
        ),
        (String::new(), E::NotPairing),
        (
            format!("{}#frag", good()),
            E::Malformed("a fragment (#) is not allowed"),
        ),
        (
            with("n=Phone", "n=Ph one"),
            E::Malformed("a character must be percent-encoded"),
        ),
        (
            with("n=Phone", "n=Phöne"),
            E::Malformed("a character must be percent-encoded"),
        ),
        (
            with("n=Phone", "n=Ph%2"),
            E::Malformed("a % escape needs two hex digits"),
        ),
        (
            with("n=Phone", "n=Ph%zz"),
            E::Malformed("a % escape needs two hex digits"),
        ),
        (
            with("n=Phone", "n=%FF%FE"),
            E::Malformed("an escape is not UTF-8"),
        ),
        (
            with("n=Phone", "n=a=b"),
            E::Malformed("a value has a raw ="),
        ),
        (
            format!("{}&", good()),
            E::Malformed("a parameter has no value"),
        ),
        (
            with("&p=7443", "&&p=7443"),
            E::Malformed("a parameter has no value"),
        ),
        (
            with("&p=7443", "&p"),
            E::Malformed("a parameter has no value"),
        ),
        (with("v=1&", ""), E::Missing("v")),
        (with("&n=Phone", ""), E::Missing("n")),
        (with("&c=ABCD2345", ""), E::Missing("c")),
        (format!("{}&p=1", good()), E::Duplicate("p")),
        (format!("{}&x=1", good()), E::Unknown),
        (with("&p=", "&P="), E::Unknown),
        (with("v=1", "v=2"), E::Version),
        (with("v=1", "v=01"), E::Version),
        (with("h=home.example", "h="), E::Host),
        (with("h=home.example", "h=-home.example"), E::Host),
        (with("h=home.example", "h=home_x.example"), E::Host),
        (with("h=home.example", "h=home..example"), E::Host),
        (with("h=home.example", "h=1.2.3.999"), E::Host),
        (with("h=home.example", "h=%5Bfd7a::1%5D"), E::Host),
        (with("h=home.example", "h=fd7a::1%25eth0"), E::Host),
        (with("h=home.example", "h=a%2Fb"), E::Host),
        (
            with("h=home.example", &format!("h={}", "a".repeat(64))),
            E::Host,
        ),
        (with("p=7443", "p=0"), E::Port),
        (with("p=7443", "p=07443"), E::Port),
        (with("p=7443", "p=65536"), E::Port),
        (with("p=7443", "p=+1"), E::Port),
        (with("p=7443", "p="), E::Port),
        (with("c=ABCD2345", "c=ABCD234"), E::Code),
        (with("c=ABCD2345", "c=ABCD23451"), E::Code),
        (with("c=ABCD2345", "c=ABCD2341"), E::Code),
        (
            with(&format!("fp={FP}"), &format!("fp={}", &FP[1..])),
            E::Fingerprint,
        ),
        (
            with(&format!("fp={FP}"), &format!("fp={}g", &FP[1..])),
            E::Fingerprint,
        ),
        (with(&format!("fp={FP}"), "fp=5d:41"), E::Fingerprint),
        (with("n=Phone", "n="), E::Name),
        (with("n=Phone", "n=%20Phone"), E::Name),
        (with("n=Phone", "n=Phone%0A"), E::Name),
        (with("n=Phone", "n=Ph%07one"), E::Name),
        (with("n=Phone", "n=Ph%E2%80%AEone"), E::Name),
        (with("n=Phone", &format!("n={}", "x".repeat(65))), E::Name),
    ];
    for (text, expected) in cases {
        assert_eq!(parse_uri(&text), Err(expected), "{text}");
    }
    // The message never repeats the hostile text.
    let err = parse_uri(&format!("{}&evil<script>=1", good())).unwrap_err();
    assert!(!err.to_string().contains("evil"));
    assert!(err.to_string().starts_with("ERR"));
    // A name of 64 characters is the limit.
    assert!(
        parse_uri(&with(
            "n=Phone",
            &format!("n={}", "é".repeat(64).replace('é', "%C3%A9"))
        ))
        .is_ok()
    );
}

#[test]
fn new_accepts_bracketed_ipv6_and_colon_fingerprints() {
    let colons: String = FP
        .as_bytes()
        .chunks(2)
        .map(|c| std::str::from_utf8(c).unwrap())
        .collect::<Vec<_>>()
        .join(":");
    let uri = PairingUri::new("[fd7a::1]", 7443, "abcd2345", &colons, "Phone").unwrap();
    assert_eq!(uri.host(), "fd7a::1");
    assert_eq!(uri.fingerprint(), FP);
    assert_eq!(
        PairingUri::new("h", 0, "ABCD2345", FP, "x").unwrap_err(),
        PairingError::Port
    );
}

fn host() -> impl Strategy<Value = String> {
    prop_oneof![
        "([a-z0-9]([a-z0-9-]{0,10}[a-z0-9])?\\.){0,3}[a-z]([a-z0-9-]{0,8}[a-z0-9])?",
        any::<[u8; 4]>().prop_map(|b| std::net::Ipv4Addr::from(b).to_string()),
        any::<[u16; 8]>().prop_map(|s| std::net::Ipv6Addr::from(s).to_string()),
    ]
}

fn name() -> impl Strategy<Value = String> {
    "[^\\p{C}\\s]([^\\p{C}\\u{2028}\\u{2029}]{0,62}[^\\p{C}\\s])?"
        .prop_filter("64 characters", |n| n.chars().count() <= 64)
}

proptest! {
    #[test]
    fn parse_inverts_to_uri(
        host in host(),
        port in 1u16..,
        code in "[A-Z2-7]{8}",
        fp in "[0-9a-f]{64}",
        name in name(),
    ) {
        let uri = PairingUri::new(&host, port, &code, &fp, &name).unwrap();
        prop_assert_eq!(parse_uri(&uri.to_uri()), Ok(uri));
    }

    #[test]
    fn random_text_never_panics(text in "\\PC{0,300}") {
        let _ = parse_uri(&text);
        let _ = parse_uri(&format!("dail://pair?{text}"));
    }

    #[test]
    fn random_queries_of_known_keys_never_panic(
        parts in proptest::collection::vec(("(v|h|p|c|fp|n|x)", "[%=&a-zA-Z0-9:.\\-]{0,20}"), 0..10),
    ) {
        let query: Vec<String> = parts.iter().map(|(k, v)| format!("{k}={v}")).collect();
        let _ = parse_uri(&format!("dail://pair?{}", query.join("&")));
    }
}

fn serving(addresses: &[&str], advertise: &[&str]) -> AppState {
    let mut state = AppState::default();
    state.status.serve = Some(ServeInfo {
        addresses: addresses.iter().map(|s| (*s).to_owned()).collect(),
        advertise: advertise.iter().map(|s| (*s).to_owned()).collect(),
        fingerprint: FP.to_owned(),
    });
    state
}

#[test]
fn the_qr_code_points_where_a_device_can_dial() {
    assert!(!can_pair(&AppState::default()));
    assert_eq!(endpoint(&AppState::default()), None);
    let host = |s: &AppState| endpoint(s).map(|e| (e.host, e.port));
    // The first valid advertised host, with the first listener's port.
    let s = serving(
        &["0.0.0.0:7443", "[::]:9000"],
        &["bad host", "home.tail.ts.net"],
    );
    assert!(can_pair(&s));
    assert_eq!(host(&s), Some(("home.tail.ts.net".into(), 7443)));
    // Else the first bound address that is neither unspecified nor loopback.
    let s = serving(&["127.0.0.1:7443", "0.0.0.0:7444", "100.64.0.7:7445"], &[]);
    assert_eq!(host(&s), Some(("100.64.0.7".into(), 7445)));
    // Else loopback; never an unspecified address.
    let s = serving(&["0.0.0.0:7444", "[::1]:7443"], &[]);
    assert_eq!(host(&s), Some(("::1".into(), 7443)));
    let s = serving(&["0.0.0.0:7444"], &[]);
    assert!(can_pair(&s));
    assert_eq!(host(&s), None);
    // A device that mirrors a server: its own link wins over the server's status.
    let mut s = serving(&["100.64.0.7:7445"], &[]);
    s.link = Some(LinkState {
        server: Some(ServerEndpoint {
            host: "[fd7a::9]".into(),
            port: 8443,
            offered: None,
            pairing: None,
            fingerprint: Some(FP.into()),
            token: None,
        }),
        ..LinkState::default()
    });
    let e = endpoint(&s).unwrap();
    assert_eq!(
        (e.host.as_str(), e.port, e.fingerprint.as_str()),
        ("fd7a::9", 8443, FP)
    );
}

/// Runs the dialog's command on a server engine; returns the dialog after the reply.
fn dialog_on(state: AppState, role: DeviceRole) -> (PairDialog, Engine) {
    let mut engine = Engine::new(state);
    let mut dialog = PairDialog::default();
    assert!(!dialog.can_submit(), "a name is needed");
    assert_eq!(dialog.submit(engine.state()), None);
    dialog.role = role;
    dialog.name = "  Phone ".into();
    let cmd = dialog.submit(engine.state()).unwrap();
    assert_eq!(
        cmd,
        Command::StartPairing {
            role,
            name: "Phone".into()
        }
    );
    assert_eq!(dialog.stage(), &Stage::Waiting);
    assert_eq!(dialog.submit(engine.state()), None, "once");
    let ctx = Ctx {
        now_ms: 1_000,
        id_seed: 42 << 64,
        origin: Origin::Gui,
    };
    let result = engine.dispatch(cmd, &ctx).map(|o| o.receipt);
    let state = engine.state().clone();
    dialog.accept(result.as_ref().map_err(ToString::to_string), &state);
    (dialog, engine)
}

#[test]
fn the_dialog_shows_the_code_its_expiry_and_the_qr_code() {
    let (dialog, _) = dialog_on(serving(&["100.64.0.7:7443"], &[]), DeviceRole::Agent);
    let Stage::Opened(opened) = dialog.stage() else {
        panic!("{:?}", dialog.stage());
    };
    assert_eq!(opened.code, app_core::link::pairing_code(42 << 64));
    assert_eq!(opened.role, DeviceRole::Agent);
    assert_eq!(opened.expires_at_ms, 1_000 + app_core::PAIRING_TTL_MS);
    assert_eq!(countdown(opened.remaining_ms(1_000)), "10:00");
    assert_eq!(countdown(opened.remaining_ms(1_000 + 599_001)), "0:01");
    assert_eq!(opened.remaining_ms(i64::MAX), 0);
    let uri = parse_uri(opened.uri.as_deref().unwrap()).unwrap();
    assert_eq!(
        (uri.host(), uri.port(), uri.code()),
        ("100.64.0.7", 7443, opened.code.as_str())
    );
    assert!(opened.qr.is_some());
    assert!(
        !format!("{dialog:?}").contains(&opened.code),
        "Debug hides the code"
    );
    // The code never reaches the state.
    let mut again = dialog.clone();
    again.again();
    assert_eq!(again.stage(), &Stage::Form);
    assert!(again.name.is_empty());
    assert_eq!(again.role, DeviceRole::Agent);
}

#[test]
fn the_dialog_shows_failures() {
    // Without an address there is a code but no QR code.
    let (dialog, mut engine) = dialog_on(serving(&["0.0.0.0:7443"], &[]), DeviceRole::User);
    let Stage::Opened(opened) = dialog.stage() else {
        panic!("{:?}", dialog.stage());
    };
    assert_eq!((opened.uri.as_ref(), opened.qr.as_ref()), (None, None));
    // The server could not open the code.
    let ctx = Ctx {
        now_ms: 2_000,
        id_seed: 7,
        origin: Origin::Runtime,
    };
    engine
        .ingest(
            Event::ServeFailed {
                error: "ERR2xx: not serving".into(),
            },
            &ctx,
        )
        .unwrap();
    let mut dialog = dialog;
    dialog.sync(engine.state());
    assert_eq!(dialog.stage(), &Stage::Failed("ERR2xx: not serving".into()));
    // A refusal, and a receipt without a code.
    let mut dialog = PairDialog::submitted(DeviceRole::User, "Phone", engine.state());
    dialog.sync(engine.state());
    assert_eq!(
        dialog.stage(),
        &Stage::Waiting,
        "an old failure is not this one"
    );
    dialog.accept(Err("ERR014: refused".into()), engine.state());
    assert_eq!(dialog.stage(), &Stage::Failed("ERR014: refused".into()));
    let mut dialog = PairDialog::submitted(DeviceRole::User, "Phone", engine.state());
    let receipt = engine
        .dispatch(Command::ListDevices {}, &ctx)
        .unwrap()
        .receipt;
    dialog.accept(Ok(&receipt), engine.state());
    assert!(matches!(dialog.stage(), Stage::Failed(e) if e.starts_with("ERR474")));
}

#[test]
fn invisible_characters_are_refused_in_names_everywhere() {
    for (name, encoded) in [
        ("Ph\u{200B}one", "Ph%E2%80%8Bone"),
        ("Ph\u{061C}one", "Ph%D8%9Cone"),
        ("Ph\u{180E}one", "Ph%E1%A0%8Eone"),
    ] {
        assert_eq!(
            parse_uri(&with("n=Phone", &format!("n={encoded}"))),
            Err(PairingError::Name),
            "{name:?}"
        );
        assert!(app_front::pairing::forbidden_in_name(
            name.chars().nth(2).unwrap()
        ));
        let mut dialog = PairDialog::default();
        dialog.name = name.into();
        assert!(!dialog.can_submit(), "{name:?}");
        assert_eq!(dialog.submit(&serving(&["100.64.0.7:7443"], &[])), None);
    }
}

#[test]
fn a_name_the_link_refuses_is_reported_not_taken_for_a_missing_address() {
    // The engine accepts a zero-width space (only controls are refused there); the palette may
    // submit one. The dialog says why there is no QR code.
    let state = serving(&["100.64.0.7:7443"], &[]);
    let mut engine = Engine::new(state);
    let mut dialog = PairDialog::submitted(DeviceRole::User, "Ph\u{200B}one", engine.state());
    let ctx = Ctx {
        now_ms: 1_000,
        id_seed: 42 << 64,
        origin: Origin::Gui,
    };
    let receipt = engine
        .dispatch(
            Command::StartPairing {
                role: DeviceRole::User,
                name: "Ph\u{200B}one".into(),
            },
            &ctx,
        )
        .unwrap()
        .receipt;
    let state = engine.state().clone();
    dialog.accept(Ok(&receipt), &state);
    let Stage::Opened(opened) = dialog.stage() else {
        panic!("{:?}", dialog.stage());
    };
    assert!(opened.endpoint.is_some());
    assert_eq!((opened.uri.as_ref(), opened.qr.as_ref()), (None, None));
    assert!(opened.link_error.as_deref().unwrap().starts_with("ERR473"));
}
