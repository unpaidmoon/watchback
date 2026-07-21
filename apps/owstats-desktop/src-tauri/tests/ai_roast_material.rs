use anyhow::Result;
use owstats_desktop_lib::{
    ai_roast::material::{
        assert_material_privacy, build_roast_material, AnonymousOrdinal, AnonymousPlayerRole,
        FactSubject, RoastFactEvidence, RoastMaterial, RoastRelationship, RoastRelationshipKind,
        RoastRelationshipSubject, MATERIAL_VERSION,
    },
    OwDatabase,
};
use rusqlite::{params, Connection};
use std::collections::HashSet;
use tempfile::tempdir;

const OUTPUT_HEAVY_SUPPORT_GUID: &str = "207165582859043234";
const INSUFFICIENT_DETAIL: &str = "\u{5f53}\u{524d}\u{5bf9}\u{5c40}\u{7f3a}\u{5c11}\u{8db3}\u{591f}\u{7684}\u{53ef}\u{6838}\u{9a8c}\u{9510}\u{8bc4}\u{7ec6}\u{8282}";

#[derive(Clone, Copy)]
struct Stats(i64, i64, i64, i64, i64, i64);

fn stats(
    kills: i64,
    assists: i64,
    deaths: i64,
    damage: i64,
    healing: i64,
    mitigation: i64,
) -> Stats {
    Stats(kills, assists, deaths, damage, healing, mitigation)
}

fn database() -> Result<(tempfile::TempDir, OwDatabase)> {
    let dir = tempdir()?;
    let db = OwDatabase::new(dir.path().join("ai-roast-material.sqlite3"));
    db.initialize()?;
    Ok((dir, db))
}

fn seed_match(
    conn: &Connection,
    match_id: &str,
    season: i64,
    result: &str,
    begin_ts: i64,
) -> Result<()> {
    seed_match_with_kind(conn, match_id, season, result, begin_ts, "normal")
}

