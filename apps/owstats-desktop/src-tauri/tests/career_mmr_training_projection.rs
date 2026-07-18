use anyhow::Result;
use owstats_desktop_lib::{
    career_mmr::input::ModeFamily,
    career_mmr::training::{build_training_projection, RoleTrainingAudit},
    OwDatabase,
};
use rusqlite::{params, Connection};
use tempfile::TempDir;

fn fixture() -> Result<(TempDir, Connection)> {
    let dir = tempfile::tempdir()?;
    let db = OwDatabase::new(dir.path().join("test.db"));
    db.initialize()?;
    let conn = db.connect()?;
    conn.execute(
        "INSERT INTO hero_dict(guid,name,role,updated_at) VALUES('hero-a','匿名英雄','damage','now')",
        [],
    )?;
    conn.execute(
        "INSERT INTO matches(match_id,season,bucket,game_mode,match_kind,result,map_guid,hero_guid,begin_ts,team_score,opponent_score,kills,assists,deaths,damage,healing,mitigation,total_worth,duration_sec,raw_entry_json,updated_at) VALUES(?1,22,?2,?3,?4,'win','map','hero-a',100,1,0,9,8,7,6000,500,400,0,600,?5,'now')",
        params!["private-match", "preset", "SportPreset", "normal", r#"{"roleType":"damage","kill":9,"assist":8,"death":7,"heroDamage":6000,"cure":500,"resistDamage":400,"finalHit":6,"healingTaken":2000,"damageTaken":7000,"targetCompetingTime":45,"heroPlayTimeList":[{"heroGuid":"hero-a","durationSec":480}]}"#],
    )?;
    conn.execute(
        "INSERT INTO match_players(match_id,season,side,bnet_id,player_name,hero_guid,is_current_player,kill,assist,death,hero_damage,cure,resist_damage,healing_taken,damage_taken,final_hit,target_competing_time,role_type,raw_json,updated_at) VALUES(?1,22,'teammate','private-player','隐私名称','hero-a',1,9,8,7,6000,500,400,2000,7000,6,45,'damage',?2,'now')",
        params!["private-match", r#"{"finalHit":6,"healingTaken":2000,"damageTaken":7000,"targetCompetingTime":45,"selfHealing":500}"#],
    )?;
    Ok((dir, conn))
}

fn insert_stadium_match(
    conn: &Connection,
    match_id: &str,
    game_mode: &str,
    role_type: &str,
    rounds: serde_json::Value,
) -> Result<()> {
    conn.execute(
        "INSERT INTO hero_dict(guid,name,role,updated_at) VALUES('support-hero','safe','support','now') ON CONFLICT(guid) DO NOTHING",
        [],
    )?;
    conn.execute(
        "INSERT INTO matches(match_id,season,bucket,game_mode,match_kind,result,map_guid,hero_guid,begin_ts,team_score,opponent_score,kills,assists,deaths,damage,healing,mitigation,total_worth,duration_sec,raw_entry_json,updated_at) VALUES(?1,23,'stadium',?2,'arena','win','map','entry-hero',200,1,0,0,0,0,0,0,0,0,600,?3,'now')",
        params![match_id, game_mode, serde_json::json!({"roleType": role_type}).to_string()],
    )?;
    conn.execute(
        "INSERT INTO match_players(match_id,season,side,bnet_id,player_name,hero_guid,is_current_player,raw_json,updated_at) VALUES(?1,23,'teammate','current-local','safe','entry-hero',1,'{}','now')",
        [match_id],
    )?;
    conn.execute(
        "INSERT INTO match_details(match_id,match_kind,raw_detail_json,updated_at) VALUES(?1,'arena',?2,'now')",
        params![match_id, serde_json::json!({"payload":{"data":{"roundCountList":rounds}}}).to_string()],
    )?;
    Ok(())
}

fn insert_ordinary_mode(conn: &Connection, match_id: &str, game_mode: &str) -> Result<()> {
    conn.execute(
        "INSERT INTO matches(match_id,season,bucket,game_mode,match_kind,result,map_guid,hero_guid,begin_ts,team_score,opponent_score,kills,assists,deaths,damage,healing,mitigation,total_worth,duration_sec,raw_entry_json,updated_at) VALUES(?1,23,'preset',?2,'normal','win','map','hero-a',300,1,0,0,0,0,0,0,0,0,600,'{\"roleType\":\"damage\"}','now')",
        params![match_id, game_mode],
    )?;
    Ok(())
}

#[test]
fn projects_exact_hero_segments_and_external_healing_for_ordinary_matches() -> Result<()> {
    let (_dir, conn) = fixture()?;
    conn.execute(
        "INSERT INTO match_details(match_id,match_kind,raw_detail_json,updated_at) VALUES('private-match','normal',?1,'now')",
        [r#"{"payload":{"data":{"heroList":[{"heroId":"hero-a","userTimeSec":480,"useTimeRate":0.8,"statMap":{"603482350067646495":9,"603482350067646507":6,"603482350067648392":8,"603482350067646506":7,"603482350067647671":6000,"603482350067646913":500}}]}}}"#],
    )?;
    let projection = build_training_projection(&conn)?;
    assert_eq!(projection.rows.len(), 1);
    let row = &projection.rows[0];
    assert_eq!(row.season, 22);
    assert_eq!(row.role, "damage");
    assert_eq!(row.hero_duration_sec, Some(480.0));
    assert_eq!(row.final_hit, Some(6.0));
    assert_eq!(row.kill, Some(9.0));
    assert_eq!(row.assist, Some(8.0));
    assert_eq!(row.death, Some(7.0));
    assert_eq!(row.damage, Some(6000.0));
    assert_eq!(row.healing, Some(500.0));
    assert_eq!(row.healing_taken, Some(2000.0));
    assert!(row.hero_group.is_some());
    assert!(row.allow_performance_by_hero);
    assert!(row.allow_performance);
    assert_eq!(row.healing_taken_semantic, "external");
    assert!(row.usable_for_resource_debt);
    assert_eq!(projection.audit.performance_eligible_rows, 1);
    assert_eq!(projection.audit.resource_debt_eligible_rows, 1);
    assert_eq!(projection.audit.minor_segment_rows, 0);
    assert_eq!(row.expected_win_rate, 0.5);
    assert!(row.data_quality > 0.0);
    let serialized = serde_json::to_string(&projection.public_audit())?;
    for secret in ["private-match", "private-player", "隐私名称", "hero-a"] {
        assert!(!serialized.contains(secret));
    }
    Ok(())
}

#[test]
fn excludes_unrecognized_stadium_and_reports_role_coverage() -> Result<()> {
    let (_dir, conn) = fixture()?;
    conn.execute(
        "INSERT INTO matches(match_id,season,bucket,game_mode,match_kind,result,map_guid,hero_guid,begin_ts,team_score,opponent_score,kills,assists,deaths,damage,healing,mitigation,total_worth,duration_sec,raw_entry_json,updated_at) VALUES('stadium-private',22,'stadium','Stadium','fight','win','map','hero-a',200,1,0,1,1,1,1,1,1,0,100,'{}','now')",
        [],
    )?;
    let projection = build_training_projection(&conn)?;
    assert_eq!(projection.audit.stadium_ambiguous_excluded, 1);
    assert_eq!(projection.audit.ordinary_candidates, 1);
    assert_eq!(projection.audit.projected_rows, 1);
    assert_eq!(
        projection.audit.roles,
        vec![RoleTrainingAudit {
            role: "damage".into(),
            samples: 1
        }]
    );
    Ok(())
}

#[test]
fn stadium_competitive_projects_only_the_final_snapshot() -> Result<()> {
    let (_dir, conn) = fixture()?;
    insert_stadium_match(
        &conn,
        "stadium-competitive",
        "SportFight",
        "tank",
        serde_json::json!([
            {
                "roundIndex": 1,
                "startTime": 100,
                "teammateList": [{"bnetId":"current-local","heroGuid":"support-hero","finalHit":99}],
                "enemyList": []
            },
            "malformed",
            {
                "roundIndex": 2,
                "startTime": 200,
                "teammateList": [
                    {"bnetId":"current-local","heroGuid":"support-hero","finalHit":7,"cure":500},
                    {"bnetId":"ally-local","heroGuid":"support-hero","cure":3200}
                ],
                "enemyList": []
            }
        ]),
    )?;

    let projection = build_training_projection(&conn)?;
    let stadium_rows = projection
        .rows
        .iter()
        .filter(|row| {
            row.mode_family
                == owstats_desktop_lib::career_mmr::input::ModeFamily::StadiumCompetitive
        })
        .collect::<Vec<_>>();
    assert_eq!(stadium_rows.len(), 1);
    let row = stadium_rows[0];
    assert_eq!(row.role, "support");
    assert_eq!(row.final_hit, Some(7.0));
    assert_ne!(row.final_hit, Some(99.0));
    assert_eq!(row.duration_sec, 600.0);
    assert_eq!(row.available_team_healing, Some(3200.0));
    assert_eq!(row.hero_duration_sec, Some(600.0));
    assert!(!row.outcome_only);
    assert!(row.allow_performance);
    assert!(row.allow_performance_by_hero);
    assert_eq!(projection.audit.stadium_competitive_candidates, 1);
    assert_eq!(projection.audit.stadium_performance_eligible_rows, 1);
    assert_eq!(projection.audit.stadium_quick_outcome_only_rows, 0);
    Ok(())
}

#[test]
fn stadium_training_quality_and_eligibility_follow_snapshot_coverage() -> Result<()> {
    let cases = [
        ("empty", serde_json::json!({}), 0.55, false),
        (
            "invalid",
            serde_json::json!({"finalHit":-1,"kill":"NaN","assist":"bad"}),
            0.55,
            false,
        ),
        (
            "partial",
            serde_json::json!({"finalHit":"7","kill":3,"assist":-1}),
            0.62,
            true,
        ),
        (
            "complete",
            serde_json::json!({
                "kill":1,"assist":"2","death":3,"heroDamage":"4",
                "cure":5,"resistDamage":"6","finalHit":7,
                "targetCompetingTime":"8","healingTaken":9,"damageTaken":"10"
            }),
            0.90,
            true,
        ),
    ];
    for (suffix, statistics, expected_quality, performance_eligible) in cases {
        let (_dir, conn) = fixture()?;
        let mut player = serde_json::json!({
            "bnetId":"current-local",
            "heroGuid":"support-hero"
        });
        player
            .as_object_mut()
            .unwrap()
            .extend(statistics.as_object().unwrap().clone());
        insert_stadium_match(
            &conn,
            &format!("stadium-quality-{suffix}"),
            "SportFight",
            "support",
            serde_json::json!([{
                "roundIndex":1,"teammateList":[player],"enemyList":[]
            }]),
        )?;

        let projection = build_training_projection(&conn)?;
        let row = projection
            .rows
            .iter()
            .find(|row| row.mode_family == ModeFamily::StadiumCompetitive)
            .unwrap();
        assert!(
            (row.data_quality - expected_quality).abs() < 1e-12,
            "{suffix}"
        );
        assert_eq!(row.allow_performance, performance_eligible, "{suffix}");
        assert_eq!(row.outcome_only, !performance_eligible, "{suffix}");
        assert_eq!(
            projection.audit.stadium_performance_eligible_rows,
            usize::from(performance_eligible),
            "{suffix}"
        );
    }
    Ok(())
}

#[test]
fn stadium_quick_keeps_outcome_row_but_never_trains_performance() -> Result<()> {
    let (_dir, conn) = fixture()?;
    insert_stadium_match(
        &conn,
        "stadium-quick",
        "LeisureFight",
        "support",
        serde_json::json!([{
            "roundIndex": 1,
            "teammateList": [{"bnetId":"current-local","heroGuid":"support-hero","finalHit":7}],
            "enemyList": []
        }]),
    )?;

    let projection = build_training_projection(&conn)?;
    let row = projection
        .rows
        .iter()
        .find(|row| {
            row.mode_family == owstats_desktop_lib::career_mmr::input::ModeFamily::StadiumQuick
        })
        .expect("LeisureFight should remain as an outcome-only training row");
    assert_eq!(row.role, "support");
    assert!(row.outcome_only);
    assert!(!row.allow_performance);
    assert!(!row.allow_performance_by_hero);
    assert_eq!(row.final_hit, None);
    assert!((row.data_quality - 0.585).abs() < 1e-12);
    assert_eq!(row.duration_sec, 600.0);
    assert_eq!(row.hero_duration_sec, Some(600.0));
    assert_eq!(projection.audit.stadium_competitive_candidates, 0);
    assert_eq!(projection.audit.stadium_performance_eligible_rows, 0);
    assert_eq!(projection.audit.stadium_quick_outcome_only_rows, 1);
    Ok(())
}

#[test]
fn stadium_blank_current_player_key_does_not_create_a_training_row() -> Result<()> {
    let (_dir, conn) = fixture()?;
    insert_stadium_match(
        &conn,
        "stadium-blank-current",
        "SportFight",
        "support",
        serde_json::json!([{
            "roundIndex": 1,
            "teammateList": [{"bnetId":"current-local","heroGuid":"support-hero","finalHit":7}],
            "enemyList": []
        }]),
    )?;
    conn.execute(
        "UPDATE match_players SET bnet_id='   ' WHERE match_id='stadium-blank-current' AND is_current_player=1",
        [],
    )?;

    let projection = build_training_projection(&conn)?;
    assert!(projection
        .rows
        .iter()
        .all(|row| row.mode_family != ModeFamily::StadiumCompetitive));
    assert_eq!(projection.audit.stadium_performance_eligible_rows, 0);
    Ok(())
}

#[test]
fn legacy_ambiguous_shape_is_diagnostic_only_for_stadium_competitive() -> Result<()> {
    let (_dir, conn) = fixture()?;
    insert_stadium_match(
        &conn,
        "stadium-single-round",
        "SportFight",
        "tank",
        serde_json::json!([{
            "roundIndex": 1,
            "teammateList": [{"bnetId":"current-local","heroGuid":"support-hero","finalHit":7}],
            "enemyList": []
        }]),
    )?;

    let projection = build_training_projection(&conn)?;
    let row = projection
        .rows
        .iter()
        .find(|row| row.mode_family == ModeFamily::StadiumCompetitive)
        .unwrap();
    assert!(row.allow_performance);
    assert_eq!(row.final_hit, Some(7.0));
    assert_eq!(projection.audit.stadium_ambiguous_excluded, 0);
    Ok(())
}

#[test]
fn all_six_mode_families_reach_the_training_projection() -> Result<()> {
    let (_dir, conn) = fixture()?;
    insert_ordinary_mode(&conn, "role-quick", "LeisurePreset")?;
    insert_ordinary_mode(&conn, "open-competitive", "SportOpen")?;
    insert_ordinary_mode(&conn, "open-quick", "Leisure6v6")?;
    for (match_id, game_mode) in [
        ("stadium-competitive", "SportFight"),
        ("stadium-quick", "QuickFight"),
    ] {
        insert_stadium_match(
            &conn,
            match_id,
            game_mode,
            "support",
            serde_json::json!([{
                "roundIndex": 1,
                "teammateList": [{"bnetId":"current-local","heroGuid":"support-hero","finalHit":7}],
                "enemyList": []
            }]),
        )?;
    }

    let families = build_training_projection(&conn)?
        .rows
        .into_iter()
        .map(|row| row.mode_family)
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(
        families,
        [
            ModeFamily::RoleCompetitive,
            ModeFamily::RoleQuick,
            ModeFamily::OpenCompetitive,
            ModeFamily::OpenQuick,
            ModeFamily::StadiumCompetitive,
            ModeFamily::StadiumQuick,
        ]
        .into_iter()
        .collect()
    );
    Ok(())
}

#[test]
fn audits_healing_taken_as_external_teammate_healing() -> Result<()> {
    let (_dir, conn) = fixture()?;
    let projection = build_training_projection(&conn)?;
    let coverage = &projection.audit.coverage;
    assert_eq!(coverage.final_hit, 1);
    assert_eq!(coverage.kill, 1);
    assert_eq!(coverage.duration, 1);
    assert_eq!(coverage.exact_hero_duration, 1);
    assert_eq!(coverage.final_hero, 1);
    assert_eq!(
        projection
            .audit
            .healing_taken_semantics
            .explicit_external_healing_rows,
        1
    );
    assert_eq!(projection.audit.healing_taken_semantics.unknown_rows, 0);
    Ok(())
}

#[test]
fn projects_entry_stats_when_current_player_marker_is_unavailable() -> Result<()> {
    let (_dir, conn) = fixture()?;
    conn.execute("UPDATE match_players SET is_current_player=0", [])?;
    conn.execute(
        r#"UPDATE matches SET raw_entry_json='{"roleType":"damage","finalHit":4,"healingTaken":900,"damageTaken":3000,"targetCompetingTime":12,"selfHealing":100}'"#,
        [],
    )?;
    let projection = build_training_projection(&conn)?;
    assert_eq!(projection.rows.len(), 1);
    assert_eq!(projection.rows[0].final_hit, None);
    assert_eq!(projection.rows[0].healing_taken, None);
    assert!(projection.rows[0].outcome_only);
    assert!(!projection.rows[0].allow_performance);
    assert!(projection.rows[0].data_quality <= 0.55);
    assert_eq!(
        projection
            .audit
            .healing_taken_semantics
            .explicit_external_healing_rows,
        1
    );
    Ok(())
}

#[test]
fn normalizes_upstream_healer_role_to_support() -> Result<()> {
    let (_dir, conn) = fixture()?;
    conn.execute(
        r#"UPDATE matches SET raw_entry_json='{"roleType":"healer"}'"#,
        [],
    )?;
    let projection = build_training_projection(&conn)?;
    assert_eq!(projection.rows[0].role, "support");
    Ok(())
}

#[test]
fn coverage_distinguishes_valid_zero_from_missing_source_field() -> Result<()> {
    let (_dir, conn) = fixture()?;
    conn.execute(
        r#"UPDATE matches SET raw_entry_json='{"roleType":"damage","finalHit":0}'"#,
        [],
    )?;
    conn.execute("UPDATE match_players SET is_current_player=0", [])?;
    let projection = build_training_projection(&conn)?;
    assert_eq!(projection.audit.coverage.final_hit, 1);
    assert_eq!(projection.audit.coverage.healing_taken, 0);
    assert_eq!(projection.rows[0].final_hit, None);
    assert!(!projection.rows[0].usable_for_resource_debt);
    Ok(())
}

#[test]
fn external_healing_uses_explicit_external_value_not_mixed_total() -> Result<()> {
    let (_dir, conn) = fixture()?;
    conn.execute(r#"UPDATE matches SET raw_entry_json='{"roleType":"damage","healingTaken":900,"healingReceivedFromOthers":300,"heroPlayTimeList":[{"heroGuid":"hero-a","durationSec":10}]}'"#, [])?;
    let projection = build_training_projection(&conn)?;
    assert_eq!(projection.rows[0].healing_taken_semantic, "external");
    assert_eq!(projection.rows[0].healing_taken, None);
    assert!(!projection.rows[0].usable_for_resource_debt);
    Ok(())
}

#[test]
fn hero_duration_never_enables_whole_match_stats_as_hero_stats() -> Result<()> {
    let (_dir, conn) = fixture()?;
    for duration in [10, 600] {
        conn.execute("UPDATE matches SET raw_entry_json=?1", [format!(r#"{{"roleType":"damage","heroPlayTimeList":[{{"heroGuid":"hero-a","durationSec":{duration}}}]}}"#)])?;
        let row = build_training_projection(&conn)?.rows.remove(0);
        assert!(row.outcome_only);
        assert!(!row.allow_performance);
        assert!(!row.allow_performance_by_hero);
        assert_eq!(row.final_hit, None);
        assert_eq!(row.kill, None);
        assert_eq!(row.healing_taken, None);
        assert_eq!(row.objective_time_sec, None);
    }
    Ok(())
}

#[test]
fn duplicate_current_player_rows_do_not_depend_on_insert_order() -> Result<()> {
    let (_dir, conn) = fixture()?;
    conn.execute("INSERT INTO match_players(match_id,season,side,bnet_id,player_name,hero_guid,is_current_player,raw_json,updated_at) VALUES('private-match',22,'teammate','other','匿名','hero-a',1,'{\"finalHit\":99}','now')", [])?;
    conn.execute(r#"UPDATE matches SET raw_entry_json='{"roleType":"damage","heroPlayTimeList":[{"heroGuid":"hero-a","durationSec":10}]}'"#, [])?;
    assert_eq!(build_training_projection(&conn)?.rows[0].final_hit, None);
    Ok(())
}

#[test]
fn expected_rate_comes_only_from_active_model() -> Result<()> {
    let (_dir, conn) = fixture()?;
    for (version, active, expected) in [(1, 1, 0.4), (99, 0, 0.9)] {
        conn.execute(
            "INSERT INTO mmr_model_versions VALUES(?1,'test','{}',?2,'now')",
            params![version, active],
        )?;
        conn.execute("INSERT INTO career_mmr_events VALUES(?1,'private-match','damage',22,100,'role_competitive',1,1,?2,.8,0,2500,200,1,1,2501,199,.8,'hash','now')", params![version,expected])?;
    }
    let row = build_training_projection(&conn)?.rows.remove(0);
    assert_eq!(row.expected_win_rate, 0.4);
    assert_eq!(row.base_delta, Some(1.0));
    Ok(())
}

#[test]
fn stable_hero_key_does_not_change_when_an_earlier_hero_is_added() -> Result<()> {
    let (_dir, conn) = fixture()?;
    conn.execute(r#"UPDATE matches SET raw_entry_json='{"roleType":"damage","heroPlayTimeList":[{"heroGuid":"hero-a","durationSec":10}]}'"#, [])?;
    let first = build_training_projection(&conn)?.rows[0].hero_group.clone();
    conn.execute(
        "INSERT INTO hero_dict(guid,name,role,updated_at) VALUES('aaa','另一个英雄','tank','now')",
        [],
    )?;
    let second = build_training_projection(&conn)?.rows[0].hero_group.clone();
    assert_eq!(first, second);
    assert!(first.is_some());
    Ok(())
}

#[test]
fn multi_hero_segments_do_not_duplicate_whole_match_healing_taken() -> Result<()> {
    let (_dir, conn) = fixture()?;
    conn.execute(
        "INSERT INTO hero_dict(guid,name,role,updated_at) VALUES('hero-b','B','support','now')",
        [],
    )?;
    conn.execute(
        "INSERT INTO match_details(match_id,match_kind,raw_detail_json,updated_at) VALUES('private-match','normal',?1,'now')",
        [r#"{"payload":{"data":{"heroList":[{"heroId":"hero-a","userTimeSec":300,"useTimeRate":0.5,"statMap":{"603482350067646507":4}},{"heroId":"hero-b","userTimeSec":300,"useTimeRate":0.5,"statMap":{"603482350067646507":2}}]}}}"#],
    )?;

    let projection = build_training_projection(&conn)?;
    assert_eq!(projection.rows.len(), 2);
    assert_eq!(projection.rows[0].role, "damage");
    assert_eq!(projection.rows[1].role, "support");
    assert!(projection.rows.iter().all(|row| row.allow_performance));
    assert!(projection
        .rows
        .iter()
        .all(|row| row.healing_taken.is_none()));
    assert!(projection
        .rows
        .iter()
        .all(|row| !row.usable_for_resource_debt));
    Ok(())
}

#[test]
fn short_hero_segment_remains_outcome_only() -> Result<()> {
    let (_dir, conn) = fixture()?;
    conn.execute(
        "INSERT INTO match_details(match_id,match_kind,raw_detail_json,updated_at) VALUES('private-match','normal',?1,'now')",
        [r#"{"payload":{"data":{"heroList":[{"heroId":"hero-a","userTimeSec":30,"useTimeRate":0.05,"statMap":{"603482350067646507":9}}]}}}"#],
    )?;

    let row = build_training_projection(&conn)?.rows.remove(0);
    assert!(row.outcome_only);
    assert!(!row.allow_performance);
    assert_eq!(row.final_hit, None);
    assert!(!row.usable_for_resource_debt);
    Ok(())
}

#[test]
fn audit_projection_supports_pre_mmr_databases() -> Result<()> {
    let (_dir, conn) = fixture()?;
    conn.execute_batch("DROP TABLE career_mmr_events; DROP TABLE mmr_model_versions;")?;

    let projection = build_training_projection(&conn)?;
    assert_eq!(projection.rows.len(), 1);
    assert_eq!(projection.rows[0].expected_win_rate, 0.5);
    let row = &projection.rows[0];
    assert_eq!(
        row.base_delta,
        Some(
            owstats_desktop_lib::career_mmr::rating::outcome_delta_for_expected(
                owstats_desktop_lib::career_mmr::rating::INITIAL_SIGMA,
                row.mode_family,
                1.0,
                row.data_quality,
                row.result,
                row.expected_win_rate,
            )
        )
    );
    Ok(())
}

#[test]
fn exact_segment_without_dictionary_role_is_skipped_instead_of_guessed() -> Result<()> {
    let (_dir, conn) = fixture()?;
    conn.execute(
        "INSERT INTO match_details(match_id,match_kind,raw_detail_json,updated_at) VALUES('private-match','normal',?1,'now')",
        [r#"{"payload":{"data":{"heroList":[{"heroId":"unknown-hero","userTimeSec":600,"useTimeRate":1.0,"statMap":{"603482350067646507":9}}]}}}"#],
    )?;

    let projection = build_training_projection(&conn)?;
    assert!(projection.rows.is_empty());
    assert_eq!(projection.audit.performance_eligible_rows, 0);
    Ok(())
}
