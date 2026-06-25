//! Fabric 模组加载器的下载与安装

use std::path::{Path, PathBuf};

use futures_util::StreamExt;
use serde::Deserialize;
use thiserror::Error;
use tokio::fs;
use tokio::io::AsyncWriteExt;

use super::{DownloadSource, Downloader};
use crate::http::HttpClient;
use crate::package::PackageName;
use crate::prelude::*;
use crate::version::structs::VersionMeta;

// ============================================================================
//  错误类型
// ============================================================================

#[derive(Debug, Error)]
pub enum FabricError {
    #[error("HTTP 请求失败: {0}")]
    Http(String),

    #[error("JSON 解析失败: {0}")]
    Json(#[from] serde_json::Error),

    #[error("IO 错误: {0}")]
    Io(#[from] std::io::Error),

    #[error("无效的响应数据: {0}")]
    InvalidResponse(String),

    #[error("库文件校验失败: {0}")]
    VerificationFailed(String),

    #[error("下载失败: {0}")]
    DownloadFailed(String),

    #[error("元数据解析失败: {0}")]
    MetadataParse(String),
}

pub type FabricResult<T> = Result<T, FabricError>;

// ============================================================================
//  数据结构
// ============================================================================

/// Fabric 加载器版本元数据
#[derive(Debug, Deserialize, PartialEq, Eq, Clone)]
pub struct LoaderMetaItem {
    pub loader: LoaderStruct,
    pub intermediary: IntermediaryStruct,
}

#[derive(Debug, Deserialize, PartialEq, Eq, Clone)]
pub struct IntermediaryStruct {
    pub maven: String,
    pub version: String,
    pub stable: bool,
}

#[derive(Debug, Deserialize, PartialEq, Eq, Clone)]
pub struct LoaderStruct {
    pub maven: String,
    pub version: String,
    pub stable: bool,
}

// ============================================================================
//  镜像源 URL 辅助
// ============================================================================

/// 获取 Fabric Meta API 的基础 URL
fn get_fabric_meta_base(source: DownloadSource) -> &'static str {
    match source {
        DownloadSource::BMCLAPI => "https://bmclapi2.bangbang93.com/fabric-meta",
        DownloadSource::MCBBS => "https://download.mcbbs.net/fabric-meta",
        _ => "https://meta.fabricmc.net",
    }
}

/// 获取 Maven 仓库的基础 URL
fn get_maven_base(source: DownloadSource) -> &'static str {
    match source {
        DownloadSource::BMCLAPI => "https://bmclapi2.bangbang93.com/maven",
        DownloadSource::MCBBS => "https://download.mcbbs.net/maven",
        _ => "https://maven.fabricmc.net",
    }
}

// ============================================================================
//  Fabric 下载特质
// ============================================================================

/// Fabric 模组加载器的安装特质
pub trait FabricDownloadExt: Sync {
    /// 获取指定原版版本可用的 Fabric 加载器列表
    async fn get_available_loaders(&self, vanilla_version: &str) -> FabricResult<Vec<LoaderMetaItem>>;

    /// 下载单个 Fabric 支持库（含校验）
    async fn download_library(&self, name: &str) -> FabricResult<()>;

    /// 安装 Fabric 加载器（前期：下载库和元数据）
    async fn download_fabric_pre(
        &self,
        version_name: &str,
        version_id: &str,
        loader_version: &str,
    ) -> FabricResult<()>;

    /// 合并 Fabric 元数据与原版元数据（后期）
    async fn download_fabric_post(&self, version_name: &str) -> FabricResult<()>;
}

// ============================================================================
//  为 Downloader 实现
// ============================================================================

impl<R: Reporter> FabricDownloadExt for Downloader<R> {
    async fn get_available_loaders(&self, vanilla_version: &str) -> FabricResult<Vec<LoaderMetaItem>> {
        let base = get_fabric_meta_base(self.source);
        let url = format!("{}/v2/versions/loader/{}", base, vanilla_version);
        let client = HttpClient::default();
        let data: Vec<LoaderMetaItem> = client
            .get_json(&url)
            .await
            .map_err(|e| FabricError::Http(e.to_string()))?;
        Ok(data)
    }

    async fn download_library(&self, name: &str) -> FabricResult<()> {
        let package: PackageName = name
            .parse()
            .map_err(|_| FabricError::InvalidResponse(format!("无效的包名: {}", name)))?;

        let maven_base = get_maven_base(self.source);
        let file_path = PathBuf::from(package.to_maven_jar_path(maven_base));

        // 确保目录存在
        if let Some(parent) = file_path.parent() {
            fs::create_dir_all(parent).await?;
        }

        // 如果启用校验且文件已存在，则校验
        if self.verify_data && file_path.is_file() {
            let expected_sha1 = self.get_library_sha1(&package).await?;
            if self.verify_file(&file_path, &expected_sha1).await? {
                return Ok(());
            }
            // 校验失败，删除文件并重新下载
            let _ = fs::remove_file(&file_path).await;
        }

        // 下载文件
        let uris = self.build_mirror_urls(&package);
        self.download_with_retry(&uris, &file_path).await?;

        Ok(())
    }

