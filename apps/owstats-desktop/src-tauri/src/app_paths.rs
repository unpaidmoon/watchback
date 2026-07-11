use anyhow::Result;
use std::path::{Path, PathBuf};
use tauri::{AppHandle, Manager};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppPaths {
    root: PathBuf,
}

impl AppPaths {
    pub fn from_app_data_dir(root: impl AsRef<Path>) -> Self {
        Self {
            root: root.as_ref().to_path_buf(),
        }
    }

    pub fn from_app(app: &AppHandle) -> Result<Self> {
        Ok(Self::from_app_data_dir(app.path().app_data_dir()?))
    }

    pub fn database(&self) -> PathBuf {
        self.root.join("owstats.sqlite3")
    }
    pub fn user_settings(&self) -> PathBuf {
        self.root.join("user_settings.json")
    }
    pub fn player_profile(&self) -> PathBuf {
        self.root.join("player_profile.json")
    }
    pub fn credentials(&self) -> PathBuf {
        self.root.join(".dashen_credentials.json")
    }
    pub fn legacy_player_target(&self) -> PathBuf {
        self.root.join(".dashen_player_target.json")
    }
    pub fn browser_profile(&self) -> PathBuf {
        self.root.join("chrome-dashen-profile")
    }
    pub fn seasons(&self) -> PathBuf {
        self.root.join("seasons")
    }
    pub fn diagnostics(&self) -> PathBuf {
        self.root.join("diagnostics")
    }
    pub fn backups(&self) -> PathBuf {
        self.root.join("backups")
    }
    pub fn privacy_backups(&self) -> PathBuf {
        self.root.join("privacy-backups")
    }
}

#[cfg(test)]
mod tests {
    use super::AppPaths;
    use std::path::Path;

    #[test]
    fn all_runtime_paths_stay_under_app_data_with_compatible_names() {
        let root = Path::new("portable-app-data");
        let paths = AppPaths::from_app_data_dir(root);
        let expected = [
            (paths.database(), "owstats.sqlite3"),
            (paths.user_settings(), "user_settings.json"),
            (paths.player_profile(), "player_profile.json"),
            (paths.credentials(), ".dashen_credentials.json"),
            (paths.legacy_player_target(), ".dashen_player_target.json"),
            (paths.browser_profile(), "chrome-dashen-profile"),
            (paths.seasons(), "seasons"),
            (paths.diagnostics(), "diagnostics"),
            (paths.backups(), "backups"),
            (paths.privacy_backups(), "privacy-backups"),
        ];

        for (path, file_name) in expected {
            assert!(path.starts_with(root));
            assert_eq!(path.file_name().unwrap(), file_name);
        }
    }
}
