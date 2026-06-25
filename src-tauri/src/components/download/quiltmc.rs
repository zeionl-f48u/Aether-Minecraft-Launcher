//! QuiltMC 模组加载器的下载与安装

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use lazy_static::lazy_static;
use serde::Deserialize;
use thiserror::Error;
use tokio::fs;
use tokio::sync::Mutex;
use tokio::task::spawn_blocking;

use super::Downloader;
use crate::http::HttpClient;
use crate::package::PackageName;
use crate::prelude::*;
use crate::version::structs::VersionMeta;

// ============================================================================
//  错误类型
// ============================================================================

#[derive(Debug, Error)]
pub enum QuiltMCError {
    #[error("HTTP 请求失败: {0}")]
    Http(String),

    #[error("JSON 解析失败: {0}")]
    Json(#[from] serde_json::Error),

    #[error("IO 错误: {0}")]
    Io(#[from] std::io::Error),

    #[error("版本未找到: {0}")]
    VersionNotFound(String),

    #[error("库下载失败: {0}")]
    LibraryDownload(String),

    #[error("元数据合并失败: {0}")]
    MergeError(String),
}

pub type QuiltMCResult<T> = Result<T, QuiltMCError>;

// ============================================================================
//  数据结构
// ============================================================================

#[derive(Debug, Deserialize, PartialEq, Eq, Clone)]
pub struct LoaderMetaItem {
    pub loader: LoaderStruct,
    pub intermediary: IntermediaryStruct,
}

#[derive(Debug, Deserialize, PartialEq, Eq, Clone)]
pub struct IntermediaryStruct {
    pub maven: String,
    pub version: String,
}

#[derive(Debug, Deserialize, PartialEq, Eq, Clone)]
pub struct LoaderStruct {
    pub maven: String,
    pub version: String,
}

// ============================================================================
//  缓存
// ============================================================================

lazy_static! {
    static ref LOADER_CACHE: Arc<Mutex<HashMap<String, Vec<LoaderMetaItem>>>> =
        Arc::new(Mutex::new(HashMap::new()));
}

/// 清除缓存
pub async fn clear_cache() {
    LOADER_CACHE.lock().await.clear();
}

// ============================================================================
//  URL 辅助
// ============================================================================

const QUILT_META_BASE: &str = "https://meta.quiltmc.org/v3";
const QUILT_MAVEN_BASE: &str = "https://maven.quiltmc.org/repository/release";

/// 获取可用加载器列表的 URL
fn get_loader_list_url(vanilla_version: &str) -> String {
    format!("{}/versions/loader/{}", QUILT_META_BASE, vanilla_version)
}

/// 获取加载器 profile JSON 的 URL
fn get_loader_profile_url(vanilla_version: &str, loader_version: &str) -> String {
    format!(
        "{}/versions/loader/{}/{}/profile/json",
        QUILT_META_BASE, vanilla_version, loader_version
    )
}

/// 获取 Maven 镜像列表（可扩展）
fn get_maven_mirrors() -> Vec<String> {
    vec![
        QUILT_MAVEN_BASE.to_string(),
        "https://bmclapi2.bangbang93.com/maven".to_string(),
        "https://download.mcbbs.net/maven".to_string(),
    ]
}

// ============================================================================
//  核心特质
// ============================================================================

/// QuiltMC 模组加载器的安装特质
pub trait QuiltMCDownloadExt: Sync {
    /// 获取指定原版版本可用的 QuiltMC 加载器列表（带缓存）
    async fn get_available_loaders(&self, vanilla_version: &str) -> QuiltMCResult<Vec<LoaderMetaItem>>;

    /// 下载单个 QuiltMC 支持库（含校验和镜像回退）
    async fn download_library(&self, name: &str, maven_url: &str) -> QuiltMCResult<()>;

    /// 下载 QuiltMC 加载器（前置步骤）
    async fn download_quiltmc_pre(
        &self,
        version_name: &str,
        vanilla_version: &str,
        loader_version: &str,
    ) -> QuiltMCResult<()>;

    /// 合并 QuiltMC 元数据与原版元数据（后置步骤）
    async fn download_quiltmc_post(&self, version_name: &str) -> QuiltMCResult<()>;
}

// ============================================================================
//  实现
// ============================================================================

impl<R: Reporter> QuiltMCDownloadExt for Downloader<R> {
    async fn get_available_loaders(&self, vanilla_version: &str) -> QuiltMCResult<Vec<LoaderMetaItem>> {
        // 检查缓存
        {
            let cache = LOADER_CACHE.lock().await;
            if let Some(loaders) = cache.get(vanilla_version) {
                return Ok(loaders.clone());
            }
        }

        let url = get_loader_list_url(vanilla_version);
        let client = HttpClient::default();
        let loaders: Vec<LoaderMetaItem> = client
            .get_json(&url)
            .await
            .map_err(|e| QuiltMCError::Http(e.to_string()))?;

        // 存入缓存
        {
            let mut cache = LOADER_CACHE.lock().await;
            cache.insert(vanilla_version.to_string(), loaders.clone());
        }

        Ok(loaders)
    }

    async fn download_library(&self, name: &str, maven_url: &str) -> QuiltMCResult<()> {
        let package: PackageName = name
            .parse()
            .map_err(|_| QuiltMCError::LibraryDownload(format!("无效包名: {}", name)))?;

        // 确定基础 URL
        let base_url = if maven_url.is_empty() {
            QUILT_MAVEN_BASE
        } else {
            maven_url
        };
        // 构建镜像列表
        let mut mirrors = vec![base_url.to_string()];
        for mirror in get_maven_mirrors() {
            if mirror != base_url {
                mirrors.push(mirror);
            }
        }

        let lib_path = self.libraries_dir().join(package.to_maven_jar_path(""));
        let temp_path = lib_path.with_extension("tmp");

        let reporter = self.reporter.fork();
        reporter.set_message(format!("下载 QuiltMC 库: {}", name));
        reporter.add_max_progress(1.0);

        // 如果文件已存在且校验通过，则跳过
        if lib_path.is_file() && self.verify_data {
            let expected_sha1 = self.get_library_sha1(&package, &mirrors).await?;
            if self.verify_file(&lib_path, &expected_sha1).await? {
                reporter.set_progress(1.0);
                return Ok(());
            }
            // 校验失败，删除并重新下载
            let _ = fs::remove_file(&lib_path).await;
        }

        // 尝试从镜像下载
        let client = HttpClient::default();
        for (idx, url) in mirrors.iter().enumerate() {
            let jar_url = package.to_maven_jar_path(url);
            reporter.set_message(format!(
                "尝试下载 {}/{}: {}",
                idx + 1,
                mirrors.len(),
                jar_url
            ));
            if let Err(e) = client.download(&[jar_url], &temp_path).await {
                let _ = fs::remove_file(&temp_path).await;
                tracing::warn!("下载失败: {}", e);
                continue;
            }
            // 原子重命名
            fs::rename(&temp_path, &lib_path).await?;
            reporter.set_progress(1.0);
            return Ok(());
        }

        Err(QuiltMCError::LibraryDownload("所有镜像下载失败".into()))
    }

    async fn download_quiltmc_pre(
        &self,
        version_name: &str,
        vanilla_version: &str,
        loader_version: &str,
    ) -> QuiltMCResult<()> {
        let reporter = self.reporter.fork();
        reporter.set_message(format!("下载 QuiltMC {} 元数据", loader_version));
        reporter.add_max_progress(1.0);

        // 1. 获取 profile JSON
        let url = get_loader_profile_url(vanilla_version, loader_version);
        let client = HttpClient::default();
        let meta_bytes = client
            .get_bytes(&url)
            .await
            .map_err(|e| QuiltMCError::Http(e.to_string()))?;

        // 2. 保存临时元数据文件
        let version_dir = self.versions_dir().join(version_name);
        fs::create_dir_all(&version_dir).await?;
        let temp_meta_path = version_dir.join(format!("{}-quiltmc-loader.tmp.json", version_name));
        fs::write(&temp_meta_path, &meta_bytes).await?;

        // 3. 解析元数据获取库列表
        let meta: VersionMeta = serde_json::from_slice(&meta_bytes)?;

        // 4. 并发下载库（限制并发数）
        use futures::stream::{self, StreamExt};
        let libs: Vec<_> = meta
            .libraries
            .iter()
            .filter(|lib| !lib.name.is_empty())
            .map(|lib| (lib.name.clone(), lib.url.as_deref().unwrap_or("").to_string()))
            .collect();

        // 使用信号量控制并发（从 Downloader 获取）
        let semaphore = self.semaphore().clone();

        let results = stream::iter(libs)
            .map(|(name, url)| {
                let downloader = &self;
                let sem = semaphore.clone();
                async move {
                    let _permit = sem.acquire().await?;
                    downloader.download_library(&name, &url).await
                }
            })
            .buffer_unordered(10)
            .collect::<Vec<_>>()
            .await;

        // 检查错误
        for result in results {
            result?;
        }

        reporter.set_progress(1.0);
        Ok(())
    }

    async fn download_quiltmc_post(&self, version_name: &str) -> QuiltMCResult<()> {
        let reporter = self.reporter.fork();
        reporter.set_message("合并 QuiltMC 元数据".into());

        let version_dir = self.versions_dir().join(version_name);
        let vanilla_path = version_dir.join(format!("{}.json", version_name));
        let loader_temp_path = version_dir.join(format!("{}-quiltmc-loader.tmp.json", version_name));

        // 读取并合并
        let vanilla_bytes = fs::read(&vanilla_path).await?;
        let loader_bytes = fs::read(&loader_temp_path).await?;

        // 删除临时文件
        let _ = fs::remove_file(&loader_temp_path).await;

        let mut vanilla_meta: VersionMeta = serde_json::from_slice(&vanilla_bytes)?;
        let loader_meta: VersionMeta = serde_json::from_slice(&loader_bytes)?;

        // 合并
        vanilla_meta += loader_meta;

        // 写回
        let merged_bytes = serde_json::to_vec(&vanilla_meta)?;
        fs::write(&vanilla_path, &merged_bytes).await?;

        reporter.set_message("元数据合并完成".into());
        Ok(())
    }
}

// ============================================================================
//  辅助方法（扩展 Downloader）
// ============================================================================

impl<R: Reporter> Downloader<R> {
    /// 获取 libraries 目录路径
    fn libraries_dir(&self) -> PathBuf {
        self.minecraft_path().join("libraries")
    }

    /// 获取 versions 目录路径
    fn versions_dir(&self) -> PathBuf {
        self.minecraft_path().join("versions")
    }

    /// 获取库文件的 SHA1（从镜像列表中尝试）
    async fn get_library_sha1(&self, package: &PackageName, mirrors: &[String]) -> QuiltMCResult<String> {
        let client = HttpClient::default();
        for url in mirrors {
            let sha1_url = format!("{}.sha1", package.to_maven_jar_path(url));
            match client.get_string(&sha1_url).await {
                Ok(sha1) => return Ok(sha1.trim().to_string()),
                Err(e) => tracing::warn!("获取 SHA1 失败: {} - {}", sha1_url, e),
            }
        }
        Err(QuiltMCError::LibraryDownload("无法获取 SHA1 校验值".into()))
    }

    /// 校验文件完整性
    async fn verify_file(&self, path: &Path, expected_sha1: &str) -> QuiltMCResult<bool> {
        let mut file = fs::File::open(path).await?;
        let actual_sha1 = crate::utils::get_data_sha1_async(&mut file).await?;
        Ok(actual_sha1 == expected_sha1)
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
    async fn test_get_available_loaders() {
        let downloader = Downloader::new(".minecraft", DownloadSource::default(), NoopReporter);
        let loaders = downloader.get_available_loaders("1.19.2").await.unwrap();
        assert!(!loaders.is_empty());
        assert!(!loaders[0].loader.version.is_empty());
    }

    #[tokio::test]
    #[ignore]
    async fn test_download_library() {
        let dir = tempdir().unwrap();
        let mc_path = dir.path().to_str().unwrap();
        let downloader = Downloader::new(mc_path, DownloadSource::default(), NoopReporter);
        let result = downloader
            .download_library("org.quiltmc:quilt-loader:0.19.1", "")
            .await;
        assert!(result.is_ok());
    }
}