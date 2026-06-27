use owstats_desktop_lib::{
    get_friend_analytics, get_match_detail, import_payload_value, list_filter_options,
    list_matches, redact_sensitive_json, sync_import_parts, FriendAnalyticsFilters, MatchFilters,
    OwDatabase,
};
use serde_json::{json, Value};

fn import_synthetic_season(db: &OwDatabase) {
    let temp = tempfile::tempdir().unwrap();
    let data_dir = temp.path().join("data");
    std::fs::create_dir_all(&data_dir).unwrap();
    std::fs::write(data_dir.join("ow_config_cache.json"), lookup_payload()).unwrap();
    import_payload_value(db, &season_payload(), temp.path()).unwrap();
}

fn lookup_payload() -> String {
    serde_json::to_string(&json!({
        "heroes": {
            "hero-current": {"name": "安娜"},
            "hero-friend": {"name": "莱因哈特"},
            "hero-enemy": {"name": "源氏"}
        },
        "maps": {
            "map-a": {"name": "漓江塔"},
            "map-b": {"name": "国王大道"}
        }
    }))
    .unwrap()
}

fn season_payload() -> Value {
    json!({
        "season": 22,
        "profile_card": {
            "data": {
                "bnetId": 1000,
                "name": "Current#1000",
                "customerToken": "customer-secret"
            }
        },
        "matches": {
            "entries": [
                match_entry("match-win", 1, "LeisurePreset", "map-a", "hero-current", 1700000000000_i64),
                match_entry("match-loss", -1, "SportPreset", "map-b", "hero-current", 1700003600000_i64),
                match_entry("match-stadium", 1, "StadiumFight", "map-a", "hero-current", 1700007200000_i64)
            ],
            "details": [
                match_detail("match-win", "normal", "map-a", 600),
                match_detail("match-loss", "normal", "map-b", 650),
                match_detail("match-stadium", "fight", "map-a", 500)
            ],
            "detail_errors": [
                {
                    "match_id": "bad-detail",
                    "error_type": "DetailFetchError",
                    "message": "request failed token=entry-secret customerToken=detail-secret"
                }
            ]
        }
    })
}

fn match_entry(
    match_id: &str,
    match_ret: i64,
    game_mode: &str,
    map_guid: &str,
    hero_guid: &str,
    begin_ts: i64,
) -> Value {
    json!({
        "matchId": match_id,
        "matchRet": match_ret,
        "mapGuid": map_guid,
        "heroGuid": hero_guid,
        "beginTs": begin_ts,
        "gameMode": game_mode,
        "_dashenSeason": 22,
        "teamScore": 2,
        "opponentScore": if match_ret == 1 { 1 } else { 2 },
        "kill": if match_ret == 1 { 12 } else { 4 },
        "assist": 8,
        "death": if match_ret == 1 { 3 } else { 9 },
        "heroDamage": if match_ret == 1 { 9000 } else { 3000 },
        "cure": if match_ret == 1 { 8000 } else { 2000 },
        "resistDamage": 500,
        "totalWorth": 100,
        "token": "entry-secret"
    })
}

fn match_detail(match_id: &str, match_kind: &str, map_guid: &str, duration: i64) -> Value {
    json!({
        "match_id": match_id,
        "match_kind": match_kind,
        "payload": {
            "data": {
                "matchRet": if match_id == "match-loss" { -1 } else { 1 },
                "mapGuid": map_guid,
                "gameTimeSec": duration,
                "teammateList": [
                    {
                        "bnetId": 1000,
                        "name": "Current#1000",
                        "heroGuid": "hero-current",
                        "roleType": "healer",
                        "friendBnetIds": [2000],
                        "kill": 5,
                        "assist": 7,
                        "death": 2,
                        "heroDamage": 3000,
                        "cure": 7000,
                        "resistDamage": 100,
                        "healingTaken": 900,
                        "damageTaken": 1200,
                        "finalHit": 1,
                        "targetCompetingTime": 30,
                        "customerToken": "detail-secret"
                    },
                    {
                        "bnetId": 2000,
                        "name": "Friend#2000",
                        "heroGuid": "hero-friend",
                        "roleType": "tank",
                        "kill": 4,
                        "assist": 3,
                        "death": 3,
                        "heroDamage": 2500,
                        "cure": 0,
                        "resistDamage": 9000,
                        "healingTaken": 100,
                        "damageTaken": 10000
                    }
                ],
                "enemyList": [
                    {
                        "bnetId": 3000,
                        "name": "Enemy#3000",
                        "heroGuid": "hero-enemy",
                        "roleType": "dps",
                        "kill": 6,
                        "death": 4,
                        "heroDamage": 5000
                    }
                ]
            }
        }
    })
}

