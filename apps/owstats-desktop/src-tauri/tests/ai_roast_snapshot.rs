use owstats_desktop_lib::ai_roast::{
    knowledge::KnowledgeContext,
    snapshot::{
        build_roast_snapshot, ArenaContext, CurrentPlayerBaseline, DerivedMetrics, PlayerStats,
        RoastMatchContext, RoastMatchKind, RoastPlayer, RoastScoreboard, RoastSnapshot,
        RoastStreak, INPUT_SCHEMA_VERSION,
    },
    task::TaskKnowledgeContext,
};
use owstats_desktop_lib::{ArenaDetailSlice, ArenaMatchDetail, MatchDetailView, PlayerRow};
use rusqlite::Connection;
use serde_json::json;

fn synthetic_snapshot() -> RoastSnapshot {
    let current = RoastPlayer {
        subject: "我方安娜".to_string(),
        is_current_player: true,
        hero: "安娜".to_string(),
        role: Some("支援".to_string()),
        stats: PlayerStats {
            kills: Some(7),
            assists: Some(29),
            deaths: Some(4),
            damage: Some(5230),
            healing: Some(17082),
            mitigation: Some(0),
            final_hits: Some(2),
            objective_time_seconds: Some(61),
            healing_received: Some(5834),
            damage_taken: Some(9241),
        },
        derived: DerivedMetrics::default(),
    };

    RoastSnapshot {
        schema_version: INPUT_SCHEMA_VERSION,
        match_kind: RoastMatchKind::Standard,
        match_context: RoastMatchContext::default(),
        current_player: current.subject.clone(),
        scoreboard: RoastScoreboard {
            team: vec![current],
            opponent: Vec::new(),
        },
        role_matchups: Vec::new(),
        current_player_baseline: Some(CurrentPlayerBaseline::default()),
        streak: Some(RoastStreak::default()),
        game_knowledge: KnowledgeContext::default(),
        task_knowledge: TaskKnowledgeContext::default(),
        arena: None::<ArenaContext>,
    }
}

#[test]
fn v3_snapshot_serializes_complete_scoreboard_and_task_shape() {
    let value = serde_json::to_value(synthetic_snapshot()).unwrap();
    assert_eq!(value["schemaVersion"], 3);
    assert_eq!(value["matchKind"], "standard");
    assert!(value["scoreboard"]["team"].is_array());
    assert!(value["scoreboard"]["opponent"].is_array());
    assert!(value["taskKnowledge"]["assignments"].is_object());
    assert!(value.get("selectedFacts").is_none());
    assert!(value.get("anonymousRelation").is_none());
}

fn player(side: &str, id: &str, hero: &str, damage: i64, deaths: i64) -> PlayerRow {
    PlayerRow {
        side: side.to_string(),
        name: format!("SyntheticPlayer#{id}"),
        bnet_id: id.to_string(),
        rank_label: Some("隐私段位".to_string()),
        rank_key: Some("private-rank".to_string()),
        hero_guid: format!("hero-{id}"),
        hero_name: hero.to_string(),
        kills: 10,
        assists: 5,
        deaths,
        damage,
        healing: if hero == "安娜" { 12_000 } else { 0 },
        mitigation: 0,
        final_hits: 3,
        target_time_sec: 30,
        healing_taken: 2_000,
        damage_taken: 6_000,
        worth: None,
        end_worth: None,
        worth_max: false,
        mod_guids: vec!["private-mod".to_string()],
        trait_guids: vec!["private-trait".to_string()],
        has_source_max_flags: false,
        kill_max: false,
        hero_damage_max: false,
        cure_max: false,
        resist_damage_max: false,
        perks: Vec::new(),
        hero_stats: Vec::new(),
        party_group: None,
    }
}

