use super::*;

pub(super) fn last_round_map_guid(raw_detail_json: Option<&str>) -> Option<String> {
    let detail = raw_detail_json.and_then(|text| serde_json::from_str::<Value>(text).ok())?;
    final_valid_arena_round(&detail).and_then(|round| string_field(round, "mapGuid"))
}

#[derive(Default)]
pub(super) struct FightListProjection {
    pub(super) team_score: Option<i64>,
    pub(super) opponent_score: Option<i64>,
    pub(super) kills: Option<i64>,
    pub(super) assists: Option<i64>,
    pub(super) deaths: Option<i64>,
    pub(super) damage: Option<i64>,
    pub(super) healing: Option<i64>,
    pub(super) mitigation: Option<i64>,
    pub(super) duration_sec: Option<i64>,
}

pub(super) fn fight_list_projection(
    raw_detail_json: Option<&str>,
    current_bnet: Option<&str>,
) -> Option<FightListProjection> {
    let detail = raw_detail_json.and_then(|text| serde_json::from_str::<Value>(text).ok())?;
    let last_round = final_valid_arena_round(&detail)?;
    let player = current_bnet.and_then(|current_bnet| {
        last_round
            .get("teammateList")
            .and_then(Value::as_array)
            .and_then(|players| {
                players
                    .iter()
                    .find(|player| string_field(player, "bnetId").as_deref() == Some(current_bnet))
            })
    });
    Some(FightListProjection {
        team_score: optional_i64(last_round, "teamScore"),
        opponent_score: optional_i64(last_round, "opponentScore"),
        kills: player.and_then(|item| optional_i64(item, "kill")),
        assists: player.and_then(|item| optional_i64(item, "assist")),
        deaths: player.and_then(|item| optional_i64(item, "death")),
        damage: player.and_then(|item| optional_i64(item, "heroDamage")),
        healing: player.and_then(|item| optional_i64(item, "cure")),
        mitigation: player.and_then(|item| optional_i64(item, "resistDamage")),
        duration_sec: optional_i64(last_round, "gameTimeSec"),
    })
}

pub(super) fn stadium_team_awards(
    raw_detail_json: Option<&str>,
    current_bnet: Option<&str>,
) -> Vec<MatchPerformanceAward> {
    let Some(detail) = raw_detail_json.and_then(|text| serde_json::from_str::<Value>(text).ok())
    else {
        return Vec::new();
    };
    let Some(round) = final_valid_arena_round(&detail) else {
        return Vec::new();
    };
    let mut players = player_rows(round.get("teammateList"), "teammate", &HashMap::new());
    let Some(current_index) = current_bnet.and_then(|current_bnet| {
        players
            .iter()
            .position(|player| player.bnet_id == current_bnet)
    }) else {
        return Vec::new();
    };
    players.swap(0, current_index);
    team_awards_from_players(&players)
}

