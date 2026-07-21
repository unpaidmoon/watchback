use super::{
    arena::ArenaRoastMaterial,
    material::{RoastFact, RoastMaterial},
};
use serde::Deserialize;
use std::collections::{HashMap, HashSet};
use thiserror::Error;

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct RoastOutput {
    pub award: String,
    pub commentary: String,
    #[serde(rename = "usedFactIds")]
    pub used_fact_ids: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct RoastValidationVocabulary {
    known_terms: HashSet<String>,
    allowed_terms: HashSet<String>,
}

impl RoastValidationVocabulary {
    pub fn new(known_terms: HashSet<String>, allowed_terms: HashSet<String>) -> Self {
        Self {
            known_terms,
            allowed_terms,
        }
    }
}

#[derive(Debug, Clone)]
pub struct RoastValidationContext {
    fact_texts: HashMap<String, String>,
    minimum_fact_count: usize,
    known_terms: HashSet<String>,
    allowed_terms: HashSet<String>,
    allowed_numeric_literals: HashSet<String>,
    requires_relationship_fact: bool,
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum RoastValidationError {
    #[error("JSON\u{65e0}\u{6548}")]
    InvalidJson,
    #[error("\u{7f3a}\u{5b57}\u{6bb5}: {0}")]
    MissingField(&'static str),
    #[error("\u{672a}\u{77e5}\u{5b57}\u{6bb5}")]
    ExtraFields,
    #[error("\u{5956}\u{9879}\u{4e3a}\u{7a7a}")]
    EmptyAward,
    #[error("\u{5956}\u{9879}\u{5fc5}\u{987b}\u{4e2d}\u{6587}\u{4e14}\u{4e0d}\u{8d85}16\u{5b57}")]
    AwardTooLongOrNotChinese,
    #[error("\u{8bc4}\u{8bba}\u{9700}2-3\u{6bb5}")]
    SentenceCount,
    #[error("\u{8bc4}\u{8bba}\u{8d85}\u{8fc7}180\u{5b57}")]
    CommentaryTooLong,
    #[error("\u{4e0d}\u{652f}\u{6301}\u{7684}\u{8f93}\u{51fa}\u{683c}\u{5f0f}")]
    UnsupportedFormat,
    #[error("\u{672a}\u{77e5}\u{540d}\u{79f0}\u{6216}\u{82f1}\u{96c4}")]
    UnknownTerm,
    #[error("\u{672a}\u{77e5}\u{4e8b}\u{5b9e} ID")]
    UnknownFact,
    #[error("\u{4e8b}\u{5b9e} ID\u{91cd}\u{590d}")]
    DuplicateFact,
    #[error("\u{4e8b}\u{5b9e}\u{4e0d}\u{8db3}")]
    InsufficientFacts,
    #[error("\u{7f3a}\u{5c11}\u{5173}\u{7cfb}\u{4e8b}\u{5b9e}")]
    MissingRelationshipFact,
    #[error("\u{6570}\u{5b57}\u{65e0}\u{4e8b}\u{5b9e}\u{4f9d}\u{636e}")]
    UnsupportedNumber,
    #[error("\u{65e0}\u{4f9d}\u{636e}\u{7684}\u{65ad}\u{8a00}")]
    UnsupportedClaim,
    #[error("\u{653b}\u{51fb}\u{6027}\u{8868}\u{8ff0}")]
    Unfriendly,
    #[error("\u{4e0d}\u{5f97}\u{5305}\u{542b}\u{8eab}\u{4efd} ID")]
    RealIdentity,
}

impl RoastValidationContext {
    pub fn for_match(material: &RoastMaterial, vocabulary: RoastValidationVocabulary) -> Self {
        let mut facts = material.selected_facts.clone();
        let requires_relationship_fact = material.relationship.is_some();
        if let Some(relationship) = &material.relationship {
            facts.extend(relationship.facts.clone());
            facts.extend(relationship.counter_facts.clone());
        }
        Self::from_facts(
            facts,
            vocabulary,
            material.streak.before_current.to_string(),
            requires_relationship_fact,
        )
    }

    pub fn for_arena(material: &ArenaRoastMaterial, vocabulary: RoastValidationVocabulary) -> Self {
        Self::from_facts(
            material.candidate_facts.clone(),
            vocabulary,
            String::new(),
            false,
        )
    }

    fn from_facts(
        facts: Vec<RoastFact>,
        vocabulary: RoastValidationVocabulary,
        streak: String,
        requires_relationship_fact: bool,
    ) -> Self {
        let fact_texts = facts
            .into_iter()
            .map(|fact| (fact.id, fact.text))
            .collect::<HashMap<_, _>>();
        let mut numeric_context = fact_texts.values().cloned().collect::<Vec<_>>();
        if !streak.is_empty() {
            numeric_context.push(streak);
        }
        Self {
            fact_texts,
            minimum_fact_count: 2,
            known_terms: vocabulary.known_terms,
            allowed_terms: vocabulary.allowed_terms,
            allowed_numeric_literals: numeric_literals(&numeric_context.join(" ")),
            requires_relationship_fact,
        }
    }
}

pub fn validate_roast_output(
    input: &str,
    context: &RoastValidationContext,
) -> Result<RoastOutput, RoastValidationError> {
    let value: serde_json::Value =
        serde_json::from_str(input).map_err(|_| RoastValidationError::InvalidJson)?;
    let object = value.as_object().ok_or(RoastValidationError::InvalidJson)?;
    for field in ["award", "commentary", "usedFactIds"] {
        if !object.contains_key(field) {
            return Err(RoastValidationError::MissingField(field));
        }
    }
    if object.len() != 3 {
        return Err(RoastValidationError::ExtraFields);
    }
    let output: RoastOutput =
        serde_json::from_value(value).map_err(|_| RoastValidationError::InvalidJson)?;
    if output.award.is_empty() {
        return Err(RoastValidationError::EmptyAward);
    }
    if output.award.chars().count() > 16 || !output.award.chars().all(is_han) {
        return Err(RoastValidationError::AwardTooLongOrNotChinese);
    }
    let commentary = output.commentary.trim();
    if commentary.is_empty() || !(2..=3).contains(&sentence_count(commentary)) {
        return Err(RoastValidationError::SentenceCount);
    }
    if commentary.chars().count() > 180 {
        return Err(RoastValidationError::CommentaryTooLong);
    }
    if commentary != output.commentary || contains_unsupported_format(commentary) {
        return Err(RoastValidationError::UnsupportedFormat);
    }

    let used = output.used_fact_ids.iter().collect::<HashSet<_>>();
    if used.len() != output.used_fact_ids.len() {
        return Err(RoastValidationError::DuplicateFact);
    }
    if used.iter().any(|id| !context.fact_texts.contains_key(*id)) {
        return Err(RoastValidationError::UnknownFact);
    }
    if used.len() < context.minimum_fact_count {
        return Err(RoastValidationError::InsufficientFacts);
    }
    if context.requires_relationship_fact
        && !used
            .iter()
            .any(|id| id.starts_with("relationship_subject:"))
    {
        return Err(RoastValidationError::MissingRelationshipFact);
    }
    if contains_disallowed_known_term(&output.award, context)
        || contains_disallowed_known_term(commentary, context)
        || quoted_unknown_term(commentary, context)
    {
        return Err(RoastValidationError::UnknownTerm);
    }
    if numeric_literals(commentary)
        .iter()
        .any(|number| !context.allowed_numeric_literals.contains(number))
    {
        return Err(RoastValidationError::UnsupportedNumber);
    }
    if contains_unsupported_claim(commentary) {
        return Err(RoastValidationError::UnsupportedClaim);
    }
    if contains_unfriendly_text(commentary) {
        return Err(RoastValidationError::Unfriendly);
    }
    if contains_identity_text(commentary) {
        return Err(RoastValidationError::RealIdentity);
    }
    Ok(output)
}

fn contains_unsupported_format(text: &str) -> bool {
    [
        "#",
        "- ",
        "* ",
        "```",
        "AI",
        "JSON",
        "usedFactIds",
        "\u{6a21}\u{578b}",
        "\u{63d0}\u{793a}\u{8bcd}",
    ]
    .iter()
    .any(|value| text.contains(value))
}

fn contains_unsupported_claim(text: &str) -> bool {
    [
        "\u{7edd}\u{5bf9}",
        "\u{5168}\u{573a}\u{6700}\u{5f3a}",
        "\u{552f}\u{4e00}",
        "\u{6beb}\u{65e0}\u{53cd}\u{4f8b}",
        "\u{6c38}\u{8fdc}",
        "\u{4ece}\u{4e0d}",
        "\u{767e}\u{5206}\u{767e}",
    ]
    .iter()
    .any(|value| text.contains(value))
}

fn contains_unfriendly_text(text: &str) -> bool {
    [
        "\u{83dc}\u{72d7}",
        "\u{5e9f}\u{7269}",
        "\u{8822}\u{8d27}",
        "\u{95ed}\u{5634}",
        "\u{53bb}\u{6b7b}",
    ]
    .iter()
    .any(|value| text.contains(value))
}

fn contains_identity_text(text: &str) -> bool {
    [
        "bnet",
        "BattleTag",
        "player_name",
        "\u{73a9}\u{5bb6}ID",
        "\u{771f}\u{540d}",
    ]
    .iter()
    .any(|value| text.contains(value))
}

fn sentence_count(text: &str) -> usize {
    let mut count = 0;
    let mut in_terminal = false;
    for character in text.chars() {
        if matches!(character, '\u{3002}' | '\u{ff01}' | '\u{ff1f}' | '!' | '?') {
            if !in_terminal {
                count += 1;
            }
            in_terminal = true;
        } else {
            in_terminal = false;
        }
    }
    count
}

fn quoted_unknown_term(text: &str, context: &RoastValidationContext) -> bool {
    let mut current = String::new();
    let mut quote_end = None;
    for character in text.chars() {
        if quote_end.is_none() {
            quote_end = match character {
                '\u{300c}' => Some('\u{300d}'),
                '\u{300e}' => Some('\u{300f}'),
                '\u{201c}' => Some('\u{201d}'),
                '"' => Some('"'),
                '\'' => Some('\''),
                _ => None,
            };
            if quote_end.is_some() {
                current.clear();
            }
            continue;
        }
        if Some(character) == quote_end {
            if !current.is_empty()
                && !context.allowed_terms.contains(&current)
                && !context
                    .fact_texts
                    .values()
                    .any(|fact| fact.contains(&current))
            {
                return true;
            }
            quote_end = None;
            current.clear();
        } else {
            current.push(character);
        }
    }
    false
}

fn contains_disallowed_known_term(text: &str, context: &RoastValidationContext) -> bool {
    context.known_terms.iter().any(|term| {
        !term.is_empty() && !context.allowed_terms.contains(term) && text.contains(term)
    })
}

fn numeric_literals(text: &str) -> HashSet<String> {
    let characters = text.chars().collect::<Vec<_>>();
    let mut output = HashSet::new();
    let mut index = 0;
    while index < characters.len() {
        if characters[index].is_ascii_digit()
            || ('\u{ff10}'..='\u{ff19}').contains(&characters[index])
        {
            let start = index;
            while index < characters.len()
                && (characters[index].is_ascii_digit()
                    || ('\u{ff10}'..='\u{ff19}').contains(&characters[index]))
            {
                index += 1;
            }
            output.insert(characters[start..index].iter().collect());
            continue;
        }
        if is_chinese_numeral(characters[index]) {
            let start = index;
            while index < characters.len() && is_chinese_numeral(characters[index]) {
                index += 1;
            }
            let previous = start
                .checked_sub(1)
                .and_then(|position| characters.get(position));
            let next = characters.get(index);
            if previous.is_some_and(|value| is_number_marker(*value))
                || next.is_some_and(|value| is_number_marker(*value))
            {
                output.insert(characters[start..index].iter().collect());
            }
            continue;
        }
        index += 1;
    }
    output
}

fn is_number_marker(character: char) -> bool {
    matches!(
        character,
        '\u{7b2c}'
            | '\u{540d}'
            | '\u{5c40}'
            | '\u{573a}'
            | '\u{4eba}'
            | '\u{6b21}'
            | '\u{8fde}'
            | '\u{8f6e}'
            | '\u{6bd4}'
            | '\u{5206}'
            | '\u{6740}'
    )
}

fn is_chinese_numeral(character: char) -> bool {
    matches!(
        character,
        '\u{96f6}'
            | '\u{4e00}'
            | '\u{4e8c}'
            | '\u{4e09}'
            | '\u{56db}'
            | '\u{4e94}'
            | '\u{516d}'
            | '\u{4e03}'
            | '\u{516b}'
            | '\u{4e5d}'
            | '\u{5341}'
            | '\u{767e}'
            | '\u{5343}'
            | '\u{4e07}'
    )
}

fn is_han(character: char) -> bool {
    matches!(
        character,
        '\u{3400}'..='\u{4dbf}' | '\u{4e00}'..='\u{9fff}' | '\u{f900}'..='\u{faff}'
    )
}
