//! 登录验证数据结构
//!
//! 包含账户认证方式（离线、Mojang、微软、外置登录）以及相关的请求/响应模型。

use std::fmt;

use serde::{Deserialize, Serialize};

use crate::password::Password;

// ============================================================================
//  认证方式枚举（公开 API）
// ============================================================================

/// 账户认证方式，用于启动游戏时传递玩家身份信息。
///
/// 各变体包含对应认证类型所需的全部凭证和元数据。
#[derive(Debug, Deserialize, Serialize, Clone, PartialEq, Eq)]
pub enum AuthMethod {
    /// 离线账户（无需网络验证）
    Offline {
        /// 玩家名称（显示在游戏内）
        player_name: String,
        /// 玩家的唯一标识符（UUID），用于保存存档和物品栏。
        /// 如果从其他启动器迁移，请保留原 UUID 以保证数据兼容。
        uuid: String,
    },
    /// Mojang 官方账户（Yggdrasil 认证）
    Mojang {
        /// 访问令牌（Access Token），用于启动游戏和 API 请求。
        access_token: Password,
        /// 玩家的 UUID
        uuid: String,
        /// 玩家名称
        player_name: String,
        /// 头部皮肤位图数据（RGBA 格式，8x8 像素），用于显示头像
        head_skin: Vec<u8>,
        /// 头发（帽子）层皮肤位图数据（RGBA 格式，8x8 像素）
        hat_skin: Vec<u8>,
    },
    /// 微软账户（Microsoft OAuth）
    Microsoft {
        /// 访问令牌（Access Token）
        access_token: Password,
        /// 刷新令牌（Refresh Token），用于自动续期
        refresh_token: Password,
        /// 玩家的 UUID
        uuid: String,
        /// 微软 XBox 用户 ID（XUID），某些新版本 Minecraft 需要此字段
        xuid: String,
        /// 玩家名称
        player_name: String,
        /// 头部皮肤位图数据（RGBA 格式，8x8 像素）
        head_skin: Vec<u8>,
        /// 头发（帽子）层皮肤位图数据（RGBA 格式，8x8 像素）
        hat_skin: Vec<u8>,
    },
    /// Authlib-Injector 外置登录（第三方认证服务器）
    AuthlibInjector {
        /// 认证服务器的 API 基础 URL
        api_location: String,
        /// 服务器显示名称（用于 GUI）
        server_name: String,
        /// 服务器主页（用于 GUI 跳转）
        server_homepage: String,
        /// 服务器元数据（启动时需要传递给游戏的参数）
        server_meta: String,
        /// 访问令牌（Access Token）
        access_token: Password,
        /// 玩家的 UUID
        uuid: String,
        /// 玩家名称
        player_name: String,
        /// 头部皮肤位图数据（RGBA 格式，8x8 像素）
        head_skin: Vec<u8>,
        /// 头发（帽子）层皮肤位图数据（RGBA 格式，8x8 像素）
        hat_skin: Vec<u8>,
    },
}

// ============================================================================
//  AuthMethod 辅助方法
// ============================================================================

impl AuthMethod {
    /// 获取玩家名称（所有变体通用）
    pub fn player_name(&self) -> &str {
        match self {
            AuthMethod::Offline { player_name, .. } => player_name,
            AuthMethod::Mojang { player_name, .. } => player_name,
            AuthMethod::Microsoft { player_name, .. } => player_name,
            AuthMethod::AuthlibInjector { player_name, .. } => player_name,
        }
    }

    /// 获取玩家 UUID（所有变体通用）
    pub fn uuid(&self) -> &str {
        match self {
            AuthMethod::Offline { uuid, .. } => uuid,
            AuthMethod::Mojang { uuid, .. } => uuid,
            AuthMethod::Microsoft { uuid, .. } => uuid,
            AuthMethod::AuthlibInjector { uuid, .. } => uuid,
        }
    }

    /// 获取访问令牌（如果有），返回 `Option<&Password>`
    pub fn access_token(&self) -> Option<&Password> {
        match self {
            AuthMethod::Offline { .. } => None,
            AuthMethod::Mojang { access_token, .. } => Some(access_token),
            AuthMethod::Microsoft { access_token, .. } => Some(access_token),
            AuthMethod::AuthlibInjector { access_token, .. } => Some(access_token),
        }
    }

    /// 获取头部皮肤位图（如果存在），返回 `Option<&[u8]>`
    pub fn head_skin(&self) -> Option<&[u8]> {
        match self {
            AuthMethod::Offline { .. } => None,
            AuthMethod::Mojang { head_skin, .. } => Some(head_skin.as_slice()),
            AuthMethod::Microsoft { head_skin, .. } => Some(head_skin.as_slice()),
            AuthMethod::AuthlibInjector { head_skin, .. } => Some(head_skin.as_slice()),
        }
    }

    /// 获取帽子层皮肤位图（如果存在），返回 `Option<&[u8]>`
    pub fn hat_skin(&self) -> Option<&[u8]> {
        match self {
            AuthMethod::Offline { .. } => None,
            AuthMethod::Mojang { hat_skin, .. } => Some(hat_skin.as_slice()),
            AuthMethod::Microsoft { hat_skin, .. } => Some(hat_skin.as_slice()),
            AuthMethod::AuthlibInjector { hat_skin, .. } => Some(hat_skin.as_slice()),
        }
    }

    /// 判断是否为离线账户
    pub fn is_offline(&self) -> bool {
        matches!(self, AuthMethod::Offline { .. })
    }

