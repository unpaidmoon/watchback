use anyhow::Result;
use owstats_desktop_lib::ai_roast::report::{
    AiRoastReport, ClaimType, RoastInsight, RoastVerdict, TeamInsight, TeamInsightKind,
    REPORT_SCHEMA_VERSION,
};
use owstats_desktop_lib::ai_roast::snapshot::INPUT_SCHEMA_VERSION;
use owstats_desktop_lib::ai_roast::store::{
    replace_roast, roast_for_match, save_roast_once, AiRoastRecord,
};
use owstats_desktop_lib::{get_match_detail, OwDatabase};
use rusqlite::{params, Connection};
use serde_json::json;
use tempfile::tempdir;

fn seed_match(conn: &Connection, match_id: &str, kind: &str) -> Result<()> {
    conn.execute(
        "INSERT INTO matches(match_id,season,bucket,game_mode,match_kind,result,map_guid,hero_guid,begin_ts,team_score,opponent_score,kills,assists,deaths,damage,healing,mitigation,total_worth,duration_sec,raw_entry_json,updated_at) VALUES(?1,23,'competitive','SportPreset',?2,'win','map','hero',100,1,0,1,2,3,4,5,6,7,600,'{\"synthetic\":true}','now')",
        params![match_id, kind],
    )?;
    let detail = if kind == "fight" {
        json!({"payload":{"data":{"totalCount":{"teammateList":[],"enemyList":[]},"roundCountList":[]}}})
    } else {
        json!({"payload":{"data":{"teammateList":[],"enemyList":[]}}})
    };
    conn.execute(
        "INSERT INTO match_details(match_id,match_kind,raw_detail_json,updated_at) VALUES(?1,?2,?3,'now')",
        params![match_id, kind, detail.to_string()],
    )?;
    Ok(())
}

fn roast(match_id: &str, award: &str, commentary: &str) -> AiRoastRecord {
    AiRoastRecord {
        match_id: match_id.to_string(),
        award: award.to_string(),
        commentary: commentary.to_string(),
        report: None,
        used_fact_ids: vec!["fact.synthetic.1".to_string()],
        model: "synthetic-model".to_string(),
        evaluation_rule_version: 1,
        material_version: 2,
        prompt_version: 1,
        report_schema_version: None,
        input_schema_version: None,
        knowledge_version: None,
        created_at: "2026-07-21T00:00:00Z".to_string(),
    }
}

#[test]
fn migrates_v10_through_v12_idempotently_and_preserves_matches() -> Result<()> {
    let dir = tempdir()?;
    let db = OwDatabase::new(dir.path().join("migration.sqlite3"));
    db.initialize()?;
    let conn = db.connect()?;
    seed_match(&conn, "existing", "normal")?;
    conn.execute("DROP TABLE IF EXISTS match_ai_roasts", [])?;
    conn.pragma_update(None, "user_version", 10)?;

    drop(conn);
    db.initialize()?;
    db.initialize()?;
    let conn = db.connect()?;

    assert_eq!(
        conn.pragma_query_value(None, "user_version", |row| row.get::<_, i64>(0))?,
        12
    );
    assert_eq!(
        conn.query_row(
            "SELECT COUNT(*) FROM matches WHERE match_id='existing'",
            [],
            |row| row.get::<_, i64>(0)
        )?,
        1
    );
    let schema: String = conn.query_row(
        "SELECT sql FROM sqlite_master WHERE type='table' AND name='match_ai_roasts'",
        [],
        |row| row.get(0),
    )?;
    assert!(
        schema.contains("match_id TEXT PRIMARY KEY REFERENCES matches(match_id) ON DELETE CASCADE")
    );
    Ok(())
}