    async fn download_fabric_pre(
        &self,
        version_name: &str,
        version_id: &str,
        loader_version: &str,
    ) -> FabricResult<()> {
        let base = get_fabric_meta_base(self.source);
        let url = format!(
            "{}/v2/versions/loader/{}/{}/profile/json",
            base, version_id, loader_version
        );

        let client = HttpClient::default();
        let meta_bytes = client
            .get_bytes(&url)
            .await
            .map_err(|e| FabricError::Http(e.to_string()))?;

        // 解析元数据
        let meta: VersionMeta = serde_json::from_slice(&meta_bytes)?;

        // 保存临时元数据文件（使用 .tmp 后缀）
        let version_dir = Path::new(&self.minecraft_version_path).join(version_name);
        fs::create_dir_all(&version_dir).await?;
        let temp_meta_path = version_dir.join(format!("{}-fabric-loader.tmp.json", version_name));
        fs::write(&temp_meta_path, &meta_bytes).await?;

        // 并发下载所有库文件（限制并发数）
        use futures::stream::{self, StreamExt};
        let libs: Vec<_> = meta
            .libraries
            .iter()
            .filter(|lib| !lib.name.is_empty())
            .map(|lib| lib.name.clone())
            .collect();

        let total = libs.len();
        if let Some(r) = self.reporter.sub() {
            r.add_max_progress(total as f64);
            r.set_message("正在下载 Fabric 支持库".into());
        }

        let results = stream::iter(libs)
            .map(|name| {
                let downloader = &self;
                async move {
                    downloader.download_library(&name).await
                }
            })
            .buffer_unordered(10) // 限制并发数
            .collect::<Vec<_>>()
            .await;

        // 检查是否有错误
        for (idx, result) in results.into_iter().enumerate() {
            if let Err(e) = result {
                if let Some(r) = self.reporter.sub() {
                    r.set_message(format!("下载库失败: {}", e));
                }
                return Err(e);
            }
            if let Some(r) = self.reporter.sub() {
                r.add_progress(1.0);
            }
        }

        Ok(())
    }

    async fn download_fabric_post(&self, version_name: &str) -> FabricResult<()> {
        let version_dir = Path::new(&self.minecraft_version_path).join(version_name);
        let vanilla_path = version_dir.join(format!("{}.json", version_name));
        let loader_temp_path = version_dir.join(format!("{}-fabric-loader.tmp.json", version_name));

        // 读取并合并元数据
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

        Ok(())
    }
}

// ============================================================================
//  辅助方法（扩展 Downloader）
// ============================================================================

impl<R: Reporter> Downloader<R> {
    /// 获取库文件的 SHA1
    async fn get_library_sha1(&self, package: &PackageName) -> FabricResult<String> {
        let maven_base = get_maven_base(self.source);
        let sha1_url = format!("{}.sha1", package.to_maven_jar_path(maven_base));
        let client = HttpClient::default();
        let sha1 = client
            .get_string(&sha1_url)
            .await
            .map_err(|e| FabricError::Http(e.to_string()))?;
        Ok(sha1.trim().to_string())
    }

    /// 校验文件完整性
    async fn verify_file(&self, path: &Path, expected_sha1: &str) -> FabricResult<bool> {
        let mut file = fs::File::open(path).await?;
        let actual_sha1 = crate::utils::get_data_sha1_async(&mut file).await?;
        Ok(actual_sha1 == expected_sha1)
    }

    /// 构建镜像 URL 列表
    fn build_mirror_urls(&self, package: &PackageName) -> Vec<String> {
        let mut uris = Vec::new();
        // 优先使用配置的源
        let primary = get_maven_base(self.source);
        uris.push(package.to_maven_jar_path(primary));

        // 添加其他镜像作为后备
        let fallbacks = [
            "https://bmclapi2.bangbang93.com/maven",
            "https://download.mcbbs.net/maven",
            "https://maven.fabricmc.net",
        ];
        for fb in &fallbacks {
            if *fb != primary {
                uris.push(package.to_maven_jar_path(fb));
            }
        }
        uris.dedup();
        uris
    }

    /// 带重试的下载（使用镜像列表）
    async fn download_with_retry(&self, uris: &[String], dest: &Path) -> FabricResult<()> {
        let client = HttpClient::default();
        let temp_path = dest.with_extension("tmp");
        let mut last_err = None;

        for (idx, uri) in uris.iter().enumerate() {
            if let Some(r) = self.reporter.sub() {
                r.set_message(format!("尝试下载 {} ({}/{})", uri, idx + 1, uris.len()));
            }
            match client.download(&[uri.clone()], &temp_path).await {
                Ok(()) => {
                    // 原子重命名
                    fs::rename(&temp_path, dest).await?;
                    return Ok(());
                }
                Err(e) => {
                    let _ = fs::remove_file(&temp_path).await;
                    last_err = Some(e);
                    continue;
                }
            }
        }

        Err(FabricError::DownloadFailed(
            last_err.map(|e| e.to_string()).unwrap_or_else(|| "所有镜像下载失败".into()),
        ))
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
    #[ignore]
    async fn test_get_available_loaders() {
        let downloader = Downloader::new(".minecraft", NR, DownloadSource::Official);
        let loaders = downloader.get_available_loaders("1.16.5").await.unwrap();
        assert!(!loaders.is_empty());
        // 检查结构
        assert!(!loaders[0].loader.version.is_empty());
        assert!(!loaders[0].intermediary.version.is_empty());
    }

    #[tokio::test]
    #[ignore]
    async fn test_download_library() {
        let dir = tempdir().unwrap();
        let mc_path = dir.path().to_str().unwrap();
        let downloader = Downloader::new(mc_path, NR, DownloadSource::Official);
        downloader.download_library("net.fabricmc:sponge-mixin:0.9.2+mixin.0.8.2").await.unwrap();
        // 检查文件是否存在
        let lib_path = Path::new(mc_path)
            .join("libraries")
            .join("net/fabricmc/sponge-mixin/0.9.2+mixin.0.8.2/sponge-mixin-0.9.2+mixin.0.8.2.jar");
        assert!(lib_path.is_file());
    }
}