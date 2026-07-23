use anyhow::{anyhow, Result};
use std::path::{Component, Path, PathBuf};

use crate::account_store::AccountId;

pub fn data_root_from_executable(executable: &Path) -> Result<PathBuf> {
    if !executable.is_absolute()
        || executable
            .components()
            .any(|component| matches!(component, Component::CurDir | Component::ParentDir))
    {
        return Err(anyhow!("无法定位安装目录"));
    }

    let install = executable
        .parent()
        .filter(|value| !value.as_os_str().is_empty())
        .ok_or_else(|| anyhow!("无法定位安装目录"))?;
    Ok(install.join("data"))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SharedAppPaths {
    root: PathBuf,
}

impl SharedAppPaths {
    pub fn from_data_root(root: impl AsRef<Path>) -> Self {
        Self {
            root: root.as_ref().to_path_buf(),
        }
    }

    #[doc(hidden)]
    pub fn from_app_data_dir(root: impl AsRef<Path>) -> Self {
        Self::from_data_root(root)
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn install_migration_staging(&self) -> PathBuf {
        self.root.with_file_name("data.migrating")
    }

    pub fn install_migration_marker(&self) -> PathBuf {
        self.root.join(".install-data-migration-v1.json")
    }

    pub fn registry(&self) -> PathBuf {
        self.root.join("accounts.json")
    }

    pub fn legacy_database(&self) -> PathBuf {
        self.root.join("owstats.sqlite3")
    }

    pub fn legacy_database_wal(&self) -> PathBuf {
        self.root.join("owstats.sqlite3-wal")
    }

    pub fn legacy_database_shm(&self) -> PathBuf {
        self.root.join("owstats.sqlite3-shm")
    }

    pub fn legacy_credentials(&self) -> PathBuf {
        self.root.join(".dashen_credentials.json")
    }

    pub fn legacy_player_target(&self) -> PathBuf {
        self.root.join(".dashen_player_target.json")
    }

    pub fn legacy_player_profile(&self) -> PathBuf {
        self.root.join("player_profile.json")
    }

    pub fn legacy_seasons(&self) -> PathBuf {
        self.root.join("seasons")
    }

    pub fn legacy_diagnostics(&self) -> PathBuf {
        self.root.join("diagnostics")
    }

    pub fn legacy_backups(&self) -> PathBuf {
        self.root.join("backups")
    }

    pub fn user_settings(&self) -> PathBuf {
        self.root.join("user_settings.json")
    }

    pub fn browser_profile(&self) -> PathBuf {
        self.root.join("chrome-dashen-profile")
    }

    pub fn accounts_root(&self) -> PathBuf {
        self.root.join("accounts")
    }

    pub fn operations_root(&self) -> PathBuf {
        self.root.join(".account-operations")
    }

    pub fn pending_logins(&self) -> PathBuf {
        self.operations_root().join("pending-logins")
    }

    pub fn deletion_journal(&self) -> PathBuf {
        self.operations_root().join("deletion-journal.json")
    }

    pub fn migration_journal(&self) -> PathBuf {
        self.operations_root().join("migration-journal.json")
    }

    pub fn migration_eligibility(&self) -> PathBuf {
        self.operations_root().join("migration-eligibility.json")
    }

    pub fn migration_content_manifest(&self, account_id: &AccountId) -> PathBuf {
        self.operations_root()
            .join(format!("migration-content-{}.json", account_id.as_str()))
    }

    pub fn account(&self, account_id: &AccountId) -> AccountPaths {
        AccountPaths {
            root: self.accounts_root().join(account_id.as_str()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccountPaths {
    root: PathBuf,
}

impl AccountPaths {
    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn manifest(&self) -> PathBuf {
        self.root.join("account.json")
    }

    pub fn credentials(&self) -> PathBuf {
        self.root.join("credentials.json")
    }

    pub fn player_target(&self) -> PathBuf {
        self.root.join("player_target.json")
    }

    pub fn player_profile(&self) -> PathBuf {
        self.root.join("player_profile.json")
    }

    pub fn database(&self) -> PathBuf {
        self.root.join("owstats.sqlite3")
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
}

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
    use super::{data_root_from_executable, AppPaths, SharedAppPaths};
    use std::path::{Path, PathBuf};

    #[test]
    fn executable_parent_owns_data() {
        assert_eq!(
            data_root_from_executable(Path::new(r"D:\应用\Watchback\Watchback.exe")).unwrap(),
            PathBuf::from(r"D:\应用\Watchback\data"),
        );
    }

    #[cfg(windows)]
    #[test]
    fn unc_executable_parent_owns_data() {
        assert_eq!(
            data_root_from_executable(Path::new(r"\\server\share\Watchback.exe")).unwrap(),
            PathBuf::from(r"\\server\share\data"),
        );
    }

    #[test]
    fn empty_executable_path_is_rejected() {
        assert!(data_root_from_executable(Path::new("")).is_err());
    }

    #[cfg(windows)]
    #[test]
    fn windows_executable_path_must_be_absolute_without_dot_components() {
        for executable in [
            r"relative\Watchback.exe",
            r"D:Watchback.exe",
            r"\Watchback.exe",
            r"D:\apps\..\Watchback.exe",
        ] {
            assert!(data_root_from_executable(Path::new(executable)).is_err());
        }
    }

    #[test]
    fn shared_business_paths_stay_under_install_data() {
        let root = Path::new(r"D:\Watchback\data");
        let paths = SharedAppPaths::from_data_root(root);
        assert_eq!(paths.root(), root);
        assert!(paths.registry().starts_with(root));
        assert!(paths.user_settings().starts_with(root));
        assert!(paths.browser_profile().starts_with(root));
        assert!(paths.accounts_root().starts_with(root));
    }

    #[test]
    fn legacy_app_paths_stay_under_injected_root_with_compatible_names() {
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
