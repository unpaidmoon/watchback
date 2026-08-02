use crate::account_context::CredentialUseFailure;
use crate::account_login::CredentialProbeOutcome;
use crate::credential_store::DashenCredentials;
use crate::dashen_client::{DashenClient, DashenCredentials as ClientCredentials};
use crate::player_target::{
    read_player_target_for_path, save_player_target_for_path, target_from_search_payload,
    target_matches_login_role, PlayerTarget,
};
use anyhow::{anyhow, Result};
use chrono::Utc;
use std::path::Path;

pub const SYNC_IDENTITY_ERROR: &str = "当前账号的同步身份不可用，请重新添加账号";

/// Verifies the captured account against Dashen before the account is published.
/// BattleTag discovery belongs to login; this module only resolves the opaque
/// customer token required by profile and season endpoints.
pub async fn verify_credentials_with_base_url(
    credentials: &DashenCredentials,
    battle_tag: &str,
    root_base_url: &str,
) -> CredentialProbeOutcome {
    let client = match build_client(credentials, root_base_url) {
        Ok(client) => client,
        Err(_) => return CredentialProbeOutcome::TransientFailure,
    };
    let payload = match client.search_bnet_account(battle_tag).await {
        Ok(payload) => payload,
        Err(error) => return probe_failure(&error),
    };
    if ensure_business_success(&payload, "verify captured account").is_err()
        || payload_role_id(&payload).is_some_and(|role_id| role_id != credentials.role_id)
    {
        return CredentialProbeOutcome::Rejected;
    }
    let identity = match target_from_search_payload(
        battle_tag,
        &payload,
        credentials.role_id,
        &Utc::now().to_rfc3339(),
    ) {
        Ok(identity) => identity,
        Err(_) => return CredentialProbeOutcome::Rejected,
    };
    if identity
        .resolved_bnet
        .as_deref()
        .is_some_and(|resolved| !same_battle_tag(resolved, battle_tag))
    {
        return CredentialProbeOutcome::Rejected;
    }
    let card = match client.query_card(&identity.customer_token).await {
        Ok(card) => card,
        Err(error) => return probe_failure(&error),
    };
    if ensure_business_success(&card, "verify captured profile").is_err()
        || ensure_card_identity_matches(&card, battle_tag).is_err()
        || payload_role_id(&card).is_some_and(|role_id| role_id != credentials.role_id)
    {
        return CredentialProbeOutcome::Rejected;
    }
    CredentialProbeOutcome::Authenticated {
        role_id: credentials.role_id,
    }
}

/// Refreshes the current account's private sync identity after a successful
/// login publication. There is no user-selectable target and no recovery state.
pub async fn refresh_with_base_url(
    credentials: &DashenCredentials,
    battle_tag: &str,
    identity_path: &Path,
    root_base_url: &str,
) -> Result<()> {
    let identity = resolve_with_base_url(credentials, battle_tag, root_base_url).await?;
    save_player_target_for_path(identity_path, &identity)
}

/// Fails closed if persisted sync identity does not belong to the credentials
/// of the current account.
pub fn ensure_ready(credentials: &DashenCredentials, identity_path: &Path) -> Result<()> {
    let identity =
        read_player_target_for_path(identity_path).map_err(|_| anyhow!(SYNC_IDENTITY_ERROR))?;
    if identity.customer_token.trim().is_empty()
        || !target_matches_login_role(&identity, credentials.role_id)
    {
        return Err(anyhow!(SYNC_IDENTITY_ERROR));
    }
    Ok(())
}

async fn resolve_with_base_url(
    credentials: &DashenCredentials,
    battle_tag: &str,
    root_base_url: &str,
) -> Result<PlayerTarget> {
    let client = build_client(credentials, root_base_url)?;
    let payload = client.search_bnet_account(battle_tag).await?;
    ensure_business_success(&payload, "读取账号同步身份")?;
    if payload_role_id(&payload).is_some_and(|role_id| role_id != credentials.role_id) {
        return Err(anyhow!("同步身份与登录账号不一致"));
    }
    let identity = target_from_search_payload(
        battle_tag,
        &payload,
        credentials.role_id,
        &Utc::now().to_rfc3339(),
    )?;
    if identity
        .resolved_bnet
        .as_deref()
        .is_some_and(|resolved| !same_battle_tag(resolved, battle_tag))
    {
        return Err(anyhow!("同步身份与登录账号不一致"));
    }
    let card = client.query_card(&identity.customer_token).await?;
    ensure_business_success(&card, "验证账号资料卡")?;
    ensure_card_identity_matches(&card, battle_tag)?;
    if payload_role_id(&card).is_some_and(|role_id| role_id != credentials.role_id) {
        return Err(anyhow!("资料卡与登录账号不一致"));
    }
    Ok(identity)
}