#[test]
fn imports_synthetic_season_and_counts_preset_matches() {
    let temp = tempfile::tempdir().unwrap();
    let db = OwDatabase::new(temp.path().join("owstats.db"));
    import_synthetic_season(&db);

    let matches = list_matches(&db, MatchFilters::default()).unwrap();
    assert_eq!(matches.len(), 2);
    assert!(matches.iter().all(|item| item.bucket == "preset"));
}

#[test]
fn filters_matches_by_season_result_mode_hero_map_and_date_range() {
    let temp = tempfile::tempdir().unwrap();
    let db = OwDatabase::new(temp.path().join("owstats.db"));
    import_synthetic_season(&db);

    let first = list_matches(&db, MatchFilters::default())
        .unwrap()
        .into_iter()
        .find(|item| item.result == "win")
        .unwrap();
    let filtered = list_matches(
        &db,
        MatchFilters {
            season: Some(first.season),
            hero: Some(first.hero_guid.clone()),
            map: Some(first.map_guid.clone()),
            result: Some(first.result.clone()),
            mode: Some(first.game_mode.clone()),
            date_from: Some(first.begin_label[0..10].to_string()),
            date_to: Some(first.begin_label[0..10].to_string()),
        },
    )
    .unwrap();

    assert_eq!(filtered.len(), 1);
    assert_eq!(filtered[0].match_id, first.match_id);
}

#[test]
fn detail_exposes_teammates_enemies_and_redacted_raw_json() {
    let temp = tempfile::tempdir().unwrap();
    let db = OwDatabase::new(temp.path().join("owstats.db"));
    import_synthetic_season(&db);

    let detail = get_match_detail(&db, "match-win").unwrap();
    let raw = serde_json::to_string(&detail.raw_json).unwrap();

    assert_eq!(detail.match_id, "match-win");
    assert_eq!(detail.teammates.len(), 2);
    assert_eq!(detail.enemies.len(), 1);
    assert_eq!(detail.teammates[0].hero_name, "安娜");
    assert!(!raw.contains("detail-secret"));
    assert!(!raw.contains("entry-secret"));
    assert!(raw.contains("***REDACTED***"));
}

#[test]
fn filter_options_are_populated_from_imported_matches() {
    let temp = tempfile::tempdir().unwrap();
    let db = OwDatabase::new(temp.path().join("owstats.db"));
    import_synthetic_season(&db);

    let options = list_filter_options(&db).unwrap();

    assert_eq!(options.seasons, vec![22]);
    assert!(options
        .heroes
        .iter()
        .any(|option| option.id == "hero-current"));
    assert!(options.maps.iter().any(|option| option.id == "map-a"));
    assert!(options.results.contains(&"win".to_string()));
    assert!(options.modes.contains(&"LeisurePreset".to_string()));
}

#[test]
fn imports_negative_match_ret_as_loss() {
    let temp = tempfile::tempdir().unwrap();
    let db = OwDatabase::new(temp.path().join("owstats.db"));
    import_synthetic_season(&db);

    let matches = list_matches(
        &db,
        MatchFilters {
            result: Some("loss".to_string()),
            ..MatchFilters::default()
        },
    )
    .unwrap();

    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0].result, "loss");
    assert_eq!(matches[0].result_label, "失败");
}

#[test]
fn redaction_removes_token_values_recursively() {
    let redacted = redact_sensitive_json(&json!({
        "token": "secret-a",
        "payload": {
            "customerToken": "secret-b",
            "players": [{"nestedToken": "secret-c", "name": "visible"}]
        }
    }));
    let text = serde_json::to_string(&redacted).unwrap();

    assert!(!text.contains("secret-a"));
    assert!(!text.contains("secret-b"));
    assert!(!text.contains("secret-c"));
    assert!(text.contains("visible"));
}