fn ordinary_detail() -> MatchDetailView {
    MatchDetailView {
        match_id: "synthetic-match".to_string(),
        game_mode: Some("SportPreset".to_string()),
        mode_label: Some("竞技比赛".to_string()),
        result: Some("loss".to_string()),
        result_label: Some("失败".to_string()),
        map_guid: Some("private-map-guid".to_string()),
        map_name: Some("国王大道".to_string()),
        hero_guid: Some("private-current-guid".to_string()),
        hero_name: Some("安娜".to_string()),
        begin_ts: Some(1_000),
        begin_label: Some("private-time".to_string()),
        team_score: Some(2),
        opponent_score: Some(3),
        duration_sec: Some(600),
        teammates: vec![
            player("teammate", "synthetic-account", "安娜", 5_000, 4),
            player("teammate", "soldier", "士兵：76", 10_000, 4),
            player("teammate", "winston", "温斯顿", 10_000, 5),
        ],
        enemies: vec![
            player("enemy", "pharah", "法老之鹰", 12_000, 3),
            player("enemy", "ashe", "艾什", 8_000, 5),
            player("enemy", "mercy", "天使", 2_000, 4),
        ],
        arena_detail: None,
        evaluation: None,
        team_awards: Vec::new(),
        same_role_comparison: None,
        favorite: None,
        ai_roast: None,
        raw_json: json!({"private": "raw-json"}),
    }
}

fn snapshot_database() -> Connection {
    let conn = Connection::open_in_memory().unwrap();
    conn.execute_batch(
        "CREATE TABLE matches(match_id TEXT PRIMARY KEY, season INTEGER, result TEXT, begin_ts INTEGER, match_kind TEXT, game_mode TEXT);
         CREATE TABLE match_players(match_id TEXT, side TEXT, bnet_id TEXT, hero_guid TEXT, role_type TEXT, is_current_player INTEGER, kill INTEGER, assist INTEGER, death INTEGER, hero_damage INTEGER, cure INTEGER, resist_damage INTEGER, healing_taken INTEGER, damage_taken INTEGER, final_hit INTEGER, target_competing_time INTEGER);
         CREATE TABLE arena_match_players(match_id TEXT, side TEXT, bnet_id TEXT, player_name TEXT);
         INSERT INTO matches VALUES('synthetic-match',23,'loss',1000,'normal','SportPreset');
         INSERT INTO match_players VALUES('synthetic-match','teammate','synthetic-account','hero-current','support',1,10,5,4,5000,12000,0,2000,6000,3,30);
         INSERT INTO match_players VALUES('synthetic-match','teammate','soldier','hero-soldier','damage',0,10,5,4,10000,0,0,2000,6000,3,30);
         INSERT INTO match_players VALUES('synthetic-match','teammate','winston','hero-winston','tank',0,10,5,5,10000,0,0,2000,6000,3,30);
         INSERT INTO match_players VALUES('synthetic-match','enemy','pharah','hero-pharah','damage',0,10,5,3,12000,0,0,2000,6000,3,30);
         INSERT INTO match_players VALUES('synthetic-match','enemy','ashe','hero-ashe','damage',0,10,5,5,8000,0,0,2000,6000,3,30);
         INSERT INTO match_players VALUES('synthetic-match','enemy','mercy','hero-mercy','support',0,10,5,4,2000,0,0,2000,6000,3,30);",
    )
    .unwrap();
    conn.execute_batch(
        "ALTER TABLE match_players ADD COLUMN player_name TEXT NOT NULL DEFAULT '';
         UPDATE match_players SET player_name='synthetic-current' WHERE bnet_id='synthetic-account';
         UPDATE match_players SET player_name='synthetic-' || bnet_id WHERE bnet_id<>'synthetic-account';
         INSERT INTO arena_match_players VALUES('synthetic-match','teammate','synthetic-account','arena-current');",
    )
    .unwrap();
    conn
}

#[test]
fn ordinary_snapshot_uses_complete_scoreboard_and_natural_subjects() {
    let conn = snapshot_database();
    let snapshot = build_roast_snapshot(
        &conn,
        "synthetic-match",
        &ordinary_detail(),
        KnowledgeContext::default(),
    )
    .unwrap();

    assert_eq!(snapshot.scoreboard.team.len(), 3);
    assert_eq!(snapshot.scoreboard.opponent.len(), 3);
    assert_eq!(snapshot.current_player, "我方安娜");
    assert!(snapshot
        .scoreboard
        .team
        .iter()
        .any(|row| row.subject == "我方士兵：76"));
    assert!(snapshot
        .scoreboard
        .opponent
        .iter()
        .any(|row| row.subject == "敌方法老之鹰"));
    let json = serde_json::to_string(&snapshot).unwrap();
    assert!(!json.contains("SyntheticPlayer#"));
    assert!(!json.contains("synthetic-account"));
    assert!(!json.contains("private-rank"));
    assert!(!json.contains("raw-json"));
}

