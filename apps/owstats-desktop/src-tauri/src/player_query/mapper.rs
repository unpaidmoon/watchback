use super::models::*;
use chrono::{TimeZone, Utc};
use serde_json::Value;
use std::{
    collections::VecDeque,
    collections::{HashMap, HashSet},
    sync::OnceLock,
};

fn embedded_lookups() -> &'static Value {
    static LOOKUPS: OnceLock<Value> = OnceLock::new();
    LOOKUPS.get_or_init(|| {
        serde_json::from_str(include_str!("../../../runtime-config/ow-lookups.json"))
            .expect("embedded runtime lookup must be valid")
    })
}

fn lookup_name(section: &str, guid: &str) -> Option<String> {
    embedded_lookups()
        .pointer(&format!("/{section}/{guid}/name"))
        .and_then(Value::as_str)
        .filter(|name| !name.trim().is_empty())
        .map(str::to_string)
        .or_else(|| {
            (section == "maps")
                .then(|| manual_arena_map_name(guid).map(str::to_string))
                .flatten()
        })
}

fn manual_arena_map_name(guid: &str) -> Option<&'static str> {
    match guid {
        "576460752303428183" => Some("泰晤士区"),
        "576460752303428209" | "576460752303428488" => Some("火药桶矿井"),
        "576460752303428223" => Some("绿洲城 - 花园"),
        "576460752303429014" => Some("绿洲城 - 大学"),
        "576460752303428469" => Some("五行大学 - 水学院"),
        "576460752303428272" => Some("埃斯佩兰萨"),
        "576460752303428437" => Some("萨摩亚"),
        "576460752303428212" => Some("塞伦萨"),
        "576460752303428683" => Some("漓江塔 - 夜市"),
        _ => None,
    }
}

fn data(value: &Value) -> QueryResult<&Value> {
    if value.get("code").and_then(Value::as_i64).unwrap_or(0) != 0 {
        return Err(QueryError::Upstream("上游拒绝请求".into()));
    }
    value
        .get("data")
        .ok_or_else(|| QueryError::InvalidResponse("响应缺少 data".into()))
}
pub fn identity(value: &Value) -> QueryResult<(Option<String>, String)> {
    let d = data(value)?;
    Ok((
        d.get("battleTag")
            .or_else(|| d.get("bnetId"))
            .or_else(|| d.get("name"))
            .and_then(Value::as_str)
            .filter(|v| !v.is_empty())
            .map(str::to_string),
        d.get("accountKey")
            .or_else(|| d.get("customerToken"))
            .or_else(|| d.get("customer_token"))
            .and_then(Value::as_str)
            .filter(|v| !v.is_empty())
            .ok_or_else(|| QueryError::NotFound("未找到玩家".into()))?
            .to_string(),
    ))
}
pub fn card(
    value: &Value,
    battle_tag: String,
    game_action: Option<crate::models::PlayerGameAction>,
) -> QueryResult<PlayerQueryCard> {
    let d = data(value)?;
    let game_time_seconds = d.get("gameTime").and_then(|value| {
        value
            .as_f64()
            .or_else(|| value.as_str().and_then(|text| text.parse::<f64>().ok()))
            .filter(|hours| hours.is_finite() && *hours >= 0.0)
            .map(|hours| (hours * 3600.0).round() as u64)
    });
    Ok(PlayerQueryCard {
        battle_tag,
        level: d.get("level").and_then(Value::as_u64),
        game_time_seconds,
        title: d
            .get("title")
            .and_then(Value::as_str)
            .filter(|v| !v.is_empty())
            .map(str::to_string),
        game_action,
    })
}

pub fn game_action(value: &Value) -> Option<crate::models::PlayerGameAction> {
    let raw = value.pointer("/data/gameAction")?.as_object()?;
    Some(crate::models::PlayerGameAction {
        comment: raw
            .get("comment")
            .and_then(Value::as_str)
            .map(str::to_string),
        send_honors_cnt: raw.get("sendHonorsCnt").and_then(Value::as_i64),
        get_honors_cnt: raw.get("getHonorsCnt").and_then(Value::as_i64),
        reported_cnt: raw.get("reportedCnt").and_then(Value::as_i64),
    })
}

