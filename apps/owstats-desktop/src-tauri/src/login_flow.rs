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
            message: message.as_ref().to_string(),
        }
    }
}

pub trait LoginProgressReporter {
    fn report(&self, phase: LoginPhase, message: &str);
}

impl LoginProgressReporter for tauri::AppHandle {
    fn report(&self, phase: LoginPhase, message: &str) {
        use tauri::Emitter;
        let _ = self.emit(
            "credential-login-progress",
            LoginProgress::new(phase, message),
        );
    }
}
