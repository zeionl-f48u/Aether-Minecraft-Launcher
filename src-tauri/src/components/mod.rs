//! 核心功能模块声明
//!
//! 整合所有 Minecraft 启动器相关的核心逻辑：
//! - 认证（微软、Mojang、Authlib-Injector、离线）
//! - 下载（原版、Forge、Fabric、NeoForge、OptiFine、Modrinth 等）
//! - 版本管理（扫描、解析、合并）
//! - 客户端构建与启动
//! - Java 运行时检测
//! - HTTP 网络客户端
//! - 实用工具函数

// ============================================================================
//  子模块声明
// ============================================================================

/// 账户认证模块（微软、Mojang、Authlib-Injector、离线）
pub mod auth;

/// Minecraft 客户端构建与游戏进程启动
pub mod client;

/// 游戏资源下载模块（原版、模组加载器、模组仓库）
pub mod download;

/// HTTP 网络客户端（支持重试、超时、代理、并发下载）
pub mod http;

/// Java 运行时检测与管理
pub mod jave;

/// Maven 包名解析
pub mod package;

/// 安全密码包装类型
pub mod password;

/// Minecraft 目录路径管理
pub mod path;

/// 进度报告工具
pub mod progress;

/// Minecraft 版本号解析与比较（支持新旧格式）
pub mod semver;

/// 跨平台实用工具（SHA1、文件路径、架构检测等）
pub mod utils;

/// 游戏版本管理（扫描、加载、分析）
pub mod version;
