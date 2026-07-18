use super::{input::ModeFamily, training::TrainingRow};
use serde::Serialize;
use std::collections::BTreeMap;

pub const ECE_BINS: usize = 10;
pub const MIN_REPORT_SAMPLES: usize = 30;

#[derive(Debug, Clone, Serialize)]
pub struct FoldSummary {
    pub fold: usize,
    pub train_samples: usize,
    pub validation_samples: usize,
    pub validation_match_groups: usize,
    pub train_end_ts: i64,
    pub validation_start_ts: i64,
    pub validation_end_ts: i64,
    pub expected: Metrics,
    pub fixed_half: Metrics,
}

#[derive(Debug, Clone, Serialize)]
pub struct Metrics {
    pub brier: f64,
    pub auc: Option<f64>,
    pub ece: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct GroupReport {
    pub dimension: &'static str,
    pub key: String,
    pub samples: usize,
    pub status: &'static str,
    pub expected: Metrics,
    pub fixed_half: Metrics,
}

#[derive(Debug, Clone, Serialize)]
pub struct OutcomeEvaluationReport {
    pub method: &'static str,
    pub ece_bins: usize,
    pub ece_definition: &'static str,
    pub minimum_report_samples: usize,
    pub excluded_rows: usize,
    pub embargoed_rows: usize,
    pub folds: Vec<FoldSummary>,
    pub groups: Vec<GroupReport>,
    pub hero_score_validation: &'static str,
}

struct TimeComponent<'a> {
    groups: Vec<Vec<&'a TrainingRow>>,
    min_ts: i64,
    max_ts: i64,
}

