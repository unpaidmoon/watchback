use crate::db::OwDatabase;
use crate::models::*;
use anyhow::{Context, Result};
use rusqlite::params;
use std::collections::HashMap;

const MAP_MIN_SAMPLE: i64 = 2;

#[derive(Debug, Clone)]
struct ReviewMatch {
    match_id: String,
    result: String,
    map_name: String,
    hero_guid: String,
    hero_name: String,
    kills: i64,
    deaths: i64,
    damage: i64,
    healing: i64,
    mitigation: i64,
    duration_sec: i64,
}

impl ReviewMatch {
    fn won(&self) -> bool {
        self.result == "win"
    }

    fn key_stats_missing(&self) -> bool {
        self.kills == 0
            && self.deaths == 0
            && self.damage == 0
            && self.healing == 0
            && self.mitigation == 0
    }
}

#[derive(Debug, Clone, Default)]
struct Aggregate {
    matches: i64,
    wins: i64,
    kills: i64,
    deaths: i64,
    damage: i64,
    healing: i64,
}

impl Aggregate {
    fn add(&mut self, item: &ReviewMatch) {
        self.matches += 1;
        self.wins += if item.won() { 1 } else { 0 };
        self.kills += item.kills;
        self.deaths += item.deaths;
        self.damage += item.damage;
        self.healing += item.healing;
    }

    fn win_rate(&self) -> f64 {
        ratio(self.wins, self.matches)
    }
}

pub fn get_personal_season_review(
    db: &OwDatabase,
    request: PersonalReviewRequest,
) -> Result<PersonalSeasonReview> {
    db.initialize()?;
    let season = selected_season(db, request.season)?;
    let rows = review_rows(db, season)?;
    let threshold = short_match_threshold(&rows);
    let baseline_rows = rows
        .iter()
        .filter(|row| row.duration_sec > threshold && !row.key_stats_missing())
        .cloned()
        .collect::<Vec<_>>();
    let baseline_aggregate = aggregate(&baseline_rows);
    let total_aggregate = aggregate(&rows);
    let baseline = build_baseline(&rows, &baseline_rows, &baseline_aggregate, threshold);
    let personal_bests = personal_bests(&rows);
    let streaks = streaks(&rows);
    let hero_pool = hero_pool(&rows, &baseline_aggregate);
    let maps = map_adaptability(&rows, &baseline_aggregate);
    let top_conclusions = top_conclusions(&personal_bests, &hero_pool, &maps, &baseline);
    let phase5_highlights = phase5_highlights(db, season, &streaks)?;

    Ok(PersonalSeasonReview {
        summary: ReviewSummary {
            season,
            total_matches: total_aggregate.matches,
            baseline_matches: baseline_aggregate.matches,
            wins: total_aggregate.wins,
            losses: total_aggregate.matches - total_aggregate.wins,
            win_rate: round2(total_aggregate.win_rate()),
            avg_kills: round2(avg(total_aggregate.kills, total_aggregate.matches)),
            avg_deaths: round2(avg(total_aggregate.deaths, total_aggregate.matches)),
            avg_damage: round2(avg(total_aggregate.damage, total_aggregate.matches)),
            avg_healing: round2(avg(total_aggregate.healing, total_aggregate.matches)),
        },
        baseline,
        phase5_highlights,
        top_conclusions,
        personal_bests,
        streaks,
        hero_pool,
        maps,
    })
}

