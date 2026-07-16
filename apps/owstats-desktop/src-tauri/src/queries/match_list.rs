use super::*;

pub(super) fn list_matches_impl(
    db: &OwDatabase,
    filters: MatchFilters,
) -> Result<Vec<MatchSummary>> {
    db.initialize()?;
    ensure_arena_player_index(db, filters.season)?;
    let conn = db.connect()?;
    let mut sql = String::from(
        r#"
        SELECT
            m.match_id, m.season, m.bucket, m.game_mode, m.result, m.map_guid, COALESCE(m.map_name, ''),
            m.hero_guid, COALESCE(m.hero_name, ''), m.begin_ts, m.team_score, m.opponent_score,
            m.kills, m.assists, m.deaths, m.damage, m.healing, m.mitigation, m.duration_sec,
            e.grade, e.primary_tag, e.evidence_tags_json, e.confidence, m.match_kind, d.raw_detail_json,
            CASE WHEN f.match_id IS NULL THEN 0 ELSE 1 END
        FROM matches m
        LEFT JOIN match_evaluations e ON e.match_id = m.match_id
        LEFT JOIN match_details d ON d.match_id = m.match_id
        LEFT JOIN match_favorites f ON f.match_id = m.match_id
        WHERE (m.bucket = 'preset' OR m.match_kind = 'fight' OR m.game_mode IN ('LeisureFight', 'SportFight'))
        "#,
    );
    let mut owned: Vec<Box<dyn ToSql>> = Vec::new();
    append_filters(&mut sql, &mut owned, &filters);
    sql.push_str(" ORDER BY m.begin_ts DESC, m.match_id");

    let map_names = map_lookup(&conn)?;
    let params: Vec<&dyn ToSql> = owned
        .iter()
        .map(|item| item.as_ref() as &dyn ToSql)
        .collect();
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(params.as_slice(), |row| {
        let game_mode: String = row.get(3)?;
        let result: String = row.get(4)?;
        let begin_ts: i64 = row.get(9)?;
        let map_guid: String = row.get(5)?;
        let hero_guid: String = row.get(7)?;
        let map_name: String = row.get(6)?;
        let hero_name: String = row.get(8)?;
        let match_id: String = row.get(0)?;
        let match_kind: String = row.get(23)?;
        let raw_detail_json: Option<String> = row.get(24)?;
        let fight_projection = if match_kind == "fight" {
            fight_list_projection(raw_detail_json.as_deref())
        } else {
            None
        };
        let map_preview_guid = if match_kind == "fight" {
            last_round_map_guid(raw_detail_json.as_deref())
        } else {
            None
        };
        let display_map_name = map_preview_guid
            .as_deref()
            .and_then(|guid| map_names.get(guid).cloned())
            .or_else(|| {
                if match_kind == "fight" {
                    manual_arena_map_name(&map_guid).map(str::to_string)
                } else {
                    None
                }
            })
            .unwrap_or_else(|| resolved_map_name(&map_names, &map_guid, &map_name));
        Ok(MatchSummary {
            match_id: match_id.clone(),
            season: row.get(1)?,
            bucket: row.get(2)?,
            match_kind: match_kind.clone(),
            mode_label: mode_label(&game_mode).to_string(),
            game_mode,
            result_label: result_label(&result).to_string(),
            result,
            map_guid: map_guid.clone(),
            map_preview_guid,
            map_name: display_map_name,
            hero_guid: hero_guid.clone(),
            hero_name: fallback_name(hero_name, &hero_guid, "未知英雄"),
            begin_ts,
            begin_label: format_begin_ts(begin_ts),
            team_score: fight_projection
                .as_ref()
                .and_then(|projection| projection.team_score)
                .unwrap_or(row.get(10)?),
            opponent_score: fight_projection
                .as_ref()
                .and_then(|projection| projection.opponent_score)
                .unwrap_or(row.get(11)?),
            kills: fight_projection
                .as_ref()
                .and_then(|projection| projection.kills)
                .unwrap_or(row.get(12)?),
            assists: fight_projection
                .as_ref()
                .and_then(|projection| projection.assists)
                .unwrap_or(row.get(13)?),
            deaths: fight_projection
                .as_ref()
                .and_then(|projection| projection.deaths)
                .unwrap_or(row.get(14)?),
            damage: fight_projection
                .as_ref()
                .and_then(|projection| projection.damage)
                .unwrap_or(row.get(15)?),
            healing: fight_projection
                .as_ref()
                .and_then(|projection| projection.healing)
                .unwrap_or(row.get(16)?),
            mitigation: fight_projection
                .as_ref()
                .and_then(|projection| projection.mitigation)
                .unwrap_or(row.get(17)?),
            duration_sec: fight_projection
                .as_ref()
                .and_then(|projection| projection.duration_sec)
                .unwrap_or(row.get(18)?),
            is_favorite: row.get(25)?,
            evaluation: evaluation_from_row(row, 19)?,
            team_awards: Vec::new(),
        })
    })?;
    let mut matches = rows.collect::<rusqlite::Result<Vec<_>>>()?;
    drop(stmt);
    for item in &mut matches {
        item.team_awards = team_awards_from_match_players(&conn, &item.match_id)?;
    }
    Ok(matches)
}
