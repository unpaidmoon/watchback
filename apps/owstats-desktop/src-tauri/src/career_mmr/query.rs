use super::{
    input::ModeFamily, rating::outcome_delta_for_expected, replay::PERFORMANCE_MODEL_VERSION,
};
use crate::{
    CareerMmrBuildStatus, CareerMmrExplanationFactor, CareerMmrHistory, CareerMmrHistoryPoint,
    CareerMmrHistoryRequest, CareerMmrMatchExplanation, CareerMmrModeBreakdown, CareerMmrModeItem,
    CareerMmrOverview, CareerMmrRecentExplanation, CareerMmrRecentExplanations,
    CareerMmrRoleSummary, CareerMmrSeasonBreakdown, CareerMmrSeasonItem, OwDatabase,
};
use anyhow::{bail, Result};
use rusqlite::{params, OptionalExtension};
use std::collections::BTreeMap;

const ROLES: [&str; 3] = ["tank", "damage", "support"];

fn active_version(conn: &rusqlite::Connection) -> Result<Option<i64>> {
    Ok(conn.query_row("SELECT version FROM mmr_model_versions WHERE is_active=1 ORDER BY version DESC LIMIT 1", [], |r| r.get(0)).optional()?)
}
fn role_valid(role: &str) -> Result<()> {
    if ROLES.contains(&role) {
        Ok(())
    } else {
        bail!("职责参数无效")
    }
}
fn confidence_label(value: f64) -> String {
    match value {
        v if v < 0.3 => "定位中",
        v if v < 0.6 => "初步估计",
        v if v < 0.8 => "较稳定",
        _ => "高置信度",
    }
    .into()
}

fn tier_label(tier: Option<&str>) -> Option<String> {
    match tier? {
        "learned" => Some("约束学习"),
        "shrunk" => Some("强收缩"),
        "percentile" => Some("稳健百分位"),
        _ => None,
    }
    .map(String::from)
}

fn display_mmr(value: i64) -> i64 {
    value.clamp(0, 5000)
}

pub fn get_career_mmr_overview(db: &OwDatabase) -> Result<CareerMmrOverview> {
    db.initialize()?;
    let conn = db.connect()?;
    let version = active_version(&conn)?;
    let mut roles = Vec::new();
    for role in ROLES {
        let found: Option<(i64, f64, Option<i64>, Option<i64>, f64, f64)> = if let Some(v) = version
        {
            conn.query_row("SELECT mmr,confidence,peak_mmr,peak_at,effective_matches,effective_hours FROM career_role_mmr WHERE model_version=?1 AND role=?2", params![v,role], |r| Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?,r.get(5)?))).optional()?
        } else {
            None
        };
        let (current, confidence, peak, peak_at, matches, hours) = found
            .map(|x| (Some(x.0), x.1, x.2, x.3, x.4, x.5))
            .unwrap_or((None, 0.0, None, None, 0.0, 0.0));
        roles.push(CareerMmrRoleSummary {
            role: role.into(),
            current_mmr: current.map(display_mmr),
            confidence,
            confidence_label: confidence_label(confidence),
            peak_mmr: peak.map(display_mmr),
            peak_at,
            effective_matches: matches,
            effective_hours: hours,
        });
    }
    let mut seasons = Vec::new();
    if let Some(v) = version {
        let mut s=conn.prepare("SELECT DISTINCT season FROM career_role_season_summary WHERE model_version=?1 UNION SELECT DISTINCT season FROM career_mmr_events WHERE model_version=?1 ORDER BY season")?;
        seasons = s
            .query_map([v], |r| r.get(0))?
            .collect::<rusqlite::Result<_>>()?;
    }
    let positioned: Vec<_> = roles
        .iter()
        .filter(|r| r.current_mmr.is_some() && r.confidence >= 0.3)
        .collect();
    let composite_mmr = if positioned.is_empty() {
        None
    } else {
        let latest: Option<i64> =
            conn.query_row("SELECT MAX(begin_ts) FROM mmr_match_inputs", [], |r| {
                r.get(0)
            })?;
        let mut career = BTreeMap::<String, f64>::new();
        let mut recent = BTreeMap::<String, f64>::new();
        if let Some(anchor) = latest {
            let cutoff = anchor - 180 * 86_400;
            let mut s=conn.prepare("SELECT role,SUM(duration_sec*role_share*data_quality),SUM(CASE WHEN begin_ts>=?1 THEN duration_sec*role_share*data_quality ELSE 0 END) FROM mmr_match_inputs GROUP BY role")?;
            for row in s.query_map([cutoff], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, f64>(1)?,
                    r.get::<_, f64>(2)?,
                ))
            })? {
                let (role, curr, rec) = row?;
                career.insert(role.clone(), curr);
                recent.insert(role, rec);
            }
        }
        let career_total: f64 = positioned
            .iter()
            .map(|r| career.get(&r.role).copied().unwrap_or(0.0))
            .sum();
        let recent_total: f64 = positioned
            .iter()
            .map(|r| recent.get(&r.role).copied().unwrap_or(0.0))
            .sum();
        let weighted: Vec<_> = positioned
            .iter()
            .map(|r| {
                let c = if career_total > 0.0 {
                    career.get(&r.role).copied().unwrap_or(0.0) / career_total
                } else {
                    1.0 / positioned.len() as f64
                };
                let recent_share = if recent_total > 0.0 {
                    recent.get(&r.role).copied().unwrap_or(0.0) / recent_total
                } else {
                    c
                };
                (r, r.confidence * (0.7 * recent_share + 0.3 * c))
            })
            .collect();
        Some(
            (weighted
                .iter()
                .map(|(r, w)| r.current_mmr.unwrap() as f64 * w)
                .sum::<f64>()
                / weighted.iter().map(|(_, w)| w).sum::<f64>())
            .round() as i64,
        )
    };
    Ok(CareerMmrOverview {
        model_version: version,
        composite_mmr,
        roles,
        discovered_seasons: seasons,
        disclaimer: "Watchback 本地模型推算，并非暴雪官方隐藏分。".into(),
    })
}

