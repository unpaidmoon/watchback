use crate::DeepSeekKeyStatus;
use anyhow::{anyhow, Result};
use reqwest::Client;
use std::sync::Arc;
use std::time::Duration;

pub const DEFAULT_DEEPSEEK_BASE_URL: &str = "https://api.deepseek.com";
pub const DEEPSEEK_CREDENTIAL_TARGET: &str = "OWStatsLocal/DeepSeekApiKey";
pub const DEEPSEEK_REQUEST_TIMEOUT: Duration = Duration::from_secs(10);

pub trait DeepSeekCredentialStore: Send + Sync {
    fn exists(&self) -> Result<bool>;
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
        Ok(status_for(self.store.exists()?))
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

struct WindowsDeepSeekCredentialStore;

#[cfg(windows)]
impl DeepSeekCredentialStore for WindowsDeepSeekCredentialStore {
    fn exists(&self) -> Result<bool> {
        use std::ffi::c_void;
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
                return Ok(false);
            }
            return Err(anyhow!("无法读取 Windows 凭据管理器中的 DeepSeek API Key"));
        }
        if raw.is_null() {
            return Err(anyhow!("Windows 凭据管理器返回了无效结果"));
        }
        unsafe { CredFree(raw.cast::<c_void>()) };
        Ok(true)
    }

    fn write(&self, api_key: &str) -> Result<()> {
        use windows_sys::Win32::Security::Credentials::{
            CredWriteW, CREDENTIALW, CRED_PERSIST_LOCAL_MACHINE, CRED_TYPE_GENERIC,
        };

        let mut target = wide(DEEPSEEK_CREDENTIAL_TARGET);
        let mut user_name = wide("OWStatsLocal");
        let mut blob = api_key.as_bytes().to_vec();
        let blob_size = u32::try_from(blob.len()).map_err(|_| anyhow!("DeepSeek API Key 过长"))?;
        let mut credential: CREDENTIALW = unsafe { std::mem::zeroed() };
        credential.Type = CRED_TYPE_GENERIC;
        credential.TargetName = target.as_mut_ptr();
        credential.CredentialBlobSize = blob_size;
        credential.CredentialBlob = blob.as_mut_ptr();
        credential.Persist = CRED_PERSIST_LOCAL_MACHINE;
        credential.UserName = user_name.as_mut_ptr();

        if unsafe { CredWriteW(&credential, 0) } == 0 {
            return Err(anyhow!("无法将 DeepSeek API Key 保存到 Windows 凭据管理器"));
        }
        Ok(())
    }
}

#[cfg(windows)]
fn wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(std::iter::once(0)).collect()
}

#[cfg(not(windows))]
impl DeepSeekCredentialStore for WindowsDeepSeekCredentialStore {
    fn exists(&self) -> Result<bool> {
        Err(anyhow!("DeepSeek API Key 安全存储仅支持 Windows"))
    }

    fn write(&self, _api_key: &str) -> Result<()> {
        Err(anyhow!("DeepSeek API Key 安全存储仅支持 Windows"))
    }
}
