use crate::DeepSeekKeyStatus;
use anyhow::{anyhow, Result};
use reqwest::Client;
#[cfg(any(windows, test))]
use std::ffi::c_void;
#[cfg(any(windows, test))]
use std::ptr::NonNull;
use std::sync::Arc;
use std::time::Duration;
#[cfg(any(windows, test))]
use zeroize::Zeroizing;

pub const DEFAULT_DEEPSEEK_BASE_URL: &str = "https://api.deepseek.com";
pub const DEEPSEEK_CREDENTIAL_TARGET: &str = "OWStatsLocal/DeepSeekApiKey";
pub const DEEPSEEK_REQUEST_TIMEOUT: Duration = Duration::from_secs(10);

pub trait DeepSeekCredentialStore: Send + Sync {
    fn read(&self) -> Result<Option<String>>;
    fn write(&self, api_key: &str) -> Result<()>;
}

pub fn build_http_client() -> Result<Client> {
    Client::builder()
        .connect_timeout(Duration::from_secs(5))
        .timeout(DEEPSEEK_REQUEST_TIMEOUT)
        .build()
        .map_err(|_| anyhow!("无法初始化 DeepSeek 网络客户端"))
}

pub fn normalize_base_url(base_url: &str) -> Result<String> {
    let mut url = reqwest::Url::parse(base_url.trim()).map_err(|_| anyhow!("DeepSeek 地址无效"))?;
    if !matches!(url.scheme(), "http" | "https")
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return Err(anyhow!("DeepSeek 地址无效"));
    }
    let normalized_path = url.path().trim_end_matches('/').to_string();
    url.set_path(&normalized_path);
    Ok(url.as_str().trim_end_matches('/').to_string())
}

pub fn validate_models_status(status: u16) -> Result<()> {
    match status {
        200 => Ok(()),
        401 => Err(anyhow!("DeepSeek API Key 无效")),
        402 => Err(anyhow!("DeepSeek 账户余额不足")),
        429 => Err(anyhow!("DeepSeek 请求过于频繁，请稍后重试")),
        500 | 503 => Err(anyhow!("DeepSeek 服务暂时不可用，请稍后重试")),
        _ => Err(anyhow!("DeepSeek 返回了无法识别的状态")),
    }
}

pub async fn validate_api_key(client: &Client, base_url: &str, api_key: &str) -> Result<()> {
    let base_url = normalize_base_url(base_url)?;
    let api_key = api_key.trim();
    if api_key.is_empty() {
        return Err(anyhow!("请输入 DeepSeek API Key"));
    }
    let response = client
        .get(format!("{base_url}/models"))
        .bearer_auth(api_key)
        .send()
        .await
        .map_err(|_| anyhow!("无法连接 DeepSeek 服务"))?;
    validate_models_status(response.status().as_u16())
}

pub struct DeepSeekKeyManager {
    client: Client,
    base_url: String,
    store: Arc<dyn DeepSeekCredentialStore>,
}

impl DeepSeekKeyManager {
    pub fn new() -> Result<Self> {
        Self::with_store(
            build_http_client()?,
            DEFAULT_DEEPSEEK_BASE_URL,
            Arc::new(WindowsDeepSeekCredentialStore),
        )
    }

    #[doc(hidden)]
    pub fn with_store(
        client: Client,
        base_url: &str,
        store: Arc<dyn DeepSeekCredentialStore>,
    ) -> Result<Self> {
        Ok(Self {
            client,
            base_url: normalize_base_url(base_url)?,
            store,
        })
    }

    pub fn status(&self) -> Result<DeepSeekKeyStatus> {
        let configured = match self
            .store
            .read()
            .map_err(|_| anyhow!("无法安全读取 DeepSeek API Key"))?
        {
            Some(key) if key.trim().is_empty() => {
                return Err(anyhow!("无法安全读取 DeepSeek API Key"));
            }
            Some(_) => true,
            None => false,
        };
        Ok(status_for(configured))
    }

    /// Returns the credential only to Rust backend callers. This method is not a Tauri command
    /// and its value must never be placed in a DTO, event, or log.
    pub fn backend_api_key(&self) -> Result<String> {
        let key = self
            .store
            .read()
            .map_err(|_| anyhow!("无法安全读取 DeepSeek API Key"))?
            .ok_or_else(|| anyhow!("尚未配置 DeepSeek API Key"))?;
        if key.trim().is_empty() {
            return Err(anyhow!("无法安全读取 DeepSeek API Key"));
        }
        Ok(key)
    }

    pub async fn save(&self, api_key: &str) -> Result<DeepSeekKeyStatus> {
        validate_api_key(&self.client, &self.base_url, api_key).await?;
        self.store.write(api_key.trim())?;
        Ok(status_for(true))
    }
}

