use anyhow::{anyhow, bail, Context, Result};
use serde::{de::Error as _, Deserialize, Deserializer, Serialize, Serializer};
use std::collections::HashSet;
use std::fs;
use std::io::Write;
use std::path::Path;
use uuid::Uuid;

pub use crate::models::AccountSummary;

const ACCOUNT_STORE_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct AccountId(String);

impl AccountId {
    pub fn new() -> Self {
        Self(Uuid::new_v4().to_string())
    }

    pub fn parse(value: &str) -> Result<Self> {
        let parsed = Uuid::parse_str(value).context("invalid account id")?;
        Ok(Self(parsed.to_string()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Serialize for AccountId {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for AccountId {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Self::parse(&value).map_err(D::Error::custom)
    }
}

impl Default for AccountId {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountRegistry {
    pub version: u32,
    pub active_account_id: Option<AccountId>,
    pub accounts: Vec<AccountRegistryEntry>,
}

impl Default for AccountRegistry {
    fn default() -> Self {
        Self {
            version: ACCOUNT_STORE_VERSION,
            active_account_id: None,
            accounts: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountRegistryEntry {
    pub account_id: AccountId,
    pub battle_tag: String,
    pub created_at: String,
    pub last_used_at: String,
    pub credential_state: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountManifest {
    pub version: u32,
    pub account_id: AccountId,
    pub role_id: i64,
    pub battle_tag: String,
    pub created_at: String,
    pub updated_at: String,
}

pub fn read_registry(path: &Path) -> Result<AccountRegistry> {
    ensure_regular_file(path)?;
    let registry: AccountRegistry =
        serde_json::from_slice(&fs::read(path).context("failed to read account registry")?)
            .context("invalid account registry")?;
    validate_registry(&registry)?;
    Ok(registry)
}

pub fn write_registry_atomic(path: &Path, registry: &AccountRegistry) -> Result<()> {
    validate_registry(registry)?;
    ensure_regular_file_or_missing(path)?;
    let bytes = serde_json::to_vec_pretty(registry).context("failed to encode account registry")?;
    write_json_atomic(path, &bytes)
}

pub fn read_manifest(path: &Path, expected_account_id: &AccountId) -> Result<AccountManifest> {
    validate_manifest_directory(path, expected_account_id)?;
    ensure_regular_file(path)?;
    let manifest: AccountManifest =
        serde_json::from_slice(&fs::read(path).context("failed to read account manifest")?)
            .context("invalid account manifest")?;
    validate_manifest(&manifest, expected_account_id)?;
    Ok(manifest)
}

pub fn write_manifest_atomic(path: &Path, manifest: &AccountManifest) -> Result<()> {
    validate_manifest_directory(path, &manifest.account_id)?;
    validate_manifest(manifest, &manifest.account_id)?;
    ensure_regular_file_or_missing(path)?;
    let bytes = serde_json::to_vec_pretty(manifest).context("failed to encode account manifest")?;
    write_json_atomic(path, &bytes)
}

fn validate_registry(registry: &AccountRegistry) -> Result<()> {
    if registry.version != ACCOUNT_STORE_VERSION {
        bail!("unsupported account registry version")
    }

    let mut ids = HashSet::with_capacity(registry.accounts.len());
    for account in &registry.accounts {
        if !ids.insert(account.account_id.clone()) {
            bail!("duplicate account id in registry")
        }
    }
    if registry
        .active_account_id
        .as_ref()
        .is_some_and(|active| !ids.contains(active))
    {
        bail!("active account is not present in registry")
    }
    Ok(())
}

fn validate_manifest(manifest: &AccountManifest, expected_account_id: &AccountId) -> Result<()> {
    if manifest.version != ACCOUNT_STORE_VERSION {
        bail!("unsupported account manifest version")
    }
    if &manifest.account_id != expected_account_id {
        bail!("account manifest id does not match expected account")
    }
    Ok(())
}

fn validate_manifest_directory(path: &Path, expected_account_id: &AccountId) -> Result<()> {
    let directory_name = path
        .parent()
        .and_then(Path::file_name)
        .and_then(|name| name.to_str())
        .ok_or_else(|| anyhow!("account manifest has no account directory"))?;
    let directory_id = AccountId::parse(directory_name)?;
    if &directory_id != expected_account_id {
        bail!("account manifest directory does not match expected account")
    }
    Ok(())
}

fn write_json_atomic(path: &Path, bytes: &[u8]) -> Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| anyhow!("account store path has no parent directory"))?;
    fs::create_dir_all(parent)?;
    ensure_directory(parent)?;
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| anyhow!("account store path has no valid file name"))?;
    let temporary = parent.join(format!(".{file_name}.{}.tmp", Uuid::new_v4()));

    let write_result = (|| -> Result<()> {
        let mut file = fs::File::create(&temporary)?;
        file.write_all(bytes)?;
        file.flush()?;
        file.sync_all()?;
        fs::rename(&temporary, path)?;
        Ok(())
    })();
    if write_result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    write_result
}

fn ensure_regular_file(path: &Path) -> Result<()> {
    let metadata = fs::symlink_metadata(path)?;
    if metadata.file_type().is_symlink() || is_windows_reparse(&metadata) || !metadata.is_file() {
        bail!("unsafe account store file")
    }
    Ok(())
}

fn ensure_regular_file_or_missing(path: &Path) -> Result<()> {
    match fs::symlink_metadata(path) {
        Ok(_) => ensure_regular_file(path),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.into()),
    }
}

fn ensure_directory(path: &Path) -> Result<()> {
    let metadata = fs::symlink_metadata(path)?;
    if metadata.file_type().is_symlink() || is_windows_reparse(&metadata) || !metadata.is_dir() {
        bail!("unsafe account store directory")
    }
    Ok(())
}

#[cfg(windows)]
fn is_windows_reparse(metadata: &fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;
    metadata.file_attributes() & 0x0400 != 0
}

#[cfg(not(windows))]
fn is_windows_reparse(_metadata: &fs::Metadata) -> bool {
    false
}