fn structured_report() -> AiRoastReport {
    let insight = |title: &str, path: &str| RoastInsight {
        title: title.into(),
        commentary: "合成分析。".into(),
        claim_type: ClaimType::DirectFact,
        evidence_refs: vec![path.into(), "z.path".into(), path.into()],
    };
    AiRoastReport {
        verdict: RoastVerdict {
            title: "结构化标题".into(),
            summary: "结构化总评。".into(),
        },
        decisive_factors: vec![insight("因素一", "b.path"), insight("因素二", "a.path")],
        current_player: insight("你这局", "c.path"),
        team_and_matchups: vec![TeamInsight {
            kind: TeamInsightKind::RoleMatchup,
            insight: insight("对位", "d.path"),
        }],
        closing_roast: "结构化收尾。".into(),
    }
}

#[test]
fn structured_report_round_trips_and_derives_sorted_unique_evidence() -> Result<()> {
    let dir = tempdir()?;
    let db = OwDatabase::new(dir.path().join("structured.sqlite3"));
    db.initialize()?;
    let conn = db.connect()?;
    seed_match(&conn, "structured", "normal")?;
    let report = structured_report();
    let mut record = roast("structured", &report.verdict.title, &report.verdict.summary);
    record.report =
        Some(owstats_desktop_lib::ai_roast::report::StoredRoastPayload::Legacy(report.clone()));
    record.report_schema_version = Some(REPORT_SCHEMA_VERSION);
    record.input_schema_version = Some(INPUT_SCHEMA_VERSION);
    record.knowledge_version = Some(1);
    let view = save_roast_once(&conn, &record)?;
    assert_eq!(view.report, Some(report));
    let stored: String = conn.query_row(
        "SELECT used_fact_ids_json FROM match_ai_roasts WHERE match_id='structured'",
        [],
        |row| row.get(0),
    )?;
    assert_eq!(
        serde_json::from_str::<Vec<String>>(&stored)?,
        vec!["a.path", "b.path", "c.path", "d.path", "z.path"]
    );
    Ok(())
}

#[test]
fn malicious_report_json_is_rejected_and_failed_replace_preserves_old_record() -> Result<()> {
    let dir = tempdir()?;
    let db = OwDatabase::new(dir.path().join("invalid.sqlite3"));
    db.initialize()?;
    let conn = db.connect()?;
    seed_match(&conn, "invalid", "normal")?;
    save_roast_once(&conn, &roast("invalid", "旧标题", "旧正文"))?;
    conn.execute(
        "UPDATE match_ai_roasts SET report_json='{{malicious' WHERE match_id='invalid'",
        [],
    )?;
    assert!(roast_for_match(&conn, "invalid").is_err());
    conn.execute(
        "UPDATE match_ai_roasts SET report_json=NULL WHERE match_id='invalid'",
        [],
    )?;
    let mut bad = roast("invalid", "", "新正文");
    bad.report = Some(
        owstats_desktop_lib::ai_roast::report::StoredRoastPayload::Legacy(structured_report()),
    );
    assert!(replace_roast(&conn, &bad).is_err());
    let old = roast_for_match(&conn, "invalid")?.unwrap();
    assert_eq!(old.award, "旧标题");
    assert_eq!(old.commentary, "旧正文");
    Ok(())
}

#[test]
fn migrates_v11_to_v12_idempotently_without_rewriting_legacy_roasts() -> Result<()> {
    let dir = tempdir()?;
    let db = OwDatabase::new(dir.path().join("v12.sqlite3"));
    db.initialize()?;
    let conn = db.connect()?;
    seed_match(&conn, "legacy", "normal")?;
    conn.execute("INSERT INTO match_ai_roasts(match_id,award,commentary,used_fact_ids_json,model,evaluation_rule_version,material_version,prompt_version,created_at) VALUES('legacy','旧标题','旧正文','[]','legacy-model',1,1,1,'now')",[])?;
    conn.pragma_update(None, "user_version", 11)?;
    drop(conn);
    db.initialize()?;
    db.initialize()?;
    let conn = db.connect()?;
    assert_eq!(
        conn.pragma_query_value(None, "user_version", |row| row.get::<_, i64>(0))?,
        12
    );
    let mut stmt = conn.prepare("PRAGMA table_info(match_ai_roasts)")?;
    let columns = stmt
        .query_map([], |row| row.get::<_, String>(1))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    for column in [
        "report_json",
        "report_schema_version",
        "input_schema_version",
        "knowledge_version",
    ] {
        assert!(columns.contains(&column.to_string()));
    }
    let legacy = roast_for_match(&conn, "legacy")?.unwrap();
    assert_eq!(legacy.award, "旧标题");
    assert!(legacy.report.is_none());
    Ok(())
}