fn phase5_highlights(
    db: &OwDatabase,
    season: Option<i64>,
    streaks: &StreakSummary,
) -> Result<Phase5ReviewHighlights> {
    let conn = db.connect()?;
    let mut sql = String::from(
        r#"
        SELECT e.primary_tag, e.grade, e.evidence_tags_json
        FROM match_evaluations e
        JOIN matches m ON m.match_id = e.match_id
        WHERE m.bucket='preset'
        "#,
    );
    let mut rows = if let Some(season) = season {
        sql.push_str(" AND m.season=?");
        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt
            .query_map(params![season], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                ))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        rows
    } else {
        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                ))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        rows
    };

    let carry_count = rows.iter().filter(|row| row.0 == "带飞局").count() as i64;
    let try_hard_count = rows.iter().filter(|row| row.0 == "尽力局").count() as i64;
    let s_grade_count = rows.iter().filter(|row| row.1 == "S").count() as i64;
    let mut highlight_counts = HashMap::<String, i64>::new();
    for (_, _, evidence_text) in rows.drain(..) {
        for tag in serde_json::from_str::<Vec<String>>(&evidence_text).unwrap_or_default() {
            if matches!(
                tag.as_str(),
                "KDA最高" | "击杀最多" | "伤害最高" | "治疗最高" | "格挡最多"
            ) {
                *highlight_counts.entry(tag).or_default() += 1;
            }
        }
    }
    let most_common_highlight = highlight_counts
        .into_iter()
        .max_by(|a, b| a.1.cmp(&b.1).then_with(|| b.0.cmp(&a.0)))
        .map(|item| item.0);

    Ok(Phase5ReviewHighlights {
        carry_count,
        try_hard_count,
        s_grade_count,
        longest_win_streak: streaks.longest_win_streak,
        most_common_highlight,
    })
}

pub fn get_onboarding_status(
    db: &OwDatabase,
    request: OnboardingStatusRequest,
) -> Result<OnboardingStatus> {
    db.initialize()?;
    let conn = db.connect()?;
    let available_seasons = {
        let mut stmt = conn
            .prepare("SELECT DISTINCT season FROM matches WHERE bucket='preset' ORDER BY season")?;
        let rows = stmt
            .query_map([], |row| row.get(0))?
            .collect::<rusqlite::Result<Vec<i64>>>()?;
        rows
    };
    let selected = request
        .season
        .or_else(|| available_seasons.iter().copied().max());
    let (preset_matches, detail_matches, last_seen) = if let Some(season) = selected {
        conn.query_row(
            r#"
            SELECT
                COUNT(m.match_id),
                COUNT(d.match_id),
                MAX(m.updated_at)
            FROM matches m
            LEFT JOIN match_details d ON d.match_id = m.match_id
            WHERE m.bucket='preset' AND m.season=?
            "#,
            params![season],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )?
    } else {
        (0, 0, None)
    };
    let can_enter_review = preset_matches > 0;
    let mut warnings = Vec::new();
    if preset_matches == 0 {
        warnings.push("没有可读取的本地预设/开放赛季数据。".to_string());
    } else if detail_matches < preset_matches {
        warnings.push("部分比赛详情缺失，但仍可进入复盘。".to_string());
    }

    Ok(OnboardingStatus {
        selected_season: selected,
        available_seasons,
        can_enter_review,
        credential_status: CredentialStatus {
            state: "external".to_string(),
            message: "沿用第二阶段外部凭证流程。".to_string(),
            exposes_secret: false,
            role_id: None,
            saved_at: None,
        },
        local_data: LocalDataStatus {
            data_source: "本地 SQLite".to_string(),
            season: selected,
            preset_matches,
            detail_matches,
            last_synced_or_imported_at: last_seen,
            freshness_risk: if can_enter_review {
                "未知".to_string()
            } else {
                "没有本地数据".to_string()
            },
        },
        health: DataHealthStatus {
            readable: can_enter_review,
            warnings,
        },
    })
}

fn selected_season(db: &OwDatabase, requested: Option<i64>) -> Result<Option<i64>> {
    if requested.is_some() {
        return Ok(requested);
    }
    let conn = db.connect()?;
    let season = conn
        .query_row(
            "SELECT season FROM matches WHERE bucket='preset' ORDER BY season DESC LIMIT 1",
            [],
            |row| row.get(0),
        )
        .optional()?;
    Ok(season)
}

