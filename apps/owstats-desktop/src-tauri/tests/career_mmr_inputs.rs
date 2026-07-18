use owstats_desktop_lib::career_mmr::input::{
    audit_career_mmr_inputs, classify_stadium_stat_shape, extract_ordinary_match,
    extract_ordinary_match_with_context, extract_stadium_match, extract_stadium_match_with_context,
    normalize_mode_family, normalize_role, ExtractionContext, ModeFamily, Role, StadiumStatShape,
    TrainingCohort,
};
use owstats_desktop_lib::OwDatabase;
use rusqlite::{Connection, OpenFlags};
use tempfile::tempdir;

fn database() -> (tempfile::TempDir, Connection) {
    let dir = tempdir().unwrap();
    let db = OwDatabase::new(dir.path().join("audit.sqlite3"));
    db.initialize().unwrap();
    let conn = db.connect().unwrap();
    (dir, conn)
}

#[test]
fn maps_every_upstream_mode_to_a_distinct_actual_mode() {
    for (game_mode, instance_type, expected) in [
        ("SportPreset", None, ModeFamily::RoleCompetitive),
        ("sport", Some("IT_RANKED"), ModeFamily::RoleCompetitive),
        ("LeisurePreset", None, ModeFamily::RoleQuick),
        ("leisure", None, ModeFamily::RoleQuick),
        ("SportOpen", None, ModeFamily::OpenCompetitive),
        ("sport", Some("IT_RULESET"), ModeFamily::OpenCompetitive),
        ("Leisure6v6", None, ModeFamily::OpenQuick),
        ("SportFight", None, ModeFamily::StadiumCompetitive),
        ("LeisureFight", None, ModeFamily::StadiumQuick),
        ("QuickFight", None, ModeFamily::StadiumQuick),
    ] {
        assert_eq!(
            normalize_mode_family(game_mode, instance_type),
            Some(expected),
            "{game_mode} {instance_type:?}"
        );
    }
    assert_ne!(ModeFamily::RoleQuick, ModeFamily::OpenQuick);
}

#[test]
fn modes_have_stable_names_training_cohorts_and_training_flags() {
    let cases = [
        (
            ModeFamily::RoleCompetitive,
            "role_competitive",
            TrainingCohort::RoleQueue,
            false,
            true,
        ),
        (
            ModeFamily::RoleQuick,
            "role_quick",
            TrainingCohort::RoleQueue,
            true,
            true,
        ),
        (
            ModeFamily::OpenCompetitive,
            "open_competitive",
            TrainingCohort::OpenQueue,
            false,
            true,
        ),
        (
            ModeFamily::OpenQuick,
            "open_quick",
            TrainingCohort::OpenQueue,
            true,
            true,
        ),
        (
            ModeFamily::StadiumCompetitive,
            "stadium_competitive",
            TrainingCohort::StadiumCompetitive,
            false,
            true,
        ),
        (
            ModeFamily::StadiumQuick,
            "stadium_quick",
            TrainingCohort::OutcomeOnly,
            true,
            false,
        ),
    ];
    for (mode, name, cohort, quick, trains_performance) in cases {
        assert_eq!(mode.as_str(), name);
        assert_eq!(mode.training_cohort(), cohort);
        assert_eq!(mode.is_quick(), quick);
        assert_eq!(mode.trains_performance(), trains_performance);
    }

    let mut modes = cases.map(|case| case.0);
    modes.sort();
    let serialized = serde_json::to_string(&modes).unwrap();
    assert_eq!(
        serialized,
        "[\"role_competitive\",\"role_quick\",\"open_competitive\",\"open_quick\",\"stadium_competitive\",\"stadium_quick\"]"
    );
}

fn audit_read_only(
    conn: Connection,
) -> owstats_desktop_lib::career_mmr::input::CareerMmrInputAudit {
    let path = conn.path().unwrap().to_owned();
    drop(conn);
    let conn = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY).unwrap();
    audit_career_mmr_inputs(&conn).unwrap()
}