fn final_valid_arena_round(detail: &Value) -> Option<&Value> {
    detail
        .pointer("/payload/data/roundCountList")
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

pub(super) fn match_evaluation_summary(
    conn: &rusqlite::Connection,
    match_id: &str,
) -> Result<Option<MatchEvaluationSummary>> {
    conn.query_row(
        r#"
        SELECT grade, primary_tag, evidence_tags_json, confidence
        FROM match_evaluations
        WHERE match_id=?
        "#,
        params![match_id],
        |row| evaluation_from_row(row, 0),
    )
    .optional()
    .map(|value| value.flatten())
    .map_err(Into::into)
}

pub(super) fn evaluation_from_row(
    row: &rusqlite::Row<'_>,
    offset: usize,
) -> rusqlite::Result<Option<MatchEvaluationSummary>> {
    let grade: Option<String> = row.get(offset)?;
    let Some(grade) = grade else {
        return Ok(None);
    };
    let evidence_text: String = row.get(offset + 2)?;
    let evidence_tags = serde_json::from_str::<Vec<String>>(&evidence_text).unwrap_or_default();
    Ok(Some(MatchEvaluationSummary {
        grade,
        primary_tag: row.get(offset + 1)?,
        evidence_tags,
        confidence: row.get(offset + 3)?,
    }))
}

pub(super) fn player_rows(
    value: Option<&Value>,
    side: &str,
    hero_names: &HashMap<String, String>,
) -> Vec<PlayerRow> {
    value
        .and_then(Value::as_array)
        .map(|items| {
            let rows = items
                .iter()
                .filter_map(|item| {
                    if !item.is_object() {
                        return None;
                    }
                    let hero_guid = string_field(item, "heroGuid").unwrap_or_default();
                    Some(PlayerRow {
                        side: side.to_string(),
                        name: string_field(item, "name").unwrap_or_else(|| {
                            string_field(item, "bnetId").unwrap_or_else(|| "未知玩家".to_string())
                        }),
                        bnet_id: string_field(item, "bnetId").unwrap_or_default(),
                        rank_label: competitive_rank_label(item),
                        rank_key: competitive_rank_key(item),
                        hero_guid: hero_guid.clone(),
                        hero_name: hero_names.get(&hero_guid).cloned().unwrap_or_else(|| {
                            fallback_name(String::new(), &hero_guid, "未知英雄")
                        }),
                        kills: int_field(item, "kill"),
                        assists: int_field(item, "assist"),
                        deaths: int_field(item, "death"),
                        damage: int_field(item, "heroDamage"),
                        healing: int_field(item, "cure"),
                        mitigation: int_field(item, "resistDamage"),
                        final_hits: int_field(item, "finalHit"),
                        target_time_sec: int_field(item, "targetCompetingTime"),
                        healing_taken: int_field(item, "healingTaken"),
                        damage_taken: int_field(item, "damageTaken"),
                        worth: None,
                        end_worth: None,
                        worth_max: false,
                        mod_guids: Vec::new(),
                        trait_guids: Vec::new(),
                        has_source_max_flags: false,
                        kill_max: false,
                        hero_damage_max: false,
                        cure_max: false,
                        resist_damage_max: false,
                        perks: parse_player_perks(item),
                        hero_stats: parse_hero_stats(item),
                        party_group: None,
                    })
                })
                .collect();
            assign_party_groups(items, rows)
        })
        .unwrap_or_default()
}

pub(super) fn player_rows_for_side<'a>(data: &'a Value, list_name: &str) -> Option<&'a Value> {
    data.pointer(&format!("/totalCount/{list_name}"))
        .filter(|value| {
            value
                .as_array()
                .map(|items| !items.is_empty())
                .unwrap_or(false)
        })
        .or_else(|| data.get(list_name))
}