fn review_rows(db: &OwDatabase, season: Option<i64>) -> Result<Vec<ReviewMatch>> {
    let conn = db.connect()?;
    let mut sql = String::from(
        r#"
        SELECT match_id, season, result, map_guid, COALESCE(map_name, map_guid),
               hero_guid, COALESCE(hero_name, hero_guid), begin_ts, kills, assists,
               deaths, damage, healing, mitigation, duration_sec
        FROM matches
        WHERE bucket='preset'
        "#,
    );
    if season.is_some() {
        sql.push_str(" AND season=?");
    }
    sql.push_str(" ORDER BY begin_ts ASC, match_id ASC");
    let mut stmt = conn.prepare(&sql)?;
    let mapper = |row: &rusqlite::Row<'_>| {
        Ok(ReviewMatch {
            match_id: row.get(0)?,
            result: row.get(2)?,
            map_name: row.get(4)?,
            hero_guid: row.get(5)?,
            hero_name: row.get(6)?,
            kills: row.get(8)?,
            deaths: row.get(10)?,
            damage: row.get(11)?,
            healing: row.get(12)?,
            mitigation: row.get(13)?,
            duration_sec: row.get(14)?,
        })
    };
    let rows = if let Some(value) = season {
        stmt.query_map(params![value], mapper)?
            .collect::<rusqlite::Result<Vec<_>>>()?
    } else {
        stmt.query_map([], mapper)?
            .collect::<rusqlite::Result<Vec<_>>>()?
    };
    Ok(rows)
}

fn build_baseline(
    all_rows: &[ReviewMatch],
    baseline_rows: &[ReviewMatch],
    aggregate: &Aggregate,
    threshold: i64,
) -> SeasonBaseline {
    let short = all_rows
        .iter()
        .filter(|row| row.duration_sec <= threshold)
        .count() as i64;
    let missing = all_rows
        .iter()
        .filter(|row| row.duration_sec > threshold && row.key_stats_missing())
        .count() as i64;
    let mut exclusion_reasons = Vec::new();
    if short > 0 {
        exclusion_reasons.push(BaselineExclusionReason {
            reason: "short_match".to_string(),
            label: "极短局".to_string(),
            count: short,
        });
    }
    if missing > 0 {
        exclusion_reasons.push(BaselineExclusionReason {
            reason: "missing_key_stats".to_string(),
            label: "关键统计缺失".to_string(),
            count: missing,
        });
    }
    SeasonBaseline {
        match_count: baseline_rows.len() as i64,
        excluded_total: short + missing,
        short_match_threshold_sec: threshold,
        exclusion_reasons,
        avg_kills: round2(avg(aggregate.kills, aggregate.matches)),
        avg_deaths: round2(avg(aggregate.deaths, aggregate.matches)),
        avg_damage: round2(avg(aggregate.damage, aggregate.matches)),
        avg_healing: round2(avg(aggregate.healing, aggregate.matches)),
    }
}

fn personal_bests(rows: &[ReviewMatch]) -> Vec<PersonalBest> {
    let mut bests = Vec::new();
    push_best(&mut bests, rows, "kills", "最高消灭", |row| row.kills);
    push_best(&mut bests, rows, "damage", "最高伤害", |row| row.damage);
    push_best(&mut bests, rows, "healing", "最高治疗", |row| {
        row.healing
    });
    bests.sort_by(|a, b| b.value.cmp(&a.value));
    bests
}

fn push_best(
    bests: &mut Vec<PersonalBest>,
    rows: &[ReviewMatch],
    metric: &str,
    label: &str,
    value: fn(&ReviewMatch) -> i64,
) {
    if let Some(row) = rows.iter().max_by_key(|row| value(row)) {
        if value(row) > 0 {
            bests.push(PersonalBest {
                metric: metric.to_string(),
                label: label.to_string(),
                value: value(row),
                match_id: row.match_id.clone(),
                result: row.result.clone(),
                result_label: result_label(&row.result).to_string(),
                hero_name: row.hero_name.clone(),
                map_name: row.map_name.clone(),
                evidence: match_evidence(row),
            });
        }
    }
}

