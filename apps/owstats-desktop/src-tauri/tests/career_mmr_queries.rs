use anyhow::Result;
use owstats_desktop_lib::{
    career_mmr::query::{
        get_career_mmr_build_status, get_career_mmr_overview, get_mmr_match_explanation,
        get_recent_mmr_explanations, get_role_mmr_history, get_role_mmr_mode_breakdown,
        get_role_mmr_season_breakdown,
    },
    CareerMmrHistoryRequest, OwDatabase,
};
use rusqlite::{params, Connection};
use serde::Serialize;
use tempfile::TempDir;

fn database() -> Result<(TempDir, OwDatabase)> {
    let temp = TempDir::new()?;
    let db = OwDatabase::new(temp.path().join("queries.sqlite"));
    db.initialize()?;
    Ok((temp, db))
}

fn seed_model(conn: &Connection) -> Result<()> {
    seed_model_version(conn, 1)
}

fn seed_model_version(conn: &Connection, version: i64) -> Result<()> {
    conn.execute("UPDATE mmr_model_versions SET is_active=0", [])?;
    conn.execute(
        "INSERT INTO mmr_model_versions(version,algorithm,config_json,is_active,created_at) VALUES(?1,'test','{}',1,'now')",
        [version],
    )?;
    Ok(())
}

fn seed_role(conn: &Connection, role: &str, mmr: i64, confidence: f64) -> Result<()> {
    seed_role_version(conn, 1, role, mmr, confidence)
}

fn seed_role_version(
    conn: &Connection,
    version: i64,
    role: &str,
    mmr: i64,
    confidence: f64,
) -> Result<()> {
    conn.execute("INSERT INTO career_role_mmr(model_version,role,mmr,mu,sigma,confidence,effective_matches,effective_hours,peak_mmr,peak_at,last_played_at,updated_at) VALUES(?1,?2,?3,?3,100,?4,12.5,2.0,?5,200,200,'now')", params![version, role, mmr, confidence, mmr + 50])?;
    Ok(())
}

fn assert_private<T: Serialize>(value: &T) {
    let json = serde_json::to_string(value).unwrap().to_lowercase();
    for forbidden in [
        "token",
        "customertoken",
        "cookie",
        "authorization",
        "credential",
        "bnetid",
        "playerid",
        "rawjson",
        "synth-secret-777",
    ] {
        assert!(
            !json.contains(forbidden),
            "serialized DTO leaked {forbidden}: {json}"
        );
    }
}

#[test]
fn empty_overview_always_has_three_roles_and_chinese_disclaimer() -> Result<()> {
    let (_temp, db) = database()?;
    let overview = get_career_mmr_overview(&db)?;
    assert_eq!(
        overview
            .roles
            .iter()
            .map(|r| r.role.as_str())
            .collect::<Vec<_>>(),
        ["tank", "damage", "support"]
    );
    assert!(overview
        .roles
        .iter()
        .all(|r| r.confidence_label == "定位中"));
    assert!(overview.disclaimer.contains("并非暴雪官方隐藏分"));
    assert_private(&overview);
    Ok(())
}

#[test]
fn overview_uses_seeded_roles_and_dynamic_seasons() -> Result<()> {
    let (_temp, db) = database()?;
    let conn = db.connect()?;
    seed_model(&conn)?;
    seed_role(&conn, "tank", 2510, 0.72)?;
    for season in [15, 20, 23] {
        conn.execute("INSERT INTO career_role_season_summary VALUES(1,?1,'tank',5,2400,2500,2550,100,1,2,'now')", [season])?;
    }
    let overview = get_career_mmr_overview(&db)?;
    assert_eq!(overview.discovered_seasons, vec![15, 20, 23]);
    assert_eq!(overview.roles[0].current_mmr, Some(2510));
    assert_eq!(overview.roles[0].peak_at, Some(200));
    assert_eq!(overview.roles[0].confidence_label, "较稳定");
    assert_private(&overview);
    Ok(())
}

