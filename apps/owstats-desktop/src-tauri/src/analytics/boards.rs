use super::*;

pub(super) fn build_boards(eligible: &[FriendAggregate], bands: &Bands) -> Vec<FriendBoard> {
    vec![
        board(
            "front_pressure",
            "前排压力位",
            "平均承受伤害高，平均格挡也高。",
            eligible,
            |item| bands.high_damage(item) && bands.high_mitigation(item),
            |item| item.avg_damage_taken() + item.avg_mitigation(),
        ),
        board(
            "low_resource_partner",
            "在世华佗",
            "平均受到治疗量低，平均死亡数也低。",
            eligible,
            |item| bands.low_healing(item) && bands.low_deaths(item),
            |item| {
                (bands.high_healing - item.avg_healing_taken()).max(0.0)
                    + (bands.high_deaths - item.avg_deaths()).max(0.0) * 1000.0
            },
        ),
        board(
            "priority_care",
            "资源黑洞",
            "平均受到治疗量高，平均死亡数也高。",
            eligible,
            |item| bands.high_healing_taken(item) && bands.high_deaths(item),
            |item| item.avg_healing_taken() + item.avg_deaths() * 1000.0,
        ),
        board(
            "healing_conversion",
            "受疗转化手",
            "受疗不低，千疗击杀更高。",
            eligible,
            |item| bands.high_healing_taken(item) && bands.high_healing_to_kill(item),
            |item| item.healing_to_kill_rate() * 1000.0 + item.avg_kills() * 100.0,
        ),
    ]
}

fn board(
    key: &str,
    label: &str,
    description: &str,
    eligible: &[FriendAggregate],
    include: impl Fn(&FriendAggregate) -> bool,
    score: impl Fn(&FriendAggregate) -> f64,
) -> FriendBoard {
    let mut friends = eligible
        .iter()
        .filter(|item| include(item))
        .map(|item| entry(item, key, label, score(item), description))
        .collect::<Vec<_>>();
    friends.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| b.same_team_matches.cmp(&a.same_team_matches))
            .then_with(|| a.name.cmp(&b.name))
    });
    friends.truncate(2);
    FriendBoard {
        key: key.to_string(),
        label: label.to_string(),
        description: description.to_string(),
        friends,
    }
}

fn entry(
    item: &FriendAggregate,
    board_key: &str,
    board_label: &str,
    score: f64,
    reason: &str,
) -> FriendBoardEntry {
    FriendBoardEntry {
        bnet_id: item.bnet_id.clone(),
        name: item.name.clone(),
        board_key: board_key.to_string(),
        board_label: board_label.to_string(),
        same_team_matches: item.same_team_matches,
        win_rate: round2(item.win_rate()),
        avg_damage_taken: round2(item.avg_damage_taken()),
        avg_healing_taken: round2(item.avg_healing_taken()),
        avg_mitigation: round2(item.avg_mitigation()),
        avg_deaths: round2(item.avg_deaths()),
        avg_kills: round2(item.avg_kills()),
        avg_assists: round2(average(item.assists, item.same_team_matches)),
        avg_damage: round2(average(item.damage, item.same_team_matches)),
        avg_healing: round2(average(item.healing, item.same_team_matches)),
        score: round2(score),
        reasons: vec![reason.to_string()],
    }
}
