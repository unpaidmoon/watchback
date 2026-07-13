use owstats_desktop_lib::player_target::{target_matches_login_role, PlayerTarget};

#[test]
fn target_only_matches_the_login_role_that_created_it() {
    let target: PlayerTarget = serde_json::from_value(serde_json::json!({
        "inputBnet": "Tester#12345",
        "resolvedBnet": "Tester#12345",
        "customerToken": "synthetic-player-key",
        "boundAt": "2026-07-14T00:00:00Z",
        "sourceRoleId": 101
    }))
    .unwrap();

    assert_eq!(target.source_role_id, Some(101));
    assert!(target_matches_login_role(&target, 101));
    assert!(!target_matches_login_role(&target, 202));
}

#[test]
fn legacy_target_without_source_role_is_not_verified_for_any_login() {
    let target: PlayerTarget = serde_json::from_value(serde_json::json!({
        "inputBnet": "Legacy#12345",
        "resolvedBnet": null,
        "customerToken": "synthetic-legacy-player-key",
        "boundAt": "2026-07-03T00:00:00Z"
    }))
    .unwrap();

    assert_eq!(target.source_role_id, None);
    assert!(!target_matches_login_role(&target, 101));
    assert!(!target_matches_login_role(&target, 202));
}

#[test]
fn source_role_id_serializes_with_camel_case_field_name() {
    let target = PlayerTarget {
        input_bnet: "Tester#12345".to_string(),
        resolved_bnet: Some("Tester#12345".to_string()),
        customer_token: "synthetic-player-key".to_string(),
        bound_at: "2026-07-14T00:00:00Z".to_string(),
        source_role_id: Some(101),
    };

    let value = serde_json::to_value(target).unwrap();

    assert_eq!(
        value.get("sourceRoleId").and_then(|value| value.as_i64()),
        Some(101)
    );
    assert!(value.get("source_role_id").is_none());
}
