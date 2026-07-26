const SETTINGS_SOURCE: &str = include_str!("../src/settings.rs");

#[test]
fn settings_module_exposes_the_global_mode_order_seam() {
    assert!(SETTINGS_SOURCE.contains("pub const DEFAULT_PLAYER_QUERY_MODE_ORDER"));
    assert!(SETTINGS_SOURCE.contains("pub fn normalize_player_query_mode_order"));
}

#[test]
fn mode_order_preference_does_not_name_query_session_data() {
    let seam = SETTINGS_SOURCE
        .split("pub const DEFAULT_PLAYER_QUERY_MODE_ORDER")
        .nth(1)
        .and_then(|source| source.split("pub fn get_player_profile_for_path").next())
        .expect("player-query settings seam should remain before profile persistence");

    for forbidden in [
        "battle_tag",
        "season",
        "current_mode",
        "query_data",
        "session_id",
    ] {
        assert!(
            !seam.contains(forbidden),
            "global mode order must not persist {forbidden}"
        );
    }
}