pub fn get_role_mmr_history(
    db: &OwDatabase,
    request: CareerMmrHistoryRequest,
) -> Result<CareerMmrHistory> {
    role_valid(&request.role)?;
    db.initialize()?;
    let conn = db.connect()?;
    let version = active_version(&conn)?;
    let granularity = request.granularity.as_deref().unwrap_or("match");
    if !["match", "day", "week"].contains(&granularity) {
        bail!("时间粒度参数无效")
    }
    let limit = request.limit.unwrap_or(500).clamp(1, 2000);
    let mut rows = Vec::new();
    if let Some(v) = version {
        if granularity == "match" {
            let mut stmt=conn.prepare("SELECT begin_ts,season,mu_after,confidence_after,final_delta,input_hash FROM career_mmr_events WHERE model_version=?1 AND role=?2 AND (?3 IS NULL OR season=?3) AND (?4 IS NULL OR begin_ts>=?4) AND (?5 IS NULL OR begin_ts<=?5) AND (?6 IS NULL OR begin_ts<?6 OR (begin_ts=?6 AND input_hash<?7)) ORDER BY begin_ts DESC,input_hash DESC LIMIT ?8")?;
            let mut query = stmt.query(params![
                v,
                request.role,
                request.season,
                request.from,
                request.to,
                request.before,
                request.before_hash,
                limit as i64
            ])?;
            while rows.len() < limit {
                let Some(r) = query.next()? else { break };
                rows.push(CareerMmrHistoryPoint {
                    timestamp: r.get(0)?,
                    season: r.get(1)?,
                    mmr: display_mmr(r.get::<_, f64>(2)?.round() as i64),
                    confidence: r.get(3)?,
                    delta: r.get(4)?,
                    cursor_hash: r.get(5)?,
                });
            }
            rows.reverse();
        } else {
            let bucket = if granularity == "day" {
                "(begin_ts / 86400) * 86400"
            } else {
                "((begin_ts + 259200) / 604800) * 604800 - 259200"
            };
            let event_bucket = bucket.replace("begin_ts", "e.begin_ts");
            let sql=format!("WITH candidates AS MATERIALIZED (SELECT season,{bucket} bucket FROM career_mmr_events WHERE model_version=?1 AND role=?2 AND (?3 IS NULL OR season=?3) AND (?4 IS NULL OR begin_ts>=?4) AND (?5 IS NULL OR begin_ts<=?5) AND (?6 IS NULL OR {bucket}<?6 OR ({bucket}=?6 AND season<?7)) GROUP BY season,bucket ORDER BY bucket DESC,season DESC LIMIT ?8), ranked AS (SELECT e.begin_ts,e.season,e.mu_after,e.confidence_after,e.final_delta,e.input_hash,c.bucket,ROW_NUMBER() OVER(PARTITION BY e.season,c.bucket ORDER BY e.begin_ts DESC,e.input_hash DESC) rn,SUM(e.final_delta) OVER(PARTITION BY e.season,c.bucket) bucket_delta FROM career_mmr_events e JOIN candidates c ON c.season=e.season AND c.bucket={event_bucket} WHERE e.model_version=?1 AND e.role=?2 AND (?3 IS NULL OR e.season=?3) AND (?4 IS NULL OR e.begin_ts>=?4) AND (?5 IS NULL OR e.begin_ts<=?5)) SELECT bucket,season,mu_after,confidence_after,bucket_delta,input_hash FROM ranked WHERE rn=1 ORDER BY bucket DESC,season DESC");
            let mut stmt = conn.prepare(&sql)?;
            let mut query = stmt.query(params![
                v,
                request.role,
                request.season,
                request.from,
                request.to,
                request.before_bucket,
                request.before_season,
                limit as i64
            ])?;
            while let Some(r) = query.next()? {
                rows.push(CareerMmrHistoryPoint {
                    timestamp: r.get(0)?,
                    season: r.get(1)?,
                    mmr: display_mmr(r.get::<_, f64>(2)?.round() as i64),
                    confidence: r.get(3)?,
                    delta: r.get(4)?,
                    cursor_hash: r.get(5)?,
                });
            }
            rows.reverse();
        }
    }
    Ok(CareerMmrHistory {
        role: request.role,
        granularity: granularity.into(),
        points: rows,
    })
}