pub(super) fn build_arena_detail(
    data: &Value,
    match_result: Option<&str>,
    match_begin_ts: Option<i64>,
    hero_names: &HashMap<String, String>,
    map_names: &HashMap<String, String>,
) -> Option<ArenaMatchDetail> {
    let name_map = arena_name_map(data);
    let total = data.get("totalCount").unwrap_or(&Value::Null);
    let rounds_value = data.get("roundCountList");
    let summary_begin_ts = match_begin_ts
        .filter(|value| *value > 0)
        .or_else(|| first_round_start_time(rounds_value))
        .or_else(|| optional_i64(total, "startTime").map(normalize_timestamp));
    let summary_duration_sec =
        optional_i64(total, "gameTimeSec").or_else(|| round_duration_sum(rounds_value));
    let summary_source = last_arena_round(rounds_value).unwrap_or(total);
    let summary_map_guid = string_field(summary_source, "mapGuid")
        .or_else(|| string_field(total, "mapGuid"))
        .unwrap_or_else(|| "arena-victoriae".to_string());
    let summary = ArenaDetailSlice {
        key: "summary".to_string(),
        label: "角斗汇总".to_string(),
        result: match_result.map(str::to_string),
        result_label: match_result.map(result_label).map(str::to_string),
        team_score: optional_i64(summary_source, "teamScore")
            .or_else(|| optional_i64(total, "teamScore")),
        opponent_score: optional_i64(summary_source, "opponentScore")
            .or_else(|| optional_i64(total, "opponentScore")),
        duration_sec: summary_duration_sec,
        begin_ts: summary_begin_ts,
        begin_label: summary_begin_ts.map(format_begin_ts),
        map_name: Some(
            map_names
                .get(&summary_map_guid)
                .cloned()
                .unwrap_or_else(|| "未知地图".to_string()),
        ),
        map_guid: Some(summary_map_guid),
        teammates: arena_summary_player_rows(
            summary_source,
            total,
            rounds_value,
            "teammateList",
            "teammate",
            &name_map,
            hero_names,
        ),
        enemies: arena_summary_player_rows(
            summary_source,
            total,
            rounds_value,
            "enemyList",
            "enemy",
            &name_map,
            hero_names,
        ),
    };
    let rounds = rounds_value
        .and_then(Value::as_array)
        .map(|items| {
            let mut previous_team = 0;
            let mut previous_opponent = 0;
            items
                .iter()
                .enumerate()
                .map(|(index, round)| {
                    let team_score = optional_i64(round, "teamScore");
                    let opponent_score = optional_i64(round, "opponentScore");
                    let result = arena_round_result(
                        team_score,
                        opponent_score,
                        previous_team,
                        previous_opponent,
                    );
                    previous_team = team_score.unwrap_or(previous_team);
                    previous_opponent = opponent_score.unwrap_or(previous_opponent);
                    let begin_ts = optional_i64(round, "startTime").map(normalize_timestamp);
                    let map_guid = string_field(round, "mapGuid");
                    ArenaDetailSlice {
                        key: format!("round-{}", index + 1),
                        label: format!("回合 {}", index + 1),
                        result: result.map(str::to_string),
                        result_label: result.map(result_label).map(str::to_string),
                        team_score,
                        opponent_score,
                        duration_sec: optional_i64(round, "gameTimeSec"),
                        begin_ts,
                        begin_label: begin_ts.map(format_begin_ts),
                        map_name: map_guid.as_ref().map(|guid| {
                            map_names
                                .get(guid)
                                .cloned()
                                .unwrap_or_else(|| "未知地图".to_string())
                        }),
                        map_guid,
                        teammates: arena_player_rows(
                            round.get("teammateList"),
                            "teammate",
                            &name_map,
                            hero_names,
                        ),
                        enemies: arena_player_rows(
                            round.get("enemyList"),
                            "enemy",
                            &name_map,
                            hero_names,
                        ),
                    }
                })
                .collect()
        })
        .unwrap_or_default();
    Some(ArenaMatchDetail { summary, rounds })
}

pub(super) fn last_arena_round(rounds_value: Option<&Value>) -> Option<&Value> {
    rounds_value
        .and_then(Value::as_array)
        .and_then(|items| items.last())
}

pub(super) fn arena_summary_player_rows(
    summary_source: &Value,
    total: &Value,
    rounds_value: Option<&Value>,
    list_name: &str,
    side: &str,
    name_map: &HashMap<String, String>,
    hero_names: &HashMap<String, String>,
) -> Vec<PlayerRow> {
    let source_list = summary_source
        .get(list_name)
        .filter(|value| value.as_array().is_some_and(|items| !items.is_empty()))
        .or_else(|| total.get(list_name));
    let mut rows = arena_player_rows_unordered(source_list, side, name_map, hero_names);
    let aggregates = arena_round_damage_healing_totals(rounds_value, list_name);
    if let Some(items) = source_list.and_then(Value::as_array) {
        for (row, item) in rows.iter_mut().zip(items) {
            let Some(bnet_id) = string_field(item, "bnetId") else {
                continue;
            };
            if let Some((damage_taken, healing_taken)) = aggregates.get(&bnet_id) {
                row.damage_taken = *damage_taken;
                row.healing_taken = *healing_taken;
            }
        }
    }
    let Some(items) = source_list.and_then(Value::as_array) else {
        return rows;
    };
    assign_party_groups(items, rows)
}

pub(super) fn arena_round_damage_healing_totals(
    rounds_value: Option<&Value>,
    list_name: &str,
) -> HashMap<String, (i64, i64)> {
    let mut totals = HashMap::new();
    let Some(rounds) = rounds_value.and_then(Value::as_array) else {
        return totals;
    };
    for round in rounds {
        let Some(players) = round.get(list_name).and_then(Value::as_array) else {
            continue;
        };
        for player in players {
            let Some(bnet_id) = string_field(player, "bnetId") else {
                continue;
            };
            let entry = totals.entry(bnet_id).or_insert((0, 0));
            entry.0 += int_field(player, "damageTaken");
            entry.1 += int_field(player, "healingTaken");
        }
    }
    totals
}

