use std::collections::HashSet;

use thiserror::Error;

use super::{
    report::{
        evidence_catalog, ActorImpact, AiRoastNarrative, AiRoastReport, ClaimType, EvidenceCatalog,
        EvidenceEntry, EvidenceSemantic, EvidenceValue, RoastClaim, RoastInsight,
    },
    snapshot::RoastSnapshot,
};

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum RoastValidationError {
    #[error("JSON无效")]
    InvalidJson,
    #[error("缺字段: {0}")]
    MissingField(&'static str),
    #[error("未知字段")]
    ExtraFields,
    #[error("报告区块数量无效")]
    InvalidSectionCount,
    #[error("报告字段为空")]
    EmptyField,
    #[error("报告文本过长")]
    TextTooLong,
    #[error("证据引用不存在")]
    UnknownEvidence,
    #[error("证据引用重复")]
    DuplicateEvidence,
    #[error("数字无事实依据")]
    UnsupportedNumber,
    #[error("无依据的断言")]
    UnsupportedClaim,
    #[error("攻击性表述")]
    Unfriendly,
    #[error("不得包含身份 ID 或凭据")]
    RealIdentity,
}

pub fn validate_report_output(
    input: &str,
    snapshot: &RoastSnapshot,
) -> Result<AiRoastReport, RoastValidationError> {
    let value: serde_json::Value = serde_json::from_str(normalize_json_input(input))
        .map_err(|_| RoastValidationError::InvalidJson)?;
    let object = value.as_object().ok_or(RoastValidationError::InvalidJson)?;
    for field in [
        "verdict",
        "decisiveFactors",
        "currentPlayer",
        "teamAndMatchups",
        "closingRoast",
    ] {
        if !object.contains_key(field) {
            return Err(RoastValidationError::MissingField(field));
        }
    }
    if object.len() != 5 {
        return Err(RoastValidationError::ExtraFields);
    }
    let report: AiRoastReport =
        serde_json::from_value(value).map_err(|_| RoastValidationError::InvalidJson)?;
    if !(2..=3).contains(&report.decisive_factors.len())
        || !(1..=3).contains(&report.team_and_matchups.len())
    {
        return Err(RoastValidationError::InvalidSectionCount);
    }
    let insights = report
        .decisive_factors
        .iter()
        .chain(std::iter::once(&report.current_player))
        .chain(report.team_and_matchups.iter().map(|item| &item.insight))
        .collect::<Vec<_>>();
    let visible = std::iter::once(report.verdict.title.as_str())
        .chain(std::iter::once(report.verdict.summary.as_str()))
        .chain(
            insights
                .iter()
                .flat_map(|item| [item.title.as_str(), item.commentary.as_str()]),
        )
        .chain(std::iter::once(report.closing_roast.as_str()))
        .collect::<Vec<_>>();
    if visible.iter().any(|text| text.trim().is_empty()) {
        return Err(RoastValidationError::EmptyField);
    }
    if report.verdict.title.chars().count() > 24
        || report.verdict.summary.chars().count() > 100
        || report.closing_roast.chars().count() > 100
        || insights
            .iter()
            .any(|item| item.title.chars().count() > 24 || item.commentary.chars().count() > 160)
        || visible
            .iter()
            .map(|text| text.chars().count())
            .sum::<usize>()
            > 600
    {
        return Err(RoastValidationError::TextTooLong);
    }
    if visible.iter().any(|text| contains_unfriendly(text)) {
        return Err(RoastValidationError::Unfriendly);
    }
    if visible
        .iter()
        .any(|text| contains_identity_or_credential(text))
    {
        return Err(RoastValidationError::RealIdentity);
    }
    let catalog = evidence_catalog(snapshot);
    for insight in &insights {
        validate_insight(insight, &catalog, snapshot)?;
    }
    validate_current_player(&report.current_player, snapshot)?;
    if [
        &report.verdict.title,
        &report.verdict.summary,
        &report.closing_roast,
    ]
    .iter()
    .any(|text| !numeric_mentions_for_snapshot(text, snapshot).is_empty())
    {
        return Err(RoastValidationError::UnsupportedNumber);
    }
    Ok(report)
}

pub fn validate_narrative_output(
    input: &str,
    snapshot: &RoastSnapshot,
) -> Result<AiRoastNarrative, RoastValidationError> {
    let report: AiRoastNarrative = serde_json::from_str(normalize_json_input(input))
        .map_err(|_| RoastValidationError::InvalidJson)?;
    if report.title.trim().is_empty() || report.commentary.trim().is_empty() {
        return Err(RoastValidationError::EmptyField);
    }
    if report.title.chars().count() > 36 || report.commentary.chars().count() > 650 {
        return Err(RoastValidationError::TextTooLong);
    }
    if report.analysis.supporting_factors.len() > 2 || report.analysis.key_actors.len() > 3 {
        return Err(RoastValidationError::InvalidSectionCount);
    }
    if report
        .analysis
        .key_actors
        .iter()
        .filter(|actor| actor.impact == ActorImpact::Negative)
        .count()
        > 2
    {
        return Err(RoastValidationError::InvalidSectionCount);
    }
    let catalog = evidence_catalog(snapshot);
    validate_narrative_thesis(&report.analysis.match_thesis, &catalog, snapshot)?;
    validate_narrative_claim(&report.analysis.current_player, &catalog, snapshot)?;
    if !report
        .analysis
        .current_player
        .evidence_refs
        .iter()
        .any(|path| path.starts_with(&format!("{}.", snapshot.current_player)))
    {
        return Err(RoastValidationError::UnsupportedClaim);
    }
    for actor in &report.analysis.key_actors {
        if !snapshot
            .scoreboard
            .team
            .iter()
            .chain(&snapshot.scoreboard.opponent)
            .any(|player| player.subject == actor.subject)
        {
            return Err(RoastValidationError::UnsupportedClaim);
        }
        validate_narrative_claim(&actor.claim, &catalog, snapshot)?;
    }
    for factor in &report.analysis.supporting_factors {
        validate_narrative_claim(factor, &catalog, snapshot)?;
    }
    let claims = std::iter::once(&report.analysis.match_thesis)
        .chain(std::iter::once(&report.analysis.current_player))
        .chain(report.analysis.key_actors.iter().map(|actor| &actor.claim))
        .chain(report.analysis.supporting_factors.iter())
        .collect::<Vec<_>>();
    let visible = format!("{} {}", report.title, report.commentary);
    if contains_unfriendly(&visible) {
        return Err(RoastValidationError::Unfriendly);
    }
    if contains_identity_or_credential(&visible) {
        return Err(RoastValidationError::RealIdentity);
    }
    if contains_unsupported_process_claim(&visible) {
        return Err(RoastValidationError::UnsupportedClaim);
    }
    let visible_entries = claims
        .iter()
        .flat_map(|claim| claim.evidence_refs.iter())
        .filter_map(|path| catalog.get(path))
        .collect::<Vec<_>>();
    validate_comparison(&visible, &visible_entries, &catalog)?;
    for mention in numeric_mentions_for_snapshot(&visible, snapshot) {
        if !visible_entries
            .iter()
            .any(|entry| supports_number(&entry.value, mention))
        {
            return Err(RoastValidationError::UnsupportedNumber);
        }
    }
    let planned_subjects = claims
        .iter()
        .flat_map(|claim| claim.evidence_refs.iter())
        .filter_map(|path| path.split('.').next())
        .collect::<HashSet<_>>();
    for player in snapshot
        .scoreboard
        .team
        .iter()
        .chain(&snapshot.scoreboard.opponent)
    {
        if visible.contains(&player.subject)
            && player.subject != snapshot.current_player
            && !planned_subjects.contains(player.subject.as_str())
            && !report
                .analysis
                .key_actors
                .iter()
                .any(|actor| actor.subject == player.subject)
        {
            return Err(RoastValidationError::UnsupportedClaim);
        }
    }
    Ok(report)
}

fn validate_narrative_thesis(
    claim: &RoastClaim,
    catalog: &EvidenceCatalog,
    snapshot: &RoastSnapshot,
) -> Result<(), RoastValidationError> {
    let dimensions = claim
        .evidence_refs
        .iter()
        .map(|path| {
            if path.starts_with("taskKnowledge.") || path.starts_with("gameKnowledge.") {
                "knowledge"
            } else if path.starts_with("roleMatchups.") {
                "role_matchup"
            } else {
                path.split('.').next().unwrap_or("unknown")
            }
        })
        .collect::<HashSet<_>>();
    if claim.evidence_refs.len() < 2 || dimensions.len() < 2 {
        return Err(RoastValidationError::UnsupportedClaim);
    }
    validate_narrative_claim(claim, catalog, snapshot)
}

fn validate_narrative_claim(
    claim: &RoastClaim,
    catalog: &EvidenceCatalog,
    snapshot: &RoastSnapshot,
) -> Result<(), RoastValidationError> {
    if claim.judgment.trim().is_empty() {
        return Err(RoastValidationError::EmptyField);
    }
    let unique = claim.evidence_refs.iter().collect::<HashSet<_>>();
    if unique.len() != claim.evidence_refs.len() {
        return Err(RoastValidationError::DuplicateEvidence);
    }
    if claim.evidence_refs.is_empty()
        || claim
            .evidence_refs
            .iter()
            .any(|path| !catalog.contains_key(path))
    {
        return Err(RoastValidationError::UnknownEvidence);
    }
    let entries = claim
        .evidence_refs
        .iter()
        .filter_map(|path| catalog.get(path))
        .collect::<Vec<_>>();
    let has_knowledge = entries
        .iter()
        .any(|entry| entry.semantic == EvidenceSemantic::GameKnowledge);
    match claim.claim_type {
        ClaimType::GameContextInference
            if !has_knowledge
                || !entries
                    .iter()
                    .any(|entry| entry.semantic != EvidenceSemantic::GameKnowledge) =>
        {
            return Err(RoastValidationError::UnsupportedClaim)
        }
        ClaimType::DirectFact | ClaimType::StatisticalInterpretation if has_knowledge => {
            return Err(RoastValidationError::UnsupportedClaim)
        }
        _ => {}
    }
    validate_comparison(&claim.judgment, &entries, catalog)?;
    if contains_unsupported_process_claim(&claim.judgment)
        || contains_unsupported_causation(&claim.judgment)
    {
        return Err(RoastValidationError::UnsupportedClaim);
    }
    for mention in numeric_mentions_for_snapshot(&claim.judgment, snapshot) {
        if !entries
            .iter()
            .any(|entry| supports_number(&entry.value, mention))
        {
            return Err(RoastValidationError::UnsupportedNumber);
        }
    }
    Ok(())
}

fn validate_insight(
    insight: &RoastInsight,
    catalog: &EvidenceCatalog,
    snapshot: &RoastSnapshot,
) -> Result<(), RoastValidationError> {
    let unique = insight.evidence_refs.iter().collect::<HashSet<_>>();
    if unique.len() != insight.evidence_refs.len() {
        return Err(RoastValidationError::DuplicateEvidence);
    }
    if insight.evidence_refs.is_empty()
        || insight
            .evidence_refs
            .iter()
            .any(|path| !catalog.contains_key(path))
    {
        return Err(RoastValidationError::UnknownEvidence);
    }
    let entries = insight
        .evidence_refs
        .iter()
        .filter_map(|path| catalog.get(path))
        .collect::<Vec<_>>();
    let has_knowledge = entries
        .iter()
        .any(|entry| entry.semantic == EvidenceSemantic::GameKnowledge);
    match insight.claim_type {
        ClaimType::GameContextInference
            if !has_knowledge
                || !entries
                    .iter()
                    .any(|entry| entry.semantic != EvidenceSemantic::GameKnowledge) =>
        {
            return Err(RoastValidationError::UnsupportedClaim)
        }
        ClaimType::DirectFact | ClaimType::StatisticalInterpretation if has_knowledge => {
            return Err(RoastValidationError::UnsupportedClaim)
        }
        _ => {}
    }
    let text = format!("{} {}", insight.title, insight.commentary);
    validate_comparison(&text, &entries, catalog)?;
    if contains_unsupported_causation(&text) || violates_role_context(&text, insight, snapshot) {
        return Err(RoastValidationError::UnsupportedClaim);
    }
    for mention in numeric_mentions_for_snapshot(&text, snapshot) {
        if !entries
            .iter()
            .any(|entry| supports_number(&entry.value, mention))
        {
            return Err(RoastValidationError::UnsupportedNumber);
        }
    }
    Ok(())
}

fn numeric_mentions_for_snapshot(text: &str, snapshot: &RoastSnapshot) -> Vec<Mention> {
    numeric_mentions(&strip_known_numeric_labels(text, snapshot))
}

pub(crate) fn has_ascii_number_for_diagnostic(text: &str, snapshot: &RoastSnapshot) -> bool {
    strip_known_numeric_labels(text, snapshot)
        .chars()
        .any(|character| character.is_ascii_digit())
}

fn strip_known_numeric_labels(text: &str, snapshot: &RoastSnapshot) -> String {
    let mut claim = text.to_string();
    let mut labels = Vec::new();
    for player in snapshot
        .scoreboard
        .team
        .iter()
        .chain(&snapshot.scoreboard.opponent)
    {
        labels.push(player.subject.as_str());
        labels.push(player.hero.as_str());
        claim = claim.replace(&player.subject, "").replace(&player.hero, "");
    }
    if let Some(map) = snapshot.match_context.map.as_deref() {
        labels.push(map);
        claim = claim.replace(map, "");
    }
    for label in labels {
        let mut digits = String::new();
        for character in label.chars().chain(std::iter::once(' ')) {
            if character.is_ascii_digit() {
                digits.push(character);
            } else if !digits.is_empty() {
                claim = claim.replace(&digits, "");
                digits.clear();
            }
        }
    }
    claim
}

fn validate_current_player(
    insight: &RoastInsight,
    snapshot: &RoastSnapshot,
) -> Result<(), RoastValidationError> {
    let Some(player) = snapshot
        .scoreboard
        .team
        .iter()
        .chain(&snapshot.scoreboard.opponent)
        .find(|player| player.subject == snapshot.current_player)
    else {
        return Ok(());
    };
    if player.derived.team_objective_time_rank == Some(1) {
        let objective_path = format!("{}.derived.teamObjectiveTimeRank", player.subject);
        if !insight
            .evidence_refs
            .iter()
            .any(|path| path == &objective_path)
        {
            return Err(RoastValidationError::UnsupportedClaim);
        }
        let combat_bottom = [
            player.derived.team_kills_rank,
            player.derived.team_damage_rank,
            player.derived.team_final_hits_rank,
        ]
        .iter()
        .all(|rank| rank.is_some_and(|rank| rank >= 5));
        if combat_bottom
            && ["免责", "不算问题", "足以洗白"]
                .iter()
                .any(|word| insight.commentary.contains(word))
        {
            return Err(RoastValidationError::UnsupportedClaim);
        }
    }
    Ok(())
}

fn violates_role_context(text: &str, insight: &RoastInsight, snapshot: &RoastSnapshot) -> bool {
    if !["失职", "拖累", "背锅", "问题"]
        .iter()
        .any(|word| text.contains(word))
    {
        return false;
    }
    snapshot
        .scoreboard
        .team
        .iter()
        .chain(&snapshot.scoreboard.opponent)
        .any(|player| {
            if !insight
                .evidence_refs
                .iter()
                .any(|path| path.starts_with(&format!("{}.", player.subject)))
            {
                return false;
            }
            (player.hero == "禅雅塔"
                && insight
                    .evidence_refs
                    .iter()
                    .any(|path| path.contains("HealingRank"))
                && !insight
                    .evidence_refs
                    .iter()
                    .any(|path| path.starts_with("gameKnowledge.")))
                || (matches!(player.hero.as_str(), "天使" | "生命之梭")
                    && insight
                        .evidence_refs
                        .iter()
                        .any(|path| path.contains("DamageRank"))
                    && !insight
                        .evidence_refs
                        .iter()
                        .any(|path| path.starts_with("gameKnowledge.")))
        })
}

fn contains_unsupported_causation(text: &str) -> bool {
    text.contains("整局没打过")
        || (text.contains("因为")
            && text.contains("所以")
            && (text.contains("表现优秀")
                || text.contains("完成了翻盘")
                || text.contains("完成翻盘")))
}

fn contains_unsupported_process_claim(text: &str) -> bool {
    [
        "整局没打过",
        "从来没打",
        "拒绝换英雄",
        "故意送",
        "故意摆烂",
        "没交大",
        "不开麦",
    ]
    .iter()
    .any(|word| text.contains(word))
}

#[derive(Clone, Copy)]
pub(crate) struct Mention {
    pub(crate) value: f64,
    pub(crate) percent: bool,
    tolerance: f64,
}
pub(crate) fn numeric_mentions_for_diagnostic(
    text: &str,
    snapshot: &RoastSnapshot,
) -> Vec<Mention> {
    numeric_mentions_for_snapshot(text, snapshot)
}
fn numeric_mentions(text: &str) -> Vec<Mention> {
    let chars = text.chars().collect::<Vec<_>>();
    let mut out = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        if chars[i].is_ascii_digit() || ('０'..='９').contains(&chars[i]) {
            let start = i;
            while i < chars.len()
                && (chars[i].is_ascii_digit()
                    || ('０'..='９').contains(&chars[i])
                    || matches!(chars[i], '.' | ',' | '，'))
            {
                i += 1;
            }
            let raw = chars[start..i]
                .iter()
                .filter_map(|value| match value {
                    '０'..='９' => char::from_u32(*value as u32 - 0xff10 + 0x30),
                    ',' | '，' => None,
                    value => Some(*value),
                })
                .collect::<String>();
            if let Ok(mut value) = raw.parse::<f64>() {
                let multiplier = match chars.get(i) {
                    Some('万') => 10_000.0,
                    Some('千') => 1_000.0,
                    _ => 1.0,
                };
                let decimals =
                    raw.split_once('.')
                        .map_or(0, |(_, fraction)| fraction.len()) as i32;
                value *= multiplier;
                out.push(Mention {
                    value,
                    percent: chars
                        .get(i)
                        .is_some_and(|value| matches!(value, '%' | '％')),
                    tolerance: if multiplier > 1.0 {
                        multiplier * 0.5 * 10f64.powi(-decimals)
                    } else {
                        0.011
                    },
                });
            }
            continue;
        }
        if is_chinese_numeral(chars[i]) {
            let start = i;
            while i < chars.len() && is_chinese_numeral(chars[i]) {
                i += 1;
            }
            let previous = start.checked_sub(1).and_then(|index| chars.get(index));
            let next = chars.get(i);
            if previous != Some(&'第')
                && (previous.is_some_and(|value| is_number_marker(*value))
                    || next.is_some_and(|value| is_number_marker(*value)))
            {
                if let Some(value) = chinese_number(&chars[start..i]) {
                    out.push(Mention {
                        value: value as f64,
                        percent: false,
                        tolerance: 0.011,
                    });
                }
            }
            continue;
        }
        i += 1;
    }
    out
}
fn chinese_number(chars: &[char]) -> Option<i64> {
    let digit = |value| match value {
        '零' => Some(0),
        '一' => Some(1),
        '二' => Some(2),
        '三' => Some(3),
        '四' => Some(4),
        '五' => Some(5),
        '六' => Some(6),
        '七' => Some(7),
        '八' => Some(8),
        '九' => Some(9),
        _ => None,
    };
    let mut total = 0;
    let mut section = 0;
    let mut number = 0;
    for value in chars {
        match value {
            '十' | '百' | '千' => {
                let unit = match value {
                    '十' => 10,
                    '百' => 100,
                    _ => 1000,
                };
                section += if number == 0 { unit } else { number * unit };
                number = 0
            }
            '万' => {
                total += (section + number) * 10000;
                section = 0;
                number = 0
            }
            value => number = digit(*value)?,
        }
    }
    Some(total + section + number)
}
fn supports_number(value: &EvidenceValue, mention: Mention) -> bool {
    let values = match value {
        EvidenceValue::Integer(value) => vec![*value as f64],
        EvidenceValue::Decimal(value) => vec![*value],
        EvidenceValue::Text(text) => numeric_mentions(text)
            .into_iter()
            .map(|item| item.value)
            .collect(),
    };
    let tolerance = if mention.percent {
        0.501
    } else {
        mention.tolerance
    };
    values.into_iter().any(|value| {
        ((if mention.percent {
            value * 100.0
        } else {
            value
        }) - mention.value)
            .abs()
            < tolerance
    })
}

