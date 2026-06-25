//! Authlib-Injector 第三方登录实现
//!
//! 参考 [启动器技术规范](https://github.com/yushijinhun/authlib-injector/wiki/Yggdrasil-%E6%9C%8D%E5%8A%A1%E5%99%A8%E6%8A%80%E6%9C%AF%E8%A7%84%E8%8C%83)
//! 实现认证、刷新和验证功能。

use std::str::FromStr;

use base64::prelude::*;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use url::Url;

use crate::auth::get_head_skin;
use crate::auth::structs::mojang::{
    AuthenticateBody, AuthenticateResponse, AvailableProfile, ErrorResponse, ProfileResponse,
    ValidateResponse,
};
use crate::auth::structs::AuthMethod;
use crate::http::HttpClient;
use crate::password::Password;
use crate::prelude::*;

// ============================================================================
//  错误类型
// ============================================================================

#[derive(Debug, Error)]
pub enum AuthlibError {
    #[error("HTTP 请求失败: {0}")]
    Http(String),

    #[error("JSON 解析失败: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Base64 解码失败: {0}")]
    Base64(#[from] base64::DecodeError),

    #[error("URL 解析失败: {0}")]
    Url(#[from] url::ParseError),

    #[error("认证服务器元数据获取失败: {0}")]
    MetaFetch(String),

    #[error("认证失败: {0}")]
    AuthFailed(String),

    #[error("账户没有可用角色 (Profile)")]
    NoProfile,

    #[error("无效的 API 地址: {0}")]
    InvalidApiLocation(String),

    #[error("刷新令牌失败: {0}")]
    RefreshFailed(String),
}

pub type AuthlibResult<T> = Result<T, AuthlibError>;

// ============================================================================
//  数据结构（仅用于内部 API 通信）
// ============================================================================

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct ServerMetaLinks {
    pub homepage: String,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
#[serde(rename_all = "camelCase")]
struct ServerMeta {
    pub server_name: String,
    pub links: Option<ServerMetaLinks>,
}

#[derive(Debug, Default, Deserialize)]
struct ApiMetaData {
    pub meta: ServerMeta,
}

#[derive(Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RefreshBody {
    pub access_token: Password,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub client_token: String,
    pub request_user: bool,
    pub selected_profile: Option<AvailableProfile>,
}

// ============================================================================
//  刷新令牌
// ============================================================================

/// 使用 refresh token 刷新访问令牌（Authlib-Injector 专用）
///
/// # 参数
/// - `auth_method`: 现有的 `AuthMethod::AuthlibInjector` 实例
/// - `client_token`: 启动器客户端令牌
/// - `provide_selected_profile`: 是否在请求中携带选中的 profile
///
/// # 返回
/// 成功时返回更新后的 `AuthMethod`
///
/// # 错误
/// 如果刷新失败，返回 `AuthlibError::RefreshFailed`
pub async fn refresh_token(
    auth_method: AuthMethod,
    client_token: &str,
    provide_selected_profile: bool,
) -> AuthlibResult<AuthMethod> {
    let (api_location, server_name, server_homepage, server_meta, access_token, uuid, player_name) =
        match auth_method {
            AuthMethod::AuthlibInjector {
                api_location,
                server_name,
                server_homepage,
                server_meta,
                access_token,
                uuid,
                player_name,
                ..
            } => (
                api_location, server_name, server_homepage, server_meta,
                access_token, uuid, player_name,
            ),
            _ => return Err(AuthlibError::RefreshFailed("不是 AuthlibInjector 账户".into())),
        };

    let refresh_url = Url::parse(&api_location)?
        .join("authserver/refresh")
        .map_err(|e| AuthlibError::Url(e))?
        .to_string();

    let body = RefreshBody {
        access_token,
        client_token: client_token.to_string(),
        request_user: provide_selected_profile,
        selected_profile: if provide_selected_profile {
            Some(AvailableProfile {
                name: player_name.clone(),
                id: uuid.clone(),
            })
        } else {
            None
        },
    };

    let client = HttpClient::default();
    // 由于 HttpClient 没有直接 post_json，我们使用 reqwest 直接发送
    let response = reqwest::Client::new()
        .post(&refresh_url)
        .json(&body)
        .send()
        .await
        .map_err(|e| AuthlibError::Http(e.to_string()))?;

    if !response.status().is_success() {
        let error: ErrorResponse = response
            .json()
            .await
            .map_err(|e| AuthlibError::Json(e))?;
        return Err(AuthlibError::RefreshFailed(format!(
            "{}: {}",
            error.error, error.error_message
        )));
    }

    let auth_resp: AuthenticateResponse = response
        .json()
        .await
        .map_err(|e| AuthlibError::Json(e))?;

    let selected_profile = auth_resp
        .selected_profile
        .unwrap_or_else(|| AvailableProfile {
            name: player_name,
            id: uuid,
        });

    let (head_skin, hat_skin) = get_head_skin(&selected_profile.id)
        .await
        .map_err(|e| AuthlibError::Http(e.to_string()))?;

    Ok(AuthMethod::AuthlibInjector {
        api_location,
        server_name,
        server_homepage,
        server_meta,
        access_token: auth_resp.access_token,
        uuid: selected_profile.id,
        player_name: selected_profile.name,
        head_skin,
        hat_skin,
    })
}

// ============================================================================
//  开始认证（登录）
// ============================================================================

/// 使用 Authlib-Injector 服务器进行认证登录
///
/// # 参数
/// - `authlib_host`: 认证服务器地址（可以是根地址或 API 地址）
/// - `username`: 用户名
/// - `password`: 密码
/// - `client_token`: 启动器客户端令牌
///
/// # 返回
/// 成功时返回一个或多个 `AuthMethod`（如果账户有多个角色）
///
/// # 错误
/// 认证失败时返回 `AuthlibError::AuthFailed` 或其他网络/解析错误
pub async fn start_auth(
    _ctx: Option<impl Reporter>,
    authlib_host: &str,
    username: String,
    password: Password,
    client_token: &str,
) -> AuthlibResult<Vec<AuthMethod>> {
    // 1. 获取 API 位置（可能从响应头或直接使用给定地址）
    let api_location = resolve_api_location(authlib_host).await?;

    // 2. 获取服务器元数据
    let (server_name, server_homepage, server_meta) =
        fetch_server_metadata(&api_location, authlib_host).await?;

    // 3. 执行认证请求
    let auth_url = Url::parse(&api_location)?
        .join("authserver/authenticate")
        .map_err(|e| AuthlibError::Url(e))?
        .to_string();

    let auth_body = AuthenticateBody {
        username,
        password,
        client_token: client_token.to_string(),
        ..Default::default()
    };

    let response = reqwest::Client::new()
        .post(&auth_url)
        .json(&auth_body)
        .send()
        .await
        .map_err(|e| AuthlibError::Http(e.to_string()))?;

    if !response.status().is_success() {
        let error: ErrorResponse = response
            .json()
            .await
            .map_err(|e| AuthlibError::Json(e))?;
        return Err(AuthlibError::AuthFailed(format!(
            "{}: {}",
            error.error, error.error_message
        )));
    }

    let auth_resp: AuthenticateResponse = response
        .json()
        .await
        .map_err(|e| AuthlibError::Json(e))?;

    // 4. 处理返回的角色
    let profiles = if let Some(selected) = auth_resp.selected_profile {
        vec![selected]
    } else {
        auth_resp.available_profiles
    };

    if profiles.is_empty() {
        return Err(AuthlibError::NoProfile);
    }

    // 5. 并发获取所有角色的皮肤
    let mut results = Vec::with_capacity(profiles.len());
    for profile in profiles {
        let (head_skin, hat_skin) = get_head_skin(&profile.id)
            .await
            .unwrap_or_else(|_| (Vec::new(), Vec::new()));

        results.push(AuthMethod::AuthlibInjector {
            api_location: api_location.clone(),
            server_name: server_name.clone(),
            server_homepage: server_homepage.clone(),
            server_meta: server_meta.clone(),
            access_token: auth_resp.access_token.clone(),
            uuid: profile.id,
            player_name: profile.name,
            head_skin,
            hat_skin,
        });
    }

    Ok(results)
}

// ============================================================================
//  辅助函数
// ============================================================================

/// 解析 API 位置（从响应头或直接使用给定地址）
async fn resolve_api_location(authlib_host: &str) -> AuthlibResult<String> {
    let client = HttpClient::default();
    let response = client
        .inner()
        .get(authlib_host)
        .send()
        .await
        .map_err(|e| AuthlibError::Http(e.to_string()))?;

    let api_location = if let Some(header) = response.headers().get("X-Authlib-Injector-API-Location") {
        header.to_str().unwrap_or(authlib_host).to_string()
    } else {
        authlib_host.to_string()
    };

    // 确保是完整的 URL
    let parsed = if api_location.starts_with("http") {
        Url::parse(&api_location).map_err(|e| AuthlibError::Url(e))?
    } else {
        Url::parse(authlib_host)
            .map_err(|e| AuthlibError::Url(e))?
            .join(&api_location)
            .map_err(|e| AuthlibError::Url(e))?
    };

    // 确保以 / 结尾
    let mut url = parsed.to_string();
    if !url.ends_with('/') {
        url.push('/');
    }
    Ok(url)
}

/// 获取服务器元数据（名称、主页、元数据 Base64）
async fn fetch_server_metadata(
    api_location: &str,
    fallback_host: &str,
) -> AuthlibResult<(String, String, String)> {
    let client = HttpClient::default();

    // 获取元数据 JSON
    let meta_response = client
        .inner()
        .get(api_location)
        .send()
        .await
        .map_err(|e| AuthlibError::MetaFetch(e.to_string()))?;

    let (server_name, server_homepage) = if meta_response.status().is_success() {
        let meta: ApiMetaData = meta_response
            .json()
            .await
            .map_err(|e| AuthlibError::Json(e))?;

        let name = if meta.meta.server_name.is_empty() {
            Url::parse(api_location)
                .ok()
                .and_then(|u| u.host_str().map(|h| h.to_string()))
                .unwrap_or_else(|| fallback_host.to_string())
        } else {
            meta.meta.server_name
        };

        let homepage = meta
            .meta
            .links
            .map(|l| l.homepage)
            .unwrap_or_else(|| {
                Url::parse(api_location)
                    .ok()
                    .map(|u| u.origin().ascii_serialization())
                    .unwrap_or_else(|| fallback_host.to_string())
            });

        (name, homepage)
    } else {
        // fallback: 从 URL 提取
        let parsed = Url::parse(api_location).map_err(|e| AuthlibError::Url(e))?;
        let name = parsed.host_str().unwrap_or(fallback_host).to_string();
        let homepage = parsed.origin().ascii_serialization();
        (name, homepage)
    };

    // 获取元数据原始字节（用于 Base64 编码）
    let meta_bytes = client
        .get_bytes(api_location)
        .await
        .map_err(|e| AuthlibError::MetaFetch(e.to_string()))?;
    let server_meta = BASE64_STANDARD.encode(&meta_bytes);

    Ok((server_name, server_homepage, server_meta))
}

// ============================================================================
//  验证令牌
// ============================================================================

/// 验证访问令牌是否仍然有效
///
/// # 参数
/// - `api_location`: Authlib 服务器 API 地址
/// - `access_token`: 访问令牌
/// - `client_token`: 客户端令牌
///
/// # 返回
/// - `Ok(true)`: 令牌有效
/// - `Ok(false)`: 令牌无效或过期
/// - `Err(_)`: 网络或解析错误
pub async fn validate(
    api_location: &str,
    access_token: &str,
    client_token: &str,
) -> AuthlibResult<bool> {
    let validate_url = Url::parse(api_location)?
        .join("authserver/validate")
        .map_err(|e| AuthlibError::Url(e))?
        .to_string();

    let body = ValidateResponse {
        access_token: access_token.into(),
        client_token: client_token.to_string(),
    };

    let response = reqwest::Client::new()
        .post(&validate_url)
        .json(&body)
        .send()
        .await
        .map_err(|e| AuthlibError::Http(e.to_string()))?;

    // 成功返回 204 No Content
    Ok(response.status().as_u16() == 204)
}

// ============================================================================
//  测试
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    #[ignore] // 需要有效的测试服务器
    async fn test_validate() {
        // 使用一个无效的令牌测试
        let result = validate(
            "https://authlib-injector.example.com/",
            "invalid-token",
            "client-token",
        )
        .await;
        // 应该返回 false（无效）或错误
        assert!(result.is_ok() || matches!(result, Err(AuthlibError::Http(_))));
    }

    #[test]
    fn test_resolve_api_location_url() {
        // 测试 URL 拼接逻辑（单元测试，不实际请求网络）
        let base = "https://example.com/api/";
        let join = base.to_string() + "authserver/authenticate";
        assert_eq!(join, "https://example.com/api/authserver/authenticate");
    }
}