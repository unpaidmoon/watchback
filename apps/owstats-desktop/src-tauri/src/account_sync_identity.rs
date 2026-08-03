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
    match resolve_verified_identity(credentials, battle_tag, root_base_url).await {
        Ok(_) => CredentialProbeOutcome::Authenticated {
            role_id: credentials.role_id,
        },
        Err(error) => error.into_probe_outcome(),
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
    let identity = resolve_verified_identity(credentials, battle_tag, root_base_url)
        .await
        .map_err(VerifiedIdentityFailure::into_refresh_error)?;
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

async fn resolve_verified_identity(
    credentials: &DashenCredentials,
    battle_tag: &str,
    root_base_url: &str,
) -> std::result::Result<PlayerTarget, VerifiedIdentityFailure> {
    let client =
        build_client(credentials, root_base_url).map_err(VerifiedIdentityFailure::transient)?;
    let payload = client
        .search_bnet_account(battle_tag)
        .await
        .map_err(VerifiedIdentityFailure::from_upstream)?;
    ensure_business_success(&payload, "读取账号同步身份")
        .map_err(|error| VerifiedIdentityFailure::from_business(&payload, error))?;
    ensure_role_matches(&payload, credentials.role_id, "同步身份与登录账号不一致")
        .map_err(VerifiedIdentityFailure::rejected)?;
    ensure_target_key_aliases_agree(&payload).map_err(VerifiedIdentityFailure::rejected)?;
    let resolved_bnet = ensure_battle_tags_match(&payload, battle_tag, "同步身份与登录账号不一致")
        .map_err(|error| {
            if has_battle_tag_evidence(&payload) {
                VerifiedIdentityFailure::rejected(error)
            } else {
                VerifiedIdentityFailure::transient(error)
            }
        })?;
    let mut identity = target_from_search_payload(
        battle_tag,
        &payload,
        credentials.role_id,
        &Utc::now().to_rfc3339(),
    )
    .map_err(VerifiedIdentityFailure::transient)?;
    identity.resolved_bnet = Some(resolved_bnet);
    let card = client
        .query_card(&identity.customer_token)
        .await
        .map_err(VerifiedIdentityFailure::from_upstream)?;
    ensure_business_success(&card, "验证账号资料卡")
        .map_err(|error| VerifiedIdentityFailure::from_business(&card, error))?;
    ensure_role_matches(&card, credentials.role_id, "资料卡与登录账号不一致")
        .map_err(VerifiedIdentityFailure::rejected)?;
    ensure_optional_battle_tags_match(&card, battle_tag, "资料卡与登录账号不一致")
        .map_err(VerifiedIdentityFailure::rejected)?;
    Ok(identity)
}

#[derive(Debug)]
enum VerifiedIdentityFailure {
    Rejected,
    Transient(anyhow::Error),
}

impl VerifiedIdentityFailure {
    fn rejected(_error: anyhow::Error) -> Self {
        Self::Rejected
    }

    fn transient(error: anyhow::Error) -> Self {
        Self::Transient(error)
    }

    fn from_upstream(error: anyhow::Error) -> Self {
        match CredentialUseFailure::from_upstream_error(&error) {
            CredentialUseFailure::Unauthorized | CredentialUseFailure::Forbidden => Self::Rejected,
            CredentialUseFailure::Transient => Self::Transient(error),
        }
    }

    fn from_business(payload: &serde_json::Value, error: anyhow::Error) -> Self {
        let code = payload.get("code").and_then(|value| {
            value
                .as_i64()
                .or_else(|| value.as_str().and_then(|text| text.trim().parse().ok()))
        });
        if matches!(code, Some(401 | 403)) {
            Self::Rejected
        } else {
            Self::Transient(error)
        }
    }

    fn into_probe_outcome(self) -> CredentialProbeOutcome {
        match self {
            Self::Rejected => CredentialProbeOutcome::Rejected,
            Self::Transient(_) => CredentialProbeOutcome::TransientFailure,
        }
    }

    fn into_refresh_error(self) -> anyhow::Error {
        match self {
            Self::Rejected => anyhow!(SYNC_IDENTITY_ERROR),
            Self::Transient(error) => error,
        }
    }
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

fn ensure_role_matches(payload: &serde_json::Value, role_id: i64, message: &str) -> Result<()> {
    let data = payload
        .get("data")
        .and_then(serde_json::Value::as_object)
        .ok_or_else(|| anyhow!(message.to_string()))?;
    for key in ["roleId", "role_id"] {
        let Some(value) = data.get(key) else {
            continue;
        };
        let parsed = value
            .as_i64()
            .or_else(|| value.as_str().and_then(|text| text.trim().parse().ok()));
        if parsed != Some(role_id) {
            return Err(anyhow!(message.to_string()));
        }
    }
    Ok(())
}

fn ensure_target_key_aliases_agree(payload: &serde_json::Value) -> Result<()> {
    let data = payload
        .get("data")
        .and_then(serde_json::Value::as_object)
        .ok_or_else(|| anyhow!("同步身份响应缺少数据"))?;
    let mut observed: Option<&str> = None;
    for key in ["accountKey", "customerToken", "customer_token"] {
        let Some(value) = data
            .get(key)
            .and_then(serde_json::Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
        else {
            continue;
        };
        if observed.is_some_and(|existing| existing != value) {
            return Err(anyhow!("同步身份目标标识冲突"));
        }
        observed = Some(value);
    }
    Ok(())
}

fn ensure_optional_battle_tags_match(
    payload: &serde_json::Value,
    battle_tag: &str,
    message: &str,
) -> Result<()> {
    let data = payload
        .get("data")
        .and_then(serde_json::Value::as_object)
        .ok_or_else(|| anyhow!(message.to_string()))?;
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
            return Err(anyhow!(message.to_string()));
        }
    }

    Ok(())
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

fn ensure_battle_tags_match(
    payload: &serde_json::Value,
    battle_tag: &str,
    message: &str,
) -> Result<String> {
    let data = payload
        .get("data")
        .and_then(serde_json::Value::as_object)
        .ok_or_else(|| anyhow!(message.to_string()))?;
    let mut matched = None;
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
            return Err(anyhow!(message.to_string()));
        }
        matched.get_or_insert_with(|| candidate.to_string());
    }
    matched.ok_or_else(|| anyhow!(message.to_string()))
}

fn has_battle_tag_evidence(payload: &serde_json::Value) -> bool {
    payload
        .get("data")
        .and_then(serde_json::Value::as_object)
        .is_some_and(|data| {
            [
                "bnetId",
                "bnet",
                "battleTag",
                "battletag",
                "BattleTag",
                "name",
            ]
            .iter()
            .any(|key| {
                data.get(*key)
                    .and_then(serde_json::Value::as_str)
                    .is_some_and(looks_like_battle_tag)
            })
        })
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