#[test]
fn history_filters_display_without_changing_overview_current() -> Result<()> {
    let (_temp, db) = database()?;
    let conn = db.connect()?;
    seed_model(&conn)?;
    seed_role(&conn, "damage", 2700, 0.8)?;
    conn.pragma_update(None, "foreign_keys", "OFF")?;
    for (id, season, ts, after) in [("a", 20, 100, 2500.0), ("b", 23, 200, 2700.0)] {
        conn.execute("INSERT INTO career_mmr_events (model_version,match_id,role,season,begin_ts,mode_family,role_share,result,expected_win_rate,data_quality,performance_adjustment,mu_before,sigma_before,base_delta,final_delta,mu_after,sigma_after,confidence_after,input_hash,computed_at) VALUES(1,?1,'damage',?2,?3,'role_competitive',1,1,.5,.8,0,2400,200,10,10,?4,190,.8,'hash','now')", params![id, season, ts, after])?;
    }
    let history = get_role_mmr_history(
        &db,
        CareerMmrHistoryRequest {
            role: "damage".into(),
            season: Some(20),
            from: None,
            to: None,
            granularity: Some("match".into()),
            limit: None,
            before: None,
            before_hash: None,
            before_bucket: None,
            before_season: None,
        },
    )?;
    assert_eq!(history.points.len(), 1);
    assert_eq!(
        get_career_mmr_overview(&db)?.roles[1].current_mmr,
        Some(2700)
    );
    assert_private(&history);
    Ok(())
}

#[test]
fn breakdowns_and_explanation_only_report_stored_factors() -> Result<()> {
    let (_temp, db) = database()?;
    let conn = db.connect()?;
    seed_model(&conn)?;
    seed_role(&conn, "support", 2600, 0.65)?;
    conn.execute("INSERT INTO career_role_mode_summary VALUES(1,'support','role_competitive',3,3600,1,1,1,24,'now')", [])?;
    conn.execute("INSERT INTO career_role_season_summary VALUES(1,23,'support',4,2500,2600,2620,100,1,2,'now')", [])?;
    conn.pragma_update(None, "foreign_keys", "OFF")?;
    conn.execute("INSERT INTO career_mmr_events (model_version,match_id,role,season,begin_ts,mode_family,role_share,result,expected_win_rate,data_quality,performance_adjustment,mu_before,sigma_before,base_delta,final_delta,mu_after,sigma_after,confidence_after,input_hash,computed_at) VALUES(1,'safe-local-match','support',23,2,'role_competitive',1,1,.6,.82,0,2500,200,8,8,2508,190,.65,'hash','now')", [])?;
    let modes = get_role_mmr_mode_breakdown(&db, "support")?;
    let seasons = get_role_mmr_season_breakdown(&db, "support")?;
    let explanation = get_mmr_match_explanation(&db, "safe-local-match", "support")?;
    assert_eq!(
        (
            modes.items[0].wins,
            modes.items[0].losses,
            modes.items[0].draws
        ),
        (1, 1, 1)
    );
    assert_eq!(modes.items[0].cumulative_delta, 24.0);
    assert_eq!(seasons.items[0].season, 23);
    assert!(explanation.summary.contains("+8"));
    let neutral = explanation
        .factors
        .iter()
        .find(|f| f.label == "胜负基础变化")
        .unwrap()
        .value;
    let difficulty = explanation
        .factors
        .iter()
        .find(|f| f.label == "对局难度影响")
        .unwrap()
        .value;
    assert!((neutral + difficulty - 8.0).abs() < 1e-9);
    assert!(explanation.factors.iter().any(|f| f.kind == "rate"
        && f.label == "预期胜率"
        && f.value == 60.0
        && f.unit == "percent"));
    assert!(explanation.factors.iter().any(|f| f.kind == "rate"
        && f.label == "数据完整度"
        && f.value == 82.0
        && f.unit == "percent"));
    assert!(explanation.factors.iter().any(|f| f.kind == "rate"
        && f.label == "职责占比"
        && f.value == 100.0
        && f.unit == "percent"));
    assert!(explanation
        .factors
        .iter()
        .any(|f| f.kind == "delta" && f.label == "最终变化" && f.value == 8.0 && f.unit == "mmr"));
    assert!(!explanation
        .factors
        .iter()
        .any(|f| f.label.contains("对局强度")));
    assert_private(&modes);
    assert_private(&seasons);
    assert_private(&explanation);
    Ok(())
}

