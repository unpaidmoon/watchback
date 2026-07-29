use crate::models::PlayerGameAction;
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const PAGE_SIZE: usize = 12;

/// Response body returned by `customer/fight/queryCount`.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct FightCountData {
    pub role_type_count_data: Vec<Value>,
    pub hero_use_summary_list: Vec<Value>,
}

/// Response body returned by `customer/fight/getLeisureFightRoleCard`.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct LeisureFightRoleCardData {
    #[serde(rename = "roleDataList")]
    pub _role_data_list: Vec<Value>,
    pub hero_data_list: Vec<Value>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Season {
    S15,
    S20,
    S21,
    S22,
    S23,
}

impl Season {
    pub fn upstream(self) -> Option<i64> {
        match self {
            Self::S15 => Some(15),
            Self::S20 => Some(20),
            Self::S21 => Some(21),
            Self::S22 => Some(22),
            Self::S23 => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum QueryMode {
    LeisurePreset,
    Leisure6v6,
    SportPreset,
    SportOpen,
    SportFight,
    LeisureFight,
}

impl QueryMode {
    pub fn upstream(self) -> &'static str {
        match self {
            Self::LeisurePreset => "LeisurePreset",
            Self::Leisure6v6 => "Leisure6v6",
            Self::SportPreset => "SportPreset",
            Self::SportOpen => "SportOpen",
            Self::SportFight => "SportFight",
            Self::LeisureFight => "LeisureFight",
        }
    }
    pub fn kind(self) -> MatchKind {
        if matches!(self, Self::SportFight | Self::LeisureFight) {
            MatchKind::Arena
        } else {
            MatchKind::Normal
        }
    }
    /// The list/count APIs group normal modes into leisure and sport buckets.
    /// Fight endpoints retain their exact mode values and use a separate route.
    pub fn list_upstream(self) -> &'static str {
        match self {
            Self::LeisurePreset | Self::Leisure6v6 => "leisure",
            Self::SportPreset | Self::SportOpen => "sport",
            Self::SportFight => "SportFight",
            Self::LeisureFight => "LeisureFight",
        }
    }

    pub fn list_scope_key(self) -> &'static str {
        match self {
            Self::LeisurePreset | Self::Leisure6v6 => "normal:leisure",
            Self::SportPreset | Self::SportOpen => "normal:sport",
            Self::SportFight => "arena:SportFight",
            Self::LeisureFight => "arena:LeisureFight",
        }
    }

