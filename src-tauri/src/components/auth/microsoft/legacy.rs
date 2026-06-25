//! 传统微软登录模块
//!
//! 通过模仿 Minecraft 官方启动器的 OAuth 2.0 授权码流程，获取访问令牌。
//! 参考：https://wiki.vg/Microsoft_Authentication_Scheme

use serde::Deserialize;
use thiserror::Error;
use url::Url;

use crate::components::auth::parse_head_skin;
use crate::components::auth::structs::AuthMethod;
use crate::components::http::HttpClient;
use crate::components::password::Password;
use crate::prelude::*;

// ============================================================================
//  常量配置
// ============================================================================

/// Minecraft 官方启动器的客户端 ID
pub const CLIENT_ID: &str = "00000000402b5328";

/// OAuth 2.0 作用域
pub const SCOPE: &str = "service::user.auth.xboxlive.com::MBI_SSL";

/// 授权端点
pub const AUTHORIZE_URL: &str = "https://login.live.com/oauth20_authorize.srf";

/// 令牌端点
pub const TOKEN_URL: &str = "https://login.live.com/oauth20_token.srf";

/// 默认的重定向 URI（桌面应用）
pub const REDIRECT_URI: &str = "https://login.live.com/oauth20_desktop.srf";

/// 生成 Microsoft 登录 URL（用于引导用户授权）
pub fn generate_authorize_url() -> String {
    format!(
        "{}?client_id={}&response_type=code&scope={}&redirect_uri={}",
        AUTHORIZE_URL, CLIENT_ID, SCOPE, REDIRECT_URI
    )
}

// ============================================================================
//  错误类型
// ============================================================================

#[derive(Debug, Error)]
pub enum MicrosoftError {
    #[error("HTTP 请求失败: {0}")]
    Http(String),