#[test]
fn recent_explanations_are_real_ordered_and_privacy_safe() -> Result<()> {
    let (_temp, db) = database()?;
    let conn = db.connect()?;
    seed_model(&conn)?;
    conn.pragma_update(None, "foreign_keys", "OFF")?;
    for (id, ts, base, final_delta) in [
        ("private-first", 100, 8.0, 8.0),
        ("private-latest", 200, 30.0, 20.0),
    ] {
        conn.execute("INSERT INTO career_mmr_events (model_version,match_id,role,season,begin_ts,mode_family,role_share,result,expected_win_rate,data_quality,performance_adjustment,mu_before,sigma_before,base_delta,final_delta,mu_after,sigma_after,confidence_after,input_hash,computed_at) VALUES(1,?1,'tank',23,?2,'role_competitive',1,1,.6,1,0,2500,833,?3,?4,2520,800,.5,'private-hash','now')", params![id, ts, base, final_delta])?;
    }
    let recent = get_recent_mmr_explanations(&db, "tank", 1)?;
    assert_eq!(recent.role, "tank");
    assert_eq!(recent.items.len(), 1);
    assert_eq!(recent.items[0].timestamp, 200);
    assert_eq!(recent.items[0].delta, 20.0);
    assert!(recent.items[0]
        .factors
        .iter()
        .any(|factor| factor.label == "最终封顶调整" && factor.value == -10.0));
    let json = serde_json::to_string(&recent)?;
    assert!(!json.contains("private-first"));
    assert!(!json.contains("private-latest"));
    assert!(!json.contains("private-hash"));
    assert_private(&recent);
    Ok(())
}

#[test]
fn missing_explanation_is_chinese_and_build_status_is_derived() -> Result<()> {
    let (_temp, db) = database()?;
    let error = get_mmr_match_explanation(&db, "missing", "tank")
        .unwrap_err()
        .to_string();
    assert!(error.contains("未找到"));
    let status = get_career_mmr_build_status(&db)?;
    assert_eq!(status.target_model_version, 3);
    assert!(status.needs_rebuild);
    assert_eq!(status.summary_count, 0);
    assert!(!status.is_built);
    let conn = db.connect()?;
    seed_model(&conn)?;
    seed_role(&conn, "tank", 2500, 0.8)?;
    let partial = get_career_mmr_build_status(&db)?;
    assert_eq!(partial.summary_count, 1);
    assert!(!partial.is_built);
    seed_role(&conn, "damage", 2500, 0.8)?;
    seed_role(&conn, "support", 2500, 0.8)?;
    let legacy_complete = get_career_mmr_build_status(&db)?;
    assert_eq!(legacy_complete.model_version, Some(1));
    assert_eq!(legacy_complete.target_model_version, 3);
    assert!(legacy_complete.needs_rebuild);
    assert_eq!(legacy_complete.summary_count, 3);
    assert!(!legacy_complete.is_built);

    seed_model_version(&conn, 2)?;
    let current_empty = get_career_mmr_build_status(&db)?;
    assert_eq!(current_empty.model_version, Some(2));
    assert_eq!(current_empty.target_model_version, 3);
    assert!(current_empty.needs_rebuild);
    assert_eq!(current_empty.summary_count, 0);
    assert!(!current_empty.is_built);
    for role in ["tank", "damage", "support"] {
        seed_role_version(&conn, 2, role, 2500, 0.8)?;
    }
    assert!(!get_career_mmr_build_status(&db)?.is_built);

    seed_model_version(&conn, 3)?;
    for role in ["tank", "damage", "support"] {
        seed_role_version(&conn, 3, role, 2500, 0.8)?;
    }
    let future_complete = get_career_mmr_build_status(&db)?;
    assert_eq!(future_complete.model_version, Some(3));
    assert_eq!(future_complete.target_model_version, 3);
    assert!(!future_complete.needs_rebuild);
    assert_eq!(future_complete.summary_count, 3);
    assert!(future_complete.is_built);
    assert_private(&status);
    Ok(())
}

