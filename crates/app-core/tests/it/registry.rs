//! Registry tests: completeness, names, schemas.

use std::collections::BTreeSet;

use app_core::registry::{command_schema, schema_command_names};
use app_core::{AgentAccess, AppState, Command, CommandKind, CommandName};

#[test]
fn registry_is_complete() {
    let schema_names: BTreeSet<String> = schema_command_names().into_iter().collect();
    let registry_names: BTreeSet<String> = Command::specs()
        .iter()
        .map(|spec| spec.name.to_owned())
        .collect();
    assert!(!schema_names.is_empty());
    assert_eq!(schema_names, registry_names);
    assert_eq!(
        registry_names.len(),
        CommandName::ALL.len(),
        "duplicate in ALL"
    );
}

#[test]
fn examples_match_their_names_and_round_trip() {
    for &name in CommandName::ALL {
        let example = name.example();
        assert_eq!(example.command_name(), name);
        let json = serde_json::to_value(&example).unwrap();
        assert_eq!(json["cmd"], name.as_str());
        assert_eq!(Command::from_value(json).unwrap(), example);
        assert_eq!(CommandName::from_name(name.as_str()), Some(name));
    }
}

#[test]
fn every_command_has_description_title_and_object_args_schema() {
    for spec in Command::specs() {
        assert!(!spec.title.is_empty(), "{} has no title", spec.name);
        assert!(
            !spec.description().is_empty(),
            "{} has no doc comment",
            spec.name
        );
        let name = CommandName::from_name(spec.name).unwrap();
        assert_eq!(name.args_schema()["type"], "object", "{}", spec.name);
    }
}

#[test]
fn set_setting_args_schema_lists_keys() {
    let schema = CommandName::SetSetting.args_schema();
    let keys = schema.pointer("/properties/key").unwrap().to_string();
    assert!(keys.contains("poll_secs"), "{keys}");
    assert!(schema.pointer("/properties/value").is_some());
}

#[test]
fn kinds_follow_the_catalogue() {
    assert_eq!(CommandName::GetSettings.kind(), CommandKind::Query);
    assert_eq!(CommandName::SetSetting.kind(), CommandKind::Mutation);
    assert_eq!(CommandName::OpenPalette.kind(), CommandKind::View);
}

#[test]
fn the_agent_policy_is_pinned() {
    let with = |access| -> Vec<&str> {
        CommandName::ALL
            .iter()
            .filter(|name| name.agent_access() == access)
            .map(|name| name.as_str())
            .collect()
    };
    assert_eq!(
        with(AgentAccess::UserOnly),
        [
            "add_account",
            "update_account",
            "set_password",
            "set_identity",
            "remove_identity",
            "save_attachments",
            "restore_draft",
            "approve_request",
            "reject_request"
        ]
    );
    assert_eq!(with(AgentAccess::Approval), ["send_draft"]);
    for &name in CommandName::ALL {
        let spec = name.spec();
        assert_eq!(spec.agents, name.agent_access(), "{}", spec.name);
        assert_eq!(
            spec.kind == CommandKind::Outbound,
            spec.agents == AgentAccess::Approval,
            "{}: agents' sends wait for approval",
            spec.name
        );
        if spec.agents == AgentAccess::UserOnly {
            assert!(
                spec.description().contains("origin_not_allowed"),
                "{}: the description tells agents",
                spec.name
            );
        }
    }
}

#[test]
fn palette_availability_depends_on_view() {
    let mut state = AppState::default();
    assert!((CommandName::OpenPalette.spec().available)(&state));
    assert!(!(CommandName::ClosePalette.spec().available)(&state));
    state.view.palette_open = true;
    assert!(!(CommandName::OpenPalette.spec().available)(&state));
    assert!((CommandName::ClosePalette.spec().available)(&state));
}

#[test]
fn command_schema_snapshot() {
    insta::assert_json_snapshot!("command_schema", command_schema());
}