    #[error("JSON 解析失败: {0}")]
    Json(#[from] serde_json::Error),

    #[error("URL 解析失败: {0}")]
    Url(#[from] url::ParseError),

    #[error("OAuth 令牌请求失败: {0}")]
    TokenRequest(String),

    #[error("Xbox 认证失败: {0}")]
    XboxAuth(String),

    #[error("XSTS 认证失败: {0}")]
    XstsAuth(String),

    #[error("Minecraft 服务认证失败: {0}")]
    MinecraftAuth(String),

    #[error("账户未购买 Minecraft")]
    NoMinecraftLicense,

    #[error("没有找到激活的皮肤")]
    NoActiveSkin,

    #[error("刷新令牌失败: {0}")]
    RefreshFailed(String),

    #[error("无效的授权码")]
    InvalidCode,
}

pub type MicrosoftResult<T> = Result<T, MicrosoftError>;

// ============================================================================
//  内部数据结构
// ============================================================================

#[derive(Debug, Clone, Deserialize)]
struct OAuth20TokenResponse {
    #[serde(default)]
    pub error: String,
    #[serde(rename = "access_token")]
    pub access_token: Password,
    #[serde(rename = "refresh_token")]
    pub refresh_token: Password,
    // 其他字段: token_type, expires_in, scope, user_id
}

#[derive(Debug, Clone, Deserialize)]
struct XboxAuthResponse {
    #[serde(rename = "Token")]
    pub token: String,
    #[serde(rename = "DisplayClaims")]
    pub display_claims: XboxDisplayClaims,
}

#[derive(Debug, Clone, Deserialize)]
struct XboxDisplayClaims {
    pub xui: Vec<XboxUserClaim>,
}

#[derive(Debug, Clone, Deserialize)]
struct XboxUserClaim {
    pub uhs: String,
}

#[derive(Debug, Clone, Deserialize)]
struct MinecraftStoreResponse {
    pub items: Vec<serde_json::Value>,
}

#[derive(Debug, Clone, Deserialize)]
struct MinecraftXboxLoginResponse {
    pub access_token: Password,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MinecraftProfileResponse {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub error: String,
    pub skins: Vec<MinecraftSkin>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MinecraftSkin {
    pub state: String,
    pub url: String,
}

// ============================================================================
//  核心认证流程
// ============================================================================

/// 使用授权码完成完整的认证流程
///
/// # 参数
/// - `code`: 从 Microsoft 登录重定向 URL 中提取的授权码
///
/// # 返回
/// 成功时返回 `AuthMethod::Microsoft`
pub async fn authenticate_with_code(code: &str) -> MicrosoftResult<AuthMethod> {
    // 1. 用授权码换取 OAuth 2.0 令牌
    let (access_token, refresh_token) = request_oauth_token(code, false).await?;

    // 2. 完成 Xbox 认证链
    let (uhs, xsts_token) = xbox_authenticate(access_token.as_str()).await?;

    // 3. 获取 Minecraft 访问令牌
    let minecraft_token = get_minecraft_access_token(&uhs, &xsts_token).await?;

    // 4. 验证 Minecraft 许可证
    verify_minecraft_license(&minecraft_token).await?;

    // 5. 获取玩家档案和皮肤
    let profile = get_minecraft_profile(&minecraft_token).await?;

    // 6. 获取 XUID（可选）
    let xuid = get_xuid(&uhs, &xsts_token).await?;

    // 7. 解析皮肤头像
    let (head_skin, hat_skin) = download_and_parse_skin(&profile.skins).await?;

    Ok(AuthMethod::Microsoft {
        access_token: minecraft_token,
        refresh_token,
        uuid: profile.id,
        player_name: profile.name,
        xuid,
        head_skin,
        hat_skin,
    })
}

// ============================================================================
//  步骤函数
// ============================================================================

/// 步骤 1: 请求 OAuth 2.0 令牌
async fn request_oauth_token(credit: &str, is_refresh: bool) -> MicrosoftResult<(Password, Password)> {
    let client = HttpClient::default();
    let body = format!(
        "client_id={}&{}={}&grant_type={}&redirect_uri={}&scope={}",
        CLIENT_ID,
        if is_refresh { "refresh_token" } else { "code" },
        credit,
        if is_refresh { "refresh_token" } else { "authorization_code" },
        REDIRECT_URI,
        SCOPE
    );

    let response = client
        .inner()
        .post(TOKEN_URL)
        .header("Content-Type", "application/x-www-form-urlencoded")
        .body(body)
        .send()
        .await
        .map_err(|e| MicrosoftError::Http(e.to_string()))?;

    let resp: OAuth20TokenResponse = response
        .json()
        .await
        .map_err(|e| MicrosoftError::Http(e.to_string()))?;

    if !resp.error.is_empty() {
        return Err(MicrosoftError::TokenRequest(format!(
            "OAuth 错误: {}",
            resp.error
        )));
    }

    Ok((resp.access_token, resp.refresh_token))
}

/// 步骤 2: Xbox 认证（获取 userhash 和 XSTS token）
pub async fn xbox_authenticate(access_token: &str) -> MicrosoftResult<(String, String)> {
    // 2a: 认证到 Xbox Live
    let xbox_auth_body = serde_json::json!({
        "Properties": {
            "AuthMethod": "RPS",
            "SiteName": "user.auth.xboxlive.com",
            "RpsTicket": access_token
        },
        "RelyingParty": "http://auth.xboxlive.com",
        "TokenType": "JWT"
    });

    let client = HttpClient::default();
    let xbox_response = client
        .inner()
        .post("https://user.auth.xboxlive.com/user/authenticate")
        .header("Content-Type", "application/json")
        .header("Accept", "application/json")
        .json(&xbox_auth_body)
        .send()
        .await
        .map_err(|e| MicrosoftError::Http(e.to_string()))?;

    if !xbox_response.status().is_success() {
        return Err(MicrosoftError::XboxAuth(
            format!("状态码: {}", xbox_response.status())
        ));
    }

    let xbox_auth: XboxAuthResponse = xbox_response
        .json()
        .await
        .map_err(|e| MicrosoftError::Http(e.to_string()))?;

    let token = xbox_auth.token;
    let uhs = xbox_auth
        .display_claims
        .xui
        .first()
        .ok_or_else(|| MicrosoftError::XboxAuth("缺少 UHS 声明".into()))?
        .uhs
        .clone();

    // 2b: 获取 XSTS 令牌
    let xsts_body = serde_json::json!({
        "Properties": {
            "SandboxId": "RETAIL",
            "UserTokens": [token]
        },
        "RelyingParty": "rp://api.minecraftservices.com/",
        "TokenType": "JWT"
    });

    let xsts_response = client
        .inner()
        .post("https://xsts.auth.xboxlive.com/xsts/authorize")
        .header("Content-Type", "application/json")
        .header("Accept", "application/json")
        .json(&xsts_body)
        .send()
        .await
        .map_err(|e| MicrosoftError::Http(e.to_string()))?;

    if !xsts_response.status().is_success() {
        return Err(MicrosoftError::XstsAuth(
            format!("状态码: {}", xsts_response.status())
        ));
    }

    let xsts_auth: XboxAuthResponse = xsts_response
        .json()
        .await
        .map_err(|e| MicrosoftError::Http(e.to_string()))?;

    Ok((uhs, xsts_auth.token))
}

/// 步骤 3: 获取 Minecraft 访问令牌
pub async fn get_minecraft_access_token(uhs: &str, xsts_token: &str) -> MicrosoftResult<Password> {
    let body = serde_json::json!({
        "identityToken": format!("XBL3.0 x={};{}", uhs, xsts_token)
    });

    let client = HttpClient::default();
    let response = client
        .inner()
        .post("https://api.minecraftservices.com/authentication/login_with_xbox")
        .header("Content-Type", "application/json")
        .header("Accept", "application/json")
        .json(&body)
        .send()
        .await
        .map_err(|e| MicrosoftError::Http(e.to_string()))?;

    let status = response.status();
    if !status.is_success() {
        let error_text = response.text().await.unwrap_or_default();
        return Err(MicrosoftError::MinecraftAuth(format!(
            "状态码: {}, 响应: {}",
            status,
            error_text
        )));
    }

    let resp: MinecraftXboxLoginResponse = response
        .json()
        .await
        .map_err(|e| MicrosoftError::Http(e.to_string()))?;

    Ok(resp.access_token)
}

/// 步骤 4: 验证 Minecraft 许可证
pub async fn verify_minecraft_license(access_token: &Password) -> MicrosoftResult<()> {
    let client = HttpClient::default();
    let response = client
        .inner()
        .get("https://api.minecraftservices.com/entitlements/mcstore")
        .header("Authorization", format!("Bearer {}", access_token.as_str()))
        .send()
        .await
        .map_err(|e| MicrosoftError::Http(e.to_string()))?;

    if !response.status().is_success() {
        return Err(MicrosoftError::MinecraftAuth(
            "无法验证许可证".into(),
        ));
    }

    let store: MinecraftStoreResponse = response
        .json()
        .await
        .map_err(|e| MicrosoftError::Http(e.to_string()))?;

    if store.items.is_empty() {
        return Err(MicrosoftError::NoMinecraftLicense);
    }

    Ok(())
}

/// 步骤 5: 获取 Minecraft 玩家档案
pub async fn get_minecraft_profile(access_token: &Password) -> MicrosoftResult<MinecraftProfileResponse> {
    let client = HttpClient::default();
    let response = client
        .inner()
        .get("https://api.minecraftservices.com/minecraft/profile")
        .header("Authorization", format!("Bearer {}", access_token.as_str()))
        .send()
        .await
        .map_err(|e| MicrosoftError::Http(e.to_string()))?;

    if !response.status().is_success() {
        let error_text = response.text().await.unwrap_or_default();
        return Err(MicrosoftError::MinecraftAuth(format!(
            "获取档案失败: {}",
            error_text
        )));
    }

    let profile: MinecraftProfileResponse = response
        .json()
        .await
        .map_err(|e| MicrosoftError::Http(e.to_string()))?;

    if !profile.error.is_empty() {
        return Err(MicrosoftError::MinecraftAuth(profile.error));
    }

    Ok(profile)
}

/// 步骤 6: 获取 XUID（用户 ID）
pub async fn get_xuid(uhs: &str, xsts_token: &str) -> MicrosoftResult<String> {
    let client = HttpClient::default();
    let response = client
        .inner()
        .get("https://userpresence.xboxlive.com/users/me?level=user")
        .header("Authorization", format!("XBL3.0 x={};{}", uhs, xsts_token))
        .header("x-xbl-contract-version", "3.2")
        .header("Accept", "application/json")
        .header("Accept-Language", "zh-CN")
        .send()
        .await
        .map_err(|e| MicrosoftError::Http(e.to_string()))?;

    if !response.status().is_success() {
        return Ok(String::new()); // XUID 不是必需的
    }

    // 解析 JSON 获取 xuid
    let json: serde_json::Value = response
        .json()
        .await
        .map_err(|e| MicrosoftError::Http(e.to_string()))?;
    Ok(json
        .pointer("/xuid")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string())
}

/// 步骤 7: 下载并解析皮肤头像
pub async fn download_and_parse_skin(skins: &[MinecraftSkin]) -> MicrosoftResult<(Vec<u8>, Vec<u8>)> {
    let active_skin = skins
        .iter()
        .find(|s| s.state == "ACTIVE")
        .ok_or(MicrosoftError::NoActiveSkin)?;

    let client = HttpClient::default();
    let skin_bytes = client
        .get_bytes(&active_skin.url)
        .await
        .map_err(|e| MicrosoftError::Http(e.to_string()))?;

    // parse_head_skin 内部使用 spawn_blocking
    parse_head_skin(skin_bytes)
        .await
        .map_err(|e| MicrosoftError::Http(e.to_string()))
}

// ============================================================================
//  刷新令牌
// ============================================================================

/// 刷新 Microsoft 账户的访问令牌
///
/// # 参数
/// - `method`: 必须是 `AuthMethod::Microsoft` 可变引用
///
/// # 返回
/// 成功时更新 `method` 并返回 `Ok(())`
pub async fn refresh_auth(method: &mut AuthMethod) -> MicrosoftResult<()> {
    let refresh_token = match method {
        AuthMethod::Microsoft { refresh_token, .. } => refresh_token.clone(),
        _ => return Err(MicrosoftError::RefreshFailed("不是 Microsoft 账户".into())),
    };

    // 用 refresh_token 换新令牌
    let (new_access_token, new_refresh_token) =
        request_oauth_token(refresh_token.as_str(), true).await?;

    // 完成 Xbox 和 Minecraft 认证
    let (uhs, xsts_token) = xbox_authenticate(new_access_token.as_str()).await?;
    let minecraft_token = get_minecraft_access_token(&uhs, &xsts_token).await?;

    // 更新 method
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
        *access_token = minecraft_token;
        *ref_token = new_refresh_token;
    }

    Ok(())
}

// ============================================================================
//  兼容旧接口（使用 URL 解析）
// ============================================================================

/// 从重定向 URL 中提取授权码并完成认证（兼容旧版）
///
/// # 参数
/// - `url`: 重定向 URL（如 `https://login.live.com/oauth20_desktop.srf?code=...&lc=1033`）
pub async fn start_auth(_ctx: Option<impl Reporter>, url: &str) -> MicrosoftResult<AuthMethod> {
    let parsed = Url::parse(url)?;
    let code = parsed
        .query_pairs()
        .find(|(key, _)| key == "code")
        .map(|(_, value)| value.to_string())
        .ok_or(MicrosoftError::InvalidCode)?;

    authenticate_with_code(&code).await
}

// ============================================================================
//  测试
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_authorize_url() {
        let url = generate_authorize_url();
        assert!(url.contains(CLIENT_ID));
        assert!(url.contains(SCOPE));
        assert!(url.contains(REDIRECT_URI));
    }

    #[tokio::test]
    #[ignore] // 需要网络和有效的授权码
    async fn test_authenticate_with_code() {
        // 此测试需要手动获取授权码
        let code = "dummy_code"; // 替换为实际代码
        let result = authenticate_with_code(code).await;
        // 预期会失败（无效 code）
        assert!(result.is_err());
    }
}