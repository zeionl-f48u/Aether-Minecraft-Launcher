//! 错误映射模块
//!
//! 将各模块的专用错误类型统一转换为前端友好的 `String` 错误信息。
//! 所有 Tauri 命令的返回类型统一使用 `Result<T, String>`，
//! 前端可直接展示这些错误信息。

use serde::Serialize;

// ============================================================================
//  统一错误结构（可选，用于返回更详细的错误码和消息）
// ============================================================================

/// 结构化错误信息，可返回给前端用于更精细的错误处理
#[derive(Debug, Serialize)]
pub struct CommandError {
    /// 错误码，用于前端程序化判断
    pub code: &'static str,
    /// 人类可读的错误描述
    pub message: String,
}

impl CommandError {
    pub fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }
}

impl std::fmt::Display for CommandError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[{}] {}", self.code, self.message)
    }
}

// ============================================================================
//  从各模块错误类型转换为 String
// ============================================================================

/// 将通用错误转换为 `String`（用于 Tauri 命令返回）
pub fn to_command_error<E: std::fmt::Display>(err: E) -> String {
    format!("{}", err)
}

/// 将通用错误映射为结构化 `CommandError`
pub fn map_error<E: std::fmt::Display>(code: &'static str, err: E) -> CommandError {
    CommandError::new(code, err.to_string())
}

// ============================================================================
//  各模块错误映射辅助函数
// ============================================================================

/// 错误码常量定义
pub mod codes {
    /// Java 运行时相关错误
    pub const JAVA_ERROR: &str = "JAVA_ERROR";
    /// HTTP 网络请求失败
    pub const HTTP_ERROR: &str = "HTTP_ERROR";
    /// 认证失败（Mojang / 微软 / 外置登录）
    pub const AUTH_ERROR: &str = "AUTH_ERROR";
    /// 微软设备码认证错误
    pub const MICROSOFT_DEVICE_ERROR: &str = "MICROSOFT_DEVICE_ERROR";
    /// Authlib 外置登录错误
    pub const AUTH_LIB_ERROR: &str = "AUTH_LIB_ERROR";
    /// 原版游戏下载/安装错误
    pub const VANILLA_ERROR: &str = "VANILLA_ERROR";
    /// Fabric 加载器错误
    pub const FABRIC_ERROR: &str = "FABRIC_ERROR";
    /// Forge 加载器错误
    pub const FORGE_ERROR: &str = "FORGE_ERROR";
    /// 文件 I/O 错误
    pub const IO_ERROR: &str = "IO_ERROR";
    /// 配置文件读写错误
    pub const CONFIG_ERROR: &str = "CONFIG_ERROR";
    /// 版本管理错误
    pub const VERSION_ERROR: &str = "VERSION_ERROR";
    /// 模组管理错误
    pub const MOD_ERROR: &str = "MOD_ERROR";
    /// 客户端启动错误
    pub const CLIENT_ERROR: &str = "CLIENT_ERROR";
    /// 操作被用户取消
    pub const CANCELLED: &str = "CANCELLED";
    /// 信号量获取超时或失败
    pub const SEMAPHORE_ERROR: &str = "SEMAPHORE_ERROR";
    /// 下载模块通用错误
    pub const DOWNLOAD_ERROR: &str = "DOWNLOAD_ERROR";
}

// 为各错误类型实现到 `String` 的转换
// 通过在 Tauri 命令中调用 `.map_err(|e| e.to_string())` 即可使用
