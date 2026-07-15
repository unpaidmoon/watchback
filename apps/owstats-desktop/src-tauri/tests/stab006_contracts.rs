use owstats_desktop_lib::{
    get_friend_analytics, get_onboarding_status, get_personal_season_review, list_matches,
    FriendAnalyticsFilters, MatchFilters, OnboardingStatusRequest, OwDatabase,
    PersonalReviewRequest,
};

#[test]
fn public_query_and_review_dtos_keep_dynamic_season_contracts_on_an_empty_database() {
    let temp = tempfile::tempdir().unwrap();
    let db = OwDatabase::new(temp.path().join("owstats.sqlite3"));

    let matches = list_matches(&db, MatchFilters::default()).unwrap();
    let review =
        get_personal_season_review(&db, PersonalReviewRequest { season: Some(22) }).unwrap();
    let friends = get_friend_analytics(&db, FriendAnalyticsFilters::default()).unwrap();
    let onboarding =
        get_onboarding_status(&db, OnboardingStatusRequest { season: Some(22) }).unwrap();

    assert!(matches.is_empty());
    assert_eq!(review.summary.season, Some(22));
    assert!(friends.boards.iter().all(|board| board.friends.is_empty()));
    assert_eq!(
        onboarding
            .season_statuses
            .iter()
            .map(|status| status.season)
            .collect::<Vec<_>>(),
        vec![22]
    );
    let json = serde_json::to_value(&onboarding).unwrap();
    assert!(json.get("seasonStatuses").is_some());
    assert!(json.get("credentialStatus").is_some());
}
