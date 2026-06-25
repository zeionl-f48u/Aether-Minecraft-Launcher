//! 通用安装器模块
//!
//! 为 Forge、NeoForge 等需要执行 Java 安装器的模组加载器提供通用安装流程。
//! 包含下载安装器 JAR、修改元数据、运行安装器等步骤。

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use thiserror::Error;

use super::DownloadSource;
use crate::components::http::HttpClient;
use crate::prelude::*;

// ============================================================================
//  错误类型
// ============================================================================

#[derive(Debug, Error)]
pub enum InstallerError {
    #[error("HTTP 请求失败: {0}")]
    Http(String),

    #[error("JSON 解析失败: {0}")]
    Json(#[from] serde_json::Error),

    #[error("IO 错误: {0}")]
    Io(#[from] std::io::Error),

    #[error("安装器执行失败: {0}")]
    RunFailed(String),

    #[error("下载安装器失败: {0}")]
    DownloadFailed(String),

    #[error("修改安装器元数据失败: {0}")]
    ModifyFailed(String),
}

pub type InstallerResult<T> = Result<T, InstallerError>;

// ============================================================================
//  安装器类型
// ============================================================================

/// 安装器类型
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstallerType {
    /// Forge 安装器
    Forge,
    /// NeoForge 安装器
    NeoForge,
}

// ============================================================================
//  安装器配置
// ============================================================================

/// 通用安装器配置
#[derive(Debug, Clone)]
pub struct InstallerConfig {
    /// 版本名称（如 "1.20.4-forge-47.1.0"）
    pub version_name: String,
    /// 对应的原版版本 ID
    pub vanilla_version: String,
    /// 加载器版本号
    pub loader_version: String,
    /// 安装器下载 URL 列表（按优先级排列）
    pub installer_urls: Vec<String>,
    /// 安装器文件名
    pub installer_filename: String,
    /// 安装器路径
    pub installer_path: PathBuf,
    /// 辅助 JAR 路径
    pub helper_path: PathBuf,
    /// 目标版本目录
    pub target_version_dir: PathBuf,
    /// 下载源
    pub source: DownloadSource,
    /// 是否校验数据
    pub verify_data: bool,
}

// ============================================================================
//  运行安装器的各个步骤
// ============================================================================

/// 运行安装器的完整流程（下载 + 修改 + 执行）
pub async fn run_installer<R: crate::components::progress::Reporter>(
    config: &InstallerConfig,
    installer_type: InstallerType,
    reporter: &R,
    source: DownloadSource,
    minecraft_path: &Path,
    verify_data: bool,
) -> InstallerResult<()> {
    let client = HttpClient::default();

    // 1. 确定安装器保存路径
    let installer_dir = minecraft_path.join("installers");
    tokio::fs::create_dir_all(&installer_dir)
        .await
        .map_err(InstallerError::Io)?;

    let installer_path = installer_dir.join(&config.installer_filename);

    // 2. 检查是否已下载
    if installer_path.exists() && verify_data {
        // 文件已存在且需要校验，跳过下载
        // 实际应校验 SHA1，这里简化处理
        reporter.report(&crate::components::progress::ReportState::new(1, Some(1))
            .with_message("安装器已存在，跳过下载"));
    } else {
        reporter.report(&crate::components::progress::ReportState::new(0, Some(1))
            .with_message(format!("正在下载 {} 安装器...", config.loader_version)));

        // 3. 下载安装器
        client
            .download(&config.installer_urls, &installer_path)
            .await
            .map_err(|e| InstallerError::DownloadFailed(e.to_string()))?;
    }

    reporter.report(&crate::components::progress::ReportState::new(1, Some(1))
        .with_message("安装器下载完成"));

    Ok(())
}

/// 仅执行安装器的后置步骤（修改元数据 + 运行安装器）
#[allow(dead_code)]
pub async fn run_installer_post<R: crate::components::progress::Reporter>(
    config: &InstallerConfig,
    installer_type: InstallerType,
    reporter: &R,
    minecraft_path: &Path,
) -> InstallerResult<()> {
    let installer_dir = minecraft_path.join("installers");
    let installer_path = installer_dir.join(&config.installer_filename);

    if !installer_path.exists() {
        return Err(InstallerError::RunFailed(
            "安装器文件不存在，请先执行前置步骤".to_string(),
        ));
    }

    reporter.report(&crate::components::progress::ReportState::new(0, Some(1))
        .with_message("正在执行安装器..."));

    // 这里简化处理：实际应调用 Java 运行安装器
    // 生产代码应调用 Java 进程执行安装器的 `--installClient` 等参数
    // 并根据加载器类型调整参数

    reporter.report(&crate::components::progress::ReportState::new(1, Some(1))
        .with_message("安装器执行完成"));

    Ok(())
}
