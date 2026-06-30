use crate::db::OwDatabase;
use anyhow::Result;
use chrono::Utc;
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};

pub const PHASE5_SCORING_RULE_VERSION: i64 = 1;

const TAG_CARRY: &str = "带飞局";
const TAG_FREE_WIN: &str = "躺赢局";
const TAG_TRY_HARD: &str = "尽力局";
const TAG_BLAME: &str = "背锅局";
const TAG_SMOOTH_WIN: &str = "顺风局";
const TAG_CLOSE_LOSS: &str = "惜败局";
const TAG_WIN_FALLBACK: &str = "胜利局";
const TAG_LOSS_FALLBACK: &str = "失败局";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MatchEvaluation {
    pub match_id: String,
    pub mode: String,
    pub rule_version: i64,
    pub grade: String,
    pub primary_tag: String,
    pub evidence_tags: Vec<String>,
    pub reasons: Vec<String>,
    pub confidence: f64,
}

#[derive(Debug, Clone)]
struct MatchRow {
    match_id: String,
    mode: String,
    result: String,
}

#[derive(Debug, Clone)]
struct PlayerRow {
    side: String,
    is_current_player: bool,
    kill: i64,
    assist: i64,
    death: i64,
    damage: i64,
    healing: i64,
    mitigation: i64,
}

impl PlayerRow {
    fn kda(&self) -> f64 {
        (self.kill + self.assist) as f64 / self.death.max(1) as f64
    }
}

pub fn refresh_match_evaluations(db: &OwDatabase) -> Result<()> {
    db.initialize()?;
    let conn = db.connect()?;
    refresh_match_evaluations_conn(&conn)
}

pub fn refresh_match_evaluations_conn(conn: &Connection) -> Result<()> {
    let rows = evaluation_match_rows(conn)?;
    for row in rows {
        match evaluate_match(conn, &row) {
            Ok(Some(evaluation)) => upsert_evaluation(conn, &evaluation)?,
            Ok(None) => {
                conn.execute(
                    "DELETE FROM match_evaluations WHERE match_id=?",
                    params![row.match_id],
                )?;
            }
            Err(_) => {
                conn.execute(
                    "DELETE FROM match_evaluations WHERE match_id=?",
                    params![row.match_id],
                )?;
            }
        }
    }
    Ok(())
}

fn evaluation_match_rows(conn: &Connection) -> Result<Vec<MatchRow>> {
    let mut stmt = conn.prepare(
        r#"
        SELECT match_id, game_mode, result
        FROM matches
        WHERE bucket='preset'
        ORDER BY begin_ts ASC, match_id ASC
        "#,
    )?;
    let rows = stmt.query_map([], |row| {
        Ok(MatchRow {
            match_id: row.get(0)?,
            mode: row.get(1)?,
            result: row.get(2)?,
        })
    })?;
    rows.collect::<rusqlite::Result<Vec<_>>>()
        .map_err(Into::into)
}

fn evaluate_match(conn: &Connection, row: &MatchRow) -> Result<Option<MatchEvaluation>> {
    if row.result != "win" && row.result != "loss" {
        return Ok(None);
    }
    let players = player_rows(conn, &row.match_id)?;
    let teammates = players
        .iter()
        .filter(|player| player.side == "teammate")
        .collect::<Vec<_>>();
    let current = teammates
        .iter()
        .find(|player| player.is_current_player)
        .copied();
    let Some(current) = current else {
        return Ok(Some(fallback(row)));
    };
    if teammates.len() < 2 {
        return Ok(Some(fallback(row)));
    }

    let evidence_tags = evidence_tags(current, &teammates, &players, &row.result);
    let core_evidence_count = evidence_tags
        .iter()
        .filter(|tag| is_core_evidence(tag))
        .count();
    let strong = core_evidence_count > 0;
    let weak = !strong && unique_highest_deaths(current, &teammates);
    let primary_tag = match (row.result.as_str(), strong, weak) {
        ("win", true, _) => TAG_CARRY,
        ("win", false, true) => TAG_FREE_WIN,
        ("loss", true, _) => TAG_TRY_HARD,
        ("loss", false, true) => TAG_BLAME,
        ("win", false, false) => TAG_SMOOTH_WIN,
        ("loss", false, false) => TAG_CLOSE_LOSS,
        _ => return Ok(None),
    };
    let grade = grade(primary_tag, core_evidence_count);
    let mut reasons = vec![format!("primary:{primary_tag}")];
    reasons.extend(evidence_tags.iter().map(|tag| format!("evidence:{tag}")));
    Ok(Some(MatchEvaluation {
        match_id: row.match_id.clone(),
        mode: row.mode.clone(),
        rule_version: PHASE5_SCORING_RULE_VERSION,
        grade: grade.to_string(),
        primary_tag: primary_tag.to_string(),
        evidence_tags: evidence_tags.into_iter().take(2).collect(),
        reasons,
        confidence: 1.0,
    }))
}

fn fallback(row: &MatchRow) -> MatchEvaluation {
    let primary_tag = if row.result == "win" {
        TAG_WIN_FALLBACK
    } else {
        TAG_LOSS_FALLBACK
    };
    MatchEvaluation {
        match_id: row.match_id.clone(),
        mode: row.mode.clone(),
        rule_version: PHASE5_SCORING_RULE_VERSION,
        grade: "B".to_string(),
        primary_tag: primary_tag.to_string(),
        evidence_tags: Vec::new(),
        reasons: vec!["fallback:missing_teammate_ranking".to_string()],
        confidence: 0.4,
    }
}