pub async fn card_images(
    value: &Value,
    upstream: &dyn super::upstream::PlayerQueryUpstream,
) -> QueryResult<PlayerQueryCardImages> {
    let d = data(value)?;
    let icon_url = d
        .get("icon")
        .and_then(Value::as_str)
        .filter(|v| !v.is_empty());
    let title_icon_url = d
        .get("titleIcon")
        .or_else(|| d.get("titleIconUrl"))
        .and_then(Value::as_str)
        .filter(|v| !v.is_empty());
    // Keep the two small CDN downloads sequential. Some proxies/CDNs intermittently drop
    // one of two simultaneous requests, which previously left titleIcon empty while the
    // avatar still rendered.
    let icon = image(icon_url, upstream).await;
    let title_icon = image(title_icon_url, upstream).await;
    Ok(PlayerQueryCardImages { icon, title_icon })
}
async fn image(
    url: Option<&str>,
    upstream: &dyn super::upstream::PlayerQueryUpstream,
) -> Option<MemoryImage> {
    let url = url?;
    if !(url.starts_with("https://") || url.starts_with("http://")) {
        return None;
    }
    let bytes = upstream.image(url, 256 * 1024).await.ok()?;
    let mime = bytes.get("mimeType")?.as_str()?.to_string();
    if !["image/png", "image/jpeg", "image/webp"].contains(&mime.as_str()) {
        return None;
    }
    let raw = bytes.get("bytes")?.as_array()?;
    if raw.len() > 256 * 1024 {
        return None;
    }
    let raw: Vec<u8> = raw
        .iter()
        .map(Value::as_u64)
        .collect::<Option<Vec<_>>>()?
        .into_iter()
        .map(|v| v as u8)
        .collect();
    Some(MemoryImage {
        mime_type: mime,
        data_base64: encode_base64(&raw),
    })
}
fn encode_base64(bytes: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let a = chunk[0];
        let b = *chunk.get(1).unwrap_or(&0);
        let c = *chunk.get(2).unwrap_or(&0);
        out.push(T[(a >> 2) as usize] as char);
        out.push(T[(((a & 3) << 4) | (b >> 4)) as usize] as char);
        out.push(if chunk.len() > 1 {
            T[(((b & 15) << 2) | (c >> 6)) as usize] as char
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            T[(c & 63) as usize] as char
        } else {
            '='
        });
    }
    out
}
pub fn ranks(value: &Value) -> QueryResult<Vec<RankRow>> {
    let data = data(value)?;
    let rows = data
        .get("guideCountData")
        .or_else(|| data.get("roleRanks"))
        .and_then(Value::as_array)
        .ok_or_else(|| QueryError::InvalidResponse("rank summary is unavailable".into()))?;
    Ok(rows
        .iter()
        .map(|v| RankRow {
            role: v
                .get("roleType")
                .or_else(|| v.get("role"))
                .and_then(Value::as_str)
                .unwrap_or_default()
                .into(),
            rank: v
                .pointer("/lastRankInfo/rankName")
                .or_else(|| v.pointer("/lastRankInfo/rank_name"))
                .or_else(|| v.get("rank"))
                .and_then(Value::as_str)
                .map(str::to_string),
            division: v
                .pointer("/lastRankInfo/rankSubTier")
                .or_else(|| v.pointer("/lastRankInfo/rank_sub_tier"))
                .or_else(|| v.get("division"))
                .and_then(Value::as_i64),
            wins: v.get("wins").and_then(Value::as_u64),
            games: v
                .get("matchSum")
                .or_else(|| v.get("games"))
                .and_then(Value::as_u64),
            win_rate: normalize_win_rate(v.get("winRate").and_then(|v| {
                v.as_f64()
                    .or_else(|| v.as_str().and_then(|s| s.parse().ok()))
            })),
            kda: v.get("kda").and_then(|v| {
                v.as_f64()
                    .or_else(|| v.as_str().and_then(|s| s.parse().ok()))
            }),
        })
        .collect())
}
pub fn competitive_ranks(value: &Value) -> QueryResult<Vec<RankRow>> {
    Ok(ranks(value)?
        .into_iter()
        .filter(|row| row.role != "open")
        .collect())
}
pub fn open_competitive_rank(value: &Value) -> QueryResult<Vec<RankRow>> {
    Ok(ranks(value)?
        .into_iter()
        .filter(|row| row.role == "open")
        .collect())
}
pub fn stadium_rank(value: &Value) -> QueryResult<Vec<RankRow>> {
    let parsed: FightCountData = serde_json::from_value(data(value)?.clone())
        .map_err(|_| QueryError::InvalidResponse("fight count summary is unavailable".into()))?;
    rank_rows(&parsed.role_type_count_data)
}