fn insert_match(conn: &Connection, id: &str, season: i64, bucket: &str, raw: &str) {
    let hero_guid = serde_json::from_str::<serde_json::Value>(raw)
        .ok()
        .and_then(|value| {
            value
                .get("heroGuid")
                .and_then(|item| item.as_str())
                .map(str::to_owned)
        })
        .unwrap_or_else(|| "self-hero".to_string());
    conn.execute(
        "INSERT INTO matches (match_id, season, bucket, game_mode, match_kind, result, map_guid, hero_guid, begin_ts, team_score, opponent_score, kills, assists, deaths, damage, healing, mitigation, total_worth, duration_sec, raw_entry_json, updated_at) VALUES (?1, ?2, ?3, 'SportPreset', 'normal', 'win', 'map', ?4, 1, 1, 0, 0, 0, 0, 0, 0, 0, 0, 600, ?5, 'now')",
        (id, season, bucket, hero_guid, raw),
    ).unwrap();
}

#[allow(clippy::too_many_arguments)]
fn insert_player(
    conn: &Connection,
    match_id: &str,
    season: i64,
    bnet_id: &str,
    hero_guid: &str,
    role_type: Option<&str>,
    healing_taken: i64,
    final_hit: i64,
) {
    conn.execute(
        "INSERT INTO match_players (match_id, season, side, bnet_id, player_name, hero_guid, is_current_player, healing_taken, final_hit, role_type, raw_json, updated_at) VALUES (?1, ?2, 'teammate', ?3, 'private name', ?4, 1, ?5, ?6, ?7, '{}', 'now')",
        (match_id, season, bnet_id, hero_guid, healing_taken, final_hit, role_type),
    ).unwrap();
}

fn insert_stadium(conn: &Connection, id: &str, entry: serde_json::Value, data: serde_json::Value) {
    insert_match(conn, id, 23, "stadium", &entry.to_string());
    conn.execute(
        "UPDATE matches SET game_mode='SportFight', match_kind='arena', result='loss', duration_sec=900 WHERE match_id=?1",
        [id],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO match_details (match_id, match_kind, raw_detail_json, updated_at) VALUES (?1, 'arena', ?2, 'now')",
        rusqlite::params![id, serde_json::json!({"payload":{"data":data}}).to_string()],
    )
    .unwrap();
    insert_player(conn, id, 23, "current-local", "entry-hero", None, 0, 0);
}

fn stadium_round(
    round_index: Option<i64>,
    start_time: Option<i64>,
    hero: &str,
    final_hit: i64,
) -> serde_json::Value {
    let mut round = serde_json::json!({
        "teammateList":[{"bnetId":"current-local","heroGuid":hero,"finalHit":final_hit}],
        "enemyList":[]
    });
    if let Some(value) = round_index {
        round["roundIndex"] = value.into();
    }
    if let Some(value) = start_time {
        round["startTime"] = value.into();
    }
    round
}

fn install_stadium_roles(conn: &Connection) {
    for (guid, role) in [
        ("tank-hero", "tank"),
        ("damage-hero", "dps"),
        ("support-hero", "healer"),
        ("entry-hero", "tank"),
    ] {
        conn.execute(
            "INSERT INTO hero_dict (guid, name, role, updated_at) VALUES (?1, 'safe', ?2, 'now')",
            (guid, role),
        )
        .unwrap();
    }
}

#[test]
fn stadium_uses_only_last_valid_round_role_and_performance() {
    let (_dir, conn) = database();
    install_stadium_roles(&conn);
    insert_stadium(
        &conn,
        "stadium-last",
        serde_json::json!({"roleType":"tank"}),
        serde_json::json!({
            "totalCount":{"rankInfo":{"rankScore":350}},
            "roundCountList":[
                stadium_round(Some(1), Some(100), "tank-hero", 2),
                stadium_round(Some(2), Some(200), "damage-hero", 4),
                stadium_round(Some(3), Some(300), "support-hero", 7),
                "malformed"
            ]
        }),
    );

    let input = extract_stadium_match(&conn, "stadium-last")
        .unwrap()
        .unwrap();
    assert_eq!(input.mode_family, ModeFamily::StadiumCompetitive);
    assert_eq!(input.result, 0.0);
    assert_eq!(input.role, Role::Support);
    assert_eq!(input.role_share, 1.0);
    assert_eq!(input.role_source, "stadium_last_round");
    assert_eq!(input.performance_features["finalHit"], 7);
    assert_ne!(input.performance_features["finalHit"], 2 + 4 + 7);
}

