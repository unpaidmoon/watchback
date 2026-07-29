use crate::db::OwDatabase;
use crate::models::{CareerAchievement, CareerAchievementTier};
use anyhow::{Context, Result};
use chrono::{DateTime, Local};
use serde_json::Value;
use std::collections::{HashMap, HashSet};

struct AchievementDefinition {
    key: &'static str,
    title: &'static str,
    detail: &'static str,
    requirements: [&'static str; 3],
}

const DEFINITIONS: [AchievementDefinition; 19] = [
    AchievementDefinition {
        key: "win-streak",
        title: "常胜将军",
        detail: "连续表现奖章",
        requirements: [
            "单赛季最长连胜达到 5 局",
            "单赛季最长连胜达到 10 局",
            "单赛季最长连胜达到 15 局",
        ],
    },
    AchievementDefinition {
        key: "s-grade",
        title: "高光制造者",
        detail: "S 级峰值表现",
        requirements: [
            "累计获得 10 场 S 级表现",
            "累计获得 30 场 S 级表现",
            "累计获得 50 场 S 级表现",
        ],
    },
    AchievementDefinition {
        key: "carry",
        title: "战局核心",
        detail: "高价值正向表现",
        requirements: [
            "累计完成 10 场带飞局",
            "累计完成 30 场带飞局",
            "累计完成 50 场带飞局",
        ],
    },
    AchievementDefinition {
        key: "signature",
        title: "招牌打法",
        detail: "最稳定的个人高光标签",
        requirements: [
            "同一高光标签累计出现 10 次",
            "同一高光标签累计出现 30 次",
            "同一高光标签累计出现 50 次",
        ],
    },
    AchievementDefinition {
        key: "hero-focus",
        title: "本命专精",
        detail: "第一英雄使用占比",
        requirements: [
            "第一英雄生涯使用占比达到 25%",
            "第一英雄生涯使用占比达到 40%",
            "第一英雄生涯使用占比达到 55%",
        ],
    },
    AchievementDefinition {
        key: "switching",
        title: "战术变阵",
        detail: "生涯英雄池广度",
        requirements: [
            "生涯使用 20 名不同英雄",
            "生涯使用 35 名不同英雄",
            "生涯使用 50 名不同英雄",
        ],
    },
    AchievementDefinition {
        key: "glass-cannon",
        title: "玻璃大炮",
        detail: "高输出、低暴露的精准打法",
        requirements: [
            "单局伤害达到 12,000，且承伤不高于 8,000",
            "单局伤害达到 18,000，且承伤不高于 8,000",
            "单局伤害达到 25,000，且承伤不高于 8,000",
        ],
    },
    AchievementDefinition {
        key: "damage-machine",
        title: "输出机器",
        detail: "稳定持续的高伤害输出",
        requirements: [
            "单局每十分钟伤害达到 8,000",
            "单局每十分钟伤害达到 11,000",
            "单局每十分钟伤害达到 14,000",
        ],
    },
    AchievementDefinition {
        key: "bloodbath",
        title: "血雨腥风",
        detail: "大量击杀的压制性表现",
        requirements: [
            "单局总击杀数达到 20",
            "单局总击杀数达到 35",
            "单局总击杀数达到 50",
        ],
    },
    AchievementDefinition {
        key: "comeback",
        title: "逆风翻盘",
        detail: "角斗竞技逆风胜利",
        requirements: [
            "完成 1 场角斗领域的让三追四",
            "完成 5 场角斗领域的让三追四",
            "完成 15 场角斗领域的让三追四",
        ],
    },
    AchievementDefinition {
        key: "godlike-run",
        title: "超神之旅",
        detail: "零死亡的高击杀表现",
        requirements: [
            "单局死亡为 0，且击杀至少 10",
            "单局死亡为 0，且击杀至少 20",
            "单局死亡为 0，且击杀至少 30",
        ],
    },
    AchievementDefinition {
        key: "iron-wall",
        title: "铜墙铁壁",
        detail: "持续承压的前线表现",
        requirements: [
            "单局每十分钟承伤达到 10,000",
            "单局每十分钟承伤达到 14,000",
            "单局每十分钟承伤达到 18,000",
        ],
    },
    AchievementDefinition {
        key: "rank-proof",
        title: "实力的证明",
        detail: "生涯最高竞技段位",
        requirements: [
            "任一职责竞技段位达到铂金",
            "任一职责竞技段位达到大师",
            "任一职责竞技段位达到英杰",
        ],
    },
    AchievementDefinition {
        key: "battle-hardened",
        title: "久经沙场",
        detail: "长期取胜的生涯积累",
        requirements: [
            "生涯总胜场达到 50",
            "生涯总胜场达到 250",
            "生涯总胜场达到 1,000",
        ],
    },
    AchievementDefinition {
        key: "brothers-in-arms",
        title: "燃情兄弟",
        detail: "与好友长期并肩作战",
        requirements: [
            "与好友一起游戏达到 20 场",
            "与好友一起游戏达到 100 场",
            "与好友一起游戏达到 300 场",
        ],
    },
    AchievementDefinition {
        key: "best-teammate",
        title: "最佳队友",
        detail: "高助攻团队贡献",
        requirements: [
            "单局总助攻数达到 15",
            "单局总助攻数达到 30",
            "单局总助攻数达到 45",
        ],
    },
    AchievementDefinition {
        key: "unwavering",
        title: "风雨无阻",
        detail: "连续自然日完成可统计对局",
        requirements: ["连续游玩 10 天", "连续游玩 20 天", "连续游玩 30 天"],
    },
    AchievementDefinition {
        key: "wealth-tycoon",
        title: "财富大亨",
        detail: "角斗竞技最后一回合资金",
        requirements: [
            "最后一回合结束时资金达到 80,000",
            "最后一回合结束时资金达到 90,000",
            "最后一回合结束时资金达到 100,000",
        ],
    },
    AchievementDefinition {
        key: "stadium-ascendant",
        title: "登峰造极",
        detail: "角斗竞技任意职责最高段位",
        requirements: ["达到专家", "达到全明星", "达到传奇"],
    },
];

#[derive(Debug)]
struct CareerMatchFact {
    season: i64,
    result: String,
    begin_ts: i64,
    hero: String,
    kills: Option<i64>,
    assists: Option<i64>,
    deaths: Option<i64>,
    damage: Option<i64>,
    damage_taken: Option<i64>,
    duration_sec: Option<i64>,
    rank_level: i64,
    is_standard_competitive: bool,
    is_stadium: bool,
    stadium_end_worth: Option<i64>,
    stadium_rank_level: i64,
    has_friend: bool,
    arena_was_three_points_behind: bool,
    grade: Option<String>,
    primary_tag: Option<String>,
    evidence_tags: Vec<String>,
}

#[derive(Debug, Default)]
struct StadiumAchievementFact {
    end_worth: Option<i64>,
    rank_level: i64,
}

pub(super) fn get_career_achievements_impl(db: &OwDatabase) -> Result<Vec<CareerAchievement>> {
    let facts = load_career_facts(db)?;
    let mut achievements = DEFINITIONS
        .iter()
        .map(|definition| CareerAchievement {
            key: definition.key.to_owned(),
            title: definition.title.to_owned(),
            detail: definition.detail.to_owned(),
            current_level: 0,
            tiers: definition
                .requirements
                .iter()
                .enumerate()
                .map(|(index, requirement)| CareerAchievementTier {
                    level: index as i64 + 1,
                    requirement: (*requirement).to_owned(),
                    unlocked_at: None,
                })
                .collect(),
        })
        .collect::<Vec<_>>();

    evaluate_new_achievements(&facts, &mut achievements);
    evaluate_existing_achievements(&facts, &mut achievements);
    Ok(achievements)
}

fn load_career_facts(db: &OwDatabase) -> Result<Vec<CareerMatchFact>> {
    let conn = db.connect()?;
    let mut statement = conn.prepare(
        r#"
        SELECT m.match_id, m.season, m.result, m.begin_ts, player.hero_guid,
               player.kill, player.assist, player.death, player.hero_damage,
               m.duration_sec, m.raw_entry_json, player.damage_taken,
               EXISTS(SELECT 1 FROM match_players friend
                      WHERE friend.match_id = m.match_id AND friend.is_friend = 1),
               detail.raw_detail_json, evaluation.grade, evaluation.primary_tag,
               evaluation.evidence_tags_json, m.bucket, m.game_mode,
               COALESCE(
                   player.bnet_id,
                   (SELECT profile.bnet_id
                    FROM players profile
                    WHERE TRIM(COALESCE(profile.bnet_id, '')) <> ''
                    ORDER BY profile.updated_at DESC
                    LIMIT 1)
               )
        FROM matches m
        LEFT JOIN match_players player ON player.rowid = (
            SELECT current_player.rowid FROM match_players current_player
            WHERE current_player.match_id = m.match_id AND current_player.is_current_player = 1
            ORDER BY current_player.side, current_player.bnet_id LIMIT 1
        )
        LEFT JOIN match_details detail ON detail.match_id = m.match_id
        LEFT JOIN match_evaluations evaluation ON evaluation.match_id = m.match_id
        ORDER BY m.begin_ts, m.match_id
        "#,
    )?;
    let rows = statement.query_map([], |row| {
        let raw_entry: String = row.get(10)?;
        let raw_detail: Option<String> = row.get(13)?;
        let result: String = row.get(2)?;
        let bucket: String = row.get(17)?;
        let game_mode: String = row.get(18)?;
        let bnet_id: Option<String> = row.get(19)?;
        let is_standard_competitive = matches!(game_mode.as_str(), "SportPreset" | "SportOpen");
        let is_arena =
            bucket == "stadium" && matches!(game_mode.as_str(), "SportFight" | "LeisureFight");
        let is_stadium = bucket == "stadium" && game_mode == "SportFight";
        let stadium = is_stadium
            .then(|| {
                raw_detail
                    .as_deref()
                    .zip(bnet_id.as_deref())
                    .and_then(|(detail, bnet_id)| parse_stadium_fact(detail, bnet_id))
            })
            .flatten();
        Ok(CareerMatchFact {
            season: row.get(1)?,
            result: result.clone(),
            begin_ts: row.get(3)?,
            hero: row.get::<_, Option<String>>(4)?.unwrap_or_default(),
            kills: row.get(5)?,
            assists: row.get(6)?,
            deaths: row.get(7)?,
            damage: row.get(8)?,
            duration_sec: row.get(9)?,
            rank_level: parse_rank_level(&raw_entry),
            is_standard_competitive,
            is_stadium,
            stadium_end_worth: stadium.as_ref().and_then(|value| value.end_worth),
            stadium_rank_level: stadium.as_ref().map(|value| value.rank_level).unwrap_or(0),
            damage_taken: row.get(11)?,
            has_friend: row.get::<_, i64>(12)? == 1
                || (is_arena
                    && raw_detail
                        .as_deref()
                        .zip(bnet_id.as_deref())
                        .is_some_and(|(detail, bnet_id)| arena_has_friend(detail, bnet_id))),
            arena_was_three_points_behind: result == "win"
                && raw_detail.as_deref().is_some_and(was_three_points_behind),
            grade: row.get(14)?,
            primary_tag: row.get(15)?,
            evidence_tags: row
                .get::<_, Option<String>>(16)?
                .and_then(|text| serde_json::from_str(&text).ok())
                .unwrap_or_default(),
        })
    })?;
    rows.collect::<rusqlite::Result<Vec<_>>>()
        .context("无法读取生涯成就对局事实")
}

fn arena_has_friend(raw_detail: &str, bnet_id: &str) -> bool {
    fn visit(value: &Value, bnet_id: &str) -> bool {
        match value {
            Value::Object(object) => {
                let is_current_player = object
                    .get("bnetId")
                    .or_else(|| object.get("bnet_id"))
                    .and_then(value_as_string)
                    .as_deref()
                    == Some(bnet_id);
                (is_current_player
                    && object
                        .get("friendBnetIds")
                        .or_else(|| object.get("friend_bnet_ids"))
                        .and_then(Value::as_array)
                        .is_some_and(|friends| !friends.is_empty()))
                    || object.values().any(|item| visit(item, bnet_id))
            }
            Value::Array(items) => items.iter().any(|item| visit(item, bnet_id)),
            _ => false,
        }
    }

    serde_json::from_str::<Value>(raw_detail)
        .ok()
        .is_some_and(|value| visit(&value, bnet_id))
}

fn parse_stadium_fact(raw_detail: &str, bnet_id: &str) -> Option<StadiumAchievementFact> {
    let root = serde_json::from_str::<Value>(raw_detail).ok()?;
    let data = root
        .pointer("/payload/data")
        .or_else(|| root.get("data"))
        .unwrap_or(&root);
    let last_round_player = data
        .get("roundCountList")
        .and_then(Value::as_array)
        .and_then(|rounds| {
            rounds
                .iter()
                .rev()
                .find_map(|round| stadium_player(round, bnet_id))
        });
    let total_player = data
        .get("totalCount")
        .and_then(|total| stadium_player(total, bnet_id));
    if last_round_player.is_none() && total_player.is_none() {
        return None;
    }
    Some(StadiumAchievementFact {
        end_worth: last_round_player
            .and_then(|player| player.get("endWorth"))
            .and_then(Value::as_i64),
        rank_level: total_player
            .map(parse_rank_info)
            .filter(|level| *level > 0)
            .or_else(|| last_round_player.map(parse_rank_info))
            .unwrap_or(0),
    })
}

fn stadium_player<'a>(value: &'a Value, bnet_id: &str) -> Option<&'a Value> {
    value
        .get("teammateList")?
        .as_array()?
        .iter()
        .find(|player| {
            player
                .get("bnetId")
                .or_else(|| player.get("bnet_id"))
                .and_then(value_as_string)
                .as_deref()
                == Some(bnet_id)
        })
}