    pub fn count_upstream(self) -> &'static str {
        match self {
            Self::LeisurePreset | Self::Leisure6v6 => "leisure",
            Self::SportPreset | Self::SportOpen => "sport",
            Self::SportFight | Self::LeisureFight => "fight",
        }
    }

    pub fn hero_summary_key(self) -> &'static str {
        match self {
            Self::LeisurePreset | Self::SportPreset => "presetsHeroUseSummaryList",
            Self::Leisure6v6 => "v6HeroUseSummaryList",
            Self::SportOpen => "openHeroUseSummaryList",
            Self::SportFight | Self::LeisureFight => "heroList",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum MatchKind {
    Normal,
    Arena,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionStart {
    pub session_id: String,
    pub generation: u64,
    pub battle_tag: String,
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MemoryImage {
    pub mime_type: String,
    pub data_base64: String,
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlayerQueryCard {
    pub battle_tag: String,
    pub level: Option<u64>,
    pub game_time_seconds: Option<u64>,
    pub title: Option<String>,
    pub game_action: Option<PlayerGameAction>,
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlayerQueryCardImages {
    pub icon: Option<MemoryImage>,
    pub title_icon: Option<MemoryImage>,
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RankRow {
    pub role: String,
    pub rank: Option<String>,
    pub division: Option<i64>,
    pub wins: Option<u64>,
    pub games: Option<u64>,
    pub win_rate: Option<f64>,
    pub kda: Option<f64>,
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SeasonOverview {
    pub competitive: Vec<RankRow>,
    pub open_competitive: Vec<RankRow>,
    pub stadium: Vec<RankRow>,
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HeroUsage {
    pub hero_guid: String,
    pub hero_name: String,
    pub game_time_seconds: u64,
    pub wins: Option<u64>,
    pub games: Option<u64>,
    pub win_rate: Option<f64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MatchRow {
    pub match_id: String,
    pub match_kind: MatchKind,
    pub result: String,
    pub score: String,
    pub map_key: Option<String>,
    pub map_name: Option<String>,
    pub hero_key: Option<String>,
    pub hero_name: Option<String>,
    pub kills: Option<i64>,
    pub assists: Option<i64>,
    pub deaths: Option<i64>,
    pub played_at: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModePage {
    pub list_scope_key: String,
    pub page: u32,
    pub matches: Vec<MatchRow>,
    pub end_reached: bool,
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DetailPlayer {
    pub side: String,
    pub name: String,
    pub bnet_id: String,
    pub rank_label: Option<String>,
    pub rank_key: Option<String>,
    pub hero_guid: String,
    pub hero_name: String,
    pub kills: i64,
    pub assists: i64,
    pub deaths: i64,
    pub damage: i64,
    pub healing: i64,
    pub mitigation: i64,
    pub final_hits: i64,
    pub target_time_sec: i64,
    pub healing_taken: i64,
    pub damage_taken: i64,
    pub worth: Option<i64>,
    pub end_worth: Option<i64>,
    pub worth_max: bool,
    pub mod_guids: Vec<String>,
    pub trait_guids: Vec<String>,
    pub has_source_max_flags: bool,
    pub kill_max: bool,
    pub hero_damage_max: bool,
    pub cure_max: bool,
    pub resist_damage_max: bool,
    pub perks: Vec<DetailPerk>,
    pub hero_stats: Vec<DetailHeroStat>,
    pub party_group: Option<usize>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DetailPerk {
    pub guid: String,
    pub perk_level: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DetailHeroStat {
    pub guid: String,
    pub value: f64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ArenaDetailSlice {
    pub key: String,
    pub label: String,
    pub result: Option<String>,
    pub result_label: Option<String>,
    pub team_score: Option<i64>,
    pub opponent_score: Option<i64>,
    pub duration_sec: Option<i64>,
    pub begin_ts: Option<i64>,
    pub begin_label: Option<String>,
    pub map_guid: Option<String>,
    pub map_name: Option<String>,
    pub teammates: Vec<DetailPlayer>,
    pub enemies: Vec<DetailPlayer>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ArenaMatchDetail {
    pub summary: ArenaDetailSlice,
    pub rounds: Vec<ArenaDetailSlice>,
}

/// Same serialized shape consumed by the shared frontend MatchDetailContent.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MatchDetailView {
    pub match_id: String,
    pub game_mode: Option<String>,
    pub mode_label: Option<String>,
    pub result: Option<String>,
    pub result_label: Option<String>,
    pub map_guid: Option<String>,
    pub map_name: Option<String>,
    pub hero_guid: Option<String>,
    pub hero_name: Option<String>,
    pub begin_ts: Option<i64>,
    pub begin_label: Option<String>,
    pub team_score: Option<i64>,
    pub opponent_score: Option<i64>,
    pub duration_sec: Option<i64>,
    pub teammates: Vec<DetailPlayer>,
    pub enemies: Vec<DetailPlayer>,
    pub arena_detail: Option<ArenaMatchDetail>,
    pub evaluation: Option<Value>,
    pub team_awards: Vec<Value>,
    pub same_role_comparison: Option<Value>,
    pub favorite: Option<Value>,
    pub ai_roast: Option<Value>,
    #[serde(skip_serializing_if = "Value::is_null")]
    pub raw_json: Value,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", content = "message", rename_all = "camelCase")]
pub enum QueryError {
    InvalidInput(String),
    Busy(String),
    NotFound(String),
    SessionInvalid(String),
    Upstream(String),
    InvalidResponse(String),
}

pub type QueryResult<T> = Result<T, QueryError>;
