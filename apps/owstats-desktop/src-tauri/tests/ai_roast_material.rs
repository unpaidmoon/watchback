use anyhow::Result;
use owstats_desktop_lib::{
    ai_roast::material::{
        assert_material_privacy, build_roast_material, RoastMaterial, RoastRelationshipKind,
        MATERIAL_VERSION,
    },
    OwDatabase,
};
use rusqlite::{params, Connection};
use tempfile::tempdir;

const OUTPUT_HEAVY_SUPPORT_GUID: &str = "207165582859043234";

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
    conn.execute(
        "INSERT INTO matches(match_id,season,bucket,game_mode,match_kind,result,map_guid,hero_guid,begin_ts,team_score,opponent_score,kills,assists,deaths,damage,healing,mitigation,total_worth,duration_sec,raw_entry_json,updated_at) VALUES(?1,?2,'preset','SportPreset','normal',?3,'synthetic-map','synthetic-hero',?4,2,1,0,0,0,0,0,0,0,600,'{\"credential\":\"synthetic-secret-token\"}','now')",
        params![match_id, season, result, begin_ts],
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

fn assert_grounded(material: &RoastMaterial, kind: RoastRelationshipKind) {
    assert_eq!(material.material_version, MATERIAL_VERSION);
    assert_eq!(material.relationship.kind, kind);
    assert!((3..=4).contains(&material.relationship.facts.len()));
    assert!(material
        .relationship
        .facts
        .iter()
        .all(|fact| fact.strength > 0.0));
    assert!(material
        .relationship
        .facts
        .iter()
        .any(|fact| fact.id.contains("rank") || fact.id.contains("same_role")));
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
    assert_grounded(&material, RoastRelationshipKind::WeakLink);
    assert_eq!(material.relationship.subject, "我方输出位2号");
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

    assert_grounded(&material, RoastRelationshipKind::Carry);
    assert_eq!(material.relationship.subject, "我方输出位2号");
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

    assert_grounded(&material, RoastRelationshipKind::Scapegoat);
    assert_eq!(material.relationship.subject, "我方输出位2号");
    assert!(!material.relationship.counter_facts.is_empty());
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

    assert_grounded(&material, RoastRelationshipKind::SelfRoast);
    assert_eq!(material.relationship.subject, "当前玩家（支援位1号）");
    assert!(material
        .relationship
        .facts
        .iter()
        .any(|fact| fact.id.contains("same_role_healing")));
    assert!(
        material
            .relationship
            .facts
            .iter()
            .any(|fact| fact.text.contains("治疗") && fact.text.contains("伤害")),
        "selected facts: {:?}",
        material.relationship.facts
    );
    assert!(material
        .relationship
        .facts
        .iter()
        .all(|fact| !fact.text.contains("导致")));
    assert!(material
        .relationship
        .facts
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
fn lazily_evaluates_current_match_and_uses_neutral_win_loss_draw_fallbacks() -> Result<()> {
    let (_dir, db) = database()?;
    let conn = db.connect()?;
    for (index, (match_id, result, expected_tag, expected_event)) in [
        ("synthetic-neutral-win", "win", "胜利局", "本场获胜"),
        (
            "synthetic-neutral-loss",
            "loss",
            "失败局",
            "结束1场连胜并失利",
        ),
        ("synthetic-neutral-draw", "draw", "平局", "平局中断连续结果"),
    ]
    .into_iter()
    .enumerate()
    {
        seed_match(&conn, match_id, 23, result, 2_000 + index as i64)?;
        let material = build_roast_material(&conn, match_id)?;
        assert_eq!(material.evaluation.primary_tag, expected_tag);
        assert_grounded(&material, RoastRelationshipKind::Neutral);
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
fn rejects_unsafe_cached_evaluation_strings_before_serialization() -> Result<()> {
    let (_dir, db) = database()?;
    let conn = db.connect()?;
    let match_id = "synthetic-unsafe-evaluation";
    seed_match(&conn, match_id, 23, "win", 3_000)?;
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
    let mut unsafe_material = material;
    unsafe_material.relationship.facts[0].text = "customerToken=synthetic-secret-token".to_string();
    assert!(assert_material_privacy(&unsafe_material).is_err());
    Ok(())
}
