use super::material::{scan_value, valid_fact_strength, FactSubject, RoastFact, RoastFactEvidence};
use crate::{ArenaDetailSlice, ArenaMatchDetail, PlayerRow};
use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ArenaRoastMaterial {
    pub candidate_facts: Vec<RoastFact>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RoundOutcome {
    Win,
    Loss,
    Draw,
}

#[derive(Debug, Clone, Copy)]
struct ReliableRound {
    outcome: RoundOutcome,
    before_team: i64,
    before_opponent: i64,
    team_score: i64,
    opponent_score: i64,
}

pub fn build_arena_roast_material(
    detail: &ArenaMatchDetail,
    current_bnet_id: &str,
) -> ArenaRoastMaterial {
    let rounds = reliable_rounds(&detail.rounds);
    let mut candidate_facts = Vec::new();
    let sequence_evidence_is_complete = sequence_summary_matches(detail, &rounds);
    let has_comeback = sequence_evidence_is_complete && is_three_down_comeback(detail, &rounds);
    let has_decider =
        sequence_evidence_is_complete && !has_comeback && won_decider_after_tie(detail, &rounds);

    if sequence_evidence_is_complete {
        candidate_facts.extend(ordinary_round_facts(&rounds));
    }

    if has_comeback {
        candidate_facts.push(match_fact(
            "arena_three_down_comeback",
            "\u{524d}3\u{56de}\u{5408}\u{5168}\u{8d25}\u{ff0c}\u{968f}\u{540e}\u{8fde}\u{8d62}4\u{56de}\u{5408}\u{5b8c}\u{6210}\u{7ffb}\u{76d8}",
            0.98,
        ));
    } else if has_decider {
        candidate_facts.push(match_fact(
            "arena_won_decider_after_tie",
            "\u{672c}\u{573a}\u{4e00}\u{5ea6}\u{9886}\u{5148}\u{ff0c}\u{968f}\u{540e}\u{88ab}\u{8ffd}\u{5e73}\u{ff0c}\u{5e76}\u{8d62}\u{4e0b}\u{51b3}\u{80dc}\u{56de}\u{5408}",
            0.92,
        ));
    } else if let Some(streak) = sequence_evidence_is_complete
        .then(|| longest_win_streak(&rounds))
        .flatten()
        .filter(|streak| *streak >= 2)
    {
        candidate_facts.push(match_fact(
            "arena_longest_win_streak",
            &format!(
                "\u{672c}\u{573a}\u{6700}\u{957f}\u{8fde}\u{7eed}\u{8d62}\u{4e0b}{streak}\u{56de}\u{5408}"
            ),
            (0.68 + streak as f64 * 0.04).min(0.88),
        ));
    }

    if let Some(fact) = final_team_funds_rank(&detail.summary, current_bnet_id) {
        candidate_facts.push(fact);
    }

    ArenaRoastMaterial { candidate_facts }
}

fn ordinary_round_facts(rounds: &[Option<ReliableRound>]) -> [RoastFact; 2] {
    let rounds = rounds.iter().filter_map(Option::as_ref).collect::<Vec<_>>();
    let last = rounds
        .last()
        .expect("complete arena round sequence is non-empty");
    let final_score_text = match last.team_score.cmp(&last.opponent_score) {
        std::cmp::Ordering::Greater => {
            format!("本场最终以{}:{}获胜", last.team_score, last.opponent_score)
        }
        std::cmp::Ordering::Less => {
            format!("本场最终以{}:{}告负", last.team_score, last.opponent_score)
        }
        std::cmp::Ordering::Equal => {
            format!("本场最终战成{}:{}平", last.team_score, last.opponent_score)
        }
    };
    let wins = rounds
        .iter()
        .filter(|round| round.outcome == RoundOutcome::Win)
        .count();
    let losses = rounds
        .iter()
        .filter(|round| round.outcome == RoundOutcome::Loss)
        .count();
    [
        match_fact("arena_final_round_score", &final_score_text, 0.9),
        match_fact(
            "arena_round_record",
            &format!("完整回合记录为{wins}胜{losses}负"),
            0.82,
        ),
    ]
}

pub fn assert_arena_material_privacy(material: &ArenaRoastMaterial) -> Result<()> {
    if material
        .candidate_facts
        .iter()
        .any(|fact| !valid_fact_strength(fact.strength))
    {
        return Err(anyhow!("arena roast fact strength is invalid"));
    }
    if material.candidate_facts.iter().any(|fact| {
        !matches!(
            fact.subject,
            FactSubject::CurrentPlayer | FactSubject::Match
        ) || !fact.id.starts_with(fact.subject.id_prefix())
    }) {
        return Err(anyhow!("arena roast fact subject is invalid"));
    }
    let ids = material
        .candidate_facts
        .iter()
        .map(|fact| fact.id.as_str())
        .collect::<HashSet<_>>();
    if ids.len() != material.candidate_facts.len() {
        return Err(anyhow!("arena roast facts are not unique"));
    }
    scan_value(&serde_json::to_value(material)?)
}

fn reliable_rounds(rounds: &[ArenaDetailSlice]) -> Vec<Option<ReliableRound>> {
    let mut previous_scores = Some((0, 0));
    rounds
        .iter()
        .map(|round| {
            let Some((team_score, opponent_score)) = round.team_score.zip(round.opponent_score)
            else {
                previous_scores = None;
                return None;
            };
            if team_score < 0 || opponent_score < 0 {
                previous_scores = Some((team_score, opponent_score));
                return None;
            }
            let Some((before_team, before_opponent)) = previous_scores else {
                previous_scores = Some((team_score, opponent_score));
                return None;
            };
            previous_scores = Some((team_score, opponent_score));
            let team_delta = team_score - before_team;
            let opponent_delta = opponent_score - before_opponent;
            let score_outcome = match (team_delta, opponent_delta) {
                (1, 0) => Some(RoundOutcome::Win),
                (0, 1) => Some(RoundOutcome::Loss),
                (0, 0) => Some(RoundOutcome::Draw),
                _ => None,
            }?;
            let declared_outcome = match round
                .result
                .as_deref()
                .map(str::trim)
                .map(str::to_ascii_lowercase)
                .as_deref()
            {
                Some("win") => RoundOutcome::Win,
                Some("loss") => RoundOutcome::Loss,
                Some("draw") => RoundOutcome::Draw,
                _ => return None,
            };
            (score_outcome == declared_outcome).then_some(ReliableRound {
                outcome: declared_outcome,
                before_team,
                before_opponent,
                team_score,
                opponent_score,
            })
        })
        .collect()
}

fn sequence_summary_matches(detail: &ArenaMatchDetail, rounds: &[Option<ReliableRound>]) -> bool {
    if rounds.is_empty() || rounds.iter().any(Option::is_none) {
        return false;
    }
    let Some(last) = rounds.last().and_then(Option::as_ref) else {
        return false;
    };
    let summary_outcome = match detail
        .summary
        .result
        .as_deref()
        .map(str::trim)
        .map(str::to_ascii_lowercase)
        .as_deref()
    {
        Some("win") => RoundOutcome::Win,
        Some("loss") => RoundOutcome::Loss,
        Some("draw") => RoundOutcome::Draw,
        _ => return false,
    };
    let score_outcome = match last.team_score.cmp(&last.opponent_score) {
        std::cmp::Ordering::Greater => RoundOutcome::Win,
        std::cmp::Ordering::Less => RoundOutcome::Loss,
        std::cmp::Ordering::Equal => RoundOutcome::Draw,
    };

    detail.summary.team_score == Some(last.team_score)
        && detail.summary.opponent_score == Some(last.opponent_score)
        && summary_outcome == score_outcome
}

fn summary_is_win(detail: &ArenaMatchDetail) -> bool {
    detail.summary.result.as_deref() == Some("win")
        && detail
            .summary
            .team_score
            .zip(detail.summary.opponent_score)
            .is_some_and(|(team, opponent)| team > opponent)
}

fn is_three_down_comeback(detail: &ArenaMatchDetail, rounds: &[Option<ReliableRound>]) -> bool {
    const EXPECTED: [RoundOutcome; 7] = [
        RoundOutcome::Loss,
        RoundOutcome::Loss,
        RoundOutcome::Loss,
        RoundOutcome::Win,
        RoundOutcome::Win,
        RoundOutcome::Win,
        RoundOutcome::Win,
    ];
    summary_is_win(detail)
        && rounds.len() == EXPECTED.len()
        && rounds
            .iter()
            .zip(EXPECTED)
            .all(|(round, expected)| round.is_some_and(|round| round.outcome == expected))
        && rounds
            .last()
            .and_then(Option::as_ref)
            .is_some_and(|round| round.team_score == 4 && round.opponent_score == 3)
}

fn won_decider_after_tie(detail: &ArenaMatchDetail, rounds: &[Option<ReliableRound>]) -> bool {
    if !summary_is_win(detail) || rounds.is_empty() || rounds.iter().any(Option::is_none) {
        return false;
    }
    let valid = rounds.iter().filter_map(Option::as_ref).collect::<Vec<_>>();
    let Some(last) = valid.last() else {
        return false;
    };
    if last.outcome != RoundOutcome::Win || last.before_team != last.before_opponent {
        return false;
    }
    let before_decider = &valid[..valid.len() - 1];
    let had_lead = before_decider
        .iter()
        .any(|round| round.team_score > round.opponent_score);
    let tied_after_lead = before_decider.iter().enumerate().any(|(index, round)| {
        round.team_score == round.opponent_score
            && before_decider[..index]
                .iter()
                .any(|earlier| earlier.team_score > earlier.opponent_score)
    });
    had_lead && tied_after_lead
}

fn longest_win_streak(rounds: &[Option<ReliableRound>]) -> Option<usize> {
    let mut longest = 0;
    let mut current = 0;
    for round in rounds {
        match round {
            Some(round) if round.outcome == RoundOutcome::Win => {
                current += 1;
                longest = longest.max(current);
            }
            _ => current = 0,
        }
    }
    (longest > 0).then_some(longest)
}

fn final_team_funds_rank(summary: &ArenaDetailSlice, current_bnet_id: &str) -> Option<RoastFact> {
    let current_bnet_id = current_bnet_id.trim();
    let teammates = &summary.teammates;
    if current_bnet_id.is_empty() || teammates.len() != 5 || !complete_team_identity(teammates) {
        return None;
    }
    let current = teammates
        .iter()
        .filter(|player| player.bnet_id == current_bnet_id)
        .collect::<Vec<_>>();
    if current.len() != 1 {
        return None;
    }
    let values = teammates
        .iter()
        .map(|player| player.end_worth.filter(|value| *value >= 0))
        .collect::<Option<Vec<_>>>()?;
    let current_value = current[0].end_worth?;
    let rank = 1 + values
        .iter()
        .filter(|value| **value > current_value)
        .count();
    let tied = values
        .iter()
        .filter(|value| **value == current_value)
        .count();
    let text = if tied > 1 {
        format!(
            "\u{6700}\u{7ec8}\u{8d44}\u{91d1}\u{5e76}\u{5217}\u{961f}\u{5185}\u{7b2c}{rank}\u{ff08}\u{5171}{}\u{4eba}\u{ff09}",
            teammates.len()
        )
    } else {
        format!(
            "\u{6700}\u{7ec8}\u{8d44}\u{91d1}\u{4f4d}\u{5217}\u{961f}\u{5185}\u{7b2c}{rank}\u{ff08}\u{5171}{}\u{4eba}\u{ff09}",
            teammates.len()
        )
    };
    Some(RoastFact {
        id: format!(
            "{}arena_final_team_funds_rank",
            FactSubject::CurrentPlayer.id_prefix()
        ),
        subject: FactSubject::CurrentPlayer,
        text,
        strength: 0.78,
        evidence: RoastFactEvidence::Rank,
    })
}

fn complete_team_identity(teammates: &[PlayerRow]) -> bool {
    let ids = teammates
        .iter()
        .map(|player| player.bnet_id.trim())
        .collect::<HashSet<_>>();
    teammates
        .iter()
        .all(|player| player.side == "teammate" && !player.bnet_id.trim().is_empty())
        && ids.len() == teammates.len()
}

fn match_fact(suffix: &str, text: &str, strength: f64) -> RoastFact {
    RoastFact {
        id: format!("{}{suffix}", FactSubject::Match.id_prefix()),
        subject: FactSubject::Match,
        text: text.to_string(),
        strength,
        evidence: RoastFactEvidence::Result,
    }
}