fn streaks(rows: &[ReviewMatch]) -> StreakSummary {
    let mut current_win = 0;
    let mut current_loss = 0;
    let mut longest_win = 0;
    let mut longest_loss = 0;
    for row in rows {
        if row.won() {
            current_win += 1;
            current_loss = 0;
        } else if row.result == "loss" {
            current_loss += 1;
            current_win = 0;
        } else {
            current_win = 0;
            current_loss = 0;
        }
        longest_win = longest_win.max(current_win);
        longest_loss = longest_loss.max(current_loss);
    }
    StreakSummary {
        longest_win_streak: longest_win,
        longest_loss_streak: longest_loss,
    }
}

fn hero_pool(rows: &[ReviewMatch], baseline: &Aggregate) -> HeroPoolStability {
    let mut by_hero = HashMap::<String, Aggregate>::new();
    let mut names = HashMap::<String, String>::new();
    for row in rows {
        by_hero.entry(row.hero_guid.clone()).or_default().add(row);
        names.insert(row.hero_guid.clone(), row.hero_name.clone());
    }
    let mut heroes = by_hero.into_iter().collect::<Vec<_>>();
    heroes.sort_by(|a, b| b.1.matches.cmp(&a.1.matches));
    let total_matches = rows.len() as i64;
    let top1 = heroes.first().map(|item| item.1.matches).unwrap_or(0);
    let top3: i64 = heroes.iter().take(3).map(|item| item.1.matches).sum();
    let total_heroes = heroes.len() as i64;
    let switch_matches = rows
        .windows(2)
        .filter(|pair| pair[0].hero_guid != pair[1].hero_guid)
        .count() as i64;
    let label =
        if total_heroes >= 3 || ratio(switch_matches, total_matches.saturating_sub(1)) >= 0.4 {
            "英雄池活跃，适应性强"
        } else {
            "主玩英雄稳定"
        };
    let main_heroes = heroes
        .iter()
        .take(3)
        .map(|(hero, item)| HeroPoolHero {
            hero: names.get(hero).cloned().unwrap_or_else(|| hero.clone()),
            matches: item.matches,
            win_rate: round2(item.win_rate()),
            evidence: hero_evidence(&names.get(hero).cloned().unwrap_or_else(|| hero.clone())),
        })
        .collect::<Vec<_>>();
    let mut clues = Vec::new();
    if ratio(switch_matches, total_matches.saturating_sub(1)) >= 0.5
        && aggregate(rows).win_rate() < 0.45
        && avg(aggregate(rows).damage, aggregate(rows).matches)
            < avg(baseline.damage, baseline.matches) * 0.9
    {
        clues.push(ReviewConclusion {
            kind: "hero_switching_context".to_string(),
            tone: "review".to_string(),
            title: "英雄池活跃时的输出线索".to_string(),
            body: "较高切换频率同时伴随较低胜率，且伤害低于你的赛季基准。".to_string(),
            priority: 70,
            evidence: EvidenceLink {
                kind: "matches".to_string(),
                label: "查看筛选后的比赛".to_string(),
                route: "/matches".to_string(),
                match_id: None,
                hero: None,
                map: None,
            },
        });
    }
    HeroPoolStability {
        label: label.to_string(),
        total_heroes,
        top1_share: round2(ratio(top1, total_matches)),
        top3_share: round2(ratio(top3, total_matches)),
        avg_heroes_per_match: 1.0,
        switch_match_rate: round2(ratio(switch_matches, total_matches.saturating_sub(1))),
        main_heroes,
        clues,
    }
}