fn rank_rows(rows: &[Value]) -> QueryResult<Vec<RankRow>> {
    let wrapped = serde_json::json!({"code": 0, "data": {"guideCountData": rows}});
    ranks(&wrapped)
}
pub fn heroes(value: &Value) -> QueryResult<Vec<HeroUsage>> {
    heroes_from_key(value, "heroList")
}
pub fn heroes_for_mode(value: &Value, mode: QueryMode) -> QueryResult<Vec<HeroUsage>> {
    match mode {
        QueryMode::SportFight => {
            let parsed: FightCountData =
                serde_json::from_value(data(value)?.clone()).map_err(|_| {
                    QueryError::InvalidResponse("fight hero summary is unavailable".into())
                })?;
            Ok(heroes_from_rows(&parsed.hero_use_summary_list))
        }
        QueryMode::LeisureFight => {
            let parsed: LeisureFightRoleCardData = serde_json::from_value(data(value)?.clone())
                .map_err(|_| {
                    QueryError::InvalidResponse("leisure fight hero summary is unavailable".into())
                })?;
            Ok(heroes_from_rows(&parsed.hero_data_list))
        }
        _ => heroes_from_key(value, mode.hero_summary_key()),
    }
}
fn heroes_from_key(value: &Value, key: &str) -> QueryResult<Vec<HeroUsage>> {
    let data = data(value)?;
    let rows = data
        .get(key)
        .or_else(|| data.get("heroList"))
        .and_then(Value::as_array)
        .ok_or_else(|| QueryError::InvalidResponse("hero summary is unavailable".into()))?;
    Ok(heroes_from_rows(rows))
}
fn heroes_from_rows(rows: &[Value]) -> Vec<HeroUsage> {
    rows.iter()
        .filter_map(|v| {
            let duration = v
                .get("gameTime")
                .or_else(|| v.get("gameTimeSec"))
                .and_then(Value::as_u64)?;
            (duration > 0).then(|| HeroUsage {
                hero_guid: v
                    .get("heroGuid")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .into(),
                hero_name: v
                    .get("heroName")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .into(),
                game_time_seconds: duration,
                wins: v
                    .get("winSum")
                    .or_else(|| v.get("wins"))
                    .and_then(Value::as_u64),
                games: v
                    .get("matchSum")
                    .or_else(|| v.get("games"))
                    .and_then(Value::as_u64),
                win_rate: normalize_win_rate(v.get("winRate").and_then(|v| {
                    v.as_f64()
                        .or_else(|| v.as_str().and_then(|s| s.parse().ok()))
                })),
            })
        })
        .collect()
}
fn normalize_win_rate(value: Option<f64>) -> Option<f64> {
    value.map(|rate| if rate > 1.0 { rate / 100.0 } else { rate })
}
pub fn matches(value: &Value, kind: MatchKind) -> QueryResult<(Vec<MatchRow>, usize)> {
    matches_filtered(value, kind, None)
}
pub fn matches_for_mode(
    value: &Value,
    kind: MatchKind,
    exact_mode: &str,
) -> QueryResult<(Vec<MatchRow>, usize)> {
    matches_filtered(value, kind, Some(exact_mode))
}
fn matches_filtered(
    value: &Value,
    kind: MatchKind,
    exact_mode: Option<&str>,
) -> QueryResult<(Vec<MatchRow>, usize)> {
    let data = data(value)?;
    let raw = data
        .as_array()
        .or_else(|| data.get("matchList").and_then(Value::as_array))
        .ok_or_else(|| QueryError::InvalidResponse("响应缺少 matchList".into()))?;
    let mut seen = HashSet::new();
    let rows = raw
        .iter()
        .filter(|v| {
            exact_mode.is_none_or(|mode| {
                v.get("gameMode")
                    .and_then(Value::as_str)
                    .is_none_or(|actual| actual == mode)
            })
        })
        .filter_map(|v| {
            let id = v.get("matchId")?.as_str()?.to_string();
            if !seen.insert(id.clone()) {
                return None;
            }
            let score = score(v);
            let played_at = v
                .get("beginTs")
                .and_then(Value::as_i64)
                .and_then(|ts| Utc.timestamp_millis_opt(ts).single())
                .map(|d| d.to_rfc3339());
            Some(MatchRow {
                match_id: id,
                match_kind: kind,
                result: result(v.get("matchRet")),
                score,
                map_key: (kind == MatchKind::Normal)
                    .then(|| v.get("mapGuid").and_then(Value::as_str).map(str::to_string))
                    .flatten(),
                map_name: (kind == MatchKind::Normal)
                    .then(|| v.get("mapName").and_then(Value::as_str).map(str::to_string))
                    .flatten(),
                hero_key: v
                    .get("heroGuid")
                    .and_then(Value::as_str)
                    .map(str::to_string),
                hero_name: v
                    .get("heroName")
                    .and_then(Value::as_str)
                    .map(str::to_string),
                kills: v.get("kill").and_then(Value::as_i64),
                assists: v.get("assist").and_then(Value::as_i64),
                deaths: v.get("death").and_then(Value::as_i64),
                played_at,
            })
        })
        .collect();
    Ok((rows, raw.len()))
}
fn result(v: Option<&Value>) -> String {
    match v.and_then(Value::as_i64) {
        Some(1) => "win",
        Some(-1 | 2) => "loss",
        Some(3) => "draw",
        _ => "unknown",
    }
    .into()
}
fn score(v: &Value) -> String {
    match (
        v.get("teamScore").and_then(Value::as_i64),
        v.get("opponentScore").and_then(Value::as_i64),
    ) {
        (Some(a), Some(b)) => format!("{a}:{b}"),
        _ => "-".into(),
    }
}
fn item_keys(v: Option<&Value>) -> Vec<String> {
    v.and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|item| {
            item.as_str()
                .or_else(|| item.get("guid").and_then(Value::as_str))
                .or_else(|| item.get("id").and_then(Value::as_str))
                .map(str::to_string)
        })
        .collect()
}
fn string_value(value: Option<&Value>) -> Option<String> {
    value.and_then(|item| {
        item.as_str()
            .map(str::to_string)
            .or_else(|| item.as_i64().map(|number| number.to_string()))
            .or_else(|| item.as_u64().map(|number| number.to_string()))
    })
}
fn string_array(v: Option<&Value>) -> Vec<String> {
    v.and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|item| string_value(Some(item)))
        .filter(|item| !item.trim().is_empty())
        .collect()
}
fn detail_hero_stats(player: &Value) -> Vec<DetailHeroStat> {
    let mut stats = player
        .get("statMap")
        .and_then(Value::as_object)
        .into_iter()
        .flatten()
        .filter_map(|(guid, value)| {
            value
                .as_f64()
                .or_else(|| value.as_i64().map(|number| number as f64))
                .map(|number| DetailHeroStat {
                    guid: guid.clone(),
                    value: number,
                })
        })
        .collect::<Vec<_>>();
    stats.sort_by(|left, right| left.guid.cmp(&right.guid));
    stats
}
fn player_number(player: &Value, keys: &[&str]) -> i64 {
    keys.iter()
        .find_map(|key| {
            player.get(*key).and_then(|value| {
                value
                    .as_i64()
                    .or_else(|| value.as_f64().map(|number| number.round() as i64))
                    .or_else(|| {
                        value
                            .as_str()
                            .and_then(|text| text.parse::<f64>().ok())
                            .map(|number| number.round() as i64)
                    })
            })
        })
        .unwrap_or_default()
}
fn player_perks(player: &Value) -> Vec<DetailPerk> {
    player
        .get("perks")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|perk| {
            let guid = string_value(perk.get("guid"))?;
            Some(DetailPerk {
                guid,
                perk_level: player_number(perk, &["perkLevel"]),
            })
        })
        .collect()
}
fn players(v: Option<&Value>, side: &str, name_map: &HashMap<String, String>) -> Vec<DetailPlayer> {
    let Some(source) = v.and_then(Value::as_array) else {
        return Vec::new();
    };
    let rows = source
        .iter()
        .map(|p| {
            let bnet_id = string_value(p.get("bnetId")).unwrap_or_default();
            let hero_guid = string_value(p.get("heroGuid")).unwrap_or_default();
            DetailPlayer {
                side: side.to_string(),
                name: p
                    .get("playerName")
                    .or_else(|| p.get("name"))
                    .and_then(Value::as_str)
                    .filter(|name| !name.trim().is_empty())
                    .map(str::to_string)
                    .or_else(|| name_map.get(&bnet_id).cloned())
                    .unwrap_or_else(|| "未知玩家".into()),
                bnet_id,
                rank_label: p
                    .pointer("/rankInfo/rankName")
                    .or_else(|| p.pointer("/rankInfo/rank_name"))
                    .and_then(Value::as_str)
                    .map(|rank| {
                        let sub_tier = p
                            .pointer("/rankInfo/rankSubTier")
                            .or_else(|| p.pointer("/rankInfo/rank_sub_tier"))
                            .and_then(|value| {
                                value
                                    .as_i64()
                                    .or_else(|| value.as_str().and_then(|text| text.parse().ok()))
                            });
                        match sub_tier {
                            Some(value) if value > 0 => format!("{rank} {value}"),
                            _ => rank.to_string(),
                        }
                    }),
                rank_key: p
                    .pointer("/rankInfo/rankName")
                    .or_else(|| p.pointer("/rankInfo/rank_name"))
                    .and_then(Value::as_str)
                    .map(str::to_string),
                hero_guid: hero_guid.clone(),
                hero_name: p
                    .get("heroName")
                    .and_then(Value::as_str)
                    .filter(|name| !name.trim().is_empty())
                    .map(str::to_string)
                    .or_else(|| lookup_name("heroes", &hero_guid))
                    .unwrap_or_else(|| "未知英雄".into()),
                kills: player_number(p, &["kill", "kills"]),
                assists: player_number(p, &["assist", "assists"]),
                deaths: player_number(p, &["death", "deaths"]),
                damage: player_number(p, &["heroDamage", "damage"]),
                healing: player_number(p, &["cure", "healing"]),
                mitigation: player_number(p, &["resistDamage", "mitigation"]),
                final_hits: player_number(p, &["finalHit", "finalHits"]),
                target_time_sec: player_number(p, &["targetCompetingTime", "targetTimeSec"]),
                healing_taken: player_number(p, &["healingTaken"]),
                damage_taken: player_number(p, &["damageTaken"]),
                worth: p.get("worth").and_then(Value::as_i64),
                end_worth: p.get("endWorth").and_then(Value::as_i64),
                worth_max: p.get("worthMax").and_then(Value::as_bool).unwrap_or(false),
                mod_guids: {
                    let direct = string_array(p.get("modGuids"));
                    if direct.is_empty() {
                        item_keys(p.get("equipment"))
                    } else {
                        direct
                    }
                },
                trait_guids: {
                    let direct = string_array(p.get("traitGuids"));
                    if direct.is_empty() {
                        item_keys(p.get("powers"))
                    } else {
                        direct
                    }
                },
                has_source_max_flags: true,
                kill_max: p.get("killMax").and_then(Value::as_bool).unwrap_or(false),
                hero_damage_max: p
                    .get("heroDamageMax")
                    .and_then(Value::as_bool)
                    .unwrap_or(false),
                cure_max: p.get("cureMax").and_then(Value::as_bool).unwrap_or(false),
                resist_damage_max: p
                    .get("resistDamageMax")
                    .and_then(Value::as_bool)
                    .unwrap_or(false),
                perks: player_perks(p),
                hero_stats: detail_hero_stats(p),
                party_group: None,
            }
        })
        .collect();
    assign_party_groups(source, rows)
}