#[test]
fn stadium_missing_current_player_rounds_never_borrows_previous_performance() {
    let (_dir, conn) = database();
    install_stadium_roles(&conn);
    insert_stadium(
        &conn,
        "stadium-outcome-only",
        serde_json::json!({"roleType":"tank"}),
        serde_json::json!({
            "roundCountList":[{
                "roundIndex":1,
                "teammateList":[{"bnetId":"someone-else","heroGuid":"support-hero","finalHit":99}]
            }]
        }),
    );

    let input = extract_stadium_match(&conn, "stadium-outcome-only")
        .unwrap()
        .unwrap();
    assert_eq!(input.role, Role::Tank);
    assert!(input.performance_features.as_object().unwrap().is_empty());
    assert!(input.data_quality < 0.5);

    conn.execute(
        "UPDATE matches SET raw_entry_json='{}' WHERE match_id='stadium-outcome-only'",
        [],
    )
    .unwrap();
    assert!(extract_stadium_match(&conn, "stadium-outcome-only")
        .unwrap()
        .is_none());
}

#[test]
fn stadium_duplicate_current_player_markers_fail_closed() {
    let (_dir, conn) = database();
    install_stadium_roles(&conn);
    insert_stadium(
        &conn,
        "stadium-duplicate-current",
        serde_json::json!({"roleType":"tank"}),
        serde_json::json!({
            "roundCountList":[{
                "roundIndex":1,
                "teammateList":[
                    {"bnetId":"current-local","heroGuid":"support-hero","finalHit":7},
                    {"bnetId":"duplicate-current","heroGuid":"damage-hero","finalHit":99}
                ],
                "enemyList":[]
            }]
        }),
    );
    insert_player(
        &conn,
        "stadium-duplicate-current",
        23,
        "duplicate-current",
        "damage-hero",
        Some("damage"),
        0,
        99,
    );

    let context = ExtractionContext::build(&conn).unwrap();
    assert!(
        extract_stadium_match_with_context(&conn, "stadium-duplicate-current", &context)
            .unwrap()
            .is_none()
    );
    assert!(extract_stadium_match(&conn, "stadium-duplicate-current")
        .unwrap()
        .is_none());
}

#[test]
fn stadium_blank_or_padded_current_player_keys_fail_closed() {
    for (id, invalid_key) in [
        ("stadium-empty-current", ""),
        ("stadium-space-current", "   "),
        ("stadium-padded-current", " current-local "),
    ] {
        let (_dir, conn) = database();
        install_stadium_roles(&conn);
        insert_stadium(
            &conn,
            id,
            serde_json::json!({"roleType":"tank"}),
            serde_json::json!({
                "roundCountList":[stadium_round(Some(1), None, "support-hero", 7)]
            }),
        );
        conn.execute(
            "UPDATE match_players SET bnet_id=?1 WHERE match_id=?2 AND is_current_player=1",
            (invalid_key, id),
        )
        .unwrap();

        let context = ExtractionContext::build(&conn).unwrap();
        assert!(extract_stadium_match_with_context(&conn, id, &context)
            .unwrap()
            .is_none());
        assert!(extract_stadium_match(&conn, id).unwrap().is_none());
    }
}

#[test]
fn stadium_structurally_valid_final_round_without_current_player_does_not_borrow_previous_round() {
    let (_dir, conn) = database();
    install_stadium_roles(&conn);
    insert_stadium(
        &conn,
        "stadium-final-without-current",
        serde_json::json!({"roleType":"tank"}),
        serde_json::json!({
            "roundCountList":[
                stadium_round(Some(1), Some(100), "support-hero", 41),
                {
                    "roundIndex":2,
                    "startTime":200,
                    "teammateList":[{"bnetId":"someone-else","heroGuid":"damage-hero","finalHit":9}],
                    "enemyList":[]
                }
            ]
        }),
    );

    let input = extract_stadium_match(&conn, "stadium-final-without-current")
        .unwrap()
        .unwrap();
    assert_eq!(input.role, Role::Tank);
    assert!(input.performance_features.as_object().unwrap().is_empty());
    assert_ne!(input.performance_features["finalHit"], 41);
    assert!(input.data_quality < 0.5);
}

