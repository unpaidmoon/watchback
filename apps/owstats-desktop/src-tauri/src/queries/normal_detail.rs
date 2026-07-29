use super::*;

type NormalDetailRow = (
    String,
    String,
    Option<String>,
    Option<String>,
    Option<String>,
    Option<String>,
    Option<String>,
    Option<String>,
    Option<i64>,
    Option<i64>,
    Option<i64>,
    Option<i64>,
    String,
);

pub(super) fn get_match_detail_impl(db: &OwDatabase, match_id: &str) -> Result<MatchDetailView> {
    db.initialize()?;
    let conn = db.connect()?;
    let (
        entry_text,
        detail_text,
        game_mode,
        result,
        map_guid,
        map_name,
        hero_guid,
        hero_name,
        begin_ts,
        team_score,
        opponent_score,
        duration_sec,
        match_kind,
    ): NormalDetailRow = conn
        .query_row(
            r#"
            SELECT
                matches.raw_entry_json,
                match_details.raw_detail_json,
                NULLIF(matches.game_mode, ''),
                NULLIF(matches.result, ''),
                NULLIF(matches.map_guid, ''),
                NULLIF(matches.map_name, ''),
                NULLIF(matches.hero_guid, ''),
                NULLIF(matches.hero_name, ''),
                matches.begin_ts,
                matches.team_score,
                matches.opponent_score,
                matches.duration_sec,
                match_details.match_kind
            FROM matches
            JOIN match_details ON match_details.match_id = matches.match_id
            WHERE matches.match_id = ?
            "#,
            params![match_id],
            |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                    row.get(5)?,
                    row.get(6)?,
                    row.get(7)?,
                    row.get(8)?,
                    row.get(9)?,
                    row.get(10)?,
                    row.get(11)?,
                    row.get(12)?,
                ))
            },
        )
        .with_context(|| format!("未找到对局详情：{match_id}"))?;
    let entry: Value = serde_json::from_str(&entry_text)?;
    let detail: Value = serde_json::from_str(&detail_text)?;
    let data = detail.pointer("/payload/data").unwrap_or(&Value::Null);
    let hero_names = hero_lookup(&conn)?;
    let map_names = map_lookup(&conn)?;
    let display_map_name = map_guid
        .as_deref()
        .map(|guid| resolved_map_name(&map_names, guid, map_name.as_deref().unwrap_or_default()));
    let arena_detail = if match_kind == "fight" {
        build_arena_detail(data, result.as_deref(), begin_ts, &hero_names, &map_names)
    } else {
        None
    };
    let teammates = if let Some(arena) = &arena_detail {
        arena.summary.teammates.clone()
    } else {
        player_rows(
            player_rows_for_side(data, "teammateList"),
            "teammate",
            &hero_names,
        )
    };
    let enemies = if let Some(arena) = &arena_detail {
        arena.summary.enemies.clone()
    } else {
        player_rows(
            player_rows_for_side(data, "enemyList"),
            "enemy",
            &hero_names,
        )
    };
    let team_awards = team_awards_from_match_players(&conn, match_id)?;
    let raw_json = if match_kind == "fight" {
        Value::Null
    } else {
        json!({ "entry": entry, "detail": detail })
    };
    Ok(MatchDetailView {
        match_id: match_id.to_string(),
        mode_label: game_mode.as_deref().map(mode_label).map(str::to_string),
        result_label: result.as_deref().map(result_label).map(str::to_string),
        game_mode,
        result,
        map_guid,
        map_name: display_map_name,
        hero_guid,
        hero_name,
        begin_label: begin_ts.map(format_begin_ts),
        begin_ts,
        team_score,
        opponent_score,
        duration_sec,
        teammates,
        enemies,
        arena_detail,
        evaluation: match_evaluation_summary(&conn, match_id)?,
        team_awards,
        same_role_comparison: same_role_comparison(&conn, match_id)?,
        favorite: favorite_for_match(&conn, match_id)?,
        ai_roast: roast_for_match(&conn, match_id)?,
        raw_json,
    })
}

