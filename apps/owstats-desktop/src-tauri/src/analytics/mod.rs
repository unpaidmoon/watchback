use crate::db::OwDatabase;
use crate::importer::rebuild_match_players_from_existing_details;
use crate::models::{
    CommonFriendHero, FriendAnalytics, FriendAnalyticsFilters, FriendAnalyticsSummary, FriendBoard,
    FriendBoardEntry, FriendBreakdown, FriendDetail, FriendRecentMatch, LineupCombination,
    LineupMember, MapAnalysisCard, MapAnalysisCards, SeasonAnalysis,
};
use anyhow::{Context, Result};
use chrono::{DateTime, Local, NaiveDateTime, TimeZone};
use rusqlite::ToSql;
use std::collections::HashMap;

mod arena;
mod boards;
use arena::{arena_analytics_rows, looks_like_private_id};
use boards::build_boards;

const MIN_SAMPLE: i64 = 5;
const PHASE5_MIN_SAMPLE: i64 = 3;

#[derive(Debug, Clone)]
pub(super) struct FriendMatchRow {
    bnet_id: String,
    name: String,
    match_id: String,
    result: String,
    begin_ts: i64,
    map_name: String,
    hero_guid: String,
    hero_name: String,
    kills: i64,
    assists: i64,
    deaths: i64,
    damage: i64,
    healing: i64,
    mitigation: i64,
    damage_taken: i64,
    healing_taken: i64,
    group_size: i64,
}

#[derive(Debug, Clone)]
pub(super) struct FriendAggregate {
    pub(super) bnet_id: String,
    pub(super) name: String,
    pub(super) same_team_matches: i64,
    pub(super) wins: i64,
    pub(super) kills: i64,
    pub(super) assists: i64,
    pub(super) deaths: i64,
    pub(super) damage: i64,
    pub(super) healing: i64,
    pub(super) mitigation: i64,
    pub(super) damage_taken: i64,
    pub(super) healing_taken: i64,
    name_begin_ts: i64,
    name_match_id: String,
}

impl FriendAggregate {
    pub(super) fn win_rate(&self) -> f64 {
        ratio(self.wins, self.same_team_matches)
    }

    pub(super) fn avg_deaths(&self) -> f64 {
        average(self.deaths, self.same_team_matches)
    }

    pub(super) fn avg_damage_taken(&self) -> f64 {
        average(self.damage_taken, self.same_team_matches)
    }

    pub(super) fn avg_healing_taken(&self) -> f64 {
        average(self.healing_taken, self.same_team_matches)
    }

    pub(super) fn avg_mitigation(&self) -> f64 {
        average(self.mitigation, self.same_team_matches)
    }

    pub(super) fn avg_kills(&self) -> f64 {
        average(self.kills, self.same_team_matches)
    }

    pub(super) fn healing_to_kill_rate(&self) -> f64 {
        if self.healing_taken <= 0 {
            0.0
        } else {
            self.kills as f64 / (self.healing_taken as f64 / 1000.0)
        }
    }
}

pub fn get_friend_analytics(
    db: &OwDatabase,
    filters: FriendAnalyticsFilters,
) -> Result<FriendAnalytics> {
    db.initialize()?;
    let mut rows = friend_rows(db, &filters, None)?;
    if should_rebuild_projection(db, &rows)? {
        rebuild_match_players_from_existing_details(db)?;
        rows = friend_rows(db, &filters, None)?;
    }
    let arena_rows = arena_analytics_rows(db, &filters)?;
    rows.extend(arena_rows.friend_rows);
    let aggregates = aggregate_rows(&rows);
    let intimacy_map = compute_intimacy_scores(&rows);
    let eligible: Vec<FriendAggregate> = aggregates
        .into_iter()
        .filter(|item| item.same_team_matches >= MIN_SAMPLE)
        .collect();
    let bands = Bands::from(&eligible);
    let boards = build_boards(&eligible, &bands);
    let conn = db.connect()?;
    let preset_matches = filtered_preset_count(&conn, &filters)?;

    Ok(FriendAnalytics {
        summary: FriendAnalyticsSummary {
            season: filters.season,
            preset_matches,
            friend_rows: rows.len() as i64,
            eligible_friends: eligible.len(),
            min_sample: MIN_SAMPLE,
        },
        boards,
        common_friend_heroes: common_friend_heroes(&rows, &intimacy_map),
        lineup_combinations: lineup_combinations(db, &filters, arena_rows.lineup_rows)?,
    })
}