#[test]
fn stadium_outcome_only_preserves_complete_total_count_strengths() {
    let (_dir, conn) = database();
    install_stadium_roles(&conn);
    insert_stadium(
        &conn,
        "stadium-outcome-strength",
        serde_json::json!({"roleType":"tank","rankInfo":{"rankScore":999}}),
        serde_json::json!({
            "totalCount":{
                "teammateList":[
                    {"bnetId":"current-local","rankInfo":{"rankScore":300}},
                    {"bnetId":"team-2","rankInfo":{"rankScore":340}}
                ],
                "enemyList":[
                    {"bnetId":"enemy-1","rankInfo":{"rankScore":380}},
                    {"bnetId":"enemy-2","rankInfo":{"rankScore":420}}
                ]
            },
            "roundCountList":[]
        }),
    );

    let input = extract_stadium_match(&conn, "stadium-outcome-strength")
        .unwrap()
        .unwrap();
    assert_eq!(input.team_strength, Some(320.0));
    assert_eq!(input.opponent_strength, Some(400.0));
    assert_eq!(input.rank_coverage, 1.0);
    assert!(input.performance_features.as_object().unwrap().is_empty());
}

#[test]
fn stadium_single_entry_rank_does_not_fabricate_two_sided_strength() {
    let (_dir, conn) = database();
    install_stadium_roles(&conn);
    insert_stadium(
        &conn,
        "stadium-single-entry-rank",
        serde_json::json!({"roleType":"tank","rankInfo":{"rankScore":999}}),
        serde_json::json!({"roundCountList":[]}),
    );

    let input = extract_stadium_match(&conn, "stadium-single-entry-rank")
        .unwrap()
        .unwrap();
    assert_eq!(input.team_strength, None);
    assert_eq!(input.opponent_strength, None);
    assert_eq!(input.rank_coverage, 0.0);
}

#[test]
fn stadium_ordering_prioritizes_round_index_then_time_then_array_order() {
    for (id, rounds, expected) in [
        (
            "by-index",
            vec![
                stadium_round(Some(3), None, "support-hero", 3),
                stadium_round(None, Some(9999), "damage-hero", 9),
                stadium_round(Some(2), Some(1), "tank-hero", 2),
            ],
            Role::Support,
        ),
        (
            "by-time",
            vec![
                stadium_round(None, Some(300), "damage-hero", 3),
                stadium_round(None, Some(200), "tank-hero", 2),
            ],
            Role::Damage,
        ),
        (
            "by-array",
            vec![
                stadium_round(None, None, "tank-hero", 1),
                stadium_round(None, None, "support-hero", 2),
            ],
            Role::Support,
        ),
    ] {
        let (_dir, conn) = database();
        install_stadium_roles(&conn);
        insert_stadium(
            &conn,
            id,
            serde_json::json!({}),
            serde_json::json!({"roundCountList":rounds}),
        );
        assert_eq!(
            extract_stadium_match(&conn, id).unwrap().unwrap().role,
            expected
        );
    }
}

#[test]
fn stadium_stat_shape_distinguishes_local_cumulative_and_ambiguous() {
    let local = vec![
        stadium_round(Some(1), None, "tank-hero", 8),
        stadium_round(Some(2), None, "tank-hero", 3),
    ];
    let cumulative = vec![
        stadium_round(Some(1), None, "tank-hero", 3),
        stadium_round(Some(2), None, "tank-hero", 8),
    ];
    let mut ambiguous = cumulative.clone();
    ambiguous[0]["teammateList"][0]["kill"] = 8.into();
    ambiguous[1]["teammateList"][0]["kill"] = 3.into();

    assert_eq!(
        classify_stadium_stat_shape(&local, "current-local"),
        StadiumStatShape::RoundLocal
    );
    assert_eq!(
        classify_stadium_stat_shape(&cumulative, "current-local"),
        StadiumStatShape::Cumulative
    );
    assert_eq!(
        classify_stadium_stat_shape(&ambiguous, "current-local"),
        StadiumStatShape::Ambiguous
    );
}

#[test]
fn stadium_stat_shape_orders_rounds_before_testing_monotonicity() {
    let rounds = vec![
        stadium_round(Some(3), Some(300), "tank-hero", 9),
        stadium_round(Some(1), Some(100), "tank-hero", 3),
        stadium_round(Some(2), Some(200), "tank-hero", 6),
    ];

    assert_eq!(
        classify_stadium_stat_shape(&rounds, "current-local"),
        StadiumStatShape::Cumulative
    );
}

