/*!
    此模块为登录验证模块，开发者可以调用此处的函数获取不同种类账户验证之后的登录令牌。
*/

use std::io::Cursor;
use std::path::Path;

use base64::prelude::*;
use image::{GenericImageView, Pixel};
use thiserror::Error;
use tokio::task::spawn_blocking;

use structs::mojang::{ProfileResponse, ProfileTexture};

use self::structs::AuthMethod;
use crate::{
    http::HttpClient,
    password::Password,
    prelude::*,
};

pub mod authlib;
pub mod microsoft;
pub mod structs;

// ============================================================================
//  错误类型
// ============================================================================

#[derive(Debug, Error)]
pub enum AuthError {
    #[error("HTTP 请求失败: {0}")]
    Http(String),

    #[error("JSON 解析失败: {0}")]
    Json(#[from] serde_json::Error),

    #[error("图片解码失败: {0}")]
    Image(#[from] image::ImageError),

    #[error("Base64 解码失败: {0}")]
    Base64(#[from] base64::DecodeError),

    #[error("账户认证失败: {0}")]
    AuthFailed(String),

    #[error("账户没有可用的游戏档案 (Profile)")]
    NoProfile,

    #[error("缺少玩家皮肤信息")]
    NoSkin,

    #[error("IO 错误: {0}")]
    Io(#[from] std::io::Error),

    #[error("阻塞任务失败: {0}")]
    Join(#[from] tokio::task::JoinError),
}

pub type AuthResult<T> = Result<T, AuthError>;

// ============================================================================
//  离线 UUID 生成
// ============================================================================

/// 根据玩家名称生成一个固定的离线 UUID
///
/// 生成方式参考：<https://github.com/PrismarineJS/node-minecraft-protocol/blob/21240f8ab2fd41c76f50b64e3b3a945f50b25b5e/src/datatypes/uuid.js#L14>
///
/// # 示例
/// ```rust
/// # use scl_core::auth::generate_offline_uuid;
/// # fn main() {
/// assert_eq!(format!("{:x}", generate_offline_uuid("Steve")), "5627dd98e6be3c21b8a8e92344183641");
/// assert_eq!(format!("{:x}", generate_offline_uuid("Alex")), "36532b5ec4423dbba24cc7e55d0f979a");
/// # }
/// ```
pub fn generate_offline_uuid(player_name: &str) -> md5::Digest {
    let mut ctx = md5::Context::new();
    ctx.consume("OfflinePlayer:");
    ctx.consume(player_name);
    let mut result = ctx.compute().0;

    // 设置版本和变体位
    result[6] = (result[6] & 0x0f) | 0x30;
    result[8] = (result[8] & 0x3f) | 0x80;

    md5::Digest(result)
}

// ============================================================================
//  皮肤头像解析
// ============================================================================

/// 提取皮肤位图的正面头部部分（8x8 像素），用于 GUI 展示头像。
///
/// 返回 `(头部皮肤, 帽子层皮肤)` 两个 RGBA 位图数据，每个大小为 8x8x4 字节。
///
/// # 参数
/// - `skin_data`: PNG 格式的皮肤图片字节数据（必须是 64x32 或 64x64 格式）
///
/// # 错误
/// 如果图片格式无效或尺寸不正确，返回 `AuthError::Image`。
///
/// # 注意
/// 此函数内部使用 `spawn_blocking` 执行图像解码，避免阻塞异步运行时。
pub async fn parse_head_skin(skin_data: Vec<u8>) -> AuthResult<(Vec<u8>, Vec<u8>)> {
    spawn_blocking(move || {
        let cursor = Cursor::new(skin_data);
        let skin = image::load(cursor, image::ImageFormat::Png)?;

        // 验证皮肤尺寸（标准 Minecraft 皮肤为 64x32 或 64x64）
        let (width, height) = skin.dimensions();
        if width != 64 || (height != 32 && height != 64) {
            return Err(AuthError::Image(image::ImageError::Unsupported(
                image::error::UnsupportedError::from_format_and_kind(
                    image::ImageFormat::Png,
                    format!("不支持的皮肤尺寸: {}x{}", width, height),
                ),
            )));
        }

        let mut head_data = Vec::with_capacity(8 * 8 * 4);
        let mut hat_data = Vec::with_capacity(8 * 8 * 4);

        // 提取头部（8-15, 8-15）
        for y in 8..16 {
            for x in 8..16 {
                let pixel = skin.get_pixel(x, y).to_rgba();
                head_data.extend_from_slice(&pixel.0);
            }
        }

        // 提取帽子层（40-47, 8-15）
        for y in 8..16 {
            for x in 40..48 {
                let pixel = skin.get_pixel(x, y).to_rgba();
                hat_data.extend_from_slice(&pixel.0);
            }
        }

        Ok((head_data, hat_data))
    })
    .await
    .map_err(|e| AuthError::Join(e))?
}

// ============================================================================
//  获取皮肤头像（从 Mojang API）
// ============================================================================

/// 从 Mojang Session Server 获取玩家的皮肤头像
///
/// # 返回
/// - `Ok((head_skin, hat_skin))`: 成功获取皮肤
/// - `Err(AuthError::NoSkin)`: 玩家没有皮肤
/// - `Err(AuthError::Http)`: 网络请求失败
/// - `Err(AuthError::Image)`: 图片解析失败
pub async fn get_head_skin(uuid: &str) -> AuthResult<(Vec<u8>, Vec<u8>)> {
    let url = format!("https://sessionserver.mojang.com/session/minecraft/profile/{}", uuid);
    let client = HttpClient::default();

    let profile: ProfileResponse = client
        .get_json(&url)
        .await
        .map_err(|e| AuthError::Http(e.to_string()))?;

    // 查找 textures 属性
    let texture_prop = profile
        .properties
        .iter()
        .find(|p| p.name == "textures")
        .ok_or(AuthError::NoSkin)?;

    // Base64 解码
    let texture_json = String::from_utf8_lossy(&BASE64_STANDARD.decode(&texture_prop.value)?);
    let texture_data: ProfileTexture = serde_json::from_str(&texture_json)?;

    // 提取皮肤 URL
    let skin_url = texture_data
        .textures
        .as_ref()
        .and_then(|t| t.skin.as_ref())
        .map(|s| &s.url)
        .ok_or(AuthError::NoSkin)?;

    // 下载皮肤图片
    let skin_bytes = client
        .get_bytes(skin_url)
        .await
        .map_err(|e| AuthError::Http(e.to_string()))?;

    parse_head_skin(skin_bytes).await
}

// ============================================================================
//  Mojang 正版验证（已弃用）
// ============================================================================

/// 进行 Mojang 正版验证
///
/// **此验证方式已被 Mojang 废弃**，请迁移到 Microsoft 账户验证。
///
/// 建议使用 [`crate::auth::microsoft::start_auth`] 进行 Microsoft 正版验证。
///
/// # 弃用原因
/// Mojang 账户已全面迁移到 Microsoft 账户体系，此方法可能随时失效。
#[deprecated(
    since = "0.2.0",
    note = "Mojang 账户验证已被废弃，请使用 `microsoft::start_auth`"
)]
pub async fn auth_mojang(
    _ctx: Option<impl Reporter>,
    username: &str,
    password: &Password,
    client_token: &str,
) -> AuthResult<AuthMethod> {
    let body = structs::mojang::AuthenticateBody {
        username: username.to_string(),
        password: password.clone(),
        client_token: client_token.to_string(),
        ..Default::default()
    };

    let client = HttpClient::default();
    // 注意：HttpClient 目前没有直接提供 post_json 方法，我们使用 reqwest 直接发送
    // 这里为了演示，使用标准 reqwest
    let response = reqwest::Client::new()
        .post("https://authserver.mojang.com/authenticate")
        .json(&body)
        .send()
        .await
        .map_err(|e| AuthError::Http(e.to_string()))?;

    if !response.status().is_success() {
        // 尝试解析错误响应
        if let Ok(error_resp) = response.json::<structs::mojang::ErrorResponse>().await {
            return Err(AuthError::AuthFailed(format!(
                "{}: {}",
                error_resp.error, error_resp.error_message
            )));
        }
        return Err(AuthError::AuthFailed(format!(
            "认证失败 (HTTP {})",
            response.status()
        )));
    }

    let auth_resp: structs::mojang::AuthenticateResponse = response
        .json()
        .await
        .map_err(|e| AuthError::Json(e))?;

    let selected_profile = auth_resp
        .selected_profile
        .or_else(|| auth_resp.available_profiles.into_iter().next())
        .ok_or(AuthError::NoProfile)?;

    let (head_skin, hat_skin) = get_head_skin(&selected_profile.id).await?;

    Ok(AuthMethod::Mojang {
        access_token: auth_resp.access_token,
        uuid: selected_profile.id,
        player_name: selected_profile.name,
        head_skin,
        hat_skin,
    })
}

// ============================================================================
//  刷新访问令牌
// ============================================================================

/// 刷新/续期访问令牌
///
/// # 返回
/// - `Ok(true)`: 刷新成功
/// - `Ok(false)`: 刷新失败（令牌无效或网络问题）
/// - `Err(_)`: 发生严重错误
pub async fn refresh_auth(am: &mut AuthMethod, client_token: &str) -> AuthResult<bool> {
    match am {
        AuthMethod::Mojang { access_token, .. } => {
            refresh_mojang_token(access_token, client_token).await
        }
        AuthMethod::Microsoft { .. } => {
            // 调用微软登录模块的刷新方法
            microsoft::leagcy::refresh_auth(am).await
                .map_err(|e| AuthError::AuthFailed(e.to_string()))
        }
        AuthMethod::AuthlibInjector { .. } => {
            let new_am = crate::auth::authlib::refresh_token(am.clone(), client_token, false)
                .await
                .map_err(|e| AuthError::AuthFailed(e.to_string()))?;
            *am = new_am;
            Ok(true)
        }
        AuthMethod::Offline { .. } => {
            // 离线账户无需刷新
            Ok(true)
        }
    }
}

/// 刷新 Mojang 访问令牌
async fn refresh_mojang_token(access_token: &Password, client_token: &str) -> AuthResult<bool> {
    let body = structs::mojang::ValidateResponse {
        access_token: access_token.clone(),
        client_token: client_token.to_string(),
    };

    let response = reqwest::Client::new()
        .post("https://authserver.mojang.com/validate")
        .json(&body)
        .send()
        .await
        .map_err(|e| AuthError::Http(e.to_string()))?;

    if response.status().is_success() {
        Ok(true)
    } else {
        // 检查是否是令牌过期（通常返回 403）
        if response.status() == 403 {
            Ok(false)
        } else {
            // 其他错误视为临时失败，返回 false
            tracing::warn!("Mojang 令牌刷新失败: HTTP {}", response.status());
            Ok(false)
        }
    }
}

// ============================================================================
//  便捷函数：离线登录
// ============================================================================

/// 创建一个离线账户
pub fn create_offline_account(player_name: &str) -> AuthMethod {
    let uuid = generate_offline_uuid(player_name);
    AuthMethod::Offline {
        player_name: player_name.to_string(),
        uuid: format!("{:x}", uuid),
    }
}

// ============================================================================
//  测试
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_offline_uuid() {
        assert_eq!(
            format!("{:x}", generate_offline_uuid("Steve")),
            "5627dd98e6be3c21b8a8e92344183641"
        );
        assert_eq!(
            format!("{:x}", generate_offline_uuid("Alex")),
            "36532b5ec4423dbba24cc7e55d0f979a"
        );
    }

    #[test]
    fn test_create_offline_account() {
        let account = create_offline_account("TestPlayer");
        match account {
            AuthMethod::Offline { player_name, uuid } => {
                assert_eq!(player_name, "TestPlayer");
                assert!(!uuid.is_empty());
            }
            _ => panic!("Expected Offline account"),
        }
    }

    #[tokio::test]
    #[ignore] // 需要网络
    async fn test_get_head_skin() {
        // 使用 Notch 的 UUID（已知有皮肤）
        let result = get_head_skin("069a79f4-44e9-4726-a5be-fca90e38aaf5").await;
        assert!(result.is_ok() || matches!(result, Err(AuthError::NoSkin)));
    }
}