    /// 判断是否为正版（Mojang 或 Microsoft）
    pub fn is_premium(&self) -> bool {
        matches!(self, AuthMethod::Mojang { .. } | AuthMethod::Microsoft { .. })
    }

    /// 判断是否为外置登录
    pub fn is_authlib_injector(&self) -> bool {
        matches!(self, AuthMethod::AuthlibInjector { .. })
    }
}

// ============================================================================
//  Mojang 认证子模块（内部 API 模型）
// ============================================================================

pub(crate) mod mojang {
    use serde::{Deserialize, Serialize};

    use crate::password::Password;

    /// 认证请求体
    #[derive(Debug, Deserialize, Serialize, PartialEq, Eq)]
    #[serde(rename_all = "camelCase")]
    pub(crate) struct AuthenticateBody {
        pub agent: AuthenticateAgent,
        pub username: String,
        pub password: Password,
        #[serde(skip_serializing_if = "String::is_empty")]
        pub client_token: String,
        pub request_user: bool,
    }

    impl Default for AuthenticateBody {
        fn default() -> Self {
            Self {
                request_user: true,
                username: String::new(),
                password: Password::default(),
                client_token: String::new(),
                agent: Default::default(),
            }
        }
    }

    /// 认证代理信息（固定为 Minecraft/1）
    #[derive(Debug, Deserialize, Serialize, PartialEq, Eq)]
    pub(crate) struct AuthenticateAgent {
        pub name: String,
        pub version: usize,
    }

    impl Default for AuthenticateAgent {
        fn default() -> Self {
            Self {
                name: "Minecraft".into(),
                version: 1,
            }
        }
    }

    /// 认证成功响应
    #[derive(Debug, Deserialize, PartialEq, Eq)]
    #[serde(rename_all = "camelCase")]
    pub(crate) struct AuthenticateResponse {
        pub access_token: Password,
        pub client_token: String,
        pub available_profiles: Vec<AvailableProfile>,
        pub selected_profile: Option<AvailableProfile>,
    }

    /// 验证/刷新令牌请求体（用于刷新）
    #[derive(Debug, Serialize, PartialEq, Eq)]
    #[serde(rename_all = "camelCase")]
    pub(crate) struct ValidateResponse {
        pub access_token: Password,
        pub client_token: String,
    }

    /// 可用配置文件（玩家档案）
    #[derive(Debug, Deserialize, Serialize, PartialEq, Eq)]
    pub(crate) struct AvailableProfile {
        pub name: String,
        pub id: String,
    }

    /// Mojang API 错误响应
    #[derive(Debug, Deserialize, PartialEq, Eq, Clone, Default)]
    #[serde(rename_all = "camelCase")]
    pub struct ErrorResponse {
        /// 错误码（如 "ForbiddenOperationException"）
        pub error: String,
        /// 人类可读的错误描述
        pub error_message: String,
        /// 错误原因（通常为空）
        #[serde(default)]
        pub cause: String,
    }

    impl std::fmt::Display for ErrorResponse {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(
                f,
                "{}: {}",
                self.error,
                self.error_message
            )?;
            if !self.cause.is_empty() {
                write!(f, " (cause: {})", self.cause)?;
            }
            Ok(())
        }
    }

    impl std::error::Error for ErrorResponse {}

    /// 玩家档案响应（含皮肤/披风信息）
    #[derive(Debug, Deserialize, PartialEq, Eq, Clone)]
    pub(crate) struct ProfileResponse {
        pub id: String,
        pub name: String,
        pub properties: Vec<ProfileProperty>,
    }

    /// 档案属性
    #[derive(Debug, Deserialize, PartialEq, Eq, Clone)]
    pub(crate) struct ProfileProperty {
        pub name: String,
        pub value: String,
    }

    /// 纹理数据（从 `value` 字段 Base64 解码后解析）
    #[derive(Debug, Deserialize, PartialEq, Eq, Clone)]
    #[serde(rename_all = "camelCase")]
    pub(crate) struct ProfileTexture {
        pub timestamp: u64,
        pub profile_id: String,
        pub profile_name: String,
        pub textures: Option<TextureData>,
    }

    /// 纹理集合（皮肤和披风）
    #[derive(Debug, Deserialize, PartialEq, Eq, Clone)]
    pub(crate) struct TextureData {
        #[serde(rename = "SKIN")]
        pub skin: Option<SkinData>,
        #[serde(rename = "CAPE")]
        pub cape: Option<SkinData>,
    }

    /// 单个纹理信息（URL 和元数据）
    #[derive(Debug, Deserialize, PartialEq, Eq, Clone)]
    pub(crate) struct SkinData {
        pub url: String,
    }
}

// ============================================================================
//  测试
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_auth_method_helpers() {
        let offline = AuthMethod::Offline {
            player_name: "Steve".into(),
            uuid: "00000000-0000-0000-0000-000000000000".into(),
        };
        assert_eq!(offline.player_name(), "Steve");
        assert_eq!(offline.uuid(), "00000000-0000-0000-0000-000000000000");
        assert!(offline.access_token().is_none());
        assert!(offline.head_skin().is_none());
        assert!(offline.is_offline());
        assert!(!offline.is_premium());

        let mojang = AuthMethod::Mojang {
            access_token: Password::from("token"),
            uuid: "abc".into(),
            player_name: "Alex".into(),
            head_skin: vec![1, 2, 3],
            hat_skin: vec![4, 5, 6],
        };
        assert_eq!(mojang.player_name(), "Alex");
        assert!(mojang.access_token().is_some());
        assert_eq!(mojang.head_skin().unwrap(), &[1, 2, 3]);
        assert!(!mojang.is_offline());
        assert!(mojang.is_premium());
    }
}