#[test]
fn import_rebuilds_match_players_for_preset_matches_only() {
    let temp = tempfile::tempdir().unwrap();
    let db = OwDatabase::new(temp.path().join("owstats.db"));
    import_synthetic_season(&db);
    let conn = db.connect().unwrap();

    let player_rows: i64 = conn
        .query_row("SELECT COUNT(*) FROM match_players", [], |row| row.get(0))
        .unwrap();
    let stadium_rows: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM match_players WHERE match_id IN (SELECT match_id FROM matches WHERE bucket='stadium')",
            [],
            |row| row.get(0),
        )
        .unwrap();
    let friend_rows: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM match_players WHERE side='teammate' AND is_friend=1",
            [],
            |row| row.get(0),
        )
        .unwrap();
    let projection_raw: String = conn
        .query_row(
            "SELECT raw_json FROM match_players WHERE raw_json LIKE '%customerToken%' LIMIT 1",
            [],
            |row| row.get(0),
        )
        .unwrap_or_default();

    assert_eq!(player_rows, 6);
    assert_eq!(stadium_rows, 0);
    assert_eq!(friend_rows, 2);
    assert!(!projection_raw.contains("customerToken"));
}

#[test]
fn friend_analytics_backfills_empty_match_players_from_existing_details() {
    let temp = tempfile::tempdir().unwrap();
    let db = OwDatabase::new(temp.path().join("owstats.db"));
    import_synthetic_season(&db);
    let conn = db.connect().unwrap();
    conn.execute("DELETE FROM match_players", []).unwrap();
    drop(conn);

    let analytics = get_friend_analytics(&db, FriendAnalyticsFilters::default()).unwrap();

    assert!(analytics.summary.friend_rows > 0);
    let conn = db.connect().unwrap();
    let player_rows: i64 = conn
        .query_row("SELECT COUNT(*) FROM match_players", [], |row| row.get(0))
        .unwrap();
    assert!(player_rows > 0);
}

fn seed_friend_row(
    db: &OwDatabase,
    match_id: &str,
    bnet_id: &str,
    name: &str,
    begin_ts: i64,
    kills: i64,
    deaths: i64,
    resist_damage: i64,
    healing_taken: i64,
    damage_taken: i64,
) {
    let conn = db.connect().unwrap();
    conn.execute(
        r#"
        INSERT OR IGNORE INTO matches (
            match_id, season, bucket, game_mode, match_kind, result, map_guid, map_name,
            hero_guid, hero_name, begin_ts, team_score, opponent_score, kills, assists,
            deaths, damage, healing, mitigation, total_worth, duration_sec, raw_entry_json, updated_at
        )
        VALUES (?1, 22, 'preset', 'SportPreset', 'normal', 'win', 'map-a', '漓江塔',
            'hero-current', '安娜', ?2, 2, 1, 1, 1, 1, 1000, 1000, 100, 0, 600, '{}',
            '2026-06-27T00:00:00Z')
        "#,
        rusqlite::params![match_id, begin_ts],
    )
    .unwrap();
    conn.execute(
        r#"
        INSERT INTO match_players (
            match_id, season, side, bnet_id, player_name, hero_guid, hero_name,
            is_current_player, is_friend, kill, assist, death, hero_damage, cure,
            resist_damage, healing_taken, damage_taken, final_hit, target_competing_time,
            raw_json, updated_at
        )
        VALUES (?1, 22, 'teammate', ?2, ?3, 'hero-friend', '莱因哈特',
            0, 1, ?4, 0, ?5, 1000, 0, ?6, ?7, ?8, 0, 0, '{}',
            '2026-06-27T00:00:00Z')
        "#,
        rusqlite::params![
            match_id,
            bnet_id,
            name,
            kills,
            deaths,
            resist_damage,
            healing_taken,
            damage_taken
        ],
    )
    .unwrap();
}