fn validate_comparison(
    text: &str,
    entries: &[&EvidenceEntry],
    catalog: &EvidenceCatalog,
) -> Result<(), RoastValidationError> {
    let max_worth = catalog
        .values()
        .filter(|entry| entry.semantic == EvidenceSemantic::FinalWorth)
        .filter_map(|entry| match entry.value {
            EvidenceValue::Integer(value) => Some(value),
            _ => None,
        })
        .max();
    let has_max_worth = entries.iter().any(|entry| {
        entry.semantic == EvidenceSemantic::FinalWorth
            && matches!(entry.value,EvidenceValue::Integer(value)if Some(value)==max_worth)
    });
    let money_superlative = (text.contains("资金") || text.contains("最有钱")) && has_max_worth;
    if ["最高", "第一"].iter().any(|word| text.contains(word))
        && !money_superlative
        && !entries.iter().any(|entry| {
            entry.semantic == EvidenceSemantic::Rank
                && matches!(entry.value, EvidenceValue::Integer(1))
        })
    {
        return Err(RoastValidationError::UnsupportedClaim);
    }
    if ["最低", "垫底"].iter().any(|word| text.contains(word))
        && !entries
            .iter()
            .any(|entry| entry.semantic == EvidenceSemantic::Rank)
    {
        return Err(RoastValidationError::UnsupportedClaim);
    }
    if ["领先", "落后", "高于", "低于"]
        .iter()
        .any(|word| text.contains(word))
    {
        let supported = entries.iter().any(|entry| {
            matches!(
                entry.semantic,
                EvidenceSemantic::Delta | EvidenceSemantic::Rank
            )
        }) || entries
            .iter()
            .filter(|entry| entry.semantic == EvidenceSemantic::RawStat)
            .count()
            >= 2;
        if !supported {
            return Err(RoastValidationError::UnsupportedClaim);
        }
    }
    if ["让三追四", "连赢", "决胜回合"]
        .iter()
        .any(|word| text.contains(word))
        && !entries
            .iter()
            .any(|entry| entry.semantic == EvidenceSemantic::RoundResult)
    {
        return Err(RoastValidationError::UnsupportedClaim);
    }
    if ["最有钱", "资金第一"]
        .iter()
        .any(|word| text.contains(word))
        && !has_max_worth
    {
        return Err(RoastValidationError::UnsupportedClaim);
    }
    Ok(())
}