fn status_for(configured: bool) -> DeepSeekKeyStatus {
    if configured {
        DeepSeekKeyStatus {
            state: "ready".to_string(),
            message: "DeepSeek API Key 已安全保存".to_string(),
        }
    } else {
        DeepSeekKeyStatus {
            state: "missing".to_string(),
            message: "尚未配置 DeepSeek API Key".to_string(),
        }
    }
}

#[cfg(any(windows, test))]
type CredentialReleaser = unsafe extern "system" fn(*const c_void);

#[cfg(any(windows, test))]
struct OwnedCredential<T> {
    pointer: NonNull<T>,
    release: CredentialReleaser,
}

#[cfg(any(windows, test))]
impl<T> OwnedCredential<T> {
    /// A successful Win32 read transfers ownership only when it returns a non-null pointer.
    /// A success-plus-null result owns no allocation, so there is nothing safe to pass to CredFree.
    unsafe fn from_raw(pointer: *mut T, release: CredentialReleaser) -> Option<Self> {
        NonNull::new(pointer).map(|pointer| Self { pointer, release })
    }

    fn as_ref(&self) -> &T {
        unsafe { self.pointer.as_ref() }
    }
}

#[cfg(any(windows, test))]
impl<T> Drop for OwnedCredential<T> {
    fn drop(&mut self) {
        unsafe { (self.release)(self.pointer.as_ptr().cast::<c_void>()) };
    }
}

#[cfg(any(windows, test))]
fn prepare_api_key_blob(api_key: &str) -> Result<(Zeroizing<Vec<u8>>, u32)> {
    prepare_api_key_blob_with_size_conversion(api_key, |length| {
        u32::try_from(length).map_err(|_| anyhow!("DeepSeek API Key 过长"))
    })
}

#[cfg(any(windows, test))]
fn prepare_api_key_blob_with_size_conversion(
    api_key: &str,
    convert_size: impl FnOnce(usize) -> Result<u32>,
) -> Result<(Zeroizing<Vec<u8>>, u32)> {
    let blob = Zeroizing::new(api_key.as_bytes().to_vec());
    let blob_size = convert_size(blob.len())?;
    Ok((blob, blob_size))
}

struct WindowsDeepSeekCredentialStore;

#[cfg(windows)]
impl DeepSeekCredentialStore for WindowsDeepSeekCredentialStore {
    fn read(&self) -> Result<Option<String>> {
        use std::ptr::null_mut;
        use windows_sys::Win32::Foundation::ERROR_NOT_FOUND;
        use windows_sys::Win32::Security::Credentials::{
            CredFree, CredReadW, CREDENTIALW, CRED_TYPE_GENERIC,
        };

        let target = wide(DEEPSEEK_CREDENTIAL_TARGET);
        let mut raw: *mut CREDENTIALW = null_mut();
        let succeeded = unsafe { CredReadW(target.as_ptr(), CRED_TYPE_GENERIC, 0, &mut raw) };
        if succeeded == 0 {
            let error = std::io::Error::last_os_error();
            if error.raw_os_error() == Some(ERROR_NOT_FOUND as i32) {
                return Ok(None);
            }
            return Err(anyhow!("无法读取 Windows 凭据管理器中的 DeepSeek API Key"));
        }
        let raw = unsafe { OwnedCredential::from_raw(raw, CredFree) }
            .ok_or_else(|| anyhow!("Windows 凭据管理器返回了无效结果"))?;
        let result = unsafe {
            let credential = raw.as_ref();
            if credential.CredentialBlobSize == 0 || credential.CredentialBlob.is_null() {
                Err(anyhow!("Windows 凭据管理器中的 DeepSeek API Key 无效"))
            } else {
                let blob = std::slice::from_raw_parts(
                    credential.CredentialBlob,
                    credential.CredentialBlobSize as usize,
                );
                match std::str::from_utf8(blob) {
                    Ok(key) if !key.trim().is_empty() => Ok(Some(key.to_owned())),
                    _ => Err(anyhow!("Windows 凭据管理器中的 DeepSeek API Key 无效")),
                }
            }
        };
        result
    }