fn value_as_string(value: &Value) -> Option<String> {
    match value {
        Value::String(text) => Some(text.clone()),
        Value::Number(number) => number
            .as_i64()
            .map(|value| value.to_string())
            .or_else(|| number.as_u64().map(|value| value.to_string())),
        _ => None,
    }
}

fn parse_rank_info(value: &Value) -> i64 {
    let rank = value.get("rankInfo").unwrap_or(&Value::Null);
    let name = rank
        .get("rankName")
        .or_else(|| rank.get("rank_name"))
        .and_then(Value::as_str)
        .unwrap_or_default()
        .trim()
        .to_ascii_lowercase();
    match name.as_str() {
        "diamond" => 5,
        "master" => 6,
        "grandmaster" => 7,
        _ => 0,
    }
}

fn parse_rank_level(raw_entry: &str) -> i64 {
    let value = serde_json::from_str::<Value>(raw_entry).unwrap_or(Value::Null);
    let rank = value.get("rankInfo").unwrap_or(&Value::Null);
    let name = rank
        .get("rankName")
        .or_else(|| rank.get("rank_name"))
        .and_then(Value::as_str)
        .unwrap_or_default()
        .trim()
        .to_ascii_lowercase();
    match name.as_str() {
        "bronze" => 1,
        "silver" => 2,
        "gold" => 3,
        "platinum" => 4,
        "diamond" => 5,
        "master" => 6,
        "grandmaster" => 7,
        "champion" => 8,
        _ => rank
            .get("rankScore")
            .and_then(Value::as_i64)
            .map(|score| ((score / 100) + 1).clamp(1, 8))
            .unwrap_or(0),
    }
}

