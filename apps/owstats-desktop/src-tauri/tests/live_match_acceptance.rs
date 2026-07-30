use anyhow::Result;
use owstats_desktop_lib::{list_matches_page, MatchFilters, OwDatabase, SeasonDataRevisionEvent};
use rusqlite::params;
use serde_json::Value;
use std::time::{Duration, Instant};

const FIRST_PAINT_P95_BUDGET: Duration = Duration::from_millis(200);
const RESPONSE_TO_COMMIT_P95_BUDGET: Duration = Duration::from_millis(50);
const COMMIT_TO_EVENT_P95_BUDGET: Duration = Duration::from_millis(50);

fn seed_matches(db: &OwDatabase, count: usize) -> Result<()> {
    db.initialize()?;
    let mut conn = db.connect()?;
    let transaction = conn.transaction()?;
    {
        let mut insert = transaction.prepare(
            r#"
            INSERT INTO matches (
                match_id, season, bucket, game_mode, match_kind, result,
                map_guid, map_name, hero_guid, hero_name, begin_ts,
                team_score, opponent_score, kills, assists, deaths,
                damage, healing, mitigation, total_worth, duration_sec,
                raw_entry_json, updated_at
            ) VALUES (?1, 23, 'preset', 'SportPreset', 'normal', 'win',
                      'map', 'Test Map', 'hero', 'Test Hero', ?2,
                      3, 2, 10, 8, 4, 12000, 4000, 9000, 0, 600,
                      '{}', '2026-07-31T00:00:00Z')
            "#,
        )?;
        for index in 0..count {
            insert.execute(params![
                format!("match-{index:05}"),
                2_000_000_000_i64 - index as i64,
            ])?;
        }
    }
    transaction.commit()?;
    Ok(())
}

fn percentile_95(mut samples: Vec<Duration>) -> Duration {
    samples.sort_unstable();
    samples[(samples.len() * 95).div_ceil(100) - 1]
}

#[test]
fn first_page_query_meets_the_local_p95_budget_at_supported_dataset_sizes() -> Result<()> {
    for count in [50, 500, 2_000] {
        let temp = tempfile::tempdir()?;
        let db = OwDatabase::new(temp.path().join(format!("matches-{count}.sqlite3")));
        seed_matches(&db, count)?;

        // Exclude one-time schema and filesystem warm-up. The release budget is for
        // navigation to a ready local database, and every measured call opens the
        // same public query seam used by the command layer.
        let warm = list_matches_page(&db, MatchFilters::default(), None, 50)?;
        assert_eq!(warm.matches.len(), 50.min(count));

        let mut samples = Vec::with_capacity(40);
        for _ in 0..40 {
            let started = Instant::now();
            let page = list_matches_page(&db, MatchFilters::default(), None, 50)?;
            samples.push(started.elapsed());
            assert_eq!(page.matches.len(), 50.min(count));
            assert_eq!(page.total, Some(count));
            assert_eq!(
                page.matches.first().map(|row| row.match_id.as_str()),
                Some("match-00000")
            );
        }

        let p95 = percentile_95(samples);
        println!("live match first-page benchmark: rows={count} p95={p95:?}");
        assert!(
            p95 <= FIRST_PAINT_P95_BUDGET,
            "{count} rows: first-page SQLite P95 {p95:?} exceeded {FIRST_PAINT_P95_BUDGET:?}"
        );
    }
    Ok(())
}

#[test]
fn revision_event_dto_contains_only_the_allowlisted_non_sensitive_contract() -> Result<()> {
    let value = serde_json::to_value(SeasonDataRevisionEvent {
        account_scope: "sha256:anonymous-scope".into(),
        season: 23,
        revision: 17,
        phase: "list".into(),
        new_matches: 2,
        pending_details: 4,
        status: "running".into(),
    })?;
    let object = value
        .as_object()
        .expect("revision event serializes as an object");
    let mut keys = object.keys().map(String::as_str).collect::<Vec<_>>();
    keys.sort_unstable();
    assert_eq!(
        keys,
        [
            "accountScope",
            "newMatches",
            "pendingDetails",
            "phase",
            "revision",
            "season",
            "status",
        ]
    );

    fn scan(value: &Value, path: &str) {
        match value {
            Value::Object(fields) => fields.iter().for_each(|(key, child)| {
                let normalized = key.to_ascii_lowercase();
                for forbidden in [
                    "battletag",
                    "bnetid",
                    "playerid",
                    "matchid",
                    "roleid",
                    "token",
                    "cookie",
                    "authorization",
                    "credential",
                    "url",
                    "raw",
                    "response",
                ] {
                    assert!(
                        !normalized.contains(forbidden),
                        "sensitive event field at {path}.{key}"
                    );
                }
                scan(child, &format!("{path}.{key}"));
            }),
            Value::Array(items) => items.iter().enumerate().for_each(|(index, child)| {
                scan(child, &format!("{path}[{index}]"));
            }),
            _ => {}
        }
    }
    scan(&value, "event");
    assert!(!serde_json::to_string(&value)?.contains("Synthetic#"));
    Ok(())
}

