//! 微软登录模块（设备码流程）
//!
//! 使用设备码（Device Code）方式获取 Microsoft 账户授权，适用于无图形界面的环境。
//! 参考：https://learn.microsoft.com/zh-cn/azure/active-directory/develop/v2-oauth2-device-code

use std::time::Duration;

use serde::Deserialize;
use thiserror::Error;
use tokio::time::sleep;

use crate::auth::structs::AuthMethod;
use crate::auth::microsoft::legacy::{
    xbox_authenticate, get_minecraft_access_token, get_minecraft_profile, verify_minecraft_license,
    download_and_parse_skin, get_xuid,
};
use crate::http::HttpClient;
use crate::password::Password;
use crate::prelude::*;

// ============================================================================
//  常量（默认设备码客户端 ID）
// ============================================================================

/// Minecraft 官方设备码客户端 ID（可用于设备码流）
pub const DEFAULT_DEVICE_CLIENT_ID: &str = "00000000402b5328";

/// 设备码授权端点
const DEVICE_CODE_URL: &str = "https://login.microsoftonline.com/consumers/oauth2/v2.0/devicecode";

/// 令牌端点
const TOKEN_URL: &str = "https://login.microsoftonline.com/consumers/oauth2/v2.0/token";

/// 设备码流的作用域
const DEVICE_SCOPE: &str = "XboxLive.signin offline_access";

// ============================================================================
//  错误类型
// ============================================================================

#[derive(Debug, Error)]
pub enum MicrosoftDeviceError {
    #[error("HTTP 请求失败: {0}")]
    Http(String),

    #[error("JSON 解析失败: {0}")]
    Json(#[from] serde_json::Error),

    #[error("设备码请求失败: {0}")]
    DeviceCodeRequest(String),

    #[error("令牌轮询失败: {0}")]
    TokenPolling(String),

    #[error("认证超时")]
    Timeout,

    #[error("用户取消了认证")]
    UserCancelled,

    #[error("账户未购买 Minecraft")]
    NoMinecraftLicense,

    #[error("获取玩家档案失败: {0}")]
    ProfileError(String),

    #[error("Xbox 认证失败: {0}")]
    XboxAuth(String),

    #[error("XSTS 认证失败: {0}")]
    XstsAuth(String),

    #[error("刷新令牌失败: {0}")]
    RefreshFailed(String),
}

pub type MicrosoftDeviceResult<T> = Result<T, MicrosoftDeviceError>;

// ============================================================================
//  数据结构
// ============================================================================

/// 设备码响应（首次请求）
#[derive(Debug, Clone, Deserialize)]
pub struct DeviceCodeResponse {
    pub device_code: String,
    pub user_code: String,
    pub verification_uri: String,
    pub expires_in: usize,
    pub interval: usize,
    pub message: String,
    #[serde(default)]
    pub error: Option<String>,
}

/// 令牌响应（轮询获得）
#[derive(Debug, Clone, Deserialize)]
pub struct TokenResponse {
    pub token_type: String,
    pub scope: String,
    pub expires_in: usize,
    pub access_token: Password,
    pub id_token: Option<String>,
    #[serde(default)]
    pub refresh_token: Option<Password>,
    #[serde(default)]
    pub error: Option<String>,
    #[serde(default)]
    pub error_description: Option<String>,
}

// ============================================================================
//  设备码认证流程（核心结构）
// ============================================================================

/// 微软设备码认证器
pub struct MicrosoftDeviceAuth {
    client_id: String,
}

impl MicrosoftDeviceAuth {
    /// 使用自定义客户端 ID 创建认证器
    pub fn with_client_id(client_id: impl Into<String>) -> Self {
        Self {
            client_id: client_id.into(),
        }
    }

    /// 使用默认客户端 ID 创建认证器（推荐）
    pub fn default_client() -> Self {
        Self {
            client_id: DEFAULT_DEVICE_CLIENT_ID.to_string(),
        }
    }

    /// 步骤 1：获取设备码
    ///
    /// 返回 `DeviceCodeResponse`，其中包含 `user_code` 和 `verification_uri`，
    /// 需要将其展示给用户，让用户在浏览器中完成授权。
    pub async fn get_device_code(&self) -> MicrosoftDeviceResult<DeviceCodeResponse> {
        let client = HttpClient::default();
        let body = format!(
            "client_id={}&scope={}",
            self.client_id,
            urlencoding::encode(DEVICE_SCOPE)
        );

        let response = client
            .inner()
            .post(DEVICE_CODE_URL)
            .header("Content-Type", "application/x-www-form-urlencoded")
            .body(body)
            .send()
            .await
            .map_err(|e| MicrosoftDeviceError::Http(e.to_string()))?;

        let resp: DeviceCodeResponse = response
            .json()
            .await
            .map_err(|e| MicrosoftDeviceError::Json(e))?;

        if let Some(err) = resp.error {
            return Err(MicrosoftDeviceError::DeviceCodeRequest(err));
        }

        Ok(resp)
    }