#[test]
fn ordinary_snapshot_numbers_duplicate_heroes_stably() {
    let conn = snapshot_database();
    let mut detail = ordinary_detail();
    detail.teammates[1].hero_name = "安娜".to_string();
    let snapshot = build_roast_snapshot(
        &conn,
        "synthetic-match",
        &detail,
        KnowledgeContext::default(),
    )
    .unwrap();
    assert_eq!(snapshot.scoreboard.team[0].subject, "我方安娜一号");
    assert_eq!(snapshot.scoreboard.team[1].subject, "我方安娜二号");
}

#[test]
fn derived_metrics_use_competition_ranks_and_safe_denominators() {
    let conn = snapshot_database();
    let snapshot = build_roast_snapshot(
        &conn,
        "synthetic-match",
        &ordinary_detail(),
        KnowledgeContext::default(),
    )
    .unwrap();
    let soldier = snapshot
        .scoreboard
        .team
        .iter()
        .find(|row| row.hero == "士兵：76")
        .unwrap();
    assert_eq!(soldier.derived.team_damage_rank, Some(1));
    assert_eq!(soldier.derived.match_damage_rank, Some(2));
    assert_eq!(soldier.derived.team_damage_share, Some(0.4));
    assert_eq!(soldier.derived.damage_per_death, Some(2_500.0));
    assert_eq!(soldier.derived.kill_participation_rate, Some(0.5));

    let mut detail = ordinary_detail();
    for row in &mut detail.teammates {
        row.damage = 0;
        row.deaths = 0;
    }
    let snapshot = build_roast_snapshot(
        &conn,
        "synthetic-match",
        &detail,
        KnowledgeContext::default(),
    )
    .unwrap();
    assert!(snapshot
        .scoreboard
        .team
        .iter()
        .all(|row| row.derived.team_damage_share.is_none()));
    assert!(snapshot
        .scoreboard
        .team
        .iter()
        .all(|row| row.derived.damage_per_death.is_none()));
}

#[test]
fn role_matchup_aggregates_only_reliably_mapped_roles() {
    let conn = snapshot_database();
    let snapshot = build_roast_snapshot(
        &conn,
        "synthetic-match",
        &ordinary_detail(),
        KnowledgeContext::default(),
    )
    .unwrap();
    let damage = snapshot
        .role_matchups
        .iter()
        .find(|item| item.role == "damage")
        .unwrap();
    assert_eq!(damage.team_subjects, vec!["我方士兵：76"]);
    assert_eq!(damage.opponent_subjects, vec!["敌方法老之鹰", "敌方艾什"]);
    assert_eq!(damage.team_totals.damage, Some(10_000));
    assert_eq!(damage.opponent_totals.damage, Some(20_000));
    assert_eq!(damage.deltas.damage, Some(-10_000));
}

#[test]
fn ordinary_baseline_is_season_scoped_and_draw_breaks_streak() {
    let conn = snapshot_database();
    conn.execute_batch(
        "INSERT INTO matches VALUES('season-22',22,'win',700,'normal','SportPreset');
         INSERT INTO match_players VALUES('season-22','teammate','old','hero-old','support',1,100,0,1,50000,50000,0,0,0,0,0,'synthetic-old');
         INSERT INTO matches VALUES('same-season',23,'draw',900,'normal','SportPreset');
         INSERT INTO match_players VALUES('same-season','teammate','prior','hero-prior','support',1,6,3,2,3000,8000,0,0,0,0,0,'synthetic-prior');",
    ).unwrap();
    let snapshot = build_roast_snapshot(
        &conn,
        "synthetic-match",
        &ordinary_detail(),
        KnowledgeContext::default(),
    )
    .unwrap();
    let baseline = snapshot.current_player_baseline.unwrap();
    assert_eq!(baseline.averages.kills, Some(8));
    assert_eq!(baseline.averages.damage, Some(4_000));
    assert_eq!(snapshot.streak.unwrap().count_before_match, Some(0));
}

#[test]
fn ordinary_baseline_is_none_without_reliable_role_history() {
    let conn = snapshot_database();
    conn.execute(
        "UPDATE match_players SET role_type='' WHERE is_current_player=1",
        [],
    )
    .unwrap();
    let mut detail = ordinary_detail();
    detail.teammates[0].hero_guid = "unknown-role".to_string();
    let snapshot = build_roast_snapshot(
        &conn,
        "synthetic-match",
        &detail,
        KnowledgeContext::default(),
    )
    .unwrap();
    assert!(snapshot.current_player_baseline.is_none());
}