pub(super) fn team_awards_from_match_players(
    conn: &rusqlite::Connection,
    match_id: &str,
) -> Result<Vec<MatchPerformanceAward>> {
    Ok(team_awards_for_matches(conn, &[match_id.to_string()])?
        .remove(match_id)
        .unwrap_or_default())
}

pub(super) fn team_awards_for_matches(
    conn: &rusqlite::Connection,
    match_ids: &[String],
) -> Result<HashMap<String, Vec<MatchPerformanceAward>>> {
    if match_ids.is_empty() {
        return Ok(HashMap::new());
    }
    let placeholders = std::iter::repeat_n("?", match_ids.len())
        .collect::<Vec<_>>()
        .join(",");
    let mut stmt = conn.prepare(
        &format!(r#"
        SELECT match_id, side, player_name, bnet_id, hero_guid, hero_name, kill, assist, death,
               hero_damage, cure, resist_damage, final_hit, target_competing_time,
               healing_taken, damage_taken
        FROM match_players
        WHERE match_id IN ({placeholders}) AND side='teammate'
        ORDER BY match_id, is_current_player DESC, rowid ASC
        "#),
    )?;
    let rows = stmt.query_map(rusqlite::params_from_iter(match_ids), |row| {
        Ok((row.get::<_, String>(0)?, PlayerRow {
            side: row.get(1)?,
            name: row.get::<_, Option<String>>(2)?.unwrap_or_default(),
            bnet_id: row.get::<_, Option<String>>(3)?.unwrap_or_default(),
            rank_label: None,
            rank_key: None,
            hero_guid: row.get::<_, Option<String>>(4)?.unwrap_or_default(),
            hero_name: row.get::<_, Option<String>>(5)?.unwrap_or_default(),
            kills: row.get(6)?,
            assists: row.get(7)?,
            deaths: row.get(8)?,
            damage: row.get(9)?,
            healing: row.get(10)?,
            mitigation: row.get(11)?,
            final_hits: row.get(12)?,
            target_time_sec: row.get(13)?,
            healing_taken: row.get(14)?,
            damage_taken: row.get(15)?,
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
            perks: Vec::new(),
            hero_stats: Vec::new(),
            party_group: None,
        }))
    })?;
    let mut players_by_match = HashMap::<String, Vec<PlayerRow>>::new();
    for row in rows {
        let (match_id, player) = row?;
        players_by_match.entry(match_id).or_default().push(player);
    }
    Ok(players_by_match
        .into_iter()
        .map(|(match_id, players)| (match_id, team_awards_from_players(&players)))
        .collect())
}

pub(super) fn team_awards_from_players(players: &[PlayerRow]) -> Vec<MatchPerformanceAward> {
    let Some(current) = players.first() else {
        return Vec::new();
    };
    let mut awards = Vec::new();
    push_award_if_highest(
        &mut awards,
        "kill",
        "击杀最多",
        "gold",
        current.kills,
        players.iter().map(|player| player.kills),
        true,
    );
    push_award_if_highest(
        &mut awards,
        "damage",
        "伤害最高",
        "gold",
        current.damage,
        players.iter().map(|player| player.damage),
        true,
    );
    push_award_if_highest(
        &mut awards,
        "healing",
        "治疗最高",
        "gold",
        current.healing,
        players.iter().map(|player| player.healing),
        true,
    );
    push_award_if_highest(
        &mut awards,
        "mitigation",
        "格挡最多",
        "gold",
        current.mitigation,
        players.iter().map(|player| player.mitigation),
        true,
    );
    push_award_if_highest(
        &mut awards,
        "death",
        "死亡最多",
        "danger",
        current.deaths,
        players.iter().map(|player| player.deaths),
        false,
    );
    awards
}

pub(super) fn push_award_if_highest(
    awards: &mut Vec<MatchPerformanceAward>,
    code: &str,
    label: &str,
    tone: &str,
    value: i64,
    values: impl Iterator<Item = i64>,
    require_positive: bool,
) {
    if require_positive && value <= 0 {
        return;
    }
    if values
        .max()
        .map(|max| value >= max && max > 0)
        .unwrap_or(false)
    {
        awards.push(MatchPerformanceAward {
            code: code.to_string(),
            label: label.to_string(),
            tone: tone.to_string(),
        });
    }
}

pub(super) fn same_role_comparison(
    conn: &rusqlite::Connection,
    match_id: &str,
) -> Result<Option<SameRoleComparison>> {
    let current = conn
        .query_row(
            r#"
            SELECT season, role_type, kill, death, hero_damage, cure
            FROM match_players
            WHERE match_id=? AND is_current_player=1 AND TRIM(COALESCE(role_type, '')) <> ''
            LIMIT 1
            "#,
            params![match_id],
            |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, String>(1)?,
                    ComparisonMetrics {
                        kills: Some(row.get::<_, i64>(2)? as f64),
                        deaths: Some(row.get::<_, i64>(3)? as f64),
                        damage: Some(row.get::<_, i64>(4)? as f64),
                        healing: Some(row.get::<_, i64>(5)? as f64),
                    },
                ))
            },
        )
        .optional()?;
    let Some((season, raw_role, current)) = current else {
        return Ok(None);
    };
    let average = conn.query_row(
        r#"
        SELECT AVG(kill), AVG(death), AVG(hero_damage), AVG(cure)
        FROM match_players
        WHERE season=? AND is_current_player=1 AND role_type=?
        "#,
        params![season, raw_role],
        |row| {
            Ok(ComparisonMetrics {
                kills: row.get(0)?,
                deaths: row.get(1)?,
                damage: row.get(2)?,
                healing: row.get(3)?,
            })
        },
    )?;
    Ok(Some(SameRoleComparison {
        role_label: role_label(&raw_role).map(str::to_string),
        raw_role: Some(raw_role),
        current,
        average,
    }))
}

