use crate::ai_roast::store::roast_for_match;
use crate::db::OwDatabase;
use crate::importer::{int_field, rebuild_match_players_from_existing_details, string_field};
use crate::match_favorites::favorite_for_match;
use crate::models::{
    ArenaDetailSlice, ArenaMatchDetail, ComparisonMetrics, FilterOption, FilterOptions, HeroStat,
    MatchCursor, MatchDetailView, MatchEvaluationSummary, MatchFilters, MatchPage,
    MatchPerformanceAward, MatchSummary, PlayerPerk, PlayerRow, PlayerSearchOption,
    SameRoleComparison,
};
use anyhow::{Context, Result};
use chrono::{DateTime, Local, NaiveDateTime, TimeZone};
use rusqlite::{params, OptionalExtension, ToSql};
use serde_json::{json, Value};
use std::collections::HashMap;

mod arena_detail;
mod lookups;
mod match_list;
mod normal_detail;
mod party_groups;

use arena_detail::*;
use lookups::*;
use match_list::*;
use normal_detail::*;
use party_groups::*;

pub fn list_matches(db: &OwDatabase, filters: MatchFilters) -> Result<Vec<MatchSummary>> {
    list_matches_impl(db, filters, None)
}

pub fn list_matches_page(
    db: &OwDatabase,
    filters: MatchFilters,
    cursor: Option<MatchCursor>,
    limit: usize,
) -> Result<MatchPage> {
    let limit = limit.clamp(1, 100);
    let total = cursor
        .is_none()
        .then(|| count_matches_impl(db, &filters))
        .transpose()?;
    let mut matches = list_matches_impl(db, filters, Some((cursor, limit + 1)))?;
    let has_more = matches.len() > limit;
    matches.truncate(limit);
    let next_cursor = has_more.then(|| {
        let last = matches.last().expect("a non-empty limited page");
        MatchCursor {
            begin_ts: last.begin_ts,
            match_id: last.match_id.clone(),
        }
    });
    Ok(MatchPage {
        matches,
        next_cursor,
        total,
        revision_token: String::new(),
        account_scope: String::new(),
    })
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