pub fn evaluate_outcome_baselines(rows: &[TrainingRow]) -> OutcomeEvaluationReport {
    let excluded_rows = rows.len() - rows.iter().filter(|r| valid(r)).count();
    let mut role_events: BTreeMap<(&str, &str), &TrainingRow> = BTreeMap::new();
    for row in rows.iter().filter(|row| valid(row)) {
        role_events
            .entry((&row.match_group, &row.role))
            .and_modify(|selected| {
                if segment_priority(row) > segment_priority(selected) {
                    *selected = row;
                }
            })
            .or_insert(row);
    }
    let valid_rows: Vec<&TrainingRow> = role_events.into_values().collect();
    let mut matches: BTreeMap<&str, Vec<&TrainingRow>> = BTreeMap::new();
    for row in &valid_rows {
        matches.entry(&row.match_group).or_default().push(row);
    }
    let mut matches: Vec<Vec<&TrainingRow>> = matches.into_values().collect();
    matches.sort_by_key(|group| {
        (
            group.iter().map(|r| r.begin_ts).min().unwrap(),
            group[0].match_group.as_str(),
        )
    });
    let mut components: Vec<TimeComponent<'_>> = Vec::new();
    for group in matches {
        let min_ts = group.iter().map(|r| r.begin_ts).min().unwrap();
        let max_ts = group.iter().map(|r| r.begin_ts).max().unwrap();
        if let Some(component) = components.last_mut().filter(|c| min_ts <= c.max_ts) {
            component.max_ts = component.max_ts.max(max_ts);
            component.groups.push(group);
        } else {
            components.push(TimeComponent {
                groups: vec![group],
                min_ts,
                max_ts,
            });
        }
    }
    let boundaries = six_group_blocks(components.len());
    let mut folds = Vec::new();
    let mut validation = Vec::new();
    if let Some(bounds) = boundaries {
        for fold in 0..5 {
            let train_components = &components[..bounds[fold + 1]];
            let validation_components = &components[bounds[fold + 1]..bounds[fold + 2]];
            let train = train_components
                .iter()
                .flat_map(|c| c.groups.iter())
                .map(Vec::len)
                .sum();
            let mut selected: Vec<&TrainingRow> = validation_components
                .iter()
                .flat_map(|c| c.groups.iter())
                .flatten()
                .copied()
                .collect();
            selected.sort_by_key(|r| r.begin_ts);
            if selected.is_empty() {
                continue;
            }
            let train_end_ts = train_components.iter().map(|c| c.max_ts).max().unwrap();
            let expected = metrics(&selected, |r| r.expected_win_rate);
            let fixed_half = metrics(&selected, |_| 0.5);
            folds.push(FoldSummary {
                fold: fold + 1,
                train_samples: train,
                validation_samples: selected.len(),
                validation_match_groups: validation_components.iter().map(|c| c.groups.len()).sum(),
                train_end_ts,
                validation_start_ts: validation_components.first().unwrap().min_ts,
                validation_end_ts: validation_components.last().unwrap().max_ts,
                expected,
                fixed_half,
            });
            validation.extend(selected);
        }
    }
    let mut grouped: BTreeMap<(&'static str, String), Vec<&TrainingRow>> = BTreeMap::new();
    if !validation.is_empty() {
        grouped.insert(("overall", "all".into()), validation.clone());
    }
    for row in validation {
        grouped
            .entry(("role", row.role.clone()))
            .or_default()
            .push(row);
        grouped
            .entry(("mode", mode_key(row.mode_family).into()))
            .or_default()
            .push(row);
    }
    let groups = grouped
        .into_iter()
        .map(|((dimension, key), values)| group_report(dimension, key, &values))
        .collect();
    OutcomeEvaluationReport {
        method: "five strictly forward chronological folds over non-overlapping match-time components; tied timestamps and complete match [min,max] intervals stay intact; metrics use validation windows only",
        ece_bins: ECE_BINS,
        ece_definition: "10 equal-width probability bins: [0.0,0.1), [0.1,0.2), ..., [0.9,1.0]; ECE is sample-weighted absolute confidence-outcome gap",
        minimum_report_samples: MIN_REPORT_SAMPLES,
        excluded_rows,
        embargoed_rows: 0,
        folds,
        groups,
        hero_score_validation: "S22 official hero scores remain external validation data; hero-level performance weights have not been fitted yet, so correlation is not computable",
    }
}

fn valid(row: &TrainingRow) -> bool {
    row.begin_ts >= 0
        && row.result.is_finite()
        && matches!(row.result, 0.0 | 0.5 | 1.0)
        && row.expected_win_rate.is_finite()
        && (0.0..=1.0).contains(&row.expected_win_rate)
}

fn segment_priority(row: &TrainingRow) -> (bool, i64) {
    (
        row.allow_performance,
        row.hero_duration_sec.unwrap_or(0.0).round() as i64,
    )
}

fn six_group_blocks(group_count: usize) -> Option<[usize; 7]> {
    if group_count < 6 {
        return None;
    }
    let base = group_count / 6;
    let remainder = group_count % 6;
    let mut indexes = [0usize; 7];
    for block in 0..6 {
        indexes[block + 1] = indexes[block] + base + usize::from(block < remainder);
    }
    Some(indexes)
}

fn group_report(dimension: &'static str, key: String, rows: &[&TrainingRow]) -> GroupReport {
    let status = if rows.len() < MIN_REPORT_SAMPLES {
        "insufficient_sample"
    } else if !has_both_binary_classes(rows) {
        "auc_unavailable_single_class"
    } else {
        "ok"
    };
    GroupReport {
        dimension,
        key,
        samples: rows.len(),
        status,
        expected: metrics(rows, |r| r.expected_win_rate),
        fixed_half: metrics(rows, |_| 0.5),
    }
}

fn metrics(rows: &[&TrainingRow], probability: impl Fn(&TrainingRow) -> f64) -> Metrics {
    let predictions: Vec<(f64, f64)> = rows.iter().map(|r| (probability(r), r.result)).collect();
    let n = predictions.len() as f64;
    let brier = predictions
        .iter()
        .map(|(p, y)| (p - y).powi(2))
        .sum::<f64>()
        / n;
    let mut bins = vec![(0usize, 0.0, 0.0); ECE_BINS];
    for &(p, y) in &predictions {
        let index = ((p * ECE_BINS as f64).floor() as usize).min(ECE_BINS - 1);
        bins[index].0 += 1;
        bins[index].1 += p;
        bins[index].2 += y;
    }
    let ece = bins
        .into_iter()
        .filter(|b| b.0 > 0)
        .map(|(count, ps, ys)| (count as f64 / n) * ((ps - ys) / count as f64).abs())
        .sum();
    Metrics {
        brier,
        auc: auc(&predictions),
        ece,
    }
}

fn has_both_binary_classes(rows: &[&TrainingRow]) -> bool {
    rows.iter().any(|r| r.result == 0.0) && rows.iter().any(|r| r.result == 1.0)
}

fn auc(values: &[(f64, f64)]) -> Option<f64> {
    rank_sum_auc(values)
}

fn rank_sum_auc(values: &[(f64, f64)]) -> Option<f64> {
    let mut binary = values
        .iter()
        .filter(|(_, result)| *result == 0.0 || *result == 1.0)
        .copied()
        .collect::<Vec<_>>();
    let positives = binary.iter().filter(|(_, result)| *result == 1.0).count();
    let negatives = binary.len() - positives;
    if positives == 0 || negatives == 0 {
        return None;
    }
    binary.sort_by(|left, right| left.0.total_cmp(&right.0));

    let mut positive_rank_sum = 0.0;
    let mut start = 0;
    while start < binary.len() {
        let mut end = start + 1;
        while end < binary.len() && binary[end].0 == binary[start].0 {
            end += 1;
        }
        let average_rank = ((start + 1 + end) as f64) / 2.0;
        let positive_count = binary[start..end]
            .iter()
            .filter(|(_, result)| *result == 1.0)
            .count();
        positive_rank_sum += average_rank * positive_count as f64;
        start = end;
    }

    let positive_pairs = positives * (positives + 1) / 2;
    Some((positive_rank_sum - positive_pairs as f64) / (positives * negatives) as f64)
}

fn mode_key(mode: ModeFamily) -> &'static str {
    mode.as_str()
}