fn seed_match_with_kind(
    conn: &Connection,
    match_id: &str,
    season: i64,
    result: &str,
    begin_ts: i64,
    match_kind: &str,
) -> Result<()> {
    conn.execute(
        "INSERT INTO matches(match_id,season,bucket,game_mode,match_kind,result,map_guid,hero_guid,begin_ts,team_score,opponent_score,kills,assists,deaths,damage,healing,mitigation,total_worth,duration_sec,raw_entry_json,updated_at) VALUES(?1,?2,?3,?4,?5,?6,'synthetic-map','synthetic-hero',?7,2,1,0,0,0,0,0,0,0,600,'{\"credential\":\"synthetic-secret-token\"}','now')",
        params![
            match_id,
            season,
            if match_kind == "normal" {
                "preset"
            } else {
                "stadium"
            },
            if match_kind == "normal" {
                "SportPreset"
            } else {
                "SportFight"
            },
            match_kind,
            result,
            begin_ts
        ],
    )?;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn seed_player(
    conn: &Connection,
    match_id: &str,
    season: i64,
    side: &str,
    account: &str,
    name: &str,
    hero: &str,
    role: &str,
    current: bool,
    value: Stats,
) -> Result<()> {
    conn.execute(
        "INSERT INTO match_players(match_id,season,side,bnet_id,player_name,hero_guid,hero_name,is_current_player,is_friend,kill,assist,death,hero_damage,cure,resist_damage,healing_taken,damage_taken,final_hit,target_competing_time,role_type,raw_json,updated_at) VALUES(?1,?2,?3,?4,?5,?6,'合成英雄',?7,0,?8,?9,?10,?11,?12,?13,0,0,0,0,?14,'{\"note\":\"synthetic-private-note\",\"path\":\"C:/synthetic/profile\"}','now')",
        params![match_id, season, side, account, name, hero, i64::from(current), value.0, value.1, value.2, value.3, value.4, value.5, role],
    )?;
    Ok(())
}

fn seed_evaluation(conn: &Connection, match_id: &str, primary_tag: &str) -> Result<()> {
    conn.execute(
        "INSERT INTO match_evaluations(match_id,mode,rule_version,grade,primary_tag,evidence_tags_json,reasons_json,confidence,computed_at) VALUES(?1,'SportPreset',1,'A',?2,'[\"伤害最高\",\"KDA最高\"]','[\"primary:synthetic\"]',0.91,'now')",
        params![match_id, primary_tag],
    )?;
    Ok(())
}

fn seed_team(conn: &Connection, match_id: &str, rows: &[(&str, bool, Stats)]) -> Result<()> {
    for (index, (role, current, value)) in rows.iter().copied().enumerate() {
        seed_player(
            conn,
            match_id,
            23,
            "teammate",
            &format!("synthetic-account-{index}"),
            &format!("SyntheticPlayer#{index}"),
            &format!("synthetic-hero-{index}"),
            role,
            current,
            value,
        )?;
    }
    for (index, (role, value)) in [
        ("dps", stats(12, 5, 5, 8_000, 0, 0)),
        ("dps", stats(10, 4, 6, 7_000, 0, 0)),
        ("healer", stats(8, 8, 5, 4_000, 5_000, 0)),
        ("healer", stats(7, 9, 4, 3_000, 6_000, 0)),
        ("tank", stats(6, 3, 7, 4_000, 0, 7_000)),
    ]
    .into_iter()
    .enumerate()
    {
        seed_player(
            conn,
            match_id,
            23,
            "enemy",
            &format!("synthetic-enemy-{index}"),
            &format!("SyntheticEnemy#{index}"),
            &format!("synthetic-enemy-hero-{index}"),
            role,
            false,
            value,
        )?;
    }
    Ok(())
}

fn assert_grounded(material: &RoastMaterial, kind: RoastRelationshipKind) -> &RoastRelationship {
    assert_eq!(material.material_version, MATERIAL_VERSION);
    assert!((3..=4).contains(&material.selected_facts.len()));
    assert!(material
        .selected_facts
        .iter()
        .all(|fact| fact.strength >= 0.35));
    let fact_ids = material
        .selected_facts
        .iter()
        .map(|fact| fact.id.as_str())
        .collect::<HashSet<_>>();
    assert_eq!(fact_ids.len(), material.selected_facts.len());
    let relationship = material
        .relationship
        .as_ref()
        .expect("expected relationship");
    assert_eq!(relationship.kind, kind);
    assert!(relationship.facts.len() >= 2);
    assert!(relationship.facts.iter().any(|fact| matches!(
        fact.evidence,
        RoastFactEvidence::Rank | RoastFactEvidence::SameRole
    )));
    relationship
}

fn assert_neutral(material: &RoastMaterial) {
    assert_eq!(material.material_version, MATERIAL_VERSION);
    assert!(material.relationship.is_none());
    assert!((3..=4).contains(&material.selected_facts.len()));
    assert!(material.selected_facts.iter().all(|fact| {
        fact.strength >= 0.35 && !fact.text.contains("数据不足") && !fact.text.contains("仅保留")
    }));
}

fn assert_private(material: &RoastMaterial, forbidden: &[&str]) -> Result<()> {
    assert_material_privacy(material)?;
    let serialized = serde_json::to_string(material)?;
    for value in forbidden {
        assert!(!serialized.contains(value), "leaked {value}");
    }
    for key in [
        "matchId",
        "match_id",
        "playerName",
        "bnetId",
        "accountId",
        "rawJson",
        "notes",
        "path",
        "credential",
        "lineup",
    ] {
        assert!(!serialized.contains(key), "leaked key {key}");
    }
    Ok(())
}

fn complete_lineup() -> [(&'static str, bool, Stats); 5] {
    [
        ("tank", true, stats(8, 5, 4, 7_000, 0, 9_000)),
        ("dps", false, stats(12, 3, 4, 10_000, 0, 0)),
        ("dps", false, stats(6, 2, 8, 5_000, 0, 0)),
        ("healer", false, stats(3, 11, 5, 2_000, 8_000, 0)),
        ("healer", false, stats(4, 10, 6, 2_500, 7_000, 0)),
    ]
}

#[test]
fn selects_weak_link_with_reused_evaluation_and_grounded_facts() -> Result<()> {
    let (_dir, db) = database()?;
    let conn = db.connect()?;
    let match_id = "synthetic-current-weak-link";
    seed_match(&conn, match_id, 23, "win", 1_000)?;
    seed_evaluation(&conn, match_id, "带飞局")?;
    seed_team(
        &conn,
        match_id,
        &[
            ("tank", true, stats(12, 8, 3, 7_000, 0, 12_000)),
            ("dps", false, stats(18, 4, 4, 13_000, 0, 0)),
            ("dps", false, stats(3, 2, 11, 2_800, 0, 0)),
            ("healer", false, stats(5, 15, 4, 3_500, 8_000, 0)),
            ("healer", false, stats(4, 13, 5, 3_000, 7_000, 0)),
        ],
    )?;

    let material = build_roast_material(&conn, match_id)?;

    assert_eq!(material.evaluation.primary_tag, "带飞局");
    let relationship = assert_grounded(&material, RoastRelationshipKind::WeakLink);
    assert_eq!(
        relationship.subject,
        RoastRelationshipSubject::Teammate {
            role: AnonymousPlayerRole::Damage,
            ordinal: AnonymousOrdinal::Second,
        }
    );
    assert_private(
        &material,
        &[match_id, "SyntheticPlayer#2", "synthetic-secret-token"],
    )
}

#[test]
fn selects_carry_big_daddy_and_reports_stable_streak_context() -> Result<()> {
    let (_dir, db) = database()?;
    let conn = db.connect()?;
    for (match_id, result, begin_ts) in [
        ("synthetic-streak-older", "loss", 100),
        ("synthetic-streak-b", "win", 200),
        ("synthetic-streak-a", "win", 200),
        ("synthetic-current-carry", "win", 300),
    ] {
        seed_match(&conn, match_id, 23, result, begin_ts)?;
    }
    seed_evaluation(&conn, "synthetic-current-carry", "躺赢局")?;
    seed_team(
        &conn,
        "synthetic-current-carry",
        &[
            ("tank", false, stats(9, 7, 4, 6_000, 0, 11_000)),
            ("dps", true, stats(8, 5, 6, 6_500, 0, 0)),
            ("dps", false, stats(24, 6, 2, 16_000, 0, 0)),
            ("healer", false, stats(5, 16, 4, 3_000, 8_500, 0)),
            ("healer", false, stats(4, 14, 4, 2_500, 7_500, 0)),
        ],
    )?;

    let material = build_roast_material(&conn, "synthetic-current-carry")?;

    let relationship = assert_grounded(&material, RoastRelationshipKind::Carry);
    println!(
        "carry facts: {:?}",
        relationship
            .facts
            .iter()
            .map(|fact| fact.id.as_str())
            .collect::<Vec<_>>()
    );
    assert_eq!(
        relationship.subject,
        RoastRelationshipSubject::Teammate {
            role: AnonymousPlayerRole::Damage,
            ordinal: AnonymousOrdinal::Second,
        }
    );
    assert_eq!(material.streak.before_current, 2);
    assert_eq!(material.streak.current_event, "将连胜延长到3场");
    assert_private(&material, &["synthetic-current-carry", "SyntheticPlayer#2"])
}

#[test]
fn selects_main_scapegoat_on_loss() -> Result<()> {
    let (_dir, db) = database()?;
    let conn = db.connect()?;
    let match_id = "synthetic-current-scapegoat";
    seed_match(&conn, match_id, 23, "loss", 1_000)?;
    seed_evaluation(&conn, match_id, "惜败局")?;
    seed_team(
        &conn,
        match_id,
        &[
            ("tank", true, stats(10, 8, 5, 7_000, 0, 10_000)),
            ("dps", false, stats(15, 4, 5, 11_000, 0, 0)),
            ("dps", false, stats(2, 1, 13, 2_100, 0, 0)),
            ("healer", false, stats(4, 12, 6, 3_000, 7_500, 0)),
            ("healer", false, stats(5, 13, 5, 3_500, 8_000, 0)),
        ],
    )?;

    let material = build_roast_material(&conn, match_id)?;

    let relationship = assert_grounded(&material, RoastRelationshipKind::Scapegoat);
    assert_eq!(
        relationship.subject,
        RoastRelationshipSubject::Teammate {
            role: AnonymousPlayerRole::Damage,
            ordinal: AnonymousOrdinal::Second,
        }
    );
    assert!(!relationship.counter_facts.is_empty());
    assert_private(&material, &[match_id, "SyntheticPlayer#2"])
}

#[test]
fn self_roasts_output_heavy_support_using_same_role_season_average() -> Result<()> {
    let (_dir, db) = database()?;
    let conn = db.connect()?;
    for (index, value) in [
        stats(9, 14, 5, 7_000, 9_000, 0),
        stats(8, 15, 4, 6_500, 10_000, 0),
    ]
    .into_iter()
    .enumerate()
    {
        let historical = format!("synthetic-history-{index}");
        seed_match(&conn, &historical, 23, "loss", 100 + index as i64)?;
        seed_player(
            &conn,
            &historical,
            23,
            "teammate",
            &format!("synthetic-history-account-{index}"),
            &format!("SyntheticHistory#{index}"),
            &format!("different-hero-{index}"),
            "healer",
            true,
            value,
        )?;
    }
    let match_id = "synthetic-current-self-roast";
    seed_match(&conn, match_id, 23, "loss", 1_000)?;
    seed_evaluation(&conn, match_id, "背锅局")?;
    for (index, (hero, role, current, value)) in [
        (
            OUTPUT_HEAVY_SUPPORT_GUID,
            "healer",
            true,
            stats(7, 5, 12, 8_000, 1_200, 0),
        ),
        (
            "synthetic-tank",
            "tank",
            false,
            stats(9, 8, 6, 7_000, 0, 11_000),
        ),
        (
            "synthetic-dps-a",
            "dps",
            false,
            stats(14, 5, 5, 11_000, 0, 0),
        ),
        (
            "synthetic-dps-b",
            "dps",
            false,
            stats(12, 4, 6, 10_000, 0, 0),
        ),
        (
            "synthetic-support",
            "healer",
            false,
            stats(5, 16, 4, 3_000, 9_000, 0),
        ),
    ]
    .into_iter()
    .enumerate()
    {
        seed_player(
            &conn,
            match_id,
            23,
            "teammate",
            &format!("synthetic-self-account-{index}"),
            &format!("SyntheticSelf#{index}"),
            hero,
            role,
            current,
            value,
        )?;
    }
    seed_team_enemies(&conn, match_id)?;

    let material = build_roast_material(&conn, match_id)?;

    let relationship = assert_grounded(&material, RoastRelationshipKind::SelfRoast);
    assert_eq!(
        relationship.subject,
        RoastRelationshipSubject::CurrentPlayer
    );
    assert!(material
        .selected_facts
        .iter()
        .any(|fact| fact.id.contains("same_role_healing")));
    assert!(
        material
            .selected_facts
            .iter()
            .any(|fact| fact.text.contains("治疗") && fact.text.contains("伤害")),
        "selected facts: {:?}",
        material.selected_facts
    );
    assert!(material
        .selected_facts
        .iter()
        .all(|fact| !fact.text.contains("导致")));
    assert!(material
        .selected_facts
        .iter()
        .all(|fact| !fact.text.contains("当前英雄均值")));
    assert_private(&material, &[match_id, "SyntheticSelf#0"])
}

fn seed_team_enemies(conn: &Connection, match_id: &str) -> Result<()> {
    for (index, (role, value)) in [
        ("dps", stats(12, 5, 5, 8_000, 0, 0)),
        ("dps", stats(10, 4, 6, 7_000, 0, 0)),
        ("healer", stats(8, 8, 5, 4_000, 5_000, 0)),
        ("healer", stats(7, 9, 4, 3_000, 6_000, 0)),
        ("tank", stats(6, 3, 7, 4_000, 0, 7_000)),
    ]
    .into_iter()
    .enumerate()
    {
        seed_player(
            conn,
            match_id,
            23,
            "enemy",
            &format!("synthetic-self-enemy-{index}"),
            &format!("SyntheticSelfEnemy#{index}"),
            &format!("synthetic-self-enemy-hero-{index}"),
            role,
            false,
            value,
        )?;
    }
    Ok(())
}

#[test]
fn lazily_evaluates_current_match_and_uses_safe_win_loss_draw_fallbacks() -> Result<()> {
    let (_dir, db) = database()?;
    let conn = db.connect()?;
    for (index, (match_id, result, expected_tag, expected_event)) in [
        ("synthetic-neutral-win", "win", "带飞局", "本场获胜"),
        (
            "synthetic-neutral-loss",
            "loss",
            "尽力局",
            "结束1场连胜并失利",
        ),
        ("synthetic-neutral-draw", "draw", "平局", "平局中断连续结果"),
    ]
    .into_iter()
    .enumerate()
    {
        seed_match(&conn, match_id, 23, result, 2_000 + index as i64)?;
        seed_team(
            &conn,
            match_id,
            &[
                ("tank", true, stats(8, 5, 4, 7_000, 0, 9_000)),
                ("dps", false, stats(12, 4, 5, 9_000, 0, 0)),
                ("dps", false, stats(7, 3, 7, 6_000, 0, 0)),
                ("healer", false, stats(4, 12, 5, 2_500, 8_000, 0)),
                ("healer", false, stats(3, 10, 6, 2_000, 7_000, 0)),
            ],
        )?;
        let material = build_roast_material(&conn, match_id)?;
        assert_eq!(material.evaluation.primary_tag, expected_tag);
        match result {
            "win" => {
                assert_grounded(&material, RoastRelationshipKind::WeakLink);
            }
            "loss" => {
                assert_grounded(&material, RoastRelationshipKind::Scapegoat);
            }
            "draw" => assert_neutral(&material),
            _ => unreachable!(),
        }
        assert_eq!(material.streak.current_event, expected_event);
        assert_private(&material, &[match_id, "synthetic-secret-token"])?;
    }
    let cached: i64 = conn.query_row(
        "SELECT COUNT(*) FROM match_evaluations WHERE match_id IN ('synthetic-neutral-win','synthetic-neutral-loss')",
        [], |row| row.get(0),
    )?;
    assert_eq!(cached, 2);
    let draw_cached: i64 = conn.query_row(
        "SELECT COUNT(*) FROM match_evaluations WHERE match_id='synthetic-neutral-draw'",
        [],
        |row| row.get(0),
    )?;
    assert_eq!(draw_cached, 0);
    Ok(())
}

#[test]
fn keeps_relationship_when_two_distinct_supporting_facts_are_grounded() -> Result<()> {
    let (_dir, db) = database()?;
    let conn = db.connect()?;
    let match_id = "synthetic-two-facts";
    seed_match(&conn, match_id, 23, "win", 2_500)?;
    seed_team(
        &conn,
        match_id,
        &[
            ("tank", true, stats(8, 4, 2, 0, 0, 8_000)),
            ("dps", false, stats(0, 0, 12, 0, 0, 0)),
            ("healer", false, stats(2, 3, 5, 0, 0, 0)),
            ("dps", false, stats(4, 2, 6, 1_000, 0, 0)),
            ("healer", false, stats(3, 4, 5, 0, 1_000, 0)),
        ],
    )?;

    let material = build_roast_material(&conn, match_id)?;

    let relationship = assert_grounded(&material, RoastRelationshipKind::WeakLink);
    assert!(relationship.facts.len() >= 2);
    assert_private(&material, &[match_id])
}

#[test]
fn neutral_material_has_grounded_facts_without_relationship_or_meta_padding() -> Result<()> {
    let (_dir, db) = database()?;
    let conn = db.connect()?;
    let match_id = "synthetic-neutral-evidence";
    seed_match(&conn, match_id, 23, "draw", 2_600)?;
    seed_team(
        &conn,
        match_id,
        &[
            ("tank", true, stats(8, 4, 2, 7_500, 0, 9_000)),
            ("dps", false, stats(12, 3, 4, 10_000, 0, 0)),
            ("dps", false, stats(6, 2, 8, 5_000, 0, 0)),
            ("healer", false, stats(3, 11, 5, 2_000, 8_000, 0)),
            ("healer", false, stats(4, 10, 6, 2_500, 7_000, 0)),
        ],
    )?;

    let material = build_roast_material(&conn, match_id)?;

    assert_neutral(&material);
    assert!(material
        .selected_facts
        .iter()
        .any(|fact| fact.evidence == RoastFactEvidence::Result));
    let mut too_short = material;
    too_short.selected_facts.truncate(2);
    assert!(assert_material_privacy(&too_short).is_err());
    Ok(())
}

#[test]
fn rejects_material_when_fewer_than_three_grounded_facts_exist() -> Result<()> {
    let (_dir, db) = database()?;
    let conn = db.connect()?;
    let match_id = "synthetic-insufficient-detail";
    seed_match(&conn, match_id, 23, "draw", 2_650)?;

    let error = build_roast_material(&conn, match_id).unwrap_err();

    assert!(error.to_string().contains("可核验锐评细节"));
    Ok(())
}

#[test]
fn requires_exactly_five_valid_teammates_and_one_current_player() -> Result<()> {
    let (_dir, db) = database()?;
    let conn = db.connect()?;
    let lineup = complete_lineup();

    for lineup_size in 2..=4 {
        let match_id = format!("synthetic-partial-lineup-{lineup_size}");
        seed_match(&conn, &match_id, 23, "win", 3_100 + lineup_size as i64)?;
        seed_team(&conn, &match_id, &lineup[..lineup_size])?;
        assert_eq!(
            build_roast_material(&conn, &match_id)
                .unwrap_err()
                .to_string(),
            INSUFFICIENT_DETAIL
        );
    }

    let no_current = "synthetic-no-current-player";
    seed_match(&conn, no_current, 23, "win", 3_200)?;
    let mut no_current_lineup = complete_lineup();
    no_current_lineup[0].1 = false;
    seed_team(&conn, no_current, &no_current_lineup)?;
    assert_eq!(
        build_roast_material(&conn, no_current)
            .unwrap_err()
            .to_string(),
        INSUFFICIENT_DETAIL
    );

    let multiple_current = "synthetic-multiple-current-players";
    seed_match(&conn, multiple_current, 23, "win", 3_300)?;
    let mut multiple_current_lineup = complete_lineup();
    multiple_current_lineup[1].1 = true;
    seed_team(&conn, multiple_current, &multiple_current_lineup)?;
    assert_eq!(
        build_roast_material(&conn, multiple_current)
            .unwrap_err()
            .to_string(),
        INSUFFICIENT_DETAIL
    );
    Ok(())
}

#[test]
fn relationship_subject_serializes_as_anonymous_structure() -> Result<()> {
    let (_dir, db) = database()?;
    let conn = db.connect()?;
    let match_id = "synthetic-structured-subject";
    seed_match(&conn, match_id, 23, "win", 3_400)?;
    seed_team(
        &conn,
        match_id,
        &[
            ("tank", true, stats(12, 8, 3, 7_000, 0, 12_000)),
            ("dps", false, stats(18, 4, 4, 13_000, 0, 0)),
            ("dps", false, stats(3, 2, 11, 2_800, 0, 0)),
            ("healer", false, stats(5, 15, 4, 3_500, 8_000, 0)),
            ("healer", false, stats(4, 13, 5, 3_000, 7_000, 0)),
        ],
    )?;

    let material = build_roast_material(&conn, match_id)?;
    let serialized = serde_json::to_value(&material)?;

    assert_eq!(
        serialized.pointer("/relationship/subject"),
        Some(&serde_json::json!({
            "teammate": { "role": "damage", "ordinal": "second" }
        }))
    );
    assert_private(&material, &["SyntheticPlayer#2"])
}

#[test]
fn rejects_non_finite_and_out_of_range_outbound_numbers() -> Result<()> {
    let (_dir, db) = database()?;
    let conn = db.connect()?;
    let match_id = "synthetic-invalid-outbound-numbers";
    seed_match(&conn, match_id, 23, "win", 3_500)?;
    seed_team(&conn, match_id, &complete_lineup())?;
    let material = build_roast_material(&conn, match_id)?;

    for invalid in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY, -0.01, 1.01] {
        let mut mutated = material.clone();
        mutated.evaluation.confidence = invalid;
        assert!(assert_material_privacy(&mutated).is_err());

        let mut mutated = material.clone();
        mutated.selected_facts[0].strength = invalid;
        assert!(assert_material_privacy(&mutated).is_err());

        let mut mutated = material.clone();
        let relationship = mutated
            .relationship
            .as_mut()
            .expect("expected synthetic relationship");
        relationship.confidence = invalid;
        assert!(assert_material_privacy(&mutated).is_err());

        let mut mutated = material.clone();
        let relationship = mutated
            .relationship
            .as_mut()
            .expect("expected synthetic relationship");
        relationship.facts[0].strength = invalid;
        assert!(assert_material_privacy(&mutated).is_err());
    }
    Ok(())
}

#[test]
fn extreme_i64_stats_never_panic_or_emit_non_finite_numbers() -> Result<()> {
    let (_dir, db) = database()?;
    let conn = db.connect()?;
    let match_id = "synthetic-i64-max-material";
    seed_match(&conn, match_id, 23, "win", 3_600)?;
    let extreme = stats(i64::MAX, i64::MAX, i64::MAX, i64::MAX, i64::MAX, i64::MAX);
    seed_team(
        &conn,
        match_id,
        &[
            ("tank", true, extreme),
            ("dps", false, extreme),
            ("dps", false, extreme),
            ("healer", false, extreme),
            ("healer", false, extreme),
        ],
    )?;

    if let Ok(material) = build_roast_material(&conn, match_id) {
        assert!(material.evaluation.confidence.is_finite());
        assert!(material
            .selected_facts
            .iter()
            .all(|fact| fact.strength.is_finite()));
        assert_material_privacy(&material)?;
    }
    Ok(())
}

#[test]
fn tied_identical_players_do_not_create_rank_or_relationship_evidence() -> Result<()> {
    let (_dir, db) = database()?;
    let conn = db.connect()?;
    let match_id = "synthetic-tied-identical";
    seed_match(&conn, match_id, 23, "draw", 2_675)?;
    seed_team(
        &conn,
        match_id,
        &[
            ("dps", true, stats(8, 4, 4, 8_000, 0, 0)),
            ("dps", false, stats(8, 4, 4, 8_000, 0, 0)),
        ],
    )?;

    assert!(build_roast_material(&conn, match_id).is_err());
    Ok(())
}

#[test]
fn tied_players_use_competition_rank_without_row_order_relationships() -> Result<()> {
    let (_dir, db) = database()?;
    let conn = db.connect()?;
    let match_id = "synthetic-competition-rank";
    seed_match(&conn, match_id, 23, "win", 2_677)?;
    seed_team(
        &conn,
        match_id,
        &[
            ("dps", true, stats(5, 2, 5, 5_000, 0, 0)),
            ("dps", false, stats(10, 4, 2, 10_000, 0, 0)),
            ("dps", false, stats(5, 2, 5, 5_000, 0, 0)),
            ("healer", false, stats(2, 6, 5, 1_500, 4_000, 0)),
            ("healer", false, stats(2, 6, 5, 1_500, 4_000, 0)),
        ],
    )?;

    let material = build_roast_material(&conn, match_id)?;
    let rank_fact = material
        .selected_facts
        .iter()
        .find(|fact| fact.id == "current_player:team_rank_composite")
        .expect("expected current-player competition rank fact");

    assert!(rank_fact.text.ends_with('2'));
    assert!(material.relationship.is_none());
    Ok(())
}

#[test]
fn negligible_same_role_delta_does_not_create_comparison_evidence() -> Result<()> {
    let (_dir, db) = database()?;
    let conn = db.connect()?;
    let match_id = "synthetic-negligible-role-delta";
    seed_match(&conn, match_id, 23, "draw", 2_680)?;
    seed_team(
        &conn,
        match_id,
        &[
            ("dps", true, stats(8, 4, 4, 10_000, 0, 0)),
            ("dps", false, stats(8, 4, 4, 10_001, 0, 0)),
        ],
    )?;

    assert!(build_roast_material(&conn, match_id).is_err());
    Ok(())
}

#[test]
fn single_player_draw_returns_insufficient_detail_without_fake_comparisons() -> Result<()> {
    let (_dir, db) = database()?;
    let conn = db.connect()?;
    let match_id = "synthetic-single-player-draw";
    seed_match(&conn, match_id, 23, "draw", 2_685)?;
    seed_team(
        &conn,
        match_id,
        &[("dps", true, stats(8, 4, 4, 8_000, 0, 0))],
    )?;

    assert!(build_roast_material(&conn, match_id).is_err());
    Ok(())
}

#[test]
fn single_player_with_same_role_history_still_returns_insufficient_detail() -> Result<()> {
    let (_dir, db) = database()?;
    let conn = db.connect()?;
    for (index, value) in [stats(16, 6, 3, 14_000, 0, 0), stats(14, 5, 4, 12_000, 0, 0)]
        .into_iter()
        .enumerate()
    {
        let historical = format!("synthetic-single-player-history-{index}");
        seed_match(&conn, &historical, 23, "win", 2_686 + index as i64)?;
        seed_player(
            &conn,
            &historical,
            23,
            "teammate",
            &format!("synthetic-history-account-{index}"),
            &format!("SyntheticHistory#{index}"),
            &format!("synthetic-history-hero-{index}"),
            "dps",
            true,
            value,
        )?;
    }
    let match_id = "synthetic-single-player-with-history";
    seed_match(&conn, match_id, 23, "loss", 2_688)?;
    seed_team(
        &conn,
        match_id,
        &[("dps", true, stats(1, 1, 12, 1_000, 0, 0))],
    )?;

    let error = build_roast_material(&conn, match_id).unwrap_err();

    assert_eq!(
        error.to_string(),
        "\u{5f53}\u{524d}\u{5bf9}\u{5c40}\u{7f3a}\u{5c11}\u{8db3}\u{591f}\u{7684}\u{53ef}\u{6838}\u{9a8c}\u{9510}\u{8bc4}\u{7ec6}\u{8282}"
    );
    Ok(())
}

#[test]
fn selected_facts_are_subject_scoped_and_uniquely_identified() -> Result<()> {
    let (_dir, db) = database()?;
    let conn = db.connect()?;
    let match_id = "synthetic-subject-scoped-facts";
    seed_match(&conn, match_id, 23, "win", 2_690)?;
    seed_team(
        &conn,
        match_id,
        &[
            ("tank", true, stats(8, 6, 3, 8_000, 0, 10_000)),
            ("dps", false, stats(1, 1, 12, 1_000, 0, 0)),
            ("dps", false, stats(12, 4, 4, 10_000, 0, 0)),
            ("healer", false, stats(4, 12, 5, 2_500, 8_000, 0)),
            ("healer", false, stats(3, 11, 6, 2_000, 7_000, 0)),
        ],
    )?;

    let material = build_roast_material(&conn, match_id)?;
    let ids = material
        .selected_facts
        .iter()
        .map(|fact| fact.id.as_str())
        .collect::<HashSet<_>>();

    assert_eq!(ids.len(), material.selected_facts.len());
    assert!(material
        .selected_facts
        .iter()
        .any(|fact| fact.subject == FactSubject::CurrentPlayer));
    assert!(material
        .selected_facts
        .iter()
        .any(|fact| fact.subject == FactSubject::RelationshipSubject));
    assert!(material
        .selected_facts
        .iter()
        .all(|fact| fact.id.starts_with(fact.subject.id_prefix())));
    Ok(())
}

#[test]
fn rejects_non_ordinary_target_without_lazy_evaluation_side_effects() -> Result<()> {
    let (_dir, db) = database()?;
    let conn = db.connect()?;
    let match_id = "synthetic-stadium-target";
    seed_match_with_kind(&conn, match_id, 23, "win", 2_700, "fight")?;
    seed_team(
        &conn,
        match_id,
        &[
            ("tank", true, stats(8, 4, 2, 7_000, 0, 8_000)),
            ("dps", false, stats(5, 2, 7, 4_000, 0, 0)),
        ],
    )?;

    assert!(build_roast_material(&conn, match_id).is_err());
    let cached: i64 = conn.query_row(
        "SELECT COUNT(*) FROM match_evaluations WHERE match_id=?",
        params![match_id],
        |row| row.get(0),
    )?;
    assert_eq!(cached, 0);
    Ok(())
}

#[test]
fn non_ordinary_history_does_not_change_ordinary_streak() -> Result<()> {
    let (_dir, db) = database()?;
    let conn = db.connect()?;
    seed_match(&conn, "synthetic-ordinary-win-a", 23, "win", 100)?;
    seed_match(&conn, "synthetic-ordinary-win-b", 23, "win", 200)?;
    seed_match_with_kind(&conn, "synthetic-stadium-loss", 23, "loss", 250, "fight")?;
    let match_id = "synthetic-ordinary-target";
    seed_match(&conn, match_id, 23, "win", 300)?;
    seed_team(
        &conn,
        match_id,
        &[
            ("tank", true, stats(8, 5, 4, 7_000, 0, 9_000)),
            ("dps", false, stats(2, 1, 10, 2_000, 0, 0)),
            ("dps", false, stats(9, 3, 5, 8_000, 0, 0)),
            ("healer", false, stats(4, 10, 5, 2_500, 7_500, 0)),
            ("healer", false, stats(3, 11, 6, 2_000, 7_000, 0)),
        ],
    )?;

    let material = build_roast_material(&conn, match_id)?;

    assert_eq!(material.streak.before_current, 2);
    Ok(())
}

#[test]
fn rejects_unsafe_cached_evaluation_strings_before_serialization() -> Result<()> {
    let (_dir, db) = database()?;
    let conn = db.connect()?;
    let match_id = "synthetic-unsafe-evaluation";
    seed_match(&conn, match_id, 23, "win", 3_000)?;
    seed_team(
        &conn,
        match_id,
        &[
            ("tank", true, stats(8, 5, 4, 7_000, 0, 9_000)),
            ("dps", false, stats(0, 0, 12, 0, 0, 0)),
            ("dps", false, stats(9, 3, 5, 8_000, 0, 0)),
            ("healer", false, stats(4, 10, 5, 2_500, 7_500, 0)),
            ("healer", false, stats(3, 11, 6, 2_000, 7_000, 0)),
        ],
    )?;
    conn.execute(
        r#"INSERT INTO match_evaluations(match_id,mode,rule_version,grade,primary_tag,evidence_tags_json,reasons_json,confidence,computed_at) VALUES(?1,'SportPreset',1,'A','SyntheticLeak#1000','["synthetic-account-id"]','[]',0.9,'now')"#,
        params![match_id],
    )?;

    let material = build_roast_material(&conn, match_id)?;

    assert_eq!(material.evaluation.primary_tag, "胜利局");
    assert!(material.evaluation.evidence_tags.is_empty());
    assert_private(
        &material,
        &[match_id, "SyntheticLeak#1000", "synthetic-account-id"],
    )?;
    let mut unsafe_selected = material.clone();
    unsafe_selected.selected_facts[0].text = "customerToken=synthetic-secret-token".to_string();
    assert!(assert_material_privacy(&unsafe_selected).is_err());
    let mut unsafe_relationship = material;
    unsafe_relationship
        .relationship
        .as_mut()
        .expect("expected synthetic relationship")
        .facts[0]
        .text = "cookie=synthetic-secret-cookie".to_string();
    assert!(assert_material_privacy(&unsafe_relationship).is_err());
    for forbidden in [
        "token=synthetic-value",
        "credential: synthetic-value",
        "AUTHORIZATION synthetic-value",
        "Cookie = synthetic-value",
    ] {
        let mut unsafe_material = unsafe_relationship.clone();
        unsafe_material.selected_facts[0].text = forbidden.to_string();
        assert!(
            assert_material_privacy(&unsafe_material).is_err(),
            "accepted forbidden outbound text: {forbidden}"
        );
    }
    Ok(())
}
