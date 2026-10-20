//! The search query language: parser, canonical form and dates.

// reason: helpers outside `#[test]` fns may panic; clippy.toml only exempts `#[test]` bodies.
#![allow(clippy::unwrap_used)]

use app_core::search::{Condition, Date, ParseError, Query, Status, Term};
use proptest::prelude::*;

fn term(condition: Condition) -> Term {
    Term {
        negated: false,
        condition,
    }
}

fn not(condition: Condition) -> Term {
    Term {
        negated: true,
        condition,
    }
}

fn parse(input: &str) -> Vec<Term> {
    Query::parse(input).unwrap().terms
}

fn error(input: &str) -> ParseError {
    Query::parse(input).unwrap_err()
}

fn date(year: u16, month: u8, day: u8) -> Date {
    Date::new(year, month, day).unwrap()
}

#[test]
fn empty_queries_have_no_terms() {
    assert!(Query::parse("").unwrap().is_empty());
    assert!(Query::parse("  \t ").unwrap().is_empty());
}

#[test]
fn words_and_phrases_are_text() {
    assert_eq!(
        parse(r#"lunch "next week"  Plans"#),
        vec![
            term(Condition::Text("lunch".into())),
            term(Condition::Text("next week".into())),
            term(Condition::Text("Plans".into())),
        ]
    );
}

#[test]
fn every_field_parses() {
    assert_eq!(
        parse(
            "from:alice to:bob@x.org cc:carol subject:Q3 is:unread is:read is:starred \
             has:attachment in:inbox before:2024-02-29 after:2023/1/5"
        ),
        vec![
            term(Condition::From("alice".into())),
            term(Condition::To("bob@x.org".into())),
            term(Condition::Cc("carol".into())),
            term(Condition::Subject("Q3".into())),
            term(Condition::Is(Status::Unread)),
            term(Condition::Is(Status::Read)),
            term(Condition::Is(Status::Starred)),
            term(Condition::HasAttachment),
            term(Condition::In("inbox".into())),
            term(Condition::Before(date(2024, 2, 29))),
            term(Condition::After(date(2023, 1, 5))),
        ]
    );
}

#[test]
fn field_names_and_keywords_are_case_insensitive() {
    assert_eq!(
        parse("FROM:Alice Is:UNREAD HAS:Attachments"),
        vec![
            term(Condition::From("Alice".into())),
            term(Condition::Is(Status::Unread)),
            term(Condition::HasAttachment),
        ]
    );
}

#[test]
fn values_can_be_quoted() {
    assert_eq!(
        parse(r#"from:"Alice Example" subject:"say \"hi\" \\o/" in:"[Gmail]/Sent Mail""#),
        vec![
            term(Condition::From("Alice Example".into())),
            term(Condition::Subject(r#"say "hi" \o/"#.into())),
            term(Condition::In("[Gmail]/Sent Mail".into())),
        ]
    );
}

#[test]
fn a_minus_negates_any_term() {
    assert_eq!(
        parse(r#"-from:bob -"out of office" -is:read -spam"#),
        vec![
            not(Condition::From("bob".into())),
            not(Condition::Text("out of office".into())),
            not(Condition::Is(Status::Read)),
            not(Condition::Text("spam".into())),
        ]
    );
    // A lone `-` negates nothing; a double one negates the text `-x`.
    assert_eq!(parse("a - b"), parse("a b"));
    assert_eq!(parse("--x"), vec![not(Condition::Text("-x".into()))]);
}

#[test]
fn unknown_prefixes_are_plain_text() {
    assert_eq!(
        parse("re:plans https://example.org/a:b"),
        vec![
            term(Condition::Text("re:plans".into())),
            term(Condition::Text("https://example.org/a:b".into())),
        ]
    );
    // Only the first `:` separates a field; the value keeps the rest.
    assert_eq!(parse("in:a:b"), vec![term(Condition::In("a:b".into()))]);
}

#[test]
fn errors_name_the_column_and_the_fix() {
    let err = error(r#"from:alice "unclosed"#);
    assert_eq!(err.column, 12);
    assert!(err.reason.contains("unterminated"), "{err}");

    let err = error("from: alice");
    assert_eq!(err.column, 6);
    assert!(err.reason.contains("needs a value"), "{err}");

    let err = error("is:important");
    assert_eq!(err.column, 4);
    assert!(err.reason.contains("is:unread"), "{err}");

    let err = error("has:pdf");
    assert!(err.reason.contains("has:attachment"), "{err}");

    let err = error(r#""""#);
    assert!(err.reason.contains("empty phrase"), "{err}");

    let err = error(r#""a"b"#);
    assert_eq!(err.column, 4);
    assert!(err.reason.contains("space after"), "{err}");

    for bad in [
        "after:2024-13-01",
        "after:2023-02-29",
        "before:24-01-01",
        "before:2024-1",
        "before:yesterday",
    ] {
        let err = error(bad);
        assert!(err.reason.contains("date"), "{bad}: {err}");
    }
    assert_eq!(
        error("before:yesterday").to_string(),
        "at column 8: invalid date `yesterday`; write YYYY-MM-DD"
    );
}

#[test]
fn columns_count_characters() {
    assert_eq!(error("é ü is:x").column, 8);
}

#[test]
fn dates_start_at_utc_midnight() {
    assert_eq!(date(1970, 1, 1).start_ms(), 0);
    assert_eq!(date(2023, 11, 14).start_ms(), 1_699_920_000_000);
    assert_eq!(date(2000, 3, 1).start_ms(), 951_868_800_000);
    assert_eq!(date(1969, 12, 31).start_ms(), -86_400_000);
    assert_eq!(date(2024, 2, 29).to_string(), "2024-02-29");
    assert_eq!(Date::new(1900, 2, 29), None);
    assert_eq!(Date::new(2000, 2, 29), Some(date(2000, 2, 29)));
    assert_eq!(Date::new(0, 1, 1), None);
}

#[test]
fn canonical_form_quotes_where_needed() {
    let query = Query::parse(
        r#"FROM:"Alice" -"a b" "-x" "from:y" re:z subject:"say \"hi\"" after:2023/1/5 HAS:attachment"#,
    )
    .unwrap();
    assert_eq!(
        query.to_string(),
        r#"from:Alice -"a b" "-x" "from:y" "re:z" subject:"say \"hi\"" after:2023-01-05 has:attachment"#
    );
}

#[test]
fn queries_serialize_as_tagged_terms() {
    let query = Query::parse("-from:bob is:unread has:attachment").unwrap();
    assert_eq!(
        serde_json::to_value(&query).unwrap(),
        serde_json::json!({ "terms": [
            { "negated": true, "condition": { "field": "from", "value": "bob" } },
            { "condition": { "field": "is", "value": "unread" } },
            { "condition": { "field": "has_attachment" } },
        ] })
    );
}

fn text() -> impl Strategy<Value = String> {
    proptest::string::string_regex(r#"[a-zé:\-" \\/@.]{1,8}"#).unwrap()
}

fn condition() -> impl Strategy<Value = Condition> {
    let date = (1_u16..=9999, 1_u8..=12, 1_u8..=28).prop_map(|(y, m, d)| date(y, m, d));
    prop_oneof![
        text().prop_map(Condition::Text),
        text().prop_map(Condition::From),
        text().prop_map(Condition::To),
        text().prop_map(Condition::Cc),
        text().prop_map(Condition::Subject),
        text().prop_map(Condition::In),
        prop_oneof![
            Just(Status::Unread),
            Just(Status::Read),
            Just(Status::Starred)
        ]
        .prop_map(Condition::Is),
        Just(Condition::HasAttachment),
        date.clone().prop_map(Condition::Before),
        date.prop_map(Condition::After),
    ]
}

proptest! {
    #[test]
    fn canonical_form_parses_back(terms in proptest::collection::vec(
        (any::<bool>(), condition()).prop_map(|(negated, condition)| Term { negated, condition }),
        0..6,
    )) {
        let query = Query { terms };
        let printed = query.to_string();
        prop_assert_eq!(Query::parse(&printed), Ok(query), "printed as {}", printed);
    }

    #[test]
    fn parsing_never_panics(input in "\\PC{0,40}") {
        if let Ok(query) = Query::parse(&input) {
            prop_assert_eq!(Query::parse(&query.to_string()), Ok(query));
        }
    }
}