#[test]
fn saves_only_the_first_roast_and_cascades_with_match() -> Result<()> {
    let dir = tempdir()?;
    let db = OwDatabase::new(dir.path().join("save.sqlite3"));
    db.initialize()?;
    let conn = db.connect()?;
    seed_match(&conn, "normal", "normal")?;

    let first = save_roast_once(&conn, &roast("normal", "首发奖", "第一次且永久的锐评"))?;
    let second = save_roast_once(&conn, &roast("normal", "覆盖奖", "不应覆盖"))?;
    assert_eq!(first, second);
    assert_eq!(second.award, "首发奖");
    assert_eq!(roast_for_match(&conn, "normal")?, Some(first));

    conn.execute("DELETE FROM matches WHERE match_id='normal'", [])?;
    assert_eq!(roast_for_match(&conn, "normal")?, None);
    Ok(())
}

#[test]
fn explicit_regeneration_replaces_existing_roast() -> Result<()> {
    let dir = tempdir()?;
    let db = OwDatabase::new(dir.path().join("prompt-upgrade.sqlite3"));
    db.initialize()?;
    let conn = db.connect()?;
    seed_match(&conn, "normal", "normal")?;
    save_roast_once(&conn, &roast("normal", "旧奖", "旧锐评"))?;
    assert_eq!(
        replace_roast(&conn, &roast("normal", "新奖", "新版锐评"))?.award,
        "新奖"
    );
    assert_eq!(
        roast_for_match(&conn, "normal")?.unwrap().commentary,
        "新版锐评"
    );
    Ok(())
}

#[test]
fn normal_and_arena_details_include_roast_without_putting_it_in_raw_json() -> Result<()> {
    let dir = tempdir()?;
    let db = OwDatabase::new(dir.path().join("synthetic.sqlite3"));
    db.initialize()?;
    let conn = db.connect()?;
    seed_match(&conn, "normal", "normal")?;
    seed_match(&conn, "arena", "fight")?;
    save_roast_once(&conn, &roast("normal", "普通奖", "普通锐评正文"))?;
    save_roast_once(&conn, &roast("arena", "角斗奖", "角斗锐评正文"))?;
    drop(conn);

    let normal = get_match_detail(&db, "normal")?;
    assert_eq!(
        normal.ai_roast.as_ref().map(|item| item.award.as_str()),
        Some("普通奖")
    );
    assert!(!normal.raw_json.to_string().contains("普通锐评正文"));
    let arena = get_match_detail(&db, "arena")?;
    assert_eq!(
        arena.ai_roast.as_ref().map(|item| item.award.as_str()),
        Some("角斗奖")
    );
    assert!(!arena.raw_json.to_string().contains("角斗锐评正文"));
    Ok(())
}

#[test]
fn serializes_used_fact_ids_as_valid_json() -> Result<()> {
    let dir = tempdir()?;
    let db = OwDatabase::new(dir.path().join("json.sqlite3"));
    db.initialize()?;
    let conn = db.connect()?;
    seed_match(&conn, "json", "normal")?;
    let mut record = roast("json", "JSON奖", "合法 JSON");
    record.used_fact_ids = vec!["quote:\"".to_string(), "slash:\\".to_string()];
    save_roast_once(&conn, &record)?;
    let stored: String = conn.query_row(
        "SELECT used_fact_ids_json FROM match_ai_roasts WHERE match_id='json'",
        [],
        |row| row.get(0),
    )?;
    assert_eq!(
        serde_json::from_str::<Vec<String>>(&stored)?,
        record.used_fact_ids
    );
    Ok(())
}