fn arena_slice(
    key: &str,
    result: &str,
    team_score: Option<i64>,
    opponent_score: Option<i64>,
    damage: i64,
) -> ArenaDetailSlice {
    let mut current = player("teammate", "synthetic-account", "安娜", damage, 2);
    current.kills = damage / 1_000;
    current.end_worth = Some(7_050);
    let mut enemy = player("enemy", "pharah", "法老之鹰", damage + 1_000, 3);
    enemy.end_worth = Some(7_120);
    ArenaDetailSlice {
        key: key.to_string(),
        label: key.to_string(),
        result: Some(result.to_string()),
        result_label: None,
        team_score,
        opponent_score,
        duration_sec: Some(90),
        begin_ts: None,
        begin_label: None,
        map_guid: None,
        map_name: Some("斗技场".to_string()),
        teammates: vec![current],
        enemies: vec![enemy],
    }
}

fn arena_detail() -> MatchDetailView {
    let mut detail = ordinary_detail();
    let summary = arena_slice("summary", "win", Some(2), Some(1), 9_000);
    let mut incomplete = arena_slice("round-2", "loss", Some(0), Some(2), 4_000);
    incomplete.enemies.clear();
    detail.arena_detail = Some(ArenaMatchDetail {
        summary,
        rounds: vec![
            arena_slice("round-1", "loss", Some(0), Some(1), 3_000),
            incomplete,
            arena_slice("round-3", "win", Some(2), Some(1), 9_000),
        ],
    });
    detail
}

#[test]
fn arena_snapshot_uses_last_complete_round_as_full_scoreboard() {
    let conn = snapshot_database();
    let snapshot = build_roast_snapshot(
        &conn,
        "synthetic-match",
        &arena_detail(),
        KnowledgeContext::default(),
    )
    .unwrap();
    assert_eq!(snapshot.match_kind, RoastMatchKind::Arena);
    assert_eq!(snapshot.scoreboard.team[0].stats.kills, Some(9));
    assert_eq!(snapshot.scoreboard.opponent.len(), 1);
    assert!(serde_json::to_value(&snapshot)
        .unwrap()
        .get("candidateFacts")
        .is_none());
}

#[test]
fn arena_snapshot_identifies_current_player_after_hero_switch_without_exposing_name() {
    let conn = snapshot_database();
    conn.execute(
        "UPDATE match_players SET is_current_player=0 WHERE match_id='synthetic-match'",
        [],
    )
    .unwrap();
    let mut detail = arena_detail();
    detail.hero_guid = Some(detail.teammates[0].hero_guid.clone());
    detail.teammates[0].name = "arena-current".to_string();
    let arena = detail.arena_detail.as_mut().unwrap();
    arena.summary.teammates[0].name = "arena-current".to_string();
    for round in &mut arena.rounds {
        round.teammates[0].name = "arena-current".to_string();
    }
    let selected = arena.rounds.last_mut().unwrap();
    selected.teammates[0].bnet_id.clear();
    selected.teammates[0].hero_guid = "hero-switched".to_string();
    selected.teammates[0].hero_name = "朱诺".to_string();
    selected.teammates[0].name = "arena-current".to_string();

    let snapshot = build_roast_snapshot(
        &conn,
        "synthetic-match",
        &detail,
        KnowledgeContext::default(),
    )
    .unwrap();

    assert_eq!(snapshot.current_player, "我方朱诺");
    assert!(snapshot.scoreboard.team[0].is_current_player);
    assert!(!serde_json::to_string(&snapshot)
        .unwrap()
        .contains("arena-current"));
}

#[test]
fn arena_snapshot_only_adds_round_results_and_final_worth() {
    let conn = snapshot_database();
    let snapshot = build_roast_snapshot(
        &conn,
        "synthetic-match",
        &arena_detail(),
        KnowledgeContext::default(),
    )
    .unwrap();
    let arena = snapshot.arena.unwrap();
    assert_eq!(arena.round_results.len(), 3);
    assert_eq!(arena.round_results[0].score_after, "0:1");
    assert_eq!(arena.final_worth["我方安娜"], 7_050);
    let json = serde_json::to_string(&arena).unwrap();
    for forbidden in ["modGuids", "traitGuids", "worthHistory", "roundScoreboard"] {
        assert!(!json.contains(forbidden));
    }
}