fn was_three_points_behind(raw_detail: &str) -> bool {
    fn visit(value: &Value) -> bool {
        match value {
            Value::Object(object) => {
                let team = object
                    .get("teamScore")
                    .or_else(|| object.get("team_score"))
                    .and_then(Value::as_i64);
                let opponent = object
                    .get("opponentScore")
                    .or_else(|| object.get("opponent_score"))
                    .and_then(Value::as_i64);
                matches!((team, opponent), (Some(team), Some(opponent)) if opponent - team >= 3)
                    || object.values().any(visit)
            }
            Value::Array(items) => items.iter().any(visit),
            _ => false,
        }
    }
    serde_json::from_str::<Value>(raw_detail)
        .ok()
        .is_some_and(|value| visit(&value))
}

fn evaluate_new_achievements(facts: &[CareerMatchFact], achievements: &mut [CareerAchievement]) {
    let mut previous_day = None;
    let mut streak = 0;
    let mut unwavering_dates = [None; 3];
    for fact in facts {
        let Some(day) = DateTime::from_timestamp(fact.begin_ts, 0)
            .map(|timestamp| timestamp.with_timezone(&Local).date_naive())
        else {
            continue;
        };
        if previous_day == Some(day) {
            continue;
        }
        streak = match previous_day {
            Some(previous) if day.signed_duration_since(previous).num_days() == 1 => streak + 1,
            _ => 1,
        };
        previous_day = Some(day);
        for (index, threshold) in [10, 20, 30].into_iter().enumerate() {
            if streak >= threshold && unwavering_dates[index].is_none() {
                unwavering_dates[index] = Some(fact.begin_ts);
            }
        }
    }
    set_dates(achievements, "unwavering", unwavering_dates);
    set_dates(
        achievements,
        "wealth-tycoon",
        [80_000, 90_000, 100_000].map(|threshold| {
            first_match_unlock(facts, |fact| {
                fact.is_stadium
                    && fact
                        .stadium_end_worth
                        .is_some_and(|worth| worth >= threshold)
            })
        }),
    );
    set_dates(
        achievements,
        "stadium-ascendant",
        [5, 6, 7].map(|threshold| {
            first_match_unlock(facts, |fact| {
                fact.is_stadium && fact.stadium_rank_level >= threshold
            })
        }),
    );
    set_dates(
        achievements,
        "glass-cannon",
        [
            first_match_unlock(facts, |fact| {
                fact.damage.is_some_and(|v| v >= 12_000)
                    && fact.damage_taken.is_some_and(|v| v <= 8_000)
            }),
            first_match_unlock(facts, |fact| {
                fact.damage.is_some_and(|v| v >= 18_000)
                    && fact.damage_taken.is_some_and(|v| v <= 8_000)
            }),
            first_match_unlock(facts, |fact| {
                fact.damage.is_some_and(|v| v >= 25_000)
                    && fact.damage_taken.is_some_and(|v| v <= 8_000)
            }),
        ],
    );
    set_dates(
        achievements,
        "damage-machine",
        [8_000.0, 11_000.0, 14_000.0].map(|threshold| {
            first_match_unlock(facts, |fact| {
                fact.damage
                    .zip(fact.duration_sec)
                    .and_then(|(v, d)| per_ten_minutes(v, d))
                    .is_some_and(|v| v >= threshold)
            })
        }),
    );
    set_dates(
        achievements,
        "bloodbath",
        [20, 35, 50].map(|threshold| {
            first_match_unlock(facts, |fact| fact.kills.is_some_and(|v| v >= threshold))
        }),
    );
    set_dates(
        achievements,
        "comeback",
        cumulative_unlock_dates(
            facts
                .iter()
                .filter(|fact| fact.arena_was_three_points_behind)
                .map(|fact| fact.begin_ts),
            [1, 5, 15],
        ),
    );
    set_dates(
        achievements,
        "godlike-run",
        [10, 20, 30].map(|threshold| {
            first_match_unlock(facts, |fact| {
                fact.deaths == Some(0) && fact.kills.is_some_and(|v| v >= threshold)
            })
        }),
    );
    set_dates(
        achievements,
        "iron-wall",
        [10_000.0, 14_000.0, 18_000.0].map(|threshold| {
            first_match_unlock(facts, |fact| {
                fact.damage_taken
                    .zip(fact.duration_sec)
                    .and_then(|(v, d)| per_ten_minutes(v, d))
                    .is_some_and(|v| v >= threshold)
            })
        }),
    );
    set_dates(
        achievements,
        "rank-proof",
        [4, 6, 8].map(|threshold| {
            first_match_unlock(facts, |fact| {
                fact.is_standard_competitive && fact.rank_level >= threshold
            })
        }),
    );
    set_dates(
        achievements,
        "battle-hardened",
        cumulative_unlock_dates(
            facts
                .iter()
                .filter(|fact| fact.result == "win")
                .map(|fact| fact.begin_ts),
            [50, 250, 1_000],
        ),
    );
    set_dates(
        achievements,
        "brothers-in-arms",
        cumulative_unlock_dates(
            facts
                .iter()
                .filter(|fact| fact.has_friend)
                .map(|fact| fact.begin_ts),
            [20, 100, 300],
        ),
    );
    set_dates(
        achievements,
        "best-teammate",
        [15, 30, 45].map(|threshold| {
            first_match_unlock(facts, |fact| fact.assists.is_some_and(|v| v >= threshold))
        }),
    );
}