pub fn get_role_mmr_mode_breakdown(db: &OwDatabase, role: &str) -> Result<CareerMmrModeBreakdown> {
    role_valid(role)?;
    db.initialize()?;
    let c = db.connect()?;
    let mut items = Vec::new();
    if let Some(v) = active_version(&c)? {
        let mut s=c.prepare("SELECT mode_family,matches_count,duration_sec,CAST(wins AS INTEGER),CAST(losses AS INTEGER),draws,cumulative_delta FROM career_role_mode_summary WHERE model_version=?1 AND role=?2 ORDER BY matches_count DESC,mode_family")?;
        items = s
            .query_map(params![v, role], |r| {
                Ok(CareerMmrModeItem {
                    mode_family: r.get(0)?,
                    matches_count: r.get(1)?,
                    duration_sec: r.get(2)?,
                    wins: r.get(3)?,
                    losses: r.get(4)?,
                    draws: r.get(5)?,
                    cumulative_delta: r.get(6)?,
                })
            })?
            .collect::<rusqlite::Result<_>>()?;
    }
    Ok(CareerMmrModeBreakdown {
        role: role.into(),
        items,
    })
}
pub fn get_role_mmr_season_breakdown(
    db: &OwDatabase,
    role: &str,
) -> Result<CareerMmrSeasonBreakdown> {
    role_valid(role)?;
    db.initialize()?;
    let c = db.connect()?;
    let mut items = Vec::new();
    if let Some(v) = active_version(&c)? {
        let mut s=c.prepare("SELECT season,matches_count,start_mmr,end_mmr,peak_mmr,net_delta,first_match_at,last_match_at FROM career_role_season_summary WHERE model_version=?1 AND role=?2 ORDER BY season")?;
        items = s
            .query_map(params![v, role], |r| {
                Ok(CareerMmrSeasonItem {
                    season: r.get(0)?,
                    matches_count: r.get(1)?,
                    start_mmr: display_mmr(r.get(2)?),
                    end_mmr: display_mmr(r.get(3)?),
                    peak_mmr: display_mmr(r.get(4)?),
                    net_delta: r.get(5)?,
                    first_match_at: r.get(6)?,
                    last_match_at: r.get(7)?,
                })
            })?
            .collect::<rusqlite::Result<_>>()?;
    }
    Ok(CareerMmrSeasonBreakdown {
        role: role.into(),
        items,
    })
}

