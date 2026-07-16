use crate::db::OwDatabase;
use crate::importer::{int_field, rebuild_match_players_from_existing_details, string_field};
use crate::match_favorites::favorite_for_match;
use crate::models::{
    ArenaDetailSlice, ArenaMatchDetail, ComparisonMetrics, FilterOption, FilterOptions, HeroStat,
    MatchDetailView, MatchEvaluationSummary, MatchFilters, MatchPerformanceAward, MatchSummary,
    PlayerPerk, PlayerRow, PlayerSearchOption, SameRoleComparison,
};
use crate::redaction::redact_sensitive_json;
use anyhow::{Context, Result};
use chrono::{DateTime, Local, NaiveDateTime, TimeZone};
use rusqlite::{params, OptionalExtension, ToSql};
use serde_json::{json, Value};
use std::collections::HashMap;

mod arena_detail;
mod lookups;
mod match_list;
mod normal_detail;

use arena_detail::*;
use lookups::*;
use match_list::*;
use normal_detail::*;

pub fn list_matches(db: &OwDatabase, filters: MatchFilters) -> Result<Vec<MatchSummary>> {
    list_matches_impl(db, filters)
}

pub fn list_filter_options(db: &OwDatabase) -> Result<FilterOptions> {
    list_filter_options_impl(db)
}

pub fn search_match_players(db: &OwDatabase, query: &str) -> Result<Vec<PlayerSearchOption>> {
    search_match_players_impl(db, query)
}

pub fn get_match_detail(db: &OwDatabase, match_id: &str) -> Result<MatchDetailView> {
    get_match_detail_impl(db, match_id)
}