fn map_adaptability(rows: &[ReviewMatch], baseline: &Aggregate) -> MapAdaptability {
    let season_win_rate = aggregate(rows).win_rate();
    let mut by_map = HashMap::<String, Vec<&ReviewMatch>>::new();
    for row in rows {
        by_map.entry(row.map_name.clone()).or_default().push(row);
    }
    let mut maps = by_map
        .into_iter()
        .map(|(map, items)| {
            let mut item_aggregate = Aggregate::default();
            for row in &items {
                item_aggregate.add(row);
            }
            let support = items
                .iter()
                .max_by_key(|row| row.deaths)
                .map(|row| row.match_id.clone());
            MapReview {
                evidence: map_evidence(&map),
                map,
                matches: item_aggregate.matches,
                wins: item_aggregate.wins,
                win_rate: round2(item_aggregate.win_rate()),
                season_win_rate: round2(season_win_rate),
                sample_label: if item_aggregate.matches >= MAP_MIN_SAMPLE {
                    "样本达标".to_string()
                } else {
                    "样本不足".to_string()
                },
                avg_deaths_delta: round2(
                    avg(item_aggregate.deaths, item_aggregate.matches)
                        - avg(baseline.deaths, baseline.matches),
                ),
                avg_damage_delta: round2(
                    avg(item_aggregate.damage, item_aggregate.matches)
                        - avg(baseline.damage, baseline.matches),
                ),
                avg_healing_delta: round2(
                    avg(item_aggregate.healing, item_aggregate.matches)
                        - avg(baseline.healing, baseline.matches),
                ),
                support_match_id: support,
            }
        })
        .collect::<Vec<_>>();
    maps.sort_by(|a, b| b.matches.cmp(&a.matches).then_with(|| a.map.cmp(&b.map)));
    let best_map = maps
        .iter()
        .filter(|item| item.matches >= MAP_MIN_SAMPLE)
        .max_by(|a, b| {
            a.win_rate
                .partial_cmp(&b.win_rate)
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .cloned();
    let conclusions = maps
        .iter()
        .filter(|item| {
            item.matches >= MAP_MIN_SAMPLE
                && item.win_rate <= round2(season_win_rate - 0.2)
                && (item.avg_deaths_delta >= 1.0
                    || item.avg_damage_delta <= -1000.0
                    || item.avg_healing_delta <= -1000.0)
        })
        .map(|item| ReviewConclusion {
            kind: "map_shortcoming".to_string(),
            tone: "review".to_string(),
            title: format!("{} 的复盘线索", item.map),
            body: format!(
                "{}：{} 场，地图胜率 {:.0}%，赛季胜率 {:.0}%，死亡差值 {:+.1}，伤害差值 {:+.0}。",
                item.map,
                item.matches,
                item.win_rate * 100.0,
                item.season_win_rate * 100.0,
                item.avg_deaths_delta,
                item.avg_damage_delta
            ),
            priority: 60,
            evidence: item.evidence.clone(),
        })
        .collect();
    MapAdaptability {
        min_sample: MAP_MIN_SAMPLE,
        best_map,
        maps,
        conclusions,
    }
}

fn top_conclusions(
    personal_bests: &[PersonalBest],
    hero_pool: &HeroPoolStability,
    maps: &MapAdaptability,
    baseline: &SeasonBaseline,
) -> Vec<ReviewConclusion> {
    let mut items = Vec::new();
    for (index, best) in personal_bests.iter().take(3).enumerate() {
        items.push(ReviewConclusion {
            kind: "personal_best".to_string(),
            tone: "positive".to_string(),
            title: format!("赛季高光：{}", best.label),
            body: format!(
                "{}，地图 {}，英雄 {}，结果 {}",
                best.value, best.map_name, best.hero_name, best.result_label
            ),
            priority: 100 - index as i64,
            evidence: best.evidence.clone(),
        });
    }
    if let Some(best_map) = &maps.best_map {
        items.push(ReviewConclusion {
            kind: "best_map".to_string(),
            tone: "positive".to_string(),
            title: format!("最佳地图：{}", best_map.map),
            body: format!(
                "{} 场，胜率 {:.0}%。",
                best_map.matches,
                best_map.win_rate * 100.0
            ),
            priority: 90,
            evidence: best_map.evidence.clone(),
        });
    }
    items.push(ReviewConclusion {
        kind: "hero_pool".to_string(),
        tone: "stable".to_string(),
        title: hero_pool.label.clone(),
        body: format!(
            "使用过 {} 个英雄；前三英雄占比 {:.0}%。",
            hero_pool.total_heroes,
            hero_pool.top3_share * 100.0
        ),
        priority: 80,
        evidence: EvidenceLink {
            kind: "matches".to_string(),
            label: "查看比赛列表".to_string(),
            route: "/matches".to_string(),
            match_id: None,
            hero: None,
            map: None,
        },
    });
    items.push(ReviewConclusion {
        kind: "season_baseline".to_string(),
        tone: "stable".to_string(),
        title: "赛季基准已可用于自我对比".to_string(),
        body: format!(
            "{} 场构成赛季基准；{} 场被排除，原因已展示。",
            baseline.match_count, baseline.excluded_total
        ),
        priority: 75,
        evidence: EvidenceLink {
            kind: "matches".to_string(),
            label: "查看赛季比赛".to_string(),
            route: "/matches".to_string(),
            match_id: None,
            hero: None,
            map: None,
        },
    });
    items.extend(hero_pool.clues.clone());
    items.extend(maps.conclusions.clone());
    items.sort_by(|a, b| b.priority.cmp(&a.priority));
    items.truncate(8);
    items
}

fn aggregate<'a>(rows: impl IntoIterator<Item = &'a ReviewMatch>) -> Aggregate {
    let mut aggregate = Aggregate::default();
    for row in rows {
        aggregate.add(row);
    }
    aggregate
}

fn short_match_threshold(rows: &[ReviewMatch]) -> i64 {
    if rows.iter().any(|row| row.duration_sec > 0) {
        60
    } else {
        0
    }
}

fn match_evidence(row: &ReviewMatch) -> EvidenceLink {
    EvidenceLink {
        kind: "match".to_string(),
        label: "查看比赛".to_string(),
        route: format!("/matches/{}", row.match_id),
        match_id: Some(row.match_id.clone()),
        hero: Some(row.hero_name.clone()),
        map: Some(row.map_name.clone()),
    }
}

fn hero_evidence(hero: &str) -> EvidenceLink {
    EvidenceLink {
        kind: "hero".to_string(),
        label: "筛选英雄".to_string(),
        route: format!("/matches?hero={}", hero),
        match_id: None,
        hero: Some(hero.to_string()),
        map: None,
    }
}

fn map_evidence(map: &str) -> EvidenceLink {
    EvidenceLink {
        kind: "map".to_string(),
        label: "筛选地图".to_string(),
        route: format!("/matches?map={}", map),
        match_id: None,
        hero: None,
        map: Some(map.to_string()),
    }
}

fn result_label(result: &str) -> &'static str {
    match result {
        "win" => "胜利",
        "loss" => "失败",
        "draw" => "平局",
        _ => "未知",
    }
}

fn avg(value: i64, count: i64) -> f64 {
    if count <= 0 {
        0.0
    } else {
        value as f64 / count as f64
    }
}

fn ratio(value: i64, total: i64) -> f64 {
    if total <= 0 {
        0.0
    } else {
        value as f64 / total as f64
    }
}

fn round2(value: f64) -> f64 {
    (value * 100.0).round() / 100.0
}

trait OptionalRow<T> {
    fn optional(self) -> Result<Option<T>>;
}

impl<T> OptionalRow<T> for rusqlite::Result<T> {
    fn optional(self) -> Result<Option<T>> {
        match self {
            Ok(value) => Ok(Some(value)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(error) => Err(error).context("query failed"),
        }
    }
}