pub(super) fn arena_player_rows(
    value: Option<&Value>,
    side: &str,
    name_map: &HashMap<String, String>,
    hero_names: &HashMap<String, String>,
) -> Vec<PlayerRow> {
    let rows = arena_player_rows_unordered(value, side, name_map, hero_names);
    let Some(items) = value.and_then(Value::as_array) else {
        return rows;
    };
    assign_party_groups(items, rows)
}

fn arena_player_rows_unordered(
    value: Option<&Value>,
    side: &str,
    name_map: &HashMap<String, String>,
    hero_names: &HashMap<String, String>,
) -> Vec<PlayerRow> {
    value
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|item| {
                    if !item.is_object() {
                        return None;
                    }
                    let hero_guid = string_field(item, "heroGuid").unwrap_or_default();
                    let bnet_id = string_field(item, "bnetId").unwrap_or_default();
                    Some(PlayerRow {
                        side: side.to_string(),
                        name: arena_player_name(item, &bnet_id, name_map),
                        bnet_id: String::new(),
                        rank_label: arena_rank_label(item),
                        rank_key: arena_rank_key(item),
                        hero_guid: hero_guid.clone(),
                        hero_name: hero_names
                            .get(&hero_guid)
                            .cloned()
                            .unwrap_or_else(|| "未知英雄".to_string()),
                        kills: int_field(item, "kill"),
                        assists: int_field(item, "assist"),
                        deaths: int_field(item, "death"),
                        damage: int_field(item, "heroDamage"),
                        healing: int_field(item, "cure"),
                        mitigation: int_field(item, "resistDamage"),
                        final_hits: int_field(item, "finalHit"),
                        target_time_sec: int_field(item, "targetCompetingTime"),
                        healing_taken: int_field(item, "healingTaken"),
                        damage_taken: int_field(item, "damageTaken"),
                        worth: optional_i64(item, "worth"),
                        end_worth: optional_i64(item, "endWorth"),
                        worth_max: bool_field(item, "worthMax"),
                        mod_guids: string_array_field(item, "modGuids"),
                        trait_guids: string_array_field(item, "traitGuids"),
                        has_source_max_flags: true,
                        kill_max: bool_field(item, "killMax"),
                        hero_damage_max: bool_field(item, "heroDamageMax"),
                        cure_max: bool_field(item, "cureMax"),
                        resist_damage_max: bool_field(item, "resistDamageMax"),
                        perks: Vec::new(),
                        hero_stats: parse_hero_stats(item),
                        party_group: None,
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

pub(super) fn arena_name_map(data: &Value) -> HashMap<String, String> {
    data.get("nameMap")
        .and_then(Value::as_object)
        .map(|items| {
            items
                .iter()
                .filter_map(|(key, value)| {
                    let name = value.as_str()?.trim();
                    if key.trim().is_empty() || name.is_empty() {
                        None
                    } else {
                        Some((key.to_string(), name.to_string()))
                    }
                })
                .collect()
        })
        .unwrap_or_default()
}

pub(super) fn arena_player_name(
    item: &Value,
    bnet_id: &str,
    name_map: &HashMap<String, String>,
) -> String {
    string_field(item, "name")
        .filter(|name| !looks_like_private_id(name))
        .or_else(|| {
            name_map
                .get(bnet_id)
                .filter(|name| !looks_like_private_id(name))
                .cloned()
        })
        .unwrap_or_else(|| "未知玩家".to_string())
}

pub(super) fn first_round_start_time(value: Option<&Value>) -> Option<i64> {
    value
        .and_then(Value::as_array)
        .and_then(|items| items.first())
        .and_then(|round| optional_i64(round, "startTime"))
        .map(normalize_timestamp)
}

pub(super) fn round_duration_sum(value: Option<&Value>) -> Option<i64> {
    let total = value
        .and_then(Value::as_array)?
        .iter()
        .filter_map(|round| optional_i64(round, "gameTimeSec"))
        .sum::<i64>();
    if total > 0 {
        Some(total)
    } else {
        None
    }
}

pub(super) fn arena_rank_label(item: &Value) -> Option<String> {
    competitive_rank_label(item)
}

pub(super) fn competitive_rank_label(item: &Value) -> Option<String> {
    let rank = item.get("rankInfo")?;
    let raw_name = string_field(rank, "rankName").or_else(|| string_field(rank, "rank_name"))?;
    let name = rank_name_label(&raw_name)?;
    let sub_tier =
        optional_i64(rank, "rankSubTier").or_else(|| optional_i64(rank, "rank_sub_tier"));
    Some(match sub_tier {
        Some(value) if value > 0 => format!("{name} {value}"),
        _ => name.to_string(),
    })
}

pub(super) fn arena_rank_key(item: &Value) -> Option<String> {
    competitive_rank_key(item)
}

pub(super) fn competitive_rank_key(item: &Value) -> Option<String> {
    let rank = item.get("rankInfo")?;
    let raw_name = string_field(rank, "rankName").or_else(|| string_field(rank, "rank_name"))?;
    let key = normalize_rank_key(&raw_name)?;
    Some(key.to_string())
}

pub(super) fn normalize_rank_key(value: &str) -> Option<&'static str> {
    match value.trim().to_ascii_lowercase().as_str() {
        "bronze" => Some("bronze"),
        "silver" => Some("silver"),
        "gold" => Some("gold"),
        "platinum" => Some("platinum"),
        "diamond" => Some("diamond"),
        "master" => Some("master"),
        "grandmaster" => Some("grandmaster"),
        "champion" => Some("champion"),
        "top 500" | "top500" => Some("top500"),
        _ => None,
    }
}

pub(super) fn rank_name_label(value: &str) -> Option<&'static str> {
    match value.trim().to_ascii_lowercase().as_str() {
        "bronze" => Some("青铜"),
        "silver" => Some("白银"),
        "gold" => Some("黄金"),
        "platinum" => Some("白金"),
        "diamond" => Some("钻石"),
        "master" => Some("大师"),
        "grandmaster" => Some("宗师"),
        "champion" => Some("冠军"),
        "top 500" | "top500" => Some("五百强"),
        _ => None,
    }
}

pub(super) fn bool_field(value: &Value, key: &str) -> bool {
    value
        .get(key)
        .and_then(|item| {
            item.as_bool()
                .or_else(|| item.as_i64().map(|number| number != 0))
                .or_else(|| item.as_str().map(|text| matches!(text, "true" | "1")))
        })
        .unwrap_or(false)
}

pub(super) fn string_array_field(value: &Value, key: &str) -> Vec<String> {
    value
        .get(key)
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|item| {
                    item.as_str()
                        .map(str::trim)
                        .filter(|text| !text.is_empty())
                        .map(str::to_string)
                        .or_else(|| item.as_i64().map(|number| number.to_string()))
                })
                .collect()
        })
        .unwrap_or_default()
}