#[test]
fn extraction_context_is_explicitly_shared_across_multiple_matches() {
    let (_dir, conn) = database();
    install_stadium_roles(&conn);
    insert_match(
        &conn,
        "ordinary-shared-context",
        23,
        "normal",
        r#"{"heroGuid":"tank-hero"}"#,
    );
    insert_stadium(
        &conn,
        "stadium-shared-context",
        serde_json::json!({}),
        serde_json::json!({
            "roundCountList":[stadium_round(Some(1), None, "support-hero", 2)]
        }),
    );

    let context = ExtractionContext::build(&conn).unwrap();
    let ordinary = extract_ordinary_match_with_context(&conn, "ordinary-shared-context", &context)
        .unwrap()
        .unwrap();
    let stadium = extract_stadium_match_with_context(&conn, "stadium-shared-context", &context)
        .unwrap()
        .unwrap();
    assert_eq!(ordinary.role, Role::Tank);
    assert_eq!(stadium.role, Role::Support);
}

#[test]
fn stadium_audit_exposes_only_aggregate_shape_counts() {
    let (_dir, conn) = database();
    install_stadium_roles(&conn);
    for (id, rounds) in [
        (
            "local-shape",
            vec![
                stadium_round(Some(1), None, "tank-hero", 8),
                stadium_round(Some(2), None, "tank-hero", 3),
            ],
        ),
        (
            "cumulative-shape",
            vec![
                stadium_round(Some(1), None, "tank-hero", 3),
                stadium_round(Some(2), None, "tank-hero", 8),
            ],
        ),
        (
            "ambiguous-shape",
            vec![stadium_round(Some(1), None, "tank-hero", 3)],
        ),
    ] {
        insert_stadium(
            &conn,
            id,
            serde_json::json!({"roleType":"tank"}),
            serde_json::json!({"roundCountList":rounds}),
        );
    }

    let audit = audit_read_only(conn);
    assert_eq!(audit.stadium_round_local_matches, 1);
    assert_eq!(audit.stadium_cumulative_matches, 1);
    assert_eq!(audit.stadium_ambiguous_matches, 1);
}

#[test]
fn normalizes_upstream_roles_without_guessing_from_stats() {
    assert_eq!(normalize_role(Some("tank")), Some(Role::Tank));
    assert_eq!(normalize_role(Some("DPS")), Some(Role::Damage));
    assert_eq!(normalize_role(Some("damage")), Some(Role::Damage));
    assert_eq!(normalize_role(Some("healer")), Some(Role::Support));
    assert_eq!(normalize_role(Some(" support ")), Some(Role::Support));
    assert_eq!(normalize_role(Some("open")), None);
    assert_eq!(normalize_role(Some("unknown")), None);
    assert_eq!(normalize_role(None), None);
}

#[test]
fn audit_does_not_guess_support_from_high_healing() {
    let (_dir, conn) = database();
    insert_match(&conn, "no-role", 29, "normal", "{}");
    insert_player(
        &conn,
        "no-role",
        29,
        "private-1",
        "unknown-hero",
        None,
        999_999,
        0,
    );

    let audit = audit_read_only(conn);
    assert_eq!(audit.mapped_player_rows, 0);
    assert_eq!(audit.unmapped_player_rows, 1);
}

#[test]
fn audit_counts_aggregate_coverage_and_discovers_seasons() {
    let (_dir, conn) = database();
    insert_match(
        &conn,
        "ordinary",
        21,
        "normal",
        r#"{"heroGuid":"summary-hero","roleType":"dps"}"#,
    );
    insert_match(&conn, "stadium", 47, "stadium", "{}");
    insert_player(
        &conn,
        "ordinary",
        21,
        "private-1",
        "explicit",
        Some("tank"),
        10,
        2,
    );
    insert_player(
        &conn,
        "ordinary",
        21,
        "private-2",
        "summary-hero",
        None,
        0,
        0,
    );
    insert_player(&conn, "stadium", 47, "private-3", "dict-hero", None, 30, 1);
    insert_player(&conn, "stadium", 47, "private-4", "unknown", None, 0, 0);
    conn.execute(
        "INSERT INTO hero_dict (guid, name, role, updated_at) VALUES ('dict-hero', 'safe', 'healer', 'now')",
        [],
    ).unwrap();

    let audit = audit_read_only(conn);
    assert_eq!(audit.matches, 2);
    assert_eq!(audit.ordinary_matches, 1);
    assert_eq!(audit.stadium_matches, 1);
    assert_eq!(audit.player_rows, 4);
    assert_eq!(audit.mapped_player_rows, 3);
    assert_eq!(audit.unmapped_player_rows, 1);
    assert_eq!(audit.final_hit_rows, 2);
    assert_eq!(audit.healing_taken_rows, 2);
    assert_eq!(audit.discovered_seasons, vec![21, 47]);
}