pub fn get_season_analysis(db: &OwDatabase, season: Option<i64>) -> Result<SeasonAnalysis> {
    db.initialize()?;
    let selected = season.or_else(|| latest_season(db).ok().flatten());
    let friend = get_friend_analytics(
        db,
        FriendAnalyticsFilters {
            season: selected,
            ..FriendAnalyticsFilters::default()
        },
    )?;
    Ok(SeasonAnalysis {
        season: selected,
        friend,
        map_cards: map_analysis_cards(db, selected)?,
    })
}

#[derive(Debug, Clone)]
struct MapAggregate {
    name: String,
    matches: i64,
    wins: i64,
    latest_ts: i64,
    earliest_ts: i64,
}

impl MapAggregate {
    fn win_rate(&self) -> f64 {
        ratio(self.wins, self.matches)
    }
}

fn latest_season(db: &OwDatabase) -> Result<Option<i64>> {
    let conn = db.connect()?;
    let season = conn.query_row(
        "SELECT season FROM matches WHERE bucket='preset' ORDER BY season DESC LIMIT 1",
        [],
        |row| row.get(0),
    );
    match season {
        Ok(value) => Ok(Some(value)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(error) => Err(error.into()),
    }
}

fn map_analysis_cards(db: &OwDatabase, season: Option<i64>) -> Result<MapAnalysisCards> {
    let conn = db.connect()?;
    let mut sql = String::from(
        r#"
        SELECT COALESCE(map_name, map_guid),
               COUNT(*) AS matches,
               SUM(CASE WHEN result='win' THEN 1 ELSE 0 END) AS wins,
               MAX(begin_ts), MIN(begin_ts)
        FROM matches
        WHERE bucket='preset'
        "#,
    );
    let mut owned: Vec<Box<dyn ToSql>> = Vec::new();
    if let Some(season) = season {
        sql.push_str(" AND season=?");
        owned.push(Box::new(season));
    }
    sql.push_str(" GROUP BY map_guid, COALESCE(map_name, map_guid)");
    let params = owned
        .iter()
        .map(|item| item.as_ref() as &dyn ToSql)
        .collect::<Vec<_>>();
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(params.as_slice(), |row| {
        Ok(MapAggregate {
            name: row.get(0)?,
            matches: row.get(1)?,
            wins: row.get(2)?,
            latest_ts: row.get(3)?,
            earliest_ts: row.get(4)?,
        })
    })?;
    let rows = rows.collect::<rusqlite::Result<Vec<_>>>()?;
    let eligible = rows
        .iter()
        .filter(|item| item.matches >= MIN_SAMPLE)
        .cloned()
        .collect::<Vec<_>>();
    Ok(MapAnalysisCards {
        min_sample: MIN_SAMPLE,
        ace_map: eligible
            .iter()
            .max_by(|a, b| compare_win_rate(a, b))
            .map(|item| map_card("ace_map", "王牌地图", item)),
        throw_zone: eligible
            .iter()
            .min_by(|a, b| compare_win_rate(a, b))
            .map(|item| map_card("throw_zone", "翻车路段", item)),
        home_field: rows
            .iter()
            .max_by(|a, b| {
                a.matches
                    .cmp(&b.matches)
                    .then_with(|| a.latest_ts.cmp(&b.latest_ts))
                    .then_with(|| b.name.cmp(&a.name))
            })
            .map(|item| map_card("home_field", "常驻战场", item)),
        rare_guest: rows
            .iter()
            .min_by(|a, b| {
                a.matches
                    .cmp(&b.matches)
                    .then_with(|| a.earliest_ts.cmp(&b.earliest_ts))
                    .then_with(|| a.name.cmp(&b.name))
            })
            .map(|item| map_card("rare_guest", "稀有来客", item)),
    })
}

fn compare_win_rate(a: &MapAggregate, b: &MapAggregate) -> std::cmp::Ordering {
    a.win_rate()
        .partial_cmp(&b.win_rate())
        .unwrap_or(std::cmp::Ordering::Equal)
        .then_with(|| a.matches.cmp(&b.matches))
        .then_with(|| b.name.cmp(&a.name))
}

fn map_card(key: &str, title: &str, item: &MapAggregate) -> MapAnalysisCard {
    MapAnalysisCard {
        key: key.to_string(),
        title: title.to_string(),
        name: item.name.clone(),
        matches: item.matches,
        win_rate: round2(item.win_rate()),
        route: "/ranking?tab=maps".to_string(),
    }
}

fn should_rebuild_projection(db: &OwDatabase, rows: &[FriendMatchRow]) -> Result<bool> {
    let conn = db.connect()?;
    let detail_count: i64 =
        conn.query_row("SELECT COUNT(*) FROM match_details", [], |row| row.get(0))?;
    let projection_count: i64 =
        conn.query_row("SELECT COUNT(*) FROM match_players", [], |row| row.get(0))?;
    let missing_projection = projection_count == 0;
    let missing_taken_stats = !rows.is_empty()
        && rows
            .iter()
            .all(|row| row.damage_taken == 0 && row.healing_taken == 0)
        && projection_has_taken_values_in_raw_json(&conn)?;
    Ok(detail_count > 0 && (missing_projection || missing_taken_stats))
}

fn projection_has_taken_values_in_raw_json(conn: &rusqlite::Connection) -> Result<bool> {
    let count: i64 = conn.query_row(
        r#"
        SELECT COUNT(*)
        FROM match_players
        WHERE raw_json LIKE '%"damageTaken":%'
          AND raw_json NOT LIKE '%"damageTaken":0,%'
          AND raw_json NOT LIKE '%"damageTaken":0.0,%'
        "#,
        [],
        |row| row.get(0),
    )?;
    Ok(count > 0)
}

pub fn get_friend_detail(
    db: &OwDatabase,
    bnet_id: &str,
    filters: FriendAnalyticsFilters,
) -> Result<FriendDetail> {
    db.initialize()?;
    let analytics = get_friend_analytics(db, filters.clone())?;
    let boards = analytics
        .boards
        .iter()
        .flat_map(|board| board.friends.iter())
        .filter(|friend| friend.bnet_id == bnet_id)
        .cloned()
        .collect::<Vec<_>>();
    let rows = friend_rows(db, &filters, Some(bnet_id))?;
    let name = rows
        .first()
        .map(|row| row.name.clone())
        .or_else(|| boards.first().map(|row| row.name.clone()))
        .with_context(|| format!("friend not found: {bnet_id}"))?;

    Ok(FriendDetail {
        bnet_id: bnet_id.to_string(),
        name,
        boards,
        common_heroes: breakdown(&rows, |row| &row.hero_name),
        common_maps: breakdown(&rows, |row| &row.map_name),
        recent_matches: rows
            .into_iter()
            .take(20)
            .map(|row| FriendRecentMatch {
                match_id: row.match_id,
                begin_label: format_begin_ts(row.begin_ts),
                result: row.result,
                map_name: row.map_name,
                hero_name: row.hero_name,
                kills: row.kills,
                assists: row.assists,
                deaths: row.deaths,
                damage: row.damage,
                healing: row.healing,
                damage_taken: row.damage_taken,
                healing_taken: row.healing_taken,
            })
            .collect(),
    })
}

fn friend_rows(
    db: &OwDatabase,
    filters: &FriendAnalyticsFilters,
    bnet_id: Option<&str>,
) -> Result<Vec<FriendMatchRow>> {
    let conn = db.connect()?;
    let mut sql = String::from(
        r#"
        SELECT
            p.bnet_id,
            p.player_name,
            p.match_id,
            m.result,
            m.begin_ts,
            COALESCE(m.map_name, m.map_guid),
            p.hero_guid,
            COALESCE(p.hero_name, p.hero_guid),
            p.kill,
            p.assist,
            p.death,
            p.hero_damage,
            p.cure,
            p.resist_damage,
            p.damage_taken,
            p.healing_taken,
            (SELECT COUNT(*) FROM match_players sp
             WHERE sp.match_id = p.match_id
               AND sp.side = 'teammate'
               AND (sp.is_current_player = 1 OR sp.is_friend = 1)) AS group_size
        FROM match_players p
        JOIN matches m ON m.match_id = p.match_id
        WHERE p.side = 'teammate'
          AND p.is_friend = 1
          AND p.is_current_player = 0
          AND m.bucket = 'preset'
        "#,
    );
    let mut owned: Vec<Box<dyn ToSql>> = Vec::new();
    append_filters(&mut sql, &mut owned, filters);
    if let Some(bnet_id) = bnet_id {
        sql.push_str(" AND p.bnet_id = ?");
        owned.push(Box::new(bnet_id.to_string()));
    }
    sql.push_str(" ORDER BY m.begin_ts DESC, p.bnet_id");
    let params: Vec<&dyn ToSql> = owned
        .iter()
        .map(|item| item.as_ref() as &dyn ToSql)
        .collect();
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(params.as_slice(), |row| {
        Ok(FriendMatchRow {
            bnet_id: row.get(0)?,
            name: row.get(1)?,
            match_id: row.get(2)?,
            result: row.get(3)?,
            begin_ts: row.get(4)?,
            map_name: row.get(5)?,
            hero_guid: row.get(6)?,
            hero_name: row.get(7)?,
            kills: row.get(8)?,
            assists: row.get(9)?,
            deaths: row.get(10)?,
            damage: row.get(11)?,
            healing: row.get(12)?,
            mitigation: row.get(13)?,
            damage_taken: row.get(14)?,
            healing_taken: row.get(15)?,
            group_size: row.get(16)?,
        })
    })?;
    rows.collect::<rusqlite::Result<Vec<_>>>()
        .map_err(Into::into)
}

fn common_friend_heroes(
    rows: &[FriendMatchRow],
    intimacy: &HashMap<String, i64>,
) -> Vec<CommonFriendHero> {
    #[derive(Default)]
    struct FriendState {
        name: String,
        name_begin_ts: i64,
        name_match_id: String,
        matches: i64,
        wins: i64,
        latest_begin_ts: i64,
        heroes: HashMap<String, HeroState>,
    }
    #[derive(Default)]
    struct HeroState {
        name: String,
        name_begin_ts: i64,
        name_match_id: String,
        matches: i64,
        latest_begin_ts: i64,
    }

    let mut by_friend = HashMap::<String, FriendState>::new();
    for row in rows {
        let friend = by_friend.entry(row.bnet_id.clone()).or_default();
        update_preferred_name(
            &mut friend.name,
            &mut friend.name_begin_ts,
            &mut friend.name_match_id,
            &row.name,
            row.begin_ts,
            &row.match_id,
            &row.bnet_id,
            "未知好友",
        );
        friend.matches += 1;
        friend.wins += if row.result == "win" { 1 } else { 0 };
        friend.latest_begin_ts = friend.latest_begin_ts.max(row.begin_ts);
        let hero = friend.heroes.entry(row.hero_guid.clone()).or_default();
        update_preferred_name(
            &mut hero.name,
            &mut hero.name_begin_ts,
            &mut hero.name_match_id,
            &row.hero_name,
            row.begin_ts,
            &row.match_id,
            &row.hero_guid,
            "未知英雄",
        );
        hero.matches += 1;
        hero.latest_begin_ts = hero.latest_begin_ts.max(row.begin_ts);
    }

    let mut items = by_friend
        .into_iter()
        .filter(|(_, friend)| friend.matches >= PHASE5_MIN_SAMPLE)
        .filter_map(|(bnet_id, friend)| {
            let (hero_guid, hero) = friend.heroes.into_iter().max_by(|a, b| {
                a.1.matches
                    .cmp(&b.1.matches)
                    .then_with(|| a.1.latest_begin_ts.cmp(&b.1.latest_begin_ts))
                    .then_with(|| b.0.cmp(&a.0))
            })?;
            let intimacy_score = intimacy.get(&bnet_id).copied().unwrap_or(0);
            Some(CommonFriendHero {
                bnet_id,
                name: friend.name,
                representative_hero_guid: hero_guid,
                representative_hero_name: hero.name,
                matches: friend.matches,
                wins: friend.wins,
                win_rate: round2(ratio(friend.wins, friend.matches)),
                latest_begin_ts: friend.latest_begin_ts,
                intimacy_score,
            })
        })
        .collect::<Vec<_>>();
    items.sort_by(|a, b| {
        b.intimacy_score
            .cmp(&a.intimacy_score)
            .then_with(|| b.matches.cmp(&a.matches))
            .then_with(|| b.latest_begin_ts.cmp(&a.latest_begin_ts))
            .then_with(|| a.name.cmp(&b.name))
    });
    items.truncate(5);
    items
}

#[derive(Debug, Clone)]
pub(super) struct LineupRow {
    match_id: String,
    result: String,
    begin_ts: i64,
    bnet_id: String,
    name: String,
    hero_guid: String,
    hero_name: String,
    is_current_player: bool,
    is_friend: bool,
}

fn lineup_combinations(
    db: &OwDatabase,
    filters: &FriendAnalyticsFilters,
    arena_rows: Vec<LineupRow>,
) -> Result<Vec<LineupCombination>> {
    let mut rows = lineup_rows(db, filters)?;
    rows.extend(arena_rows);
    let mut by_match = HashMap::<String, Vec<LineupRow>>::new();
    for row in rows {
        by_match.entry(row.match_id.clone()).or_default().push(row);
    }
    let mut match_groups = by_match.into_iter().collect::<Vec<_>>();
    match_groups.sort_by(|(match_id_a, rows_a), (match_id_b, rows_b)| {
        let begin_ts_a = rows_a.first().map(|row| row.begin_ts).unwrap_or_default();
        let begin_ts_b = rows_b.first().map(|row| row.begin_ts).unwrap_or_default();
        (begin_ts_a, match_id_a).cmp(&(begin_ts_b, match_id_b))
    });

    #[derive(Default)]
    struct ComboState {
        members: Vec<LineupMember>,
        matches: i64,
        wins: i64,
        latest_begin_ts: i64,
    }

    let mut combos = HashMap::<String, ComboState>::new();
    for (_, rows) in match_groups {
        let Some(current) = rows.iter().find(|row| row.is_current_player) else {
            continue;
        };
        let friends = rows
            .iter()
            .filter(|row| row.is_friend && !row.is_current_player)
            .collect::<Vec<_>>();
        if friends.is_empty() {
            continue;
        }
        let mut members = Vec::new();
        members.push(LineupMember {
            bnet_id: Some(current.bnet_id.clone()),
            name: safe_lineup_display_name(&current.name, &current.bnet_id, "当前玩家"),
            hero_guid: current.hero_guid.clone(),
            hero_name: safe_lineup_display_name(&current.hero_name, &current.hero_guid, "未知英雄"),
            is_current_player: true,
        });
        for friend in &friends {
            members.push(LineupMember {
                bnet_id: Some(friend.bnet_id.clone()),
                name: safe_lineup_display_name(&friend.name, &friend.bnet_id, "未知好友"),
                hero_guid: friend.hero_guid.clone(),
                hero_name: safe_lineup_display_name(
                    &friend.hero_name,
                    &friend.hero_guid,
                    "未知英雄",
                ),
                is_current_player: false,
            });
        }
        let key = members
            .iter()
            .map(|member| {
                format!(
                    "{}:{}",
                    member.bnet_id.clone().unwrap_or_default(),
                    member.hero_guid
                )
            })
            .collect::<Vec<_>>()
            .join("|");
        let state = combos.entry(key).or_default();
        if state.members.is_empty() {
            state.members = members;
        } else {
            merge_lineup_member_display(&mut state.members, &members);
        }
        state.matches += 1;
        state.wins += if current.result == "win" { 1 } else { 0 };
        state.latest_begin_ts = state.latest_begin_ts.max(current.begin_ts);
    }

    let mut items = combos
        .into_iter()
        .filter(|(_, state)| state.matches >= PHASE5_MIN_SAMPLE)
        .map(|(key, state)| LineupCombination {
            key,
            members: state.members,
            matches: state.matches,
            wins: state.wins,
            win_rate: round2(ratio(state.wins, state.matches)),
        })
        .collect::<Vec<_>>();
    items.sort_by(|a, b| {
        b.matches
            .cmp(&a.matches)
            .then_with(|| b.members.len().cmp(&a.members.len()))
            .then_with(|| a.key.cmp(&b.key))
    });
    Ok(items)
}

fn merge_lineup_member_display(current: &mut [LineupMember], candidates: &[LineupMember]) {
    for candidate in candidates {
        let Some(existing) = current.iter_mut().find(|member| {
            member.bnet_id == candidate.bnet_id && member.hero_guid == candidate.hero_guid
        }) else {
            continue;
        };
        let identity = candidate.bnet_id.as_deref().unwrap_or_default();
        let name_fallback = if candidate.is_current_player {
            "当前玩家"
        } else {
            "未知好友"
        };
        let candidate_name_quality = display_name_quality(&candidate.name, identity, name_fallback);
        let current_name_quality = display_name_quality(&existing.name, identity, name_fallback);
        if candidate_name_quality > current_name_quality
            || (candidate_name_quality == current_name_quality && candidate_name_quality > 0)
        {
            existing.name = candidate.name.clone();
        }

        let candidate_hero_quality =
            display_name_quality(&candidate.hero_name, &candidate.hero_guid, "未知英雄");
        let current_hero_quality =
            display_name_quality(&existing.hero_name, &existing.hero_guid, "未知英雄");
        if candidate_hero_quality > current_hero_quality
            || (candidate_hero_quality == current_hero_quality && candidate_hero_quality > 0)
        {
            existing.hero_name = candidate.hero_name.clone();
        }
    }
}

fn lineup_rows(db: &OwDatabase, filters: &FriendAnalyticsFilters) -> Result<Vec<LineupRow>> {
    let conn = db.connect()?;
    let mut sql = String::from(
        r#"
        SELECT
            p.match_id,
            m.result,
            m.begin_ts,
            p.bnet_id,
            p.player_name,
            p.hero_guid,
            COALESCE(p.hero_name, p.hero_guid),
            p.is_current_player,
            p.is_friend
        FROM match_players p
        JOIN matches m ON m.match_id = p.match_id
        WHERE p.side='teammate'
          AND m.bucket='preset'
          AND (p.is_current_player=1 OR p.is_friend=1)
        "#,
    );
    let mut owned: Vec<Box<dyn ToSql>> = Vec::new();
    append_filters(&mut sql, &mut owned, filters);
    sql.push_str(" ORDER BY m.begin_ts DESC, p.is_current_player DESC, p.bnet_id");
    let params = owned
        .iter()
        .map(|item| item.as_ref() as &dyn ToSql)
        .collect::<Vec<_>>();
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(params.as_slice(), |row| {
        Ok(LineupRow {
            match_id: row.get(0)?,
            result: row.get(1)?,
            begin_ts: row.get(2)?,
            bnet_id: row.get(3)?,
            name: row.get(4)?,
            hero_guid: row.get(5)?,
            hero_name: row.get(6)?,
            is_current_player: row.get::<_, i64>(7)? == 1,
            is_friend: row.get::<_, i64>(8)? == 1,
        })
    })?;
    rows.collect::<rusqlite::Result<Vec<_>>>()
        .map_err(Into::into)
}

fn append_filters(
    sql: &mut String,
    owned: &mut Vec<Box<dyn ToSql>>,
    filters: &FriendAnalyticsFilters,
) {
    if let Some(season) = filters.season {
        sql.push_str(" AND m.season = ?");
        owned.push(Box::new(season));
    }
    if let Some(hero) = non_empty(&filters.hero) {
        sql.push_str(" AND (p.hero_guid = ? OR p.hero_name = ?)");
        owned.push(Box::new(hero.clone()));
        owned.push(Box::new(hero));
    }
    if let Some(map) = non_empty(&filters.map) {
        sql.push_str(" AND (m.map_guid = ? OR m.map_name = ?)");
        owned.push(Box::new(map.clone()));
        owned.push(Box::new(map));
    }
    if let Some(result) = non_empty(&filters.result) {
        sql.push_str(" AND m.result = ?");
        owned.push(Box::new(result));
    }
    if let Some(mode) = non_empty(&filters.mode) {
        sql.push_str(" AND m.game_mode = ?");
        owned.push(Box::new(mode));
    }
    if let Some(from) = non_empty(&filters.date_from).and_then(|date| begin_bound(&date, false)) {
        sql.push_str(" AND m.begin_ts >= ?");
        owned.push(Box::new(from));
    }
    if let Some(to) = non_empty(&filters.date_to).and_then(|date| begin_bound(&date, true)) {
        sql.push_str(" AND m.begin_ts <= ?");
        owned.push(Box::new(to));
    }
}

fn filtered_preset_count(
    conn: &rusqlite::Connection,
    filters: &FriendAnalyticsFilters,
) -> Result<i64> {
    let mut sql = String::from("SELECT COUNT(*) FROM matches m WHERE m.bucket='preset'");
    let mut owned: Vec<Box<dyn ToSql>> = Vec::new();
    if let Some(season) = filters.season {
        sql.push_str(" AND m.season = ?");
        owned.push(Box::new(season));
    }
    if let Some(map) = non_empty(&filters.map) {
        sql.push_str(" AND (m.map_guid = ? OR m.map_name = ?)");
        owned.push(Box::new(map.clone()));
        owned.push(Box::new(map));
    }
    if let Some(result) = non_empty(&filters.result) {
        sql.push_str(" AND m.result = ?");
        owned.push(Box::new(result));
    }
    if let Some(mode) = non_empty(&filters.mode) {
        sql.push_str(" AND m.game_mode = ?");
        owned.push(Box::new(mode));
    }
    if let Some(from) = non_empty(&filters.date_from).and_then(|date| begin_bound(&date, false)) {
        sql.push_str(" AND m.begin_ts >= ?");
        owned.push(Box::new(from));
    }
    if let Some(to) = non_empty(&filters.date_to).and_then(|date| begin_bound(&date, true)) {
        sql.push_str(" AND m.begin_ts <= ?");
        owned.push(Box::new(to));
    }
    let params: Vec<&dyn ToSql> = owned
        .iter()
        .map(|item| item.as_ref() as &dyn ToSql)
        .collect();
    conn.query_row(&sql, params.as_slice(), |row| row.get(0))
        .map_err(Into::into)
}

fn aggregate_rows(rows: &[FriendMatchRow]) -> Vec<FriendAggregate> {
    let mut by_friend = HashMap::<String, FriendAggregate>::new();
    for row in rows {
        let item = by_friend
            .entry(row.bnet_id.clone())
            .or_insert_with(|| FriendAggregate {
                bnet_id: row.bnet_id.clone(),
                name: row.name.clone(),
                same_team_matches: 0,
                wins: 0,
                kills: 0,
                assists: 0,
                deaths: 0,
                damage: 0,
                healing: 0,
                mitigation: 0,
                damage_taken: 0,
                healing_taken: 0,
                name_begin_ts: i64::MIN,
                name_match_id: String::new(),
            });
        update_preferred_name(
            &mut item.name,
            &mut item.name_begin_ts,
            &mut item.name_match_id,
            &row.name,
            row.begin_ts,
            &row.match_id,
            &row.bnet_id,
            "未知好友",
        );
        item.same_team_matches += 1;
        item.wins += if row.result == "win" { 1 } else { 0 };
        item.kills += row.kills;
        item.assists += row.assists;
        item.deaths += row.deaths;
        item.damage += row.damage;
        item.healing += row.healing;
        item.mitigation += row.mitigation;
        item.damage_taken += row.damage_taken;
        item.healing_taken += row.healing_taken;
    }
    let mut values = by_friend.into_values().collect::<Vec<_>>();
    values.sort_by(|a, b| a.name.cmp(&b.name));
    values
}

fn update_preferred_name(
    current: &mut String,
    current_begin_ts: &mut i64,
    current_match_id: &mut String,
    candidate: &str,
    candidate_begin_ts: i64,
    candidate_match_id: &str,
    identity: &str,
    fallback: &str,
) {
    let candidate = candidate.trim();
    let candidate_quality = display_name_quality(candidate, identity, fallback);
    let current_quality = display_name_quality(current, identity, fallback);
    let candidate_is_newer = (candidate_begin_ts, candidate_match_id, candidate)
        > (
            *current_begin_ts,
            current_match_id.as_str(),
            current.as_str(),
        );
    if candidate_quality > current_quality
        || (candidate_quality == current_quality && candidate_is_newer)
    {
        *current = if candidate_quality > 0 {
            candidate.to_string()
        } else {
            fallback.to_string()
        };
        *current_begin_ts = candidate_begin_ts;
        *current_match_id = candidate_match_id.to_string();
    } else if current.is_empty() || current_quality == 0 {
        *current = fallback.to_string();
    }
}

fn display_name_quality(value: &str, identity: &str, fallback: &str) -> u8 {
    let value = value.trim();
    let trusted = !value.is_empty()
        && value != identity
        && value != fallback
        && !matches!(value, "未知玩家" | "未知好友" | "未知英雄")
        && !looks_like_private_id(value);
    if trusted {
        2
    } else {
        0
    }
}

fn safe_lineup_display_name(value: &str, identity: &str, fallback: &str) -> String {
    if display_name_quality(value, identity, fallback) > 0 {
        value.trim().to_string()
    } else {
        fallback.to_string()
    }
}

fn compute_intimacy_scores(rows: &[FriendMatchRow]) -> HashMap<String, i64> {
    let mut scores = HashMap::<String, i64>::new();
    for row in rows {
        let weight = match row.group_size {
            2 => 5,
            3 => 3,
            4 => 2,
            5 | 6 => 1,
            _ => 0,
        };
        *scores.entry(row.bnet_id.clone()).or_default() += weight;
    }
    scores
}

#[derive(Debug, Clone)]
pub(super) struct Bands {
    pub(super) high_damage: f64,
    pub(super) high_healing: f64,
    pub(super) high_mitigation: f64,
    pub(super) high_deaths: f64,
    pub(super) low_healing: f64,
    pub(super) low_deaths: f64,
    pub(super) high_kills: f64,
}

impl Bands {
    fn from(items: &[FriendAggregate]) -> Self {
        Self {
            high_damage: percentile(items.iter().map(FriendAggregate::avg_damage_taken), 0.70),
            high_healing: percentile(items.iter().map(FriendAggregate::avg_healing_taken), 0.70),
            high_mitigation: percentile(items.iter().map(FriendAggregate::avg_mitigation), 0.70),
            high_deaths: percentile(items.iter().map(FriendAggregate::avg_deaths), 0.70),
            low_healing: percentile(items.iter().map(FriendAggregate::avg_healing_taken), 0.30),
            low_deaths: percentile(items.iter().map(FriendAggregate::avg_deaths), 0.30),
            high_kills: percentile(items.iter().map(FriendAggregate::avg_kills), 0.70),
        }
    }

    pub(super) fn high_damage(&self, item: &FriendAggregate) -> bool {
        item.avg_damage_taken() >= self.high_damage
    }

    pub(super) fn high_healing_taken(&self, item: &FriendAggregate) -> bool {
        item.avg_healing_taken() >= self.high_healing
    }

    pub(super) fn high_mitigation(&self, item: &FriendAggregate) -> bool {
        item.avg_mitigation() >= self.high_mitigation
    }

    pub(super) fn high_deaths(&self, item: &FriendAggregate) -> bool {
        item.avg_deaths() >= self.high_deaths
    }

    pub(super) fn low_healing(&self, item: &FriendAggregate) -> bool {
        item.avg_healing_taken() <= self.low_healing
    }

    pub(super) fn low_deaths(&self, item: &FriendAggregate) -> bool {
        item.avg_deaths() <= self.low_deaths
    }

    pub(super) fn high_healing_to_kill(&self, item: &FriendAggregate) -> bool {
        item.avg_kills() >= self.high_kills
    }
}

fn percentile(values: impl Iterator<Item = f64>, p: f64) -> f64 {
    let mut values = values.filter(|value| value.is_finite()).collect::<Vec<_>>();
    if values.is_empty() {
        return 0.0;
    }
    values.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let index = ((values.len() - 1) as f64 * p).round() as usize;
    values[index]
}

fn breakdown(
    rows: &[FriendMatchRow],
    label: impl Fn(&FriendMatchRow) -> &str,
) -> Vec<FriendBreakdown> {
    let mut counts = HashMap::<String, i64>::new();
    for row in rows {
        let key = label(row).trim();
        if !key.is_empty() {
            *counts.entry(key.to_string()).or_default() += 1;
        }
    }
    let mut items = counts
        .into_iter()
        .map(|(label, count)| FriendBreakdown { label, count })
        .collect::<Vec<_>>();
    items.sort_by(|a, b| b.count.cmp(&a.count).then_with(|| a.label.cmp(&b.label)));
    items.truncate(8);
    items
}

pub(super) fn average(total: i64, count: i64) -> f64 {
    if count <= 0 {
        0.0
    } else {
        total as f64 / count as f64
    }
}

pub(super) fn ratio(value: i64, total: i64) -> f64 {
    if total <= 0 {
        0.0
    } else {
        value as f64 / total as f64
    }
}

pub(super) fn round2(value: f64) -> f64 {
    (value * 100.0).round() / 100.0
}

fn non_empty(value: &Option<String>) -> Option<String> {
    value
        .as_ref()
        .map(|text| text.trim().to_string())
        .filter(|text| !text.is_empty())
}

fn begin_bound(date: &str, end_of_day: bool) -> Option<i64> {
    let suffix = if end_of_day { " 23:59:59" } else { " 00:00:00" };
    let naive =
        NaiveDateTime::parse_from_str(&format!("{date}{suffix}"), "%Y-%m-%d %H:%M:%S").ok()?;
    Local
        .from_local_datetime(&naive)
        .single()
        .map(|dt| dt.timestamp())
}

fn format_begin_ts(value: i64) -> String {
    if value <= 0 {
        return String::new();
    }
    DateTime::from_timestamp(value, 0)
        .map(|dt| {
            dt.with_timezone(&Local)
                .format("%Y-%m-%d %H:%M")
                .to_string()
        })
        .unwrap_or_default()
}