fn build_client(credentials: &DashenCredentials, root_base_url: &str) -> Result<DashenClient> {
    DashenClient::new(
        ClientCredentials {
            role_id: credentials.role_id,
            token: credentials.token.clone(),
            dts: credentials
                .dts
                .as_deref()
                .and_then(|value| value.parse().ok()),
            server: credentials.server,
        },
        root_base_url.to_string(),
    )
}

fn probe_failure(error: &anyhow::Error) -> CredentialProbeOutcome {
    match CredentialUseFailure::from_upstream_error(error) {
        CredentialUseFailure::Unauthorized | CredentialUseFailure::Forbidden => {
            CredentialProbeOutcome::Rejected
        }
        CredentialUseFailure::Transient => CredentialProbeOutcome::TransientFailure,
    }
}

fn payload_role_id(payload: &serde_json::Value) -> Option<i64> {
    ["roleId", "role_id"]
        .into_iter()
        .find_map(|key| payload.get("data")?.get(key))
        .and_then(|value| {
            value
                .as_i64()
                .or_else(|| value.as_str()?.trim().parse().ok())
        })
}

fn ensure_business_success(payload: &serde_json::Value, operation: &str) -> Result<()> {
    let succeeded = payload.get("code").and_then(|value| {
        value
            .as_i64()
            .or_else(|| value.as_str().and_then(|text| text.trim().parse().ok()))
    }) == Some(0);
    if succeeded {
        Ok(())
    } else {
        Err(anyhow!("{operation}返回业务错误"))
    }
}

fn ensure_card_identity_matches(payload: &serde_json::Value, battle_tag: &str) -> Result<()> {
    let Some(data) = payload.get("data").and_then(serde_json::Value::as_object) else {
        return Ok(());
    };
    for key in [
        "bnetId",
        "bnet",
        "battleTag",
        "battletag",
        "BattleTag",
        "name",
    ] {
        let Some(candidate) = data
            .get(key)
            .and_then(serde_json::Value::as_str)
            .map(str::trim)
            .filter(|candidate| looks_like_battle_tag(candidate))
        else {
            continue;
        };
        if !same_battle_tag(candidate, battle_tag) {
            return Err(anyhow!("资料卡与登录账号不一致"));
        }
    }
    Ok(())
}

fn looks_like_battle_tag(value: &str) -> bool {
    let normalized = value.trim().replace('＃', "#");
    let Some((name, discriminator)) = normalized.split_once('#') else {
        return false;
    };
    !name.trim().is_empty()
        && !discriminator.trim().is_empty()
        && !discriminator.contains('#')
        && discriminator
            .chars()
            .all(|character| character.is_ascii_digit())
}

fn same_battle_tag(left: &str, right: &str) -> bool {
    normalize_battle_tag(left) == normalize_battle_tag(right)
}

fn normalize_battle_tag(value: &str) -> String {
    value.trim().replace('＃', "#").to_lowercase()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn credentials(role_id: i64) -> DashenCredentials {
        DashenCredentials {
            role_id,
            token: "synthetic-account-token".to_string(),
            customer_token: None,
            dts: None,
            server: 1,
            saved_at: "2026-08-02T00:00:00Z".to_string(),
        }
    }

    #[test]
    fn persisted_identity_must_belong_to_current_account() {
        let temp = tempdir().unwrap();
        let path = temp.path().join("sync-identity.json");
        let identity = PlayerTarget {
            input_bnet: "Fixture#1234".to_string(),
            resolved_bnet: Some("Fixture#1234".to_string()),
            customer_token: "synthetic-customer-token".to_string(),
            bound_at: "2026-08-02T00:00:00Z".to_string(),
            source_role_id: Some(101),
        };
        save_player_target_for_path(&path, &identity).unwrap();

        ensure_ready(&credentials(101), &path).unwrap();
        assert_eq!(
            ensure_ready(&credentials(202), &path)
                .unwrap_err()
                .to_string(),
            SYNC_IDENTITY_ERROR,
        );
    }

    #[test]
    fn battle_tag_comparison_is_stable_but_not_user_selectable() {
        assert!(same_battle_tag(" Fixture＃1234 ", "fixture#1234"));
        assert!(!same_battle_tag("Fixture#1234", "Other#5678"));
    }
}