    /// 步骤 2：轮询令牌（阻塞直到用户完成授权或超时）
    ///
    /// - `device_code`: 从 `get_device_code` 获取
    /// - `interval`: 建议的轮询间隔（秒），默认使用响应中的值
    /// - `timeout`: 总超时时间（默认 5 分钟）
    ///
    /// 返回 `TokenResponse`，包含 `access_token` 和 `refresh_token`。
    pub async fn poll_token(
        &self,
        device_code: &str,
        interval: Option<usize>,
        timeout: Option<Duration>,
    ) -> MicrosoftDeviceResult<TokenResponse> {
        let interval = interval.unwrap_or(5);
        let timeout = timeout.unwrap_or(Duration::from_secs(300));
        let client = HttpClient::default();
        let start = std::time::Instant::now();

        loop {
            if start.elapsed() > timeout {
                return Err(MicrosoftDeviceError::Timeout);
            }

            let body = format!(
                "grant_type=urn:ietf:params:oauth:grant-type:device_code&client_id={}&device_code={}",
                self.client_id, device_code
            );

            let response = client
                .inner()
                .post(TOKEN_URL)
                .header("Content-Type", "application/x-www-form-urlencoded")
                .body(body)
                .send()
                .await
                .map_err(|e| MicrosoftDeviceError::Http(e.to_string()))?;

            let resp: TokenResponse = response
                .json()
                .await
                .map_err(|e| MicrosoftDeviceError::Json(e))?;

            if let Some(err) = resp.error {
                match err.as_str() {
                    "authorization_pending" => {
                        // 用户尚未完成授权，继续等待
                        sleep(Duration::from_secs(interval as u64)).await;
                        continue;
                    }
                    "slow_down" => {
                        // 请求太快，增加间隔
                        let new_interval = (interval as u64).saturating_add(5);
                        sleep(Duration::from_secs(new_interval)).await;
                        continue;
                    }
                    "expired_token" => return Err(MicrosoftDeviceError::Timeout),
                    "access_denied" => return Err(MicrosoftDeviceError::UserCancelled),
                    _ => return Err(MicrosoftDeviceError::TokenPolling(err)),
                }
            }

            // 成功获得令牌
            return Ok(resp);
        }
    }

    /// 步骤 3：使用获得的令牌完成 Minecraft 认证
    ///
    /// 内部会依次完成 Xbox Live、XSTS、Minecraft 服务认证，并获取玩家档案和皮肤。
    pub async fn complete_auth(
        &self,
        access_token: &str,
        refresh_token: &str,
    ) -> MicrosoftDeviceResult<AuthMethod> {
        // 1. Xbox 认证
        let (uhs, xsts_token) = xbox_authenticate(access_token)
            .await
            .map_err(|e| MicrosoftDeviceError::XboxAuth(e.to_string()))?;

        // 2. 获取 XUID
        let xuid = get_xuid(&uhs, &xsts_token)
            .await
            .unwrap_or_else(|_| String::new());

        // 3. 获取 Minecraft 访问令牌
        let minecraft_token = get_minecraft_access_token(&uhs, &xsts_token)
            .await
            .map_err(|e| MicrosoftDeviceError::XstsAuth(e.to_string()))?;

        // 4. 验证许可证
        verify_minecraft_license(&minecraft_token)
            .await
            .map_err(|_| MicrosoftDeviceError::NoMinecraftLicense)?;

        // 5. 获取玩家档案
        let profile = get_minecraft_profile(&minecraft_token)
            .await
            .map_err(|e| MicrosoftDeviceError::ProfileError(e.to_string()))?;

        // 6. 下载并解析皮肤
        let (head_skin, hat_skin) = download_and_parse_skin(&profile.skins)
            .await
            .map_err(|_| MicrosoftDeviceError::ProfileError("皮肤解析失败".into()))?;

        Ok(AuthMethod::Microsoft {
            access_token: minecraft_token,
            refresh_token: refresh_token.to_string().into(),
            uuid: profile.id,
            player_name: profile.name,
            xuid,
            head_skin,
            hat_skin,
        })
    }