#[test]
fn friend_analytics_uses_block_for_front_pressure_and_adds_healing_conversion_board() {
    let temp = tempfile::tempdir().unwrap();
    let db = OwDatabase::new(temp.path().join("owstats.db"));
    db.initialize().unwrap();

    let friends = [
        ("front", "格挡前排#1", 2, 3, 9000, 100, 10000),
        ("care", "重点照顾#1", 1, 8, 100, 9000, 3000),
        ("low", "省资源#1", 2, 1, 100, 100, 2000),
        ("convert", "转化击杀#1", 8, 2, 200, 5000, 2000),
        ("middle", "普通好友#1", 2, 3, 200, 1000, 2500),
    ];
    for (friend_index, (bnet_id, name, kills, deaths, mitigation, healing_taken, damage_taken)) in
        friends.iter().enumerate()
    {
        for match_index in 0..5 {
            seed_friend_row(
                &db,
                &format!("m-{friend_index}-{match_index}"),
                bnet_id,
                name,
                (friend_index * 10 + match_index) as i64,
                *kills,
                *deaths,
                *mitigation,
                *healing_taken,
                *damage_taken,
            );
        }
    }

    let analytics = get_friend_analytics(&db, FriendAnalyticsFilters::default()).unwrap();
    let keys = analytics
        .boards
        .iter()
        .map(|board| board.key.as_str())
        .collect::<Vec<_>>();
    assert_eq!(
        keys,
        vec![
            "front_pressure",
            "low_resource_partner",
            "priority_care",
            "healing_conversion"
        ]
    );
    let front = analytics
        .boards
        .iter()
        .find(|board| board.key == "front_pressure")
        .unwrap();
    assert_eq!(front.friends[0].bnet_id, "front");
    assert!(front.description.contains("格挡"));
    let conversion = analytics
        .boards
        .iter()
        .find(|board| board.key == "healing_conversion")
        .unwrap();
    assert_eq!(conversion.friends[0].bnet_id, "convert");
    assert!(conversion.label.contains("转化"));
}

#[test]
fn sync_import_parts_is_idempotent_and_sanitizes_detail_errors() {
    let temp = tempfile::tempdir().unwrap();
    let db = OwDatabase::new(temp.path().join("owstats.db"));
    let project_root = temp.path();
    let cached_player = json!({
        "input_bnet": "Tester#1000",
        "resolved_bnet": "Tester#1000",
        "profile_card": {
            "data": {
                "bnetId": 1000,
                "name": "Tester#1000",
                "customerToken": "customer-secret"
            }
        }
    });
    let entries = vec![json!({
        "matchId": "sync-match-1",
        "matchRet": 1,
        "mapGuid": "map-a",
        "heroGuid": "hero-current",
        "beginTs": 1700000000000_i64,
        "gameMode": "LeisurePreset",
        "_dashenSeason": 22
    })];
    let details = vec![json!({
        "match_id": "sync-match-1",
        "match_kind": "normal",
        "payload": {
            "data": {
                "matchRet": 1,
                "mapGuid": "map-a",
                "gameTimeSec": 600,
                "teammateList": [
                    {"bnetId": 1000, "name": "Tester#1000", "heroGuid": "hero-current", "friendBnetIds": [2000], "kill": 1},
                    {"bnetId": 2000, "name": "Friend#2000", "heroGuid": "hero-friend", "kill": 2, "healingTaken": 100, "damageTaken": 200}
                ],
                "enemyList": [
                    {"bnetId": 3000, "name": "Enemy#3000", "heroGuid": "hero-enemy", "kill": 3}
                ]
            }
        }
    })];
    let detail_errors = vec![json!({
        "match_id": "sync-match-2",
        "error_type": "DetailFetchError",
        "message": "request failed token=token-secret customerToken=customer-secret cookie=session-secret"
    })];

    let first = sync_import_parts(
        &db,
        project_root,
        22,
        &cached_player,
        entries.clone(),
        details.clone(),
        detail_errors.clone(),
        1,
    )
    .unwrap();
    let second = sync_import_parts(
        &db,
        project_root,
        22,
        &cached_player,
        entries,
        details,
        detail_errors,
        1,
    )
    .unwrap();
    let conn = db.connect().unwrap();
    let matches_count: i64 = conn
        .query_row("SELECT COUNT(*) FROM matches", [], |row| row.get(0))
        .unwrap();
    let details_count: i64 = conn
        .query_row("SELECT COUNT(*) FROM match_details", [], |row| row.get(0))
        .unwrap();
    let player_rows: i64 = conn
        .query_row("SELECT COUNT(*) FROM match_players", [], |row| row.get(0))
        .unwrap();
    let errors_text: String = conn
        .query_row(
            "SELECT raw_json FROM sync_errors ORDER BY id DESC LIMIT 1",
            [],
            |row| row.get(0),
        )
        .unwrap();

    assert_eq!(first.failed_details, 1);
    assert_eq!(second.failed_details, 1);
    assert_eq!(matches_count, 1);
    assert_eq!(details_count, 1);
    assert_eq!(player_rows, 3);
    assert!(!errors_text.contains("token-secret"));
    assert!(!errors_text.contains("customer-secret"));
    assert!(!errors_text.contains("session-secret"));
    assert!(!first.latest_errors.join(" ").contains("token-secret"));
}
