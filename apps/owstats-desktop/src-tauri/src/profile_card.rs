use crate::account_context::AccountContextSnapshot;
use crate::dashen_client::{DashenClient, DashenCredentials as ClientCredentials};
use crate::models::{PlayerGameAction, PlayerProfile};
use anyhow::{anyhow, Result};

pub async fn refresh_for_account(
    account: &AccountContextSnapshot,
    root_base_url: &str,
) -> Result<PlayerProfile> {
    let credentials = crate::credentials::read_saved_credentials_for_account(account.paths())?;
    let target = crate::player_target::read_player_target_for_path(
        &crate::player_target::player_target_path_for_account(account.paths()),
    )?;
    if !crate::player_target::target_matches_login_role(&target, credentials.role_id) {
        return Err(anyhow!("当前账号与目标玩家不一致，请重新添加账号"));
    }

    let client = DashenClient::new(
        ClientCredentials {
            role_id: credentials.role_id,
            token: credentials.token,
            dts: credentials
                .dts
                .as_deref()
                .and_then(|value| value.parse().ok()),
            server: credentials.server,
        },
        root_base_url.to_string(),
    )?;
    let payload = client.query_card(&target.customer_token).await?;
    if !business_succeeded(&payload) {
        return Err(anyhow!("读取资料卡返回业务错误"));
    }
    crate::settings::save_profile_card(
        &account.database(),
        &account.paths().player_profile(),
        credentials.role_id,
        &payload,
    )
}

pub async fn fetch_game_action_for_account(
    account: &AccountContextSnapshot,
    root_base_url: &str,
) -> Result<PlayerGameAction> {
    let credentials = crate::credentials::read_saved_credentials_for_account(account.paths())?;
    let target = crate::player_target::read_player_target_for_path(
        &crate::player_target::player_target_path_for_account(account.paths()),
    )?;
    if !crate::player_target::target_matches_login_role(&target, credentials.role_id) {
        return Err(anyhow!("当前账号与目标玩家不一致，请重新添加账号"));
    }

    let client = DashenClient::new(
        ClientCredentials {
            role_id: credentials.role_id,
            token: credentials.token,
            dts: credentials
                .dts
                .as_deref()
                .and_then(|value| value.parse().ok()),
            server: credentials.server,
        },
        root_base_url.to_string(),
    )?;
    let payload = client
        .fight_query_count(&target.customer_token, None)
        .await?;
    if !business_succeeded(&payload) {
        return Err(anyhow!("读取本周表现返回业务错误"));
    }
    game_action_from_payload(&payload).ok_or_else(|| anyhow!("本周表现数据不完整"))
}

fn game_action_from_payload(payload: &serde_json::Value) -> Option<PlayerGameAction> {
    let raw = payload.pointer("/data/gameAction")?.as_object()?;
    Some(PlayerGameAction {
        comment: raw
            .get("comment")
            .and_then(serde_json::Value::as_str)
            .map(str::to_string),
        send_honors_cnt: raw.get("sendHonorsCnt").and_then(serde_json::Value::as_i64),
        get_honors_cnt: raw.get("getHonorsCnt").and_then(serde_json::Value::as_i64),
        reported_cnt: raw.get("reportedCnt").and_then(serde_json::Value::as_i64),
    })
}

fn business_succeeded(payload: &serde_json::Value) -> bool {
    payload.get("code").and_then(|value| {
        value
            .as_i64()
            .or_else(|| value.as_str().and_then(|text| text.trim().parse().ok()))
    }) == Some(0)
}

#[cfg(test)]
mod tests {
    use super::game_action_from_payload;

    #[test]
    fn parses_game_action_from_fight_query_count_payload() {
        let action = game_action_from_payload(&serde_json::json!({
            "code": 0,
            "data": {
                "gameAction": {
                    "comment": "良好",
                    "sendHonorsCnt": 51,
                    "getHonorsCnt": 27,
                    "reportedCnt": 0
                },
                "unrelated": { "large": "payload" }
            }
        }))
        .expect("gameAction should parse");

        assert_eq!(action.comment.as_deref(), Some("良好"));
        assert_eq!(action.send_honors_cnt, Some(51));
        assert_eq!(action.get_honors_cnt, Some(27));
        assert_eq!(action.reported_cnt, Some(0));
    }
}
