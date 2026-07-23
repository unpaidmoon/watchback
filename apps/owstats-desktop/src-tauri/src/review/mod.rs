use crate::db::OwDatabase;
use crate::models::*;
use anyhow::{Context, Result};
use rusqlite::params;
use serde_json::Value;
use std::collections::HashMap;

mod achievements;
mod command_center;
mod personal;
pub(crate) mod roles;

use achievements::*;
use command_center::*;
use personal::*;
use roles::*;

pub fn get_personal_season_review(
    db: &OwDatabase,
    request: PersonalReviewRequest,
) -> Result<PersonalSeasonReview> {
    get_personal_season_review_impl(db, request)
}

pub fn get_career_achievements(db: &OwDatabase) -> Result<Vec<CareerAchievement>> {
    get_career_achievements_impl(db)
}

pub fn get_onboarding_status(
    db: &OwDatabase,
    request: OnboardingStatusRequest,
) -> Result<OnboardingStatus> {
    get_onboarding_status_impl(db, request)
}