pub(super) fn normalize_json_input(input: &str) -> &str {
    let trimmed = input.trim();
    let Some(value) = trimmed.strip_prefix("```") else {
        return trimmed;
    };
    let Some(value) = value.strip_suffix("```") else {
        return trimmed;
    };
    let value = value.trim();
    value
        .strip_prefix("json")
        .and_then(|value| value.strip_prefix(['\r', '\n']))
        .map(str::trim)
        .unwrap_or(value)
}
fn contains_unfriendly(text: &str) -> bool {
    ["菜狗", "废物", "蠢货", "闭嘴", "去死"]
        .iter()
        .any(|value| text.contains(value))
}
fn contains_identity_or_credential(text: &str) -> bool {
    let lower = text.to_ascii_lowercase();
    [
        "bnet",
        "battletag",
        "player_name",
        "玩家id",
        "真名",
        "token",
        "customertoken",
        "cookie",
        "authorization",
        "credential",
    ]
    .iter()
    .any(|value| lower.contains(value))
}
fn is_number_marker(value: char) -> bool {
    value == '杀'
}
fn is_chinese_numeral(value: char) -> bool {
    matches!(
        value,
        '零' | '一'
            | '二'
            | '三'
            | '四'
            | '五'
            | '六'
            | '七'
            | '八'
            | '九'
            | '十'
            | '百'
            | '千'
            | '万'
    )
}