fn assign_party_groups(source: &[Value], rows: Vec<DetailPlayer>) -> Vec<DetailPlayer> {
    let mut first_index = HashMap::new();
    for (index, player) in source.iter().enumerate() {
        if let Some(id) = string_value(player.get("bnetId")).filter(|id| !id.is_empty()) {
            first_index.entry(id).or_insert(index);
        }
    }

    let mut edges = vec![HashSet::<usize>::new(); source.len()];
    for (index, player) in source.iter().enumerate() {
        let Some(friends) = player.get("friendBnetIds").and_then(Value::as_array) else {
            continue;
        };
        for friend in friends.iter().filter_map(|value| string_value(Some(value))) {
            let Some(&other) = first_index.get(&friend) else {
                continue;
            };
            if other != index {
                edges[index].insert(other);
                edges[other].insert(index);
            }
        }
    }

    let mut component_for = vec![None; source.len()];
    let mut components = Vec::<Vec<usize>>::new();
    for start in 0..source.len() {
        if component_for[start].is_some() || edges[start].is_empty() {
            continue;
        }
        let component_id = components.len();
        let mut queue = VecDeque::from([start]);
        let mut members = Vec::new();
        component_for[start] = Some(component_id);
        while let Some(index) = queue.pop_front() {
            members.push(index);
            for &next in &edges[index] {
                if component_for[next].is_none() {
                    component_for[next] = Some(component_id);
                    queue.push_back(next);
                }
            }
        }
        members.sort_unstable();
        components.push(members);
    }

    let mut rows = rows.into_iter().map(Some).collect::<Vec<_>>();
    let mut emitted = HashSet::new();
    let mut result = Vec::with_capacity(rows.len());
    for index in 0..rows.len() {
        if rows[index].is_none() {
            continue;
        }
        if let Some(component_id) = component_for.get(index).copied().flatten() {
            if emitted.insert(component_id) {
                for &member in &components[component_id] {
                    if let Some(mut row) = rows.get_mut(member).and_then(Option::take) {
                        row.party_group = Some(component_id);
                        result.push(row);
                    }
                }
            }
        } else if let Some(row) = rows[index].take() {
            result.push(row);
        }
    }
    result.extend(rows.into_iter().flatten());
    result
}
fn result_option(v: Option<&Value>) -> Option<String> {
    Some(result(v))
}
fn result_label_value(result: Option<&str>) -> Option<String> {
    result.map(|value| {
        match value {
            "win" => "胜利",
            "loss" => "失败",
            "draw" => "平局",
            _ => "未知结果",
        }
        .to_string()
    })
}
fn mode_label_value(mode: Option<&str>, kind: MatchKind) -> Option<String> {
    Some(
        match mode {
            Some("LeisurePreset") => "快速预设",
            Some("Leisure6v6") => "快速开放",
            Some("SportPreset") => "预设竞技",
            Some("SportOpen") => "开放竞技",
            Some("SportFight") => "角斗竞技",
            Some("LeisureFight") => "角斗快速",
            _ if kind == MatchKind::Arena => "角斗",
            _ => "未知模式",
        }
        .to_string(),
    )
}
fn arena_slice(
    value: &Value,
    key: String,
    label: String,
    name_map: &HashMap<String, String>,
) -> ArenaDetailSlice {
    let result = result_option(value.get("matchRet"));
    let map_guid = value
        .get("mapGuid")
        .and_then(|value| string_value(Some(value)));
    ArenaDetailSlice {
        key,
        label,
        result_label: result_label_value(result.as_deref()),
        result,
        team_score: value.get("teamScore").and_then(Value::as_i64),
        opponent_score: value.get("opponentScore").and_then(Value::as_i64),
        duration_sec: value.get("gameTimeSec").and_then(Value::as_i64),
        begin_ts: value
            .get("beginTs")
            .or_else(|| value.get("startTime"))
            .and_then(Value::as_i64),
        begin_label: None,
        map_guid: map_guid.clone(),
        map_name: value
            .get("mapName")
            .and_then(Value::as_str)
            .filter(|name| !name.trim().is_empty())
            .map(str::to_string)
            .or_else(|| {
                map_guid
                    .as_deref()
                    .and_then(|guid| lookup_name("maps", guid))
            }),
        teammates: players(value.get("teammateList"), "teammate", name_map),
        enemies: players(value.get("enemyList"), "enemy", name_map),
    }
}

