//! NeoForge 模组加载器的下载模块
//!
//! 提供 NeoForge 版本的获取、下载和安装功能。
//! 实际安装逻辑复用自通用的 `installer` 模块。

use serde::Deserialize;
use thiserror::Error;

use super::{
    structs::{NeoForgeItemInfo, NeoForgeVersionsData},
    DownloadSource, Downloader,
};
use crate::components::download::installer::{InstallerConfig, InstallerType, run_installer};
use crate::components::http::HttpClient;
use crate::components::progress::ReporterExt;
use crate::prelude::*;

// ============================================================================
//  错误类型
// ============================================================================

#[derive(Debug, Error)]
pub enum NeoForgeError {
    #[error("HTTP 请求失败: {0}")]
    Http(String),

    #[error("JSON 解析失败: {0}")]
    Json(#[from] serde_json::Error),

    #[error("安装器执行失败: {0}")]
    InstallerFailed(String),

    #[error("IO 错误: {0}")]
    Io(#[from] std::io::Error),
}

pub type NeoForgeResult<T> = Result<T, NeoForgeError>;

// ============================================================================
//  数据结构
// ============================================================================

/// NeoForge 安装器信息（API 响应）
#[derive(Debug, Deserialize)]
pub struct NeoForgeInstallerInfo {
    pub version: String,
    pub raw_version: String,
    // 可能还有其他字段
}

// ============================================================================
//  核心特质
// ============================================================================

/// NeoForge 模组加载器的安装特质
pub trait NeoForgeDownloadExt: Sync {
    /// 获取指定原版版本可用的 NeoForge 版本列表
    async fn get_available_installers(
        &self,
        vanilla_version: &str,
    ) -> NeoForgeResult<NeoForgeVersionsData>;

    /// 下载并安装 NeoForge（前置步骤：下载安装器）
    async fn install_neoforge_pre(
        &self,
        version_name: &str,
        vanilla_version: &str,
        neoforge_version: &str,
    ) -> NeoForgeResult<()>;

    /// 执行 NeoForge 安装（后置步骤：运行安装器）
    async fn install_neoforge_post(
        &self,
        version_name: &str,
        vanilla_version: &str,
        neoforge_version: &str,
    ) -> NeoForgeResult<()>;
}

// ============================================================================
//  实现
// ============================================================================

impl<R: Reporter + ReporterExt> NeoForgeDownloadExt for Downloader<R> {
    async fn get_available_installers(
        &self,
        vanilla_version: &str,
    ) -> NeoForgeResult<NeoForgeVersionsData> {
        let url = match self.source {
            DownloadSource::BMCLAPI => {
                format!("https://bmclapi2.bangbang93.com/neoforge/list/{}", vanilla_version)
            }
            _ => {
                // BMCLAPI 是目前唯一的 NeoForge 元数据镜像源，fallback 到官方 API（NeoForge 官方可能提供）
                format!("https://bmclapi2.bangbang93.com/neoforge/list/{}", vanilla_version)
            }
        };

        let client = HttpClient::default();
        let mut versions: Vec<NeoForgeItemInfo> = client
            .get_json(&url)
            .await
            .map_err(|e| NeoForgeError::Http(e.to_string()))?;

        // 只保留 NeoForge 包（过滤掉纯 Forge）
        versions.retain(|x| x.raw_version.starts_with("neoforge"));
        // 移除 "neoforge-" 前缀
        for item in &mut versions {
            if let Some(s) = item.version.strip_prefix("neoforge-") {
                item.version = s.to_string();
            }
        }
        // 按版本降序排序（最新在前）
        versions.sort_by(|a, b| b.version.cmp(&a.version));

        let latest = versions.first().cloned();

        Ok(NeoForgeVersionsData {
            recommended: None,
            latest,
            all_versions: versions,
        })
    }

    async fn install_neoforge_pre(
        &self,
        version_name: &str,
        vanilla_version: &str,
        neoforge_version: &str,
    ) -> NeoForgeResult<()> {
        self.reporter.set_max_progress(1.0);
        self.reporter.set_message(format!("下载 NeoForge 安装器 {}", neoforge_version));

        // 构建 NeoForge 安装器配置
        let config = self.build_neoforge_config(version_name, vanilla_version, neoforge_version)?;

        // 使用通用安装器执行前置步骤
        run_installer::<R>(
            &config,
            InstallerType::NeoForge,
            &self.reporter,
            self.source.clone(),
            self.minecraft_path(),
            self.verify_data,
        )
        .await
        .map_err(|e| NeoForgeError::InstallerFailed(e.to_string()))?;

        self.reporter.set_progress(1.0);
        Ok(())
    }

    async fn install_neoforge_post(
        &self,
        version_name: &str,
        vanilla_version: &str,
        neoforge_version: &str,
    ) -> NeoForgeResult<()> {
        self.reporter.set_max_progress(2.0);
        self.reporter.set_message("正在修改 NeoForge 安装器...".to_string());

        let config = self.build_neoforge_config(version_name, vanilla_version, neoforge_version)?;

        // 使用通用安装器执行后置步骤
        run_installer::<R>(
            &config,
            InstallerType::NeoForge,
            &self.reporter,
            self.source.clone(),
            self.minecraft_path(),
            self.verify_data,
        )
        .await
        .map_err(|e| NeoForgeError::InstallerFailed(e.to_string()))?;

        self.reporter.set_progress(2.0);
        self.reporter.set_message("NeoForge 安装完成".to_string());
        Ok(())
    }
}

// ============================================================================
//  辅助方法（构建配置）
// ============================================================================

impl<R: Reporter + ReporterExt> Downloader<R> {
    /// 构建 NeoForge 安装器配置
    fn build_neoforge_config(
        &self,
        version_name: &str,
        vanilla_version: &str,
        neoforge_version: &str,
    ) -> NeoForgeResult<InstallerConfig> {
        let maven_base = match self.source {
            DownloadSource::BMCLAPI => "https://bmclapi2.bangbang93.com/maven",
            DownloadSource::MCBBS => "https://download.mcbbs.net/maven",
            _ => "https://maven.neoforged.net/releases",
        };

        let installer_url = format!(
            "{}/net/neoforged/neoforge/{}/neoforge-{}-installer.jar",
            maven_base, neoforge_version, neoforge_version
        );

        let installer_path = self.libraries_dir()
            .join("net/neoforged/neoforge")
            .join(&neoforge_version)
            .join(format!("neoforge-{}-installer.jar", neoforge_version));

        let helper_path = self.libraries_dir()
            .join("com/bangbang93/forge-install-bootstrapper/0.0.0/forge-install-bootstrapper.jar");

        let target_version_dir = self.versions_dir().join(version_name);

        Ok(InstallerConfig {
            version_name: version_name.to_string(),
            vanilla_version: vanilla_version.to_string(),
            loader_version: neoforge_version.to_string(),
            installer_urls: vec![
                installer_url,
                format!("https://bmclapi2.bangbang93.com/maven/net/neoforged/neoforge/{}/neoforge-{}-installer.jar", neoforge_version, neoforge_version),
                format!("https://download.mcbbs.net/maven/net/neoforged/neoforge/{}/neoforge-{}-installer.jar", neoforge_version, neoforge_version),
                format!("https://maven.neoforged.net/releases/net/neoforged/neoforge/{}/neoforge-{}-installer.jar", neoforge_version, neoforge_version),
            ],
            installer_filename: format!("neoforge-{}-installer.jar", neoforge_version),
            installer_path,
            helper_path,
            target_version_dir,
            source: self.source.clone(),
            verify_data: self.verify_data,
        })
    }
}

// ============================================================================
//  测试
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[tokio::test]
    #[ignore] // 需要网络
    async fn test_get_available_installers() {
        let downloader = Downloader::new(".minecraft", DownloadSource::default(), NoopReporter);
        let data = downloader.get_available_installers("1.20.1").await.unwrap();
        assert!(!data.all_versions.is_empty());
        // 最新版本应该存在
        assert!(data.latest.is_some());
    }

    #[tokio::test]
    #[ignore]
    async fn test_install_neoforge() {
        let dir = tempdir().unwrap();
        let mc_path = dir.path().to_str().unwrap();
        let downloader = Downloader::new(mc_path, DownloadSource::default(), NoopReporter);
        // 注意：此测试需要实际的 NeoForge 版本
        // 这里仅示例，实际使用时需替换为真实版本
        downloader
            .install_neoforge_pre("neoforge-test", "1.20.1", "47.1.0")
            .await
            .unwrap();
        // 检查安装器是否存在
        let installer = Path::new(mc_path)
            .join("libraries/net/neoforged/neoforge/47.1.0/neoforge-47.1.0-installer.jar");
        assert!(installer.is_file());
    }
}