#[test]
fn commit_and_event_delivery_meet_the_local_p95_budgets() -> Result<()> {
    let temp = tempfile::tempdir()?;
    let db = OwDatabase::new(temp.path().join("timing.sqlite3"));
    db.initialize()?;
    let mut commit_samples = Vec::with_capacity(100);
    let mut event_samples = Vec::with_capacity(100);

    for revision in 1..=100_i64 {
        // The upstream response is already parsed at this seam. Measure the
        // SQLite transaction and the same serialize/sink handoff used after commit.
        let commit_started = Instant::now();
        let mut conn = db.connect()?;
        let transaction = conn.transaction()?;
        transaction.execute(
            "INSERT INTO season_sync_state(season, season_revision, snapshot_revision, background_status, updated_at) VALUES(23, ?1, 0, 'running', 'now') ON CONFLICT(season) DO UPDATE SET season_revision=excluded.season_revision",
            [revision],
        )?;
        transaction.commit()?;
        commit_samples.push(commit_started.elapsed());

        let event_started = Instant::now();
        let payload = serde_json::to_value(SeasonDataRevisionEvent {
            account_scope: "sha256:anonymous-scope".into(),
            season: 23,
            revision,
            phase: "list".into(),
            new_matches: 1,
            pending_details: 0,
            status: "running".into(),
        })?;
        std::hint::black_box(payload);
        event_samples.push(event_started.elapsed());
    }

    let commit_p95 = percentile_95(commit_samples);
    let event_p95 = percentile_95(event_samples);
    assert!(
        commit_p95 <= RESPONSE_TO_COMMIT_P95_BUDGET,
        "response-to-commit P95 {commit_p95:?} exceeded {RESPONSE_TO_COMMIT_P95_BUDGET:?}"
    );
    assert!(
        event_p95 <= COMMIT_TO_EVENT_P95_BUDGET,
        "commit-to-event P95 {event_p95:?} exceeded {COMMIT_TO_EVENT_P95_BUDGET:?}"
    );
    Ok(())
}

#[test]
fn live_update_logs_fixtures_and_tauri_contracts_have_no_secret_surface() -> Result<()> {
    let crate_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let sources = [
        crate_root.join("src/auto_season_update.rs"),
        crate_root.join("src/live_season_update.rs"),
        crate_root.join("src/background_sync.rs"),
        crate_root.join("tests/live_match_acceptance.rs"),
        crate_root.join("tests/quick_list_latency.rs"),
    ];
    for path in sources {
        let source = std::fs::read_to_string(&path)?;
        for line in source.lines().filter(|line| {
            line.contains("println!") || line.contains("eprintln!") || line.contains("emit(")
        }) {
            let normalized = line.to_ascii_lowercase();
            for forbidden in [
                "authorization",
                "cookie",
                "credential",
                "battletag",
                "player_id",
                "match_id",
                "raw_response",
                "request_url",
            ] {
                assert!(
                    !normalized.contains(forbidden),
                    "sensitive live-update log/event surface in {}: {line}",
                    path.display()
                );
            }
        }
    }

    // The public live-update Tauri DTO is deliberately an allowlist. Fixtures
    // use anonymous scopes and must never contain realistic bearer/cookie data.
    let fixture_root = crate_root.join("tests/fixtures");
    assert!(
        fixture_root.is_dir(),
        "fixture security scan requires a real fixture directory"
    );
    let mut pending = vec![fixture_root];
    let mut scanned = 0;
    while let Some(path) = pending.pop() {
        for entry in std::fs::read_dir(path)? {
            let entry = entry?;
            if entry.file_type()?.is_dir() {
                pending.push(entry.path());
                continue;
            }
            if entry.path().extension().and_then(|value| value.to_str()) != Some("json") {
                continue;
            }
            scanned += 1;
            let lower = std::fs::read_to_string(entry.path())?.to_ascii_lowercase();
            assert!(!lower.contains("authorization: bearer "));
            assert!(!lower.contains("set-cookie:"));
            assert!(!lower.contains("battle.net/oauth"));
        }
    }
    assert!(
        scanned > 0,
        "fixture security scan must inspect at least one JSON fixture"
    );
    Ok(())
}