fn arena_name_map(data: &Value) -> HashMap<String, String> {
    data.get("nameMap")
        .and_then(Value::as_object)
        .into_iter()
        .flatten()
        .filter_map(|(id, value)| {
            value
                .as_str()
                .filter(|name| !name.trim().is_empty())
                .map(|name| (id.clone(), name.to_string()))
        })
        .collect()
}
fn final_valid_arena_round(data: &Value) -> Option<&Value> {
    data.get("roundCountList")
        .and_then(Value::as_array)?
        .iter()
        .rev()
        .find(|round| {
            round.is_object()
                && round
                    .get("teammateList")
                    .and_then(Value::as_array)
                    .is_some_and(|players| !players.is_empty())
        })
}

fn apply_arena_incoming_totals(
    players: &mut [DetailPlayer],
    rounds: Option<&Value>,
    list_name: &str,
) {
    let mut totals = HashMap::<String, (f64, f64)>::new();
    for player in rounds
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|round| round.get(list_name).and_then(Value::as_array))
        .flatten()
    {
        let Some(bnet_id) = string_value(player.get("bnetId")) else {
            continue;
        };
        let entry = totals.entry(bnet_id).or_default();
        entry.0 += player
            .get("damageTaken")
            .and_then(|value| value.as_f64().or_else(|| value.as_i64().map(|v| v as f64)))
            .unwrap_or_default();
        entry.1 += player
            .get("healingTaken")
            .and_then(|value| value.as_f64().or_else(|| value.as_i64().map(|v| v as f64)))
            .unwrap_or_default();
    }
    for player in players {
        if let Some((damage, healing)) = totals.get(&player.bnet_id) {
            player.damage_taken = damage.round() as i64;
            player.healing_taken = healing.round() as i64;
        }
    }
}