#[test]
fn composite_uses_recent_and_career_effective_time_weights() -> Result<()> {
    let (_temp, db) = database()?;
    let conn = db.connect()?;
    seed_model(&conn)?;
    seed_role(&conn, "tank", 1000, 0.8)?;
    seed_role(&conn, "damage", 3000, 0.8)?;
    conn.pragma_update(None, "foreign_keys", "OFF")?;
    conn.execute("INSERT INTO mmr_match_inputs(match_id,role,season,begin_ts,mode_family,result,duration_sec,role_share,role_source,data_quality,team_strength,opponent_strength,rank_coverage,performance_features_json,input_hash) VALUES('recent-tank','tank',23,20000000,'role_competitive',1,10000,1,'test',1,NULL,NULL,0,'{}','h')",[])?;
    conn.execute("INSERT INTO mmr_match_inputs(match_id,role,season,begin_ts,mode_family,result,duration_sec,role_share,role_source,data_quality,team_strength,opponent_strength,rank_coverage,performance_features_json,input_hash) VALUES('old-damage','damage',20,1,'role_competitive',1,10000,1,'test',1,NULL,NULL,0,'{}','h')",[])?;
    let composite = get_career_mmr_overview(&db)?.composite_mmr.unwrap();
    assert!(composite < 2000, "{composite}");
    Ok(())
}

#[test]
fn history_aggregates_by_season_bucket_and_limits_latest_points() -> Result<()> {
    let (_temp, db) = database()?;
    let conn = db.connect()?;
    seed_model(&conn)?;
    conn.pragma_update(None, "foreign_keys", "OFF")?;
    for (id, season, ts, delta, mmr) in [
        ("a", 20, 100, 2.0, 2502.0),
        ("b", 20, 200, 3.0, 2505.0),
        ("c", 23, 300, 4.0, 2509.0),
    ] {
        conn.execute("INSERT INTO career_mmr_events (model_version,match_id,role,season,begin_ts,mode_family,role_share,result,expected_win_rate,data_quality,performance_adjustment,mu_before,sigma_before,base_delta,final_delta,mu_after,sigma_after,confidence_after,input_hash,computed_at) VALUES(1,?1,'tank',?2,?3,'role_competitive',1,1,.5,1,0,2500,200,?4,?4,?5,190,.8,'h','now')",params![id,season,ts,delta,mmr])?;
    }
    let day = get_role_mmr_history(
        &db,
        CareerMmrHistoryRequest {
            role: "tank".into(),
            season: None,
            from: None,
            to: None,
            granularity: Some("day".into()),
            limit: None,
            before: None,
            before_hash: None,
            before_bucket: None,
            before_season: None,
        },
    )?;
    assert_eq!(day.points.len(), 2);
    assert_eq!(day.points[0].delta, 5.0);
    assert_eq!(day.points[0].mmr, 2505);
    let aggregate_request = |before_bucket, before_season| CareerMmrHistoryRequest {
        role: "tank".into(),
        season: None,
        from: None,
        to: None,
        granularity: Some("day".into()),
        limit: Some(1),
        before: None,
        before_hash: None,
        before_bucket,
        before_season,
    };
    let newest = get_role_mmr_history(&db, aggregate_request(None, None))?;
    assert_eq!(newest.points[0].season, 23);
    let older = get_role_mmr_history(
        &db,
        aggregate_request(Some(newest.points[0].timestamp), Some(23)),
    )?;
    assert_eq!(older.points[0].season, 20);
    let limited = get_role_mmr_history(
        &db,
        CareerMmrHistoryRequest {
            role: "tank".into(),
            season: None,
            from: None,
            to: None,
            granularity: Some("match".into()),
            limit: Some(2),
            before: None,
            before_hash: None,
            before_bucket: None,
            before_season: None,
        },
    )?;
    assert_eq!(
        limited
            .points
            .iter()
            .map(|p| p.timestamp)
            .collect::<Vec<_>>(),
        vec![200, 300]
    );
    Ok(())
}

#[test]
fn history_clamps_match_day_and_week_mmr_to_public_display_range() -> Result<()> {
    let (_temp, db) = database()?;
    let conn = db.connect()?;
    seed_model(&conn)?;
    conn.pragma_update(None, "foreign_keys", "OFF")?;
    for (id, ts, mmr) in [("too-low", 100, -500.0), ("too-high", 200, 5500.0)] {
        conn.execute("INSERT INTO career_mmr_events (model_version,match_id,role,season,begin_ts,mode_family,role_share,result,expected_win_rate,data_quality,performance_adjustment,mu_before,sigma_before,base_delta,final_delta,mu_after,sigma_after,confidence_after,input_hash,computed_at) VALUES(1,?1,'tank',23,?2,'role_competitive',1,1,.5,1,0,2500,200,10,10,?3,190,.8,'h','now')",params![id,ts,mmr])?;
    }
    let request = |granularity: &str| CareerMmrHistoryRequest {
        role: "tank".into(),
        season: None,
        from: None,
        to: None,
        granularity: Some(granularity.into()),
        limit: Some(10),
        before: None,
        before_hash: None,
        before_bucket: None,
        before_season: None,
    };
    for granularity in ["match", "day", "week"] {
        let history = get_role_mmr_history(&db, request(granularity))?;
        assert!(history
            .points
            .iter()
            .all(|point| (0..=5000).contains(&point.mmr)));
        assert!(history.points.iter().any(|point| point.mmr == 5000));
        if granularity == "match" {
            assert!(history.points.iter().any(|point| point.mmr == 0));
        }
    }
    Ok(())
}

