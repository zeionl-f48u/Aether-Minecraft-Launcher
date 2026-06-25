//! Optifine 画质增强/性能优化模组的下载及安装模块
//!
//! 因 Optifine 不提供稳定的官方下载方式，本模块使用镜像源 API 获取版本信息。

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use lazy_static::lazy_static;
use serde::Deserialize;
use thiserror::Error;
use tokio::fs;
use tokio::process::Command;
use tokio::sync::Mutex;
use tokio::task::spawn_blocking;

use super::{structs::OptifineVersionMeta, Downloader};
use crate::download::DownloadSource;
use crate::http::HttpClient;
use crate::prelude::*;

// ============================================================================
//  错误类型
// ============================================================================

#[derive(Debug, Error)]
pub enum OptifineError {
    #[error("HTTP 请求失败: {0}")]
    Http(String),

    #[error("JSON 解析失败: {0}")]
    Json(#[from] serde_json::Error),

    #[error("IO 错误: {0}")]
    Io(#[from] std::io::Error),

    #[error("版本未找到: {0}")]
    VersionNotFound(String),

    #[error("安装器执行失败: {0}")]
    InstallerFailed(String),

    #[error("无效的 Optifine 版本格式: {0}")]
    InvalidVersion(String),
}

pub type OptifineResult<T> = Result<T, OptifineError>;

// ============================================================================
//  常量
// ============================================================================

#[cfg(target_os = "windows")]
const CLASS_PATH_SEPARATOR: &str = ";";
#[cfg(not(target_os = "windows"))]
const CLASS_PATH_SEPARATOR: &str = ":";

const OPTIFINE_INSTALL_HELPER: &[u8] = include_bytes!("../../assets/optifine-installer.jar");

// ============================================================================
//  缓存
// ============================================================================

lazy_static! {
    static ref VERSION_CACHE: Arc<Mutex<HashMap<String, Vec<OptifineVersionMeta>>>> =
        Arc::new(Mutex::new(HashMap::new()));
}

/// 清除版本缓存
pub async fn clear_cache() {
    VERSION_CACHE.lock().await.clear();
}

// ============================================================================
//  URL 辅助
// ============================================================================

/// 获取 Optifine 元数据 URL
fn get_metadata_url(source: DownloadSource, vanilla_version: &str) -> String {
    match source {
        DownloadSource::MCBBS => {
            format!("https://download.mcbbs.net/optifine/{}", vanilla_version)
        }
        _ => {
            format!("https://bmclapi2.bangbang93.com/optifine/{}", vanilla_version)
        }
    }
}

/// 获取 Optifine 下载 URL 列表（镜像源优先级）
fn get_download_urls(
    source: DownloadSource,
    vanilla_version: &str,
    optifine_type: &str,
    optifine_patch: &str,
) -> Vec<String> {
    let primary = match source {
        DownloadSource::MCBBS => format!(
            "https://download.mcbbs.net/optifine/{}/{}/{}",
            vanilla_version, optifine_type, optifine_patch
        ),
        _ => format!(
            "https://bmclapi2.bangbang93.com/optifine/{}/{}/{}",
            vanilla_version, optifine_type, optifine_patch
        ),
    };
    let fallback_bmcl = format!(
        "https://bmclapi2.bangbang93.com/optifine/{}/{}/{}",
        vanilla_version, optifine_type, optifine_patch
    );
    let fallback_mcbbs = format!(
        "https://download.mcbbs.net/optifine/{}/{}/{}",
        vanilla_version, optifine_type, optifine_patch
    );
    let mut urls = vec![primary, fallback_bmcl, fallback_mcbbs];
    urls.dedup();
    urls
}

// ============================================================================
//  扩展特质
// ============================================================================

/// Optifine 下载安装扩展特质
pub trait OptifineDownloadExt: Sync {
    /// 获取指定原版版本可用的 Optifine 版本列表（带缓存）
    async fn get_available_installers(
        &self,
        vanilla_version: &str,
    ) -> OptifineResult<Vec<OptifineVersionMeta>>;

    /// 下载 Optifine JAR 到指定路径
    async fn download_optifine(
        &self,
        vanilla_version: &str,
        optifine_type: &str,
        optifine_patch: &str,
        dest_path: &Path,
    ) -> OptifineResult<()>;

    /// 安装 Optifine（支持独立版本或模组模式）
    async fn install_optifine(
        &self,
        version_name: &str,
        vanilla_version: &str,
        optifine_type: &str,
        optifine_patch: &str,
        as_mod: bool,
    ) -> OptifineResult<()>;
}

// ============================================================================
//  实现
// ============================================================================

impl<R: Reporter> OptifineDownloadExt for Downloader<R> {
    async fn get_available_installers(
        &self,
        vanilla_version: &str,
    ) -> OptifineResult<Vec<OptifineVersionMeta>> {
        // 检查缓存
        {
            let cache = VERSION_CACHE.lock().await;
            if let Some(versions) = cache.get(vanilla_version) {
                return Ok(versions.clone());
            }
        }

        // 从 API 获取
        let url = get_metadata_url(self.source, vanilla_version);
        let client = HttpClient::default();
        let mut versions: Vec<OptifineVersionMeta> = client
            .get_json(&url)
            .await
            .map_err(|e| OptifineError::Http(e.to_string()))?;

        // 按版本排序（假设 API 返回未排序）
        versions.sort_by(|a, b| b.version.cmp(&a.version));

        // 存入缓存
        {
            let mut cache = VERSION_CACHE.lock().await;
            cache.insert(vanilla_version.to_string(), versions.clone());
        }

        Ok(versions)
    }

    async fn download_optifine(
        &self,
        vanilla_version: &str,
        optifine_type: &str,
        optifine_patch: &str,
        dest_path: &Path,
    ) -> OptifineResult<()> {
        let reporter = self.reporter.fork();
        reporter.set_message(format!(
            "正在下载 Optifine {} {}-{}",
            vanilla_version, optifine_type, optifine_patch
        ));
        reporter.add_max_progress(1.0);

        // 确保目标目录存在
        if let Some(parent) = dest_path.parent() {
            fs::create_dir_all(parent).await?;
        }

        // 使用临时文件
        let temp_path = dest_path.with_extension("tmp");
        let urls = get_download_urls(self.source, vanilla_version, optifine_type, optifine_patch);

        let client = HttpClient::default();
        for (idx, url) in urls.iter().enumerate() {
            reporter.set_message(format!(
                "尝试下载 Optifine ({}/{})",
                idx + 1,
                urls.len()
            ));
            if let Err(e) = client.download(&[url.clone()], &temp_path).await {
                let _ = fs::remove_file(&temp_path).await;
                tracing::warn!("下载失败: {}", e);
                continue;
            }
            // 原子重命名
            fs::rename(&temp_path, dest_path).await?;
            reporter.set_progress(1.0);
            return Ok(());
        }

        Err(OptifineError::Http("所有镜像下载失败".into()))
    }

    async fn install_optifine(
        &self,
        version_name: &str,
        vanilla_version: &str,
        optifine_type: &str,
        optifine_patch: &str,
        as_mod: bool,
    ) -> OptifineResult<()> {
        let reporter = self.reporter.fork();
        reporter.set_message(format!("正在安装 Optifine 到 {}", version_name));
        reporter.add_max_progress(2.0);

        if as_mod {
            // 模组模式：直接下载到 mods 目录
            let mod_file_name = format!("Optifine-{}-{}-{}.jar", vanilla_version, optifine_type, optifine_patch);
            let mod_dir = if self.game_independent {
                self.versions_dir().join(version_name).join("mods")
            } else {
                self.minecraft_path().join("mods")
            };
            let mod_path = mod_dir.join(mod_file_name);
            fs::create_dir_all(&mod_dir).await?;

            self.download_optifine(vanilla_version, optifine_type, optifine_patch, &mod_path)
                .await?;
            reporter.set_progress(1.0);
            reporter.set_message("Optifine 安装完成（模组模式）".into());
            Ok(())
        } else {
            // 独立版本模式：使用安装器
            // 1. 下载 Optifine JAR 到 libraries 目录
            let jar_path = self.libraries_dir()
                .join("net/optifine")
                .join(format!("{}-{}-{}", vanilla_version, optifine_type, optifine_patch))
                .join(format!("Optifine-{}-{}-{}.jar", vanilla_version, optifine_type, optifine_patch));

            fs::create_dir_all(jar_path.parent().unwrap()).await?;
            self.download_optifine(vanilla_version, optifine_type, optifine_patch, &jar_path)
                .await?;

            reporter.add_progress(1.0);
            reporter.set_message("正在运行 Optifine 安装器...".into());

            // 2. 准备安装辅助 JAR
            let helper_path = self.libraries_dir()
                .join("net/stevexmh/optifine-installer/0.0.0/optifine-installer.jar");
            fs::create_dir_all(helper_path.parent().unwrap()).await?;
            fs::write(&helper_path, OPTIFINE_INSTALL_HELPER).await?;

            // 3. 运行安装器
            #[cfg(windows)]
            let mut cmd = {
                let mut cmd = Command::new(&self.java_path);
                cmd.creation_flags(0x08000000);
                cmd
            };
            #[cfg(not(windows))]
            let mut cmd = Command::new(&self.java_path);

            cmd.arg("-cp");
            cmd.arg(format!(
                "{}{}{}",
                helper_path.display(),
                CLASS_PATH_SEPARATOR,
                jar_path.display()
            ));
            cmd.arg("net.stevexmh.OptifineInstaller");
            cmd.arg(self.minecraft_path().to_string_lossy().to_string());
            cmd.arg(version_name);
            cmd.stdout(std::process::Stdio::piped());
            cmd.stderr(std::process::Stdio::inherit());
            cmd.stdin(std::process::Stdio::null());

            let status = cmd.status().await?;
            if !status.success() {
                return Err(OptifineError::InstallerFailed(format!(
                    "安装器退出码: {}",
                    status.code().unwrap_or(-1)
                )));
            }

            reporter.set_progress(1.0);
            reporter.set_message("Optifine 安装完成".into());
            Ok(())
        }
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
        let versions = downloader.get_available_installers("1.16.5").await.unwrap();
        assert!(!versions.is_empty());
        // 检查至少有一个版本包含必要字段
        let v = &versions[0];
        assert!(!v.version.is_empty());
        assert!(!v.optifine_type.is_empty());
        assert!(!v.optifine_patch.is_empty());
    }

    #[tokio::test]
    #[ignore]
    async fn test_download_optifine() {
        let dir = tempdir().unwrap();
        let dest = dir.path().join("Optifine.jar");
        let downloader = Downloader::new(".minecraft", DownloadSource::default(), NoopReporter);
        downloader
            .download_optifine("1.16.5", "HD_U", "G5", &dest)
            .await
            .unwrap();
        assert!(dest.is_file());
        assert!(dest.metadata().unwrap().len() > 0);
    }
}