pub fn get_mmr_match_explanation(
    db: &OwDatabase,
    match_id: &str,
    role: &str,
) -> Result<CareerMmrMatchExplanation> {
    role_valid(role)?;
    db.initialize()?;
    let c = db.connect()?;
    let Some(v) = active_version(&c)? else {
        bail!("未找到该场 MMR 记录")
    };
    let row=c.query_row("SELECT base_delta,performance_adjustment,final_delta,data_quality,expected_win_rate,role_share,sigma_before,result,mode_family,performance_tier FROM career_mmr_events WHERE model_version=?1 AND match_id=?2 AND role=?3",params![v,match_id,role],|r|Ok((r.get::<_,f64>(0)?,r.get::<_,f64>(1)?,r.get::<_,f64>(2)?,r.get::<_,f64>(3)?,r.get::<_,f64>(4)?,r.get::<_,f64>(5)?,r.get::<_,f64>(6)?,r.get::<_,f64>(7)?,r.get::<_,String>(8)?,r.get::<_,Option<String>>(9)?))).optional()?;
    let Some((
        base,
        performance,
        final_delta,
        quality,
        expected,
        role_share,
        sigma,
        result,
        mode,
        tier,
    )) = row
    else {
        bail!("未找到该场 MMR 记录")
    };
    build_explanation(
        role,
        base,
        performance,
        final_delta,
        quality,
        expected,
        role_share,
        sigma,
        result,
        &mode,
        tier.as_deref(),
    )
}

#[allow(clippy::too_many_arguments)]
fn build_explanation(
    role: &str,
    base: f64,
    performance: f64,
    final_delta: f64,
    quality: f64,
    expected: f64,
    role_share: f64,
    sigma: f64,
    result: f64,
    mode: &str,
    tier: Option<&str>,
) -> Result<CareerMmrMatchExplanation> {
    let neutral =
        outcome_delta_for_expected(sigma, mode_family(mode)?, role_share, quality, result, 0.5);
    let difficulty = base - neutral;
    let mut factors = vec![
        CareerMmrExplanationFactor {
            kind: "delta".into(),
            label: "胜负基础变化".into(),
            value: neutral,
            unit: "mmr".into(),
        },
        CareerMmrExplanationFactor {
            kind: "delta".into(),
            label: "对局难度影响".into(),
            value: difficulty,
            unit: "mmr".into(),
        },
        CareerMmrExplanationFactor {
            kind: "delta".into(),
            label: "个人表现修正".into(),
            value: performance,
            unit: "mmr".into(),
        },
        CareerMmrExplanationFactor {
            kind: "tier".into(),
            label: tier_label(tier).unwrap_or_else(|| "未启用".into()),
            value: 0.0,
            unit: "tier".into(),
        },
        CareerMmrExplanationFactor {
            kind: "rate".into(),
            label: "预期胜率".into(),
            value: expected * 100.0,
            unit: "percent".into(),
        },
        CareerMmrExplanationFactor {
            kind: "rate".into(),
            label: "数据完整度".into(),
            value: quality * 100.0,
            unit: "percent".into(),
        },
        CareerMmrExplanationFactor {
            kind: "rate".into(),
            label: "职责占比".into(),
            value: role_share * 100.0,
            unit: "percent".into(),
        },
    ];
    let closing_adjustment = final_delta - base - performance;
    if closing_adjustment.abs() > 1e-9 {
        factors.push(CareerMmrExplanationFactor {
            kind: "delta".into(),
            label: "最终封顶调整".into(),
            value: closing_adjustment,
            unit: "mmr".into(),
        });
    }
    factors.push(CareerMmrExplanationFactor {
        kind: "delta".into(),
        label: "最终变化".into(),
        value: final_delta,
        unit: "mmr".into(),
    });
    Ok(CareerMmrMatchExplanation {
        role: role.into(),
        summary: format!("本场 MMR {final_delta:+.0}"),
        factors,
        data_quality: quality,
    })
}