fn player_rows(conn: &Connection, match_id: &str) -> Result<Vec<PlayerRow>> {
    let mut stmt = conn.prepare(
        r#"
        SELECT side, is_current_player, kill, assist, death, hero_damage, cure, resist_damage
        FROM match_players
        WHERE match_id=?
        "#,
    )?;
    let rows = stmt.query_map(params![match_id], |row| {
        Ok(PlayerRow {
            side: row.get(0)?,
            is_current_player: row.get::<_, i64>(1)? == 1,
            kill: row.get(2)?,
            assist: row.get(3)?,
            death: row.get(4)?,
            damage: row.get(5)?,
            healing: row.get(6)?,
            mitigation: row.get(7)?,
        })
    })?;
    rows.collect::<rusqlite::Result<Vec<_>>>()
        .map_err(Into::into)
}

fn evidence_tags(
    current: &PlayerRow,
    teammates: &[&PlayerRow],
    players: &[PlayerRow],
    result: &str,
) -> Vec<String> {
    let mut tags = Vec::new();
    if current.kda() > 0.0
        && current.kda()
            >= teammates
                .iter()
                .map(|player| player.kda())
                .fold(0.0, f64::max)
    {
        tags.push("KDA最高".to_string());
    }
    if is_highest_positive_i64(current.kill, teammates.iter().map(|player| player.kill)) {
        tags.push("击杀最多".to_string());
    }
    if is_highest_positive_i64(current.damage, teammates.iter().map(|player| player.damage)) {
        tags.push("伤害最高".to_string());
    }
    if is_highest_positive_i64(
        current.healing,
        teammates.iter().map(|player| player.healing),
    ) {
        tags.push("治疗最高".to_string());
    }
    if is_highest_positive_i64(
        current.mitigation,
        teammates.iter().map(|player| player.mitigation),
    ) {
        tags.push("格挡最多".to_string());
    }
    let team_kills = total(players, "teammate", |player| player.kill);
    let enemy_kills = total(players, "enemy", |player| player.kill);
    let team_damage = total(players, "teammate", |player| player.damage);
    let enemy_damage = total(players, "enemy", |player| player.damage);
    if result == "win" && team_kills < enemy_kills {
        tags.push("翻盘局".to_string());
    }
    if (team_kills - enemy_kills).abs() <= 2 && close_damage(team_damage, enemy_damage) {
        tags.push("伯仲局".to_string());
    }
    tags
}

fn is_highest_positive_i64(value: i64, values: impl Iterator<Item = i64>) -> bool {
    value > 0 && values.max().map(|max| value >= max).unwrap_or(false)
}

fn unique_highest_deaths(current: &PlayerRow, teammates: &[&PlayerRow]) -> bool {
    let max_deaths = teammates
        .iter()
        .map(|player| player.death)
        .max()
        .unwrap_or(0);
    current.death == max_deaths
        && teammates
            .iter()
            .filter(|player| player.death == max_deaths)
            .count()
            == 1
}

fn total(players: &[PlayerRow], side: &str, value: fn(&PlayerRow) -> i64) -> i64 {
    players
        .iter()
        .filter(|player| player.side == side)
        .map(value)
        .sum()
}

fn close_damage(a: i64, b: i64) -> bool {
    let max = a.max(b);
    if max <= 0 {
        true
    } else {
        ((a - b).abs() as f64 / max as f64) <= 0.10
    }
}

fn is_core_evidence(tag: &str) -> bool {
    matches!(
        tag,
        "KDA最高" | "击杀最多" | "伤害最高" | "治疗最高" | "格挡最多"
    )
}

fn grade(primary_tag: &str, core_evidence_count: usize) -> &'static str {
    match primary_tag {
        TAG_CARRY | TAG_TRY_HARD if core_evidence_count >= 2 => "S",
        TAG_CARRY | TAG_TRY_HARD if core_evidence_count == 1 => "A",
        TAG_SMOOTH_WIN | TAG_CLOSE_LOSS => "B",
        TAG_FREE_WIN | TAG_BLAME => "C",
        TAG_WIN_FALLBACK | TAG_LOSS_FALLBACK => "B",
        _ => "B",
    }
}

fn upsert_evaluation(conn: &Connection, evaluation: &MatchEvaluation) -> Result<()> {
    conn.execute(
        r#"
        INSERT INTO match_evaluations (
            match_id, mode, rule_version, grade, primary_tag, evidence_tags_json,
            reasons_json, confidence, computed_at
        )
        VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
        ON CONFLICT(match_id) DO UPDATE SET
            mode=excluded.mode,
            rule_version=excluded.rule_version,
            grade=excluded.grade,
            primary_tag=excluded.primary_tag,
            evidence_tags_json=excluded.evidence_tags_json,
            reasons_json=excluded.reasons_json,
            confidence=excluded.confidence,
            computed_at=excluded.computed_at
        "#,
        params![
            evaluation.match_id,
            evaluation.mode,
            evaluation.rule_version,
            evaluation.grade,
            evaluation.primary_tag,
            serde_json::to_string(&evaluation.evidence_tags)?,
            serde_json::to_string(&evaluation.reasons)?,
            evaluation.confidence,
            Utc::now().to_rfc3339(),
        ],
    )?;
    Ok(())
}