#[test]
fn match_cursor_keeps_same_timestamp_rows_and_capped_explanation_closes() -> Result<()> {
    let (_temp, db) = database()?;
    let conn = db.connect()?;
    seed_model(&conn)?;
    conn.pragma_update(None, "foreign_keys", "OFF")?;
    for (id, hash) in [("same-a", "aaa"), ("same-z", "zzz")] {
        conn.execute("INSERT INTO career_mmr_events (model_version,match_id,role,season,begin_ts,mode_family,role_share,result,expected_win_rate,data_quality,performance_adjustment,mu_before,sigma_before,base_delta,final_delta,mu_after,sigma_after,confidence_after,input_hash,computed_at) VALUES(1,?1,'tank',23,500,'role_competitive',1,1,0,1,0,2500,833,20,20,2520,800,.5,?2,'now')",params![id,hash])?;
    }
    let request = |before, before_hash| CareerMmrHistoryRequest {
        role: "tank".into(),
        season: None,
        from: None,
        to: None,
        granularity: Some("match".into()),
        limit: Some(1),
        before,
        before_hash,
        before_bucket: None,
        before_season: None,
    };
    let first = get_role_mmr_history(&db, request(None, None))?;
    assert_eq!(first.points[0].cursor_hash, "zzz");
    let second = get_role_mmr_history(&db, request(Some(500), Some("zzz".into())))?;
    assert_eq!(second.points[0].cursor_hash, "aaa");
    let e = get_mmr_match_explanation(&db, "same-a", "tank")?;
    let sum: f64 = e
        .factors
        .iter()
        .filter(|f| f.kind == "delta" && f.label != "最终变化")
        .map(|f| f.value)
        .sum();
    assert!((sum - 20.0).abs() < 1e-9);
    Ok(())
}

#[test]
fn active_v3_explanation_reports_performance_tier() -> Result<()> {
    let (_temp, db) = database()?;
    let conn = db.connect()?;
    seed_model_version(&conn, 3)?;
    for role in ["tank", "damage", "support"] {
        seed_role_version(&conn, 3, role, 2500, 0.8)?;
    }
    conn.pragma_update(None, "foreign_keys", "OFF")?;
    conn.execute(
        "INSERT INTO career_mmr_events (model_version,match_id,role,season,begin_ts,mode_family,role_share,result,expected_win_rate,data_quality,performance_adjustment,performance_tier,mu_before,sigma_before,base_delta,final_delta,mu_after,sigma_after,confidence_after,input_hash,computed_at) VALUES(3,'m-tier','tank',23,2,'role_competitive',1,1,.6,.82,8,'learned',2500,200,8,16,2516,190,.65,'hash-tier','now')",
        [],
    )?;
    let status = get_career_mmr_build_status(&db)?;
    assert_eq!(status.target_model_version, 3);
    assert!(status.is_built);
    let explanation = get_mmr_match_explanation(&db, "m-tier", "tank")?;
    assert!(explanation
        .factors
        .iter()
        .any(|f| f.kind == "tier" && f.label == "约束学习" && f.unit == "tier"));
    assert!(explanation
        .factors
        .iter()
        .any(|f| f.label == "个人表现修正"));
    assert!(explanation
        .factors
        .iter()
        .any(|f| f.label == "胜负基础变化"));
    let recent = get_recent_mmr_explanations(&db, "tank", 10)?;
    assert!(recent.items.iter().any(|item| item
        .factors
        .iter()
        .any(|f| f.kind == "tier" && f.label == "约束学习" && f.unit == "tier")));
    assert_private(&explanation);
    assert_private(&recent);
    Ok(())
}