#[test]
fn serialized_audit_contains_no_sensitive_fields_or_values() {
    let (_dir, conn) = database();
    insert_match(&conn, "safe", 22, "normal", "{}");
    insert_player(
        &conn,
        "safe",
        22,
        "secret-bnet",
        "secret-guid",
        Some("tank"),
        1,
        1,
    );

    let json = serde_json::to_string(&audit_read_only(conn)).unwrap();
    let lower = json.to_ascii_lowercase();
    for forbidden in [
        "bnet",
        "bnetid",
        "customertoken",
        "authorization",
        "playerid",
        "playername",
        "guid",
        "rawjson",
        "token",
        "cookie",
        "credential",
        "secret-bnet",
        "secret-guid",
    ] {
        assert!(
            !lower.contains(forbidden),
            "audit leaked {forbidden}: {json}"
        );
    }
}

#[test]
fn conflicting_summary_roles_are_unmapped_regardless_of_insert_order() {
    for (first, second) in [("tank", "dps"), ("dps", "tank")] {
        let (_dir, conn) = database();
        insert_match(
            &conn,
            "first",
            22,
            "normal",
            &format!(r#"{{"heroGuid":"conflict-hero","roleType":"{first}"}}"#),
        );
        insert_match(
            &conn,
            "second",
            23,
            "normal",
            &format!(r#"{{"heroGuid":"conflict-hero","roleType":"{second}"}}"#),
        );
        insert_player(
            &conn,
            "first",
            22,
            "private-conflict",
            "conflict-hero",
            None,
            0,
            0,
        );

        let audit = audit_read_only(conn);
        assert_eq!(audit.mapped_player_rows, 0);
        assert_eq!(audit.unmapped_player_rows, 1);
    }
}

#[test]
fn unrelated_nested_role_objects_are_not_summary_mappings() {
    let (_dir, conn) = database();
    insert_match(
        &conn,
        "nested",
        22,
        "normal",
        r#"{"metadata":{"heroGuid":"nested-hero","roleType":"healer"}}"#,
    );
    insert_player(
        &conn,
        "nested",
        22,
        "private-nested",
        "nested-hero",
        None,
        999_999,
        0,
    );

    let audit = audit_read_only(conn);
    assert_eq!(audit.mapped_player_rows, 0);
    assert_eq!(audit.unmapped_player_rows, 1);
}

#[test]
fn ordinary_entry_role_is_exact_and_final_hero_is_low_quality_fallback() {
    let (_dir, conn) = database();
    insert_match(
        &conn,
        "entry-role",
        22,
        "normal",
        r#"{"heroGuid":"tank-hero","roleType":"tank"}"#,
    );
    insert_match(
        &conn,
        "damage-mapping",
        21,
        "normal",
        r#"{"heroGuid":"damage-hero","roleType":"dps"}"#,
    );
    insert_match(
        &conn,
        "final-hero",
        22,
        "normal",
        r#"{"heroGuid":"damage-hero"}"#,
    );

    let exact = extract_ordinary_match(&conn, "entry-role")
        .unwrap()
        .unwrap();
    assert_eq!(exact.role, Role::Tank);
    assert_eq!(exact.role_share, 1.0);
    assert_eq!(exact.role_source, "entry_role");
    assert_eq!(exact.data_quality, 1.0);

    let fallback = extract_ordinary_match(&conn, "final-hero")
        .unwrap()
        .unwrap();
    assert_eq!(fallback.role, Role::Damage);
    assert_eq!(fallback.role_share, 1.0);
    assert_eq!(fallback.role_source, "final_hero");
    assert_eq!(fallback.data_quality, 0.55);
    assert!(fallback
        .performance_features
        .as_object()
        .unwrap()
        .is_empty());
}

#[test]
fn ordinary_unknown_role_or_result_is_skipped() {
    let (_dir, conn) = database();
    insert_match(
        &conn,
        "unknown-role",
        22,
        "normal",
        r#"{"heroGuid":"unknown"}"#,
    );
    assert!(extract_ordinary_match(&conn, "unknown-role")
        .unwrap()
        .is_none());

    insert_match(
        &conn,
        "unknown-result",
        22,
        "normal",
        r#"{"roleType":"tank"}"#,
    );
    conn.execute(
        "UPDATE matches SET result='unknown' WHERE match_id='unknown-result'",
        [],
    )
    .unwrap();
    assert!(extract_ordinary_match(&conn, "unknown-result")
        .unwrap()
        .is_none());
}

#[test]
fn ordinary_mode_variants_follow_existing_product_semantics() {
    assert_eq!(
        normalize_mode_family("SportPreset", None),
        Some(ModeFamily::RoleCompetitive)
    );
    assert_eq!(
        normalize_mode_family("sportpreset", None),
        Some(ModeFamily::RoleCompetitive)
    );
    assert_eq!(
        normalize_mode_family("SportOpen", None),
        Some(ModeFamily::OpenCompetitive)
    );
    assert_eq!(
        normalize_mode_family("sportopen", None),
        Some(ModeFamily::OpenCompetitive)
    );
    assert_eq!(
        normalize_mode_family("LeisurePreset", None),
        Some(ModeFamily::RoleQuick)
    );
    assert_eq!(
        normalize_mode_family("leisure6V6", None),
        Some(ModeFamily::OpenQuick)
    );
    assert_eq!(
        normalize_mode_family("sport", Some("IT_RANKED")),
        Some(ModeFamily::RoleCompetitive)
    );
    assert_eq!(
        normalize_mode_family("sport", Some("it_ruleset")),
        Some(ModeFamily::OpenCompetitive)
    );
    assert_eq!(normalize_mode_family("unknown", None), None);
}

#[test]
fn ordinary_strengths_are_none_when_rank_coverage_is_only_half() {
    let (_dir, conn) = database();
    insert_match(
        &conn,
        "partial-ranks",
        22,
        "normal",
        r#"{"roleType":"tank"}"#,
    );
    insert_player(
        &conn,
        "partial-ranks",
        22,
        "private-1",
        "tank-hero",
        Some("tank"),
        0,
        0,
    );
    insert_player(
        &conn,
        "partial-ranks",
        22,
        "private-2",
        "damage-hero",
        Some("dps"),
        0,
        0,
    );
    conn.execute(
        "UPDATE match_players SET raw_json=?1 WHERE match_id='partial-ranks' AND bnet_id='private-1'",
        [r#"{"rankInfo":{"rankScore":325}}"#],
    ).unwrap();

    let input = extract_ordinary_match(&conn, "partial-ranks")
        .unwrap()
        .unwrap();
    assert_eq!(input.rank_coverage, 0.5);
    assert_eq!(input.team_strength, None);
    assert_eq!(input.opponent_strength, None);
}

#[test]
fn ordinary_strengths_aggregate_fully_ranked_teammate_and_enemy_sides() {
    let (_dir, conn) = database();
    insert_match(&conn, "full-ranks", 22, "normal", r#"{"roleType":"tank"}"#);
    for (bnet_id, side, rank_score) in [
        ("private-team-1", "teammate", 300.0),
        ("private-team-2", "teammate", 340.0),
        ("private-enemy-1", "enemy", 360.0),
        ("private-enemy-2", "enemy", 400.0),
    ] {
        insert_player(
            &conn,
            "full-ranks",
            22,
            bnet_id,
            "tank-hero",
            Some("tank"),
            0,
            0,
        );
        conn.execute(
            "UPDATE match_players SET side=?1, raw_json=?2 WHERE match_id='full-ranks' AND bnet_id=?3",
            rusqlite::params![
                side,
                serde_json::json!({"rankInfo":{"rankScore":rank_score}}).to_string(),
                bnet_id
            ],
        )
        .unwrap();
    }

    let input = extract_ordinary_match(&conn, "full-ranks")
        .unwrap()
        .unwrap();
    assert_eq!(input.rank_coverage, 1.0);
    assert_eq!(input.team_strength, Some(320.0));
    assert_eq!(input.opponent_strength, Some(380.0));
}

#[test]
fn ordinary_strengths_use_rank_name_tier_anchors_and_prefer_rank_score() {
    let (_dir, conn) = database();
    insert_match(
        &conn,
        "rank-fallback",
        22,
        "normal",
        r#"{"roleType":"tank"}"#,
    );
    for (bnet_id, side, rank_info) in [
        (
            "team-name",
            "teammate",
            serde_json::json!({"rankName":"Gold","rankSubTier":3}),
        ),
        (
            "team-score",
            "team",
            serde_json::json!({"rankScore":325,"rankName":"Bronze","rankSubTier":5}),
        ),
        (
            "enemy-name",
            "enemy",
            serde_json::json!({"rank_name":"Diamond","rank_sub_tier":2}),
        ),
        (
            "enemy-score",
            "opponent",
            serde_json::json!({"rankScore":"420","rankName":"Champion","rankSubTier":1}),
        ),
    ] {
        insert_player(
            &conn,
            "rank-fallback",
            22,
            bnet_id,
            "hero",
            Some("tank"),
            0,
            0,
        );
        conn.execute(
            "UPDATE match_players SET side=?1, raw_json=?2 WHERE match_id='rank-fallback' AND bnet_id=?3",
            rusqlite::params![side, serde_json::json!({"rankInfo":rank_info}).to_string(), bnet_id],
        ).unwrap();
    }

    let input = extract_ordinary_match(&conn, "rank-fallback")
        .unwrap()
        .unwrap();
    assert_eq!(input.rank_coverage, 1.0);
    assert_eq!(input.team_strength, Some(292.5));
    assert_eq!(input.opponent_strength, Some(450.0));
}

#[test]
fn ordinary_strengths_reject_invalid_rank_sub_tier() {
    let (_dir, conn) = database();
    insert_match(
        &conn,
        "invalid-tier",
        22,
        "normal",
        r#"{"roleType":"tank"}"#,
    );
    for (bnet_id, side, tier) in [
        ("valid-team", "teammate", 3),
        ("valid-enemy", "enemy", 2),
        ("invalid-tier", "enemy", 6),
    ] {
        insert_player(
            &conn,
            "invalid-tier",
            22,
            bnet_id,
            "hero",
            Some("tank"),
            0,
            0,
        );
        conn.execute(
            "UPDATE match_players SET side=?1, raw_json=?2 WHERE match_id='invalid-tier' AND bnet_id=?3",
            rusqlite::params![side, serde_json::json!({"rankInfo":{"rankName":"Gold","rankSubTier":tier}}).to_string(), bnet_id],
        ).unwrap();
    }

    let input = extract_ordinary_match(&conn, "invalid-tier")
        .unwrap()
        .unwrap();
    assert!((input.rank_coverage - 2.0 / 3.0).abs() < f64::EPSILON);
    assert_eq!(input.team_strength, None);
    assert_eq!(input.opponent_strength, None);
}

#[test]
fn ordinary_strengths_are_invalidated_by_ranked_unknown_side() {
    let (_dir, conn) = database();
    insert_match(
        &conn,
        "unknown-side",
        22,
        "normal",
        r#"{"roleType":"tank"}"#,
    );
    for (bnet_id, side, score) in [
        ("valid-team", "teammate", 300),
        ("valid-enemy", "enemy", 400),
        ("unknown-side", "unknown", 350),
    ] {
        insert_player(
            &conn,
            "unknown-side",
            22,
            bnet_id,
            "hero",
            Some("tank"),
            0,
            0,
        );
        conn.execute(
            "UPDATE match_players SET side=?1, raw_json=?2 WHERE match_id='unknown-side' AND bnet_id=?3",
            rusqlite::params![side, serde_json::json!({"rankInfo":{"rankScore":score}}).to_string(), bnet_id],
        ).unwrap();
    }

    let input = extract_ordinary_match(&conn, "unknown-side")
        .unwrap()
        .unwrap();
    assert_eq!(input.rank_coverage, 1.0);
    assert_eq!(input.team_strength, None);
    assert_eq!(input.opponent_strength, None);
}