fn evaluate_existing_achievements(
    facts: &[CareerMatchFact],
    achievements: &mut [CareerAchievement],
) {
    let mut streaks = HashMap::<i64, i64>::new();
    let mut streak_dates = [None; 3];
    for fact in facts {
        let streak = streaks.entry(fact.season).or_default();
        if fact.result == "win" {
            *streak += 1;
        } else {
            *streak = 0;
        }
        for (index, threshold) in [5, 10, 15].into_iter().enumerate() {
            if *streak >= threshold && streak_dates[index].is_none() {
                streak_dates[index] = Some(fact.begin_ts);
            }
        }
    }
    set_dates(achievements, "win-streak", streak_dates);

    let mut hero_counts = HashMap::<&str, i64>::new();
    let mut heroes = HashSet::<&str>::new();
    let mut switching_dates = [None; 3];
    for fact in facts {
        if !fact.hero.is_empty() {
            *hero_counts.entry(&fact.hero).or_default() += 1;
            heroes.insert(&fact.hero);
            for (index, threshold) in [20_usize, 35, 50].into_iter().enumerate() {
                if heroes.len() >= threshold && switching_dates[index].is_none() {
                    switching_dates[index] = Some(fact.begin_ts);
                }
            }
        }
    }
    let hero_total = hero_counts.values().sum::<i64>();
    let hero_top = hero_counts.values().copied().max().unwrap_or(0);
    let hero_share = (hero_total > 0).then_some(hero_top as f64 * 100.0 / hero_total as f64);
    let hero_focus_level = [25.0, 40.0, 55.0]
        .into_iter()
        .filter(|threshold| hero_share.is_some_and(|share| share >= *threshold))
        .count() as i64;
    set_current_level_without_dates(achievements, "hero-focus", hero_focus_level);
    set_dates(achievements, "switching", switching_dates);
    set_dates(
        achievements,
        "s-grade",
        cumulative_unlock_dates(
            facts
                .iter()
                .filter(|fact| fact.grade.as_deref() == Some("S"))
                .map(|fact| fact.begin_ts),
            [10, 30, 50],
        ),
    );
    set_dates(
        achievements,
        "carry",
        cumulative_unlock_dates(
            facts
                .iter()
                .filter(|fact| fact.primary_tag.as_deref() == Some("带飞局"))
                .map(|fact| fact.begin_ts),
            [10, 30, 50],
        ),
    );

    let mut tag_occurrences = HashMap::<&str, (usize, Vec<i64>)>::new();
    for (fact_index, fact) in facts.iter().enumerate() {
        for tag in &fact.evidence_tags {
            if matches!(
                tag.as_str(),
                "KDA最高" | "击杀最多" | "伤害最高" | "治疗最高" | "格挡最多"
            ) {
                tag_occurrences
                    .entry(tag)
                    .or_insert_with(|| (fact_index, Vec::new()))
                    .1
                    .push(fact.begin_ts);
            }
        }
    }
    if let Some((tag, (_, dates))) =
        tag_occurrences
            .into_iter()
            .max_by(|(_, (first_a, dates_a)), (_, (first_b, dates_b))| {
                dates_a
                    .len()
                    .cmp(&dates_b.len())
                    .then_with(|| first_b.cmp(first_a))
            })
    {
        let signature_dates = [10_usize, 30, 50].map(|threshold| dates.get(threshold - 1).copied());
        let achievement = achievements
            .iter_mut()
            .find(|item| item.key == "signature")
            .unwrap();
        achievement.detail = tag.to_owned();
        set_dates(achievements, "signature", signature_dates);
    }
}