pub(super) fn role_label(value: &str) -> Option<&'static str> {
    match value {
        "tank" => Some("坦克"),
        "dps" => Some("输出"),
        "healer" => Some("支援"),
        _ => None,
    }
}

pub(super) fn append_filters(
    sql: &mut String,
    owned: &mut Vec<Box<dyn ToSql>>,
    filters: &MatchFilters,
) {
    if let Some(season) = filters.season {
        sql.push_str(" AND m.season = ?");
        owned.push(Box::new(season));
    }
    if let Some(hero) = non_empty(&filters.hero) {
        sql.push_str(" AND m.hero_guid = ?");
        owned.push(Box::new(hero));
    }
    if let Some(map) = non_empty(&filters.map) {
        sql.push_str(" AND m.map_guid = ?");
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
    if let Some(primary_tag) =
        non_empty(&filters.primary_tag).filter(|tag| filterable_primary_tag(tag))
    {
        sql.push_str(" AND e.primary_tag = ?");
        owned.push(Box::new(primary_tag));
    }
    if let Some(player_name) = non_empty(&filters.player_name) {
        sql.push_str(
            " AND (EXISTS (SELECT 1 FROM match_players mp WHERE mp.match_id = m.match_id AND mp.player_name = ?) \
             OR EXISTS (SELECT 1 FROM arena_match_players amp WHERE amp.match_id = m.match_id AND amp.player_name = ?))",
        );
        owned.push(Box::new(player_name.clone()));
        owned.push(Box::new(player_name));
    }
    if let Some(from) = non_empty(&filters.date_from).and_then(|date| begin_bound(&date, false)) {
        sql.push_str(" AND m.begin_ts >= ?");
        owned.push(Box::new(from));
    }
    if let Some(to) = non_empty(&filters.date_to).and_then(|date| begin_bound(&date, true)) {
        sql.push_str(" AND m.begin_ts <= ?");
        owned.push(Box::new(to));
    }
    if filters.favorite_only == Some(true) {
        sql.push_str(" AND f.match_id IS NOT NULL");
    }
}

pub(super) fn filterable_primary_tag(value: &str) -> bool {
    matches!(value, "带飞局" | "尽力局" | "躺赢局" | "背锅局")
}