pub(super) fn arena_round_result(
    team_score: Option<i64>,
    opponent_score: Option<i64>,
    previous_team: i64,
    previous_opponent: i64,
) -> Option<&'static str> {
    let team_delta = team_score.map(|score| score - previous_team);
    let opponent_delta = opponent_score.map(|score| score - previous_opponent);
    if team_delta.is_some_and(|delta| delta > 0) {
        Some("win")
    } else if opponent_delta.is_some_and(|delta| delta > 0) {
        Some("loss")
    } else {
        None
    }
}

pub(super) fn optional_i64(value: &Value, key: &str) -> Option<i64> {
    value.get(key).and_then(|item| {
        item.as_i64()
            .or_else(|| item.as_f64().map(|value| value.round() as i64))
            .or_else(|| item.as_str().and_then(|text| text.parse().ok()))
    })
}

pub(super) fn normalize_timestamp(value: i64) -> i64 {
    if value > 10_000_000_000 {
        value / 1000
    } else {
        value
    }
}

pub(super) fn looks_like_private_id(value: &str) -> bool {
    value.chars().all(|item| item.is_ascii_digit()) && value.len() >= 4
}

pub(super) fn parse_player_perks(item: &Value) -> Vec<PlayerPerk> {
    item.get("perks")
        .and_then(Value::as_array)
        .map(|perks| {
            perks
                .iter()
                .filter_map(|perk| {
                    let guid = string_field(perk, "guid")?;
                    Some(PlayerPerk {
                        guid,
                        perk_level: int_field(perk, "perkLevel"),
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

pub(super) fn parse_hero_stats(item: &Value) -> Vec<HeroStat> {
    item.get("statMap")
        .and_then(Value::as_object)
        .map(|stats| {
            let mut rows = stats
                .iter()
                .filter_map(|(guid, value)| {
                    stat_value(value).map(|number| HeroStat {
                        guid: guid.clone(),
                        value: number,
                    })
                })
                .collect::<Vec<_>>();
            rows.sort_by(|a, b| a.guid.cmp(&b.guid));
            rows
        })
        .unwrap_or_default()
}

pub(super) fn stat_value(value: &Value) -> Option<f64> {
    value
        .as_f64()
        .or_else(|| value.as_i64().map(|number| number as f64))
        .or_else(|| value.as_str().and_then(|text| text.parse::<f64>().ok()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normal_player_rows_map_competitive_rank_info() {
        let players = json!([{
            "name": "合成玩家",
            "heroGuid": "synthetic-hero",
            "rankInfo": { "rankName": "Diamond", "rankSubTier": 4 }
        }]);

        let rows = player_rows(Some(&players), "teammate", &HashMap::new());

        assert_eq!(rows[0].rank_key.as_deref(), Some("diamond"));
        assert_eq!(rows[0].rank_label.as_deref(), Some("钻石 4"));
    }

    #[test]
    fn normal_player_rows_support_snake_case_and_leave_unknown_rank_empty() {
        let known = json!([{
            "name": "合成玩家",
            "heroGuid": "synthetic-hero",
            "rankInfo": { "rank_name": "Master", "rank_sub_tier": 2 }
        }]);
        let unknown = json!([{
            "name": "合成玩家",
            "heroGuid": "synthetic-hero",
            "rankInfo": { "rankName": "UnknownTier", "rankSubTier": 1 }
        }]);

        let known_rows = player_rows(Some(&known), "enemy", &HashMap::new());
        let unknown_rows = player_rows(Some(&unknown), "enemy", &HashMap::new());

        assert_eq!(known_rows[0].rank_key.as_deref(), Some("master"));
        assert_eq!(known_rows[0].rank_label.as_deref(), Some("大师 2"));
        assert_eq!(unknown_rows[0].rank_key, None);
        assert_eq!(unknown_rows[0].rank_label, None);
    }

    #[test]
    fn normal_player_rows_group_parties_without_exposing_friend_ids() {
        let players = json!([
            {"bnetId":"solo-a","name":"单排甲","heroGuid":"hero-a"},
            {"bnetId":"party-a","name":"组队甲","heroGuid":"hero-b","friendBnetIds":["party-b"]},
            {"bnetId":"solo-b","name":"单排乙","heroGuid":"hero-c"},
            {"bnetId":"party-b","name":"组队乙","heroGuid":"hero-d","friendBnetIds":[]}
        ]);

        let rows = player_rows(Some(&players), "teammate", &HashMap::new());

        assert_eq!(
            rows.iter()
                .map(|row| (row.name.as_str(), row.party_group))
                .collect::<Vec<_>>(),
            vec![
                ("单排甲", None),
                ("组队甲", Some(0)),
                ("组队乙", Some(0)),
                ("单排乙", None),
            ]
        );
    }

    #[test]
    fn arena_player_rows_apply_the_same_party_grouping() {
        let players = json!([
            {"bnetId":"party-a","heroGuid":"hero-a","friendBnetIds":["party-b"]},
            {"bnetId":"solo","heroGuid":"hero-b"},
            {"bnetId":"party-b","heroGuid":"hero-c"}
        ]);
        let names = HashMap::from([
            ("party-a".to_string(), "组队甲".to_string()),
            ("party-b".to_string(), "组队乙".to_string()),
            ("solo".to_string(), "单排".to_string()),
        ]);

        let rows = arena_player_rows(Some(&players), "enemy", &names, &HashMap::new());

        assert_eq!(
            rows.iter()
                .map(|row| (row.name.as_str(), row.party_group))
                .collect::<Vec<_>>(),
            vec![("组队甲", Some(0)), ("组队乙", Some(0)), ("单排", None),]
        );
        assert!(rows.iter().all(|row| row.bnet_id.is_empty()));
    }
}