fn per_ten_minutes(value: i64, duration_sec: i64) -> Option<f64> {
    (duration_sec >= 300).then_some(value as f64 * 600.0 / duration_sec as f64)
}

fn first_match_unlock<F>(facts: &[CareerMatchFact], predicate: F) -> Option<i64>
where
    F: Fn(&CareerMatchFact) -> bool,
{
    facts
        .iter()
        .find(|fact| predicate(fact))
        .map(|fact| fact.begin_ts)
}

fn cumulative_unlock_dates<I>(dated_hits: I, thresholds: [i64; 3]) -> [Option<i64>; 3]
where
    I: IntoIterator<Item = i64>,
{
    let dates = dated_hits.into_iter().collect::<Vec<_>>();
    thresholds.map(|threshold| dates.get(threshold as usize - 1).copied())
}

fn set_dates(achievements: &mut [CareerAchievement], key: &str, dates: [Option<i64>; 3]) {
    let achievement = achievements
        .iter_mut()
        .find(|item| item.key == key)
        .unwrap();
    for (tier, date) in achievement.tiers.iter_mut().zip(dates) {
        tier.unlocked_at = date;
    }
    achievement.current_level = achievement
        .tiers
        .iter()
        .filter(|tier| tier.unlocked_at.is_some())
        .map(|tier| tier.level)
        .max()
        .unwrap_or(0);
}

fn set_current_level_without_dates(
    achievements: &mut [CareerAchievement],
    key: &str,
    current_level: i64,
) {
    let achievement = achievements
        .iter_mut()
        .find(|item| item.key == key)
        .unwrap();
    achievement.current_level = current_level;
    for tier in &mut achievement.tiers {
        tier.unlocked_at = None;
    }
}