    /// 一站式认证：获取设备码 → 轮询 → 完成认证
    ///
    /// # 参数
    /// - `timeout`: 轮询超时时间（默认 5 分钟）
    /// - `progress`: 可选进度报告器（用于显示消息）
    ///
    /// # 返回
    /// 完整的 `AuthMethod::Microsoft`
    pub async fn authenticate(
        &self,
        timeout: Option<Duration>,
        progress: Option<impl Reporter>,
    ) -> MicrosoftDeviceResult<AuthMethod> {
        // 步骤 1：获取设备码
        let device_code_resp = self.get_device_code().await?;

        if let Some(p) = &progress {
            p.set_message(format!(
                "请在浏览器中打开 {}，输入代码 {}",
                device_code_resp.verification_uri, device_code_resp.user_code
            ));
        }

        // 步骤 2：轮询令牌
        let token_resp = self
            .poll_token(
                &device_code_resp.device_code,
                Some(device_code_resp.interval),
                timeout,
            )
            .await?;

        // 步骤 3：完成认证
        let refresh_token = token_resp
            .refresh_token
            .as_ref()
            .ok_or_else(|| MicrosoftDeviceError::TokenPolling("缺少刷新令牌".into()))?;

        self.complete_auth(&token_resp.access_token, refresh_token.as_str())
            .await
    }

    /// 刷新访问令牌（使用 refresh_token）
    pub async fn refresh_token(
        &self,
        refresh_token: &str,
    ) -> MicrosoftDeviceResult<TokenResponse> {
        let client = HttpClient::default();
        let body = format!(
            "grant_type=refresh_token&client_id={}&refresh_token={}",
            self.client_id, refresh_token
        );

        let response = client
            .inner()
            .post(TOKEN_URL)
            .header("Content-Type", "application/x-www-form-urlencoded")
            .body(body)
            .send()
            .await
            .map_err(|e| MicrosoftDeviceError::Http(e.to_string()))?;

        let resp: TokenResponse = response
            .json()
            .await
            .map_err(|e| MicrosoftDeviceError::Json(e))?;

        if let Some(err) = resp.error {
            return Err(MicrosoftDeviceError::RefreshFailed(err));
        }

        Ok(resp)
    }

    /// 刷新并更新 `AuthMethod`
    pub async fn refresh_auth(&self, method: &mut AuthMethod) -> MicrosoftDeviceResult<()> {
        let refresh_token = match method {
            AuthMethod::Microsoft { refresh_token, .. } => refresh_token.as_str().to_string(),
            _ => return Err(MicrosoftDeviceError::RefreshFailed("不是 Microsoft 账户".into())),
        };

        let new_token = self.refresh_token(&refresh_token).await?;

        // 重新执行认证流程（获取新的 Minecraft 令牌）
        let new_method = self
            .complete_auth(
                &new_token.access_token,
                new_token
                    .refresh_token
                    .as_ref()
                    .map(|p| p.as_str())
                    .unwrap_or(&refresh_token),
            )
            .await?;

        // 更新原方法
        if let AuthMethod::Microsoft {
            access_token,
            refresh_token: ref_token,
            uuid,
            player_name,
            xuid,
            head_skin,
            hat_skin,
        } = method
        {
            *access_token = new_method.access_token().unwrap().clone();
            *ref_token = new_token.refresh_token.unwrap_or_else(|| refresh_token.into());
            // 其余字段不变（但可能应该更新，但通常不变）
        }

        Ok(())
    }
}

// ============================================================================
//  兼容旧接口的便捷函数（可选）
// ============================================================================

/// 使用默认客户端 ID 进行一站式认证（简化调用）
pub async fn authenticate(
    timeout: Option<Duration>,
    reporter: Option<impl Reporter>,
) -> MicrosoftDeviceResult<AuthMethod> {
    let auth = MicrosoftDeviceAuth::default_client();
    auth.authenticate(timeout, reporter).await
}

// ============================================================================
//  测试
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    #[ignore] // 需要用户交互
    async fn test_device_code_flow() {
        let auth = MicrosoftDeviceAuth::default_client();
        let result = auth
            .authenticate(Some(Duration::from_secs(120)), Some(NoopReporter))
            .await;
        // 实际运行会等待用户操作
        assert!(result.is_ok() || matches!(result, Err(MicrosoftDeviceError::Timeout)));
    }
}