pub fn detail(value: Value, kind: MatchKind) -> QueryResult<MatchDetailView> {
    let d = data(&value)?;
    let name_map = (kind == MatchKind::Arena)
        .then(|| arena_name_map(d))
        .unwrap_or_default();
    let summary_source = if kind == MatchKind::Arena {
        final_valid_arena_round(d)
            .or_else(|| d.get("totalCount"))
            .unwrap_or(d)
    } else {
        d
    };
    let mode = d
        .get("gameMode")
        .and_then(Value::as_str)
        .map(str::to_string);
    let result = result_option(if kind == MatchKind::Arena {
        d.get("matchRet")
    } else {
        summary_source.get("matchRet")
    });
    let teammates = players(
        summary_source
            .get("teammateList")
            .or_else(|| d.get("teammateList")),
        "teammate",
        &name_map,
    );
    let enemies = players(
        summary_source
            .get("enemyList")
            .or_else(|| d.get("enemyList")),
        "enemy",
        &name_map,
    );
    let arena_detail = (kind == MatchKind::Arena).then(|| {
        let mut summary = arena_slice(
            summary_source,
            "summary".into(),
            "全场汇总".into(),
            &name_map,
        );
        summary.result = result.clone();
        summary.result_label = result_label_value(result.as_deref());
        apply_arena_incoming_totals(
            &mut summary.teammates,
            d.get("roundCountList"),
            "teammateList",
        );
        apply_arena_incoming_totals(&mut summary.enemies, d.get("roundCountList"), "enemyList");
        let mut previous_team_score = 0;
        let mut previous_opponent_score = 0;
        let rounds = d
            .get("roundCountList")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .enumerate()
            .map(|(index, round)| {
                let number = round
                    .get("round")
                    .or_else(|| round.get("roundIndex"))
                    .and_then(Value::as_i64)
                    .unwrap_or(index as i64 + 1);
                let mut slice = arena_slice(
                    round,
                    format!("round-{number}"),
                    format!("回合-{number}"),
                    &name_map,
                );
                let team_score = slice.team_score.unwrap_or(previous_team_score);
                let opponent_score = slice.opponent_score.unwrap_or(previous_opponent_score);
                if slice.result.as_deref() == Some("unknown") {
                    slice.result = if team_score > previous_team_score {
                        Some("win".into())
                    } else if opponent_score > previous_opponent_score {
                        Some("loss".into())
                    } else {
                        None
                    };
                    slice.result_label = result_label_value(slice.result.as_deref());
                }
                previous_team_score = team_score;
                previous_opponent_score = opponent_score;
                slice
            })
            .collect();
        ArenaMatchDetail { summary, rounds }
    });
    Ok(MatchDetailView {
        match_id: d
            .get("matchId")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .into(),
        game_mode: mode.clone(),
        mode_label: mode_label_value(mode.as_deref(), kind),
        result_label: result_label_value(result.as_deref()),
        result,
        map_guid: summary_source
            .get("mapGuid")
            .or_else(|| d.get("mapGuid"))
            .and_then(Value::as_str)
            .map(str::to_string),
        map_name: summary_source
            .get("mapName")
            .or_else(|| d.get("mapName"))
            .and_then(Value::as_str)
            .filter(|name| !name.trim().is_empty())
            .map(str::to_string)
            .or_else(|| {
                summary_source
                    .get("mapGuid")
                    .and_then(|value| string_value(Some(value)))
                    .and_then(|guid| lookup_name("maps", &guid))
            }),
        hero_guid: d
            .get("heroGuid")
            .and_then(Value::as_str)
            .map(str::to_string),
        hero_name: d
            .get("heroName")
            .and_then(Value::as_str)
            .map(str::to_string),
        begin_ts: d
            .get("beginTs")
            .or_else(|| d.get("startTime"))
            .and_then(Value::as_i64),
        begin_label: None,
        team_score: summary_source.get("teamScore").and_then(Value::as_i64),
        opponent_score: summary_source.get("opponentScore").and_then(Value::as_i64),
        duration_sec: summary_source.get("gameTimeSec").and_then(Value::as_i64),
        teammates,
        enemies,
        arena_detail,
        evaluation: None,
        team_awards: Vec::new(),
        same_role_comparison: None,
        favorite: None,
        ai_roast: None,
        raw_json: Value::Null,
    })
}
