use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum LoginPhase {
    Idle,
    LocatingBrowser,
    OpeningBrowser,
    WaitingForLogin,
    RoleFound,
    LoadingSignPage,
    TokenPending,
    Saving,
    Ready,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LoginProgress {
    pub phase: LoginPhase,
    pub message: String,
}

impl LoginProgress {
    pub fn new(phase: LoginPhase, message: impl AsRef<str>) -> Self {
        Self {
            phase,
            message: crate::credential_store::redact_sensitive_text(message.as_ref()),
        }
    }
}