#[cfg(test)]
mod auc_tests {
    use super::auc;

    fn naive_auc(values: &[(f64, f64)]) -> Option<f64> {
        let positives = values
            .iter()
            .filter(|(_, result)| *result == 1.0)
            .map(|(prediction, _)| *prediction)
            .collect::<Vec<_>>();
        let negatives = values
            .iter()
            .filter(|(_, result)| *result == 0.0)
            .map(|(prediction, _)| *prediction)
            .collect::<Vec<_>>();
        if positives.is_empty() || negatives.is_empty() {
            return None;
        }
        let score = positives
            .iter()
            .flat_map(|positive| {
                negatives.iter().map(move |negative| {
                    if positive > negative {
                        1.0
                    } else if positive == negative {
                        0.5
                    } else {
                        0.0
                    }
                })
            })
            .sum::<f64>();
        Some(score / (positives.len() * negatives.len()) as f64)
    }

    #[test]
    fn mixed_prediction_ties_receive_average_rank_credit() {
        let values = [
            (0.1, 0.0),
            (0.2, 1.0),
            (0.2, 0.0),
            (0.2, 1.0),
            (0.7, 0.0),
            (0.9, 1.0),
            (0.5, 0.5),
        ];
        assert_eq!(auc(&values), Some(2.0 / 3.0));
    }

    #[test]
    fn rank_sum_auc_matches_naive_pairwise_auc_on_small_samples() {
        for seed in 0..32_u64 {
            let values = (0..37)
                .map(|index| {
                    let prediction = ((index as u64 * 17 + seed * 11) % 9) as f64 / 8.0;
                    let result = if index % 7 == 0 {
                        0.5
                    } else {
                        ((index as u64 * 5 + seed) % 2) as f64
                    };
                    (prediction, result)
                })
                .collect::<Vec<_>>();
            assert_eq!(auc(&values), naive_auc(&values), "seed={seed}");
        }
    }
}