pub fn get_recent_mmr_explanations(
    db: &OwDatabase,
    role: &str,
    limit: usize,
) -> Result<CareerMmrRecentExplanations> {
    role_valid(role)?;
    db.initialize()?;
    let conn = db.connect()?;
    let mut items = Vec::new();
    if let Some(version) = active_version(&conn)? {
        let mut stmt = conn.prepare("SELECT begin_ts,season,base_delta,performance_adjustment,final_delta,data_quality,expected_win_rate,role_share,sigma_before,result,mode_family,performance_tier FROM career_mmr_events WHERE model_version=?1 AND role=?2 ORDER BY begin_ts DESC,input_hash DESC LIMIT ?3")?;
        let rows = stmt.query_map(params![version, role, limit.clamp(1, 20) as i64], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, i64>(1)?,
                row.get::<_, f64>(2)?,
                row.get::<_, f64>(3)?,
                row.get::<_, f64>(4)?,
                row.get::<_, f64>(5)?,
                row.get::<_, f64>(6)?,
                row.get::<_, f64>(7)?,
                row.get::<_, f64>(8)?,
                row.get::<_, f64>(9)?,
                row.get::<_, String>(10)?,
                row.get::<_, Option<String>>(11)?,
            ))
        })?;
        for row in rows {
            let (
                timestamp,
                season,
                base,
                performance,
                delta,
                quality,
                expected,
                share,
                sigma,
                result,
                mode,
                tier,
            ) = row?;
            let explanation = build_explanation(
                role,
                base,
                performance,
                delta,
                quality,
                expected,
                share,
                sigma,
                result,
                &mode,
                tier.as_deref(),
            )?;
            items.push(CareerMmrRecentExplanation {
                timestamp,
                season,
                delta,
                summary: explanation.summary,
                factors: explanation.factors,
                data_quality: explanation.data_quality,
            });
        }
    }
    Ok(CareerMmrRecentExplanations {
        role: role.into(),
        items,
    })
}
fn mode_family(value: &str) -> Result<ModeFamily> {
    ModeFamily::from_stored_str(value).ok_or_else(|| anyhow::anyhow!("模式数据无效"))
}

#[cfg(test)]
mod mode_family_tests {
    use super::mode_family;
    use crate::career_mmr::input::ModeFamily;

    #[test]
    fn parses_six_stable_names_and_legacy_quick_play() {
        for (value, expected) in [
            ("role_competitive", ModeFamily::RoleCompetitive),
            ("role_quick", ModeFamily::RoleQuick),
            ("open_competitive", ModeFamily::OpenCompetitive),
            ("open_quick", ModeFamily::OpenQuick),
            ("stadium_competitive", ModeFamily::StadiumCompetitive),
            ("stadium_quick", ModeFamily::StadiumQuick),
            ("quick_play", ModeFamily::RoleQuick),
        ] {
            assert_eq!(ModeFamily::from_stored_str(value), Some(expected));
            assert_eq!(mode_family(value).unwrap(), expected);
        }
        assert_eq!(ModeFamily::from_stored_str("unknown"), None);
    }
}
pub fn get_career_mmr_build_status(db: &OwDatabase) -> Result<CareerMmrBuildStatus> {
    db.initialize()?;
    let c = db.connect()?;
    let v = active_version(&c)?;
    let count = if let Some(x) = v {
        c.query_row(
            "SELECT COUNT(*) FROM career_role_mmr WHERE model_version=?1",
            [x],
            |r| r.get(0),
        )?
    } else {
        0
    };
    let needs_rebuild = v
        .map(|version| version < PERFORMANCE_MODEL_VERSION)
        .unwrap_or(true);
    Ok(CareerMmrBuildStatus {
        model_version: v,
        target_model_version: PERFORMANCE_MODEL_VERSION,
        needs_rebuild,
        summary_count: count,
        is_built: v == Some(PERFORMANCE_MODEL_VERSION) && count == 3,
    })
}