    fn write(&self, api_key: &str) -> Result<()> {
        use windows_sys::Win32::Security::Credentials::{
            CredWriteW, CREDENTIALW, CRED_PERSIST_LOCAL_MACHINE, CRED_TYPE_GENERIC,
        };

        let mut target = wide(DEEPSEEK_CREDENTIAL_TARGET);
        let mut user_name = wide("OWStatsLocal");
        let (mut blob, blob_size) = prepare_api_key_blob(api_key)?;
        let mut credential: CREDENTIALW = unsafe { std::mem::zeroed() };
        credential.Type = CRED_TYPE_GENERIC;
        credential.TargetName = target.as_mut_ptr();
        credential.CredentialBlobSize = blob_size;
        credential.CredentialBlob = blob.as_mut_ptr();
        credential.Persist = CRED_PERSIST_LOCAL_MACHINE;
        credential.UserName = user_name.as_mut_ptr();

        let succeeded = unsafe { CredWriteW(&credential, 0) };
        if succeeded == 0 {
            return Err(anyhow!("无法将 DeepSeek API Key 保存到 Windows 凭据管理器"));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{prepare_api_key_blob, prepare_api_key_blob_with_size_conversion, OwnedCredential};
    use anyhow::{anyhow, Result};
    use std::ffi::c_void;
    use std::ptr::null_mut;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use zeroize::Zeroizing;

    static ERROR_RELEASES: AtomicUsize = AtomicUsize::new(0);
    static PANIC_RELEASES: AtomicUsize = AtomicUsize::new(0);
    static NULL_RELEASES: AtomicUsize = AtomicUsize::new(0);

    unsafe extern "system" fn release_error_pointer(pointer: *const c_void) {
        ERROR_RELEASES.fetch_add(1, Ordering::SeqCst);
        drop(Box::from_raw(pointer.cast_mut().cast::<u8>()));
    }

    unsafe extern "system" fn release_panic_pointer(pointer: *const c_void) {
        PANIC_RELEASES.fetch_add(1, Ordering::SeqCst);
        drop(Box::from_raw(pointer.cast_mut().cast::<u8>()));
    }

    unsafe extern "system" fn record_null_release(_pointer: *const c_void) {
        NULL_RELEASES.fetch_add(1, Ordering::SeqCst);
    }

    #[test]
    fn owned_credential_releases_non_null_pointer_on_error_return() {
        ERROR_RELEASES.store(0, Ordering::SeqCst);

        fn fail_after_ownership() -> Result<()> {
            let pointer = Box::into_raw(Box::new(7_u8));
            let _credential = unsafe {
                OwnedCredential::from_raw(pointer, release_error_pointer)
                    .expect("non-null pointers must become owned")
            };
            Err(anyhow!("synthetic failure"))
        }

        assert!(fail_after_ownership().is_err());
        assert_eq!(ERROR_RELEASES.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn owned_credential_releases_non_null_pointer_during_panic_unwind() {
        PANIC_RELEASES.store(0, Ordering::SeqCst);

        let unwind = std::panic::catch_unwind(|| {
            let pointer = Box::into_raw(Box::new(9_u8));
            let credential = unsafe {
                OwnedCredential::from_raw(pointer, release_panic_pointer)
                    .expect("non-null pointers must become owned")
            };
            assert_eq!(*credential.as_ref(), 9);
            panic!("synthetic panic");
        });

        assert!(unwind.is_err());
        assert_eq!(PANIC_RELEASES.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn successful_null_pointer_has_no_owned_allocation_to_release() {
        NULL_RELEASES.store(0, Ordering::SeqCst);

        let credential =
            unsafe { OwnedCredential::<u8>::from_raw(null_mut(), record_null_release) };

        assert!(credential.is_none());
        assert_eq!(NULL_RELEASES.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn prepared_api_key_blob_uses_zeroizing_storage() {
        let (blob, blob_size) = prepare_api_key_blob("synthetic-key").unwrap();

        fn assert_zeroizing_vec(_blob: &Zeroizing<Vec<u8>>) {}
        assert_zeroizing_vec(&blob);
        assert_eq!(blob.as_slice(), b"synthetic-key");
        assert_eq!(blob_size, 13);
    }

    #[test]
    fn size_conversion_failure_occurs_after_bytes_enter_zeroizing_storage() {
        let error = prepare_api_key_blob_with_size_conversion("synthetic-key", |_| {
            Err(anyhow!("DeepSeek API Key 过长"))
        })
        .unwrap_err()
        .to_string();

        assert!(error.contains("DeepSeek API Key"));
        assert!(!error.contains("synthetic-key"));
    }
}

#[cfg(windows)]
fn wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(std::iter::once(0)).collect()
}

#[cfg(not(windows))]
impl DeepSeekCredentialStore for WindowsDeepSeekCredentialStore {
    fn read(&self) -> Result<Option<String>> {
        Err(anyhow!("DeepSeek API Key 安全存储仅支持 Windows"))
    }

    fn write(&self, _api_key: &str) -> Result<()> {
        Err(anyhow!("DeepSeek API Key 安全存储仅支持 Windows"))
    }
}
