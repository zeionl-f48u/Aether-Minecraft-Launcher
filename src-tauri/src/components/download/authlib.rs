//! Authlib-Injector 第三方登录代理 JAR 下载与安装

use std::path::{Path, PathBuf};

use serde::Deserialize;
use thiserror::Error;
use tokio::fs;
use tokio::io::AsyncWriteExt;

use super::{DownloadSource, Downloader};
use crate::components::http::HttpClient;
use crate::components::progress::ReporterExt;
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

    #[error("IO 错误: {0}")]
    Io(#[from] std::io::Error),

    #[error("下载元数据失败: {0}")]
    MetadataDownload(String),

    #[error("文件写入失败: {0}")]
    WriteFailed(String),

    #[error("临时文件操作失败: {0}")]
    TempFile(String),

    #[error("镜像源不支持: {0}")]
    UnsupportedMirror(String),
}

pub type AuthlibResult<T> = Result<T, AuthlibError>;

// ============================================================================
//  元数据结构
// ============================================================================

#[derive(Debug, Deserialize)]
struct LatestData {
    pub version: String,
    #[serde(rename = "download_url")]
    pub download_url: String,
}

// ============================================================================
//  镜像源 URL 配置
// ============================================================================

/// 获取 Authlib-Injector 元数据 URL
fn get_metadata_url(source: DownloadSource) -> &'static str {
    match source {
        DownloadSource::BMCLAPI => {
            "https://bmclapi2.bangbang93.com/mirrors/authlib-injector/artifact/latest.json"
        }
        _ => "https://authlib-injector.yushi.moe/artifact/latest.json",
    }
}

// ============================================================================
//  下载特质
// ============================================================================

/// Authlib 第三方正版登录模块的下载特质
pub trait AuthlibDownloadExt: Sync {
    /// 下载最新版本的 Authlib Injector 并存放到指定路径
    async fn download_authlib_injector(&self, dest_path: &Path) -> AuthlibResult<()>;

    /// 安装 Authlib Injector 到默认位置（`.minecraft/authlib-injector.jar`）
    async fn install_authlib_injector(&self) -> AuthlibResult<()>;
}

// ============================================================================
//  为 Downloader 实现
// ============================================================================

impl<R: Reporter + ReporterExt> AuthlibDownloadExt for Downloader<R> {
    async fn download_authlib_injector(&self, dest_path: &Path) -> AuthlibResult<()> {
        let reporter = &self.reporter;
        reporter.add_max_progress(2.0);
        reporter.set_message("正在获取 Authlib-Injector 版本元数据".to_string());

        // 1. 获取元数据
        let metadata_url = get_metadata_url(self.source.clone());
        let latest_data: LatestData = HttpClient::default()
            .get_json(metadata_url)
            .await
            .map_err(|e| AuthlibError::MetadataDownload(e.to_string()))?;

        reporter.add_progress(1.0);
        reporter.set_message(format!("正在下载 Authlib-Injector {}", latest_data.version));

        // 2. 下载 JAR
        let download_url = &latest_data.download_url;
        let response = HttpClient::default()
            .get_with_retry(download_url)
            .await
            .map_err(|e| AuthlibError::Http(e.to_string()))?;

        // 获取文件大小（用于进度报告）
        let total_size = response.content_length().unwrap_or(0);

        // 3. 使用临时文件写入
        let temp_dir = dest_path
            .parent()
            .ok_or_else(|| AuthlibError::WriteFailed("无效的目标路径".to_string()))?;
        fs::create_dir_all(temp_dir).await?;

        // 创建临时文件
        let temp_file_path = temp_dir.join(format!(
            ".tmp_{}",
            dest_path.file_name().unwrap_or_default().to_string_lossy()
        ));

        let mut file = fs::File::create(&temp_file_path).await?;
        let mut stream = response.bytes_stream();

        use futures_util::StreamExt;
        let mut downloaded = 0u64;

        while let Some(chunk_result) = stream.next().await {
            let chunk = chunk_result.map_err(|e| AuthlibError::Http(e.to_string()))?;
            file.write_all(&chunk).await?;
            downloaded += chunk.len() as u64;

            // 报告下载进度（占总进度的另一半）
            if total_size > 0 {
                let progress = 1.0 + (downloaded as f64 / total_size as f64);
                reporter.set_progress(progress.min(2.0));
            }
        }

        file.flush().await?;
        drop(file); // 释放句柄

        // 4. 原子重命名
        fs::rename(&temp_file_path, dest_path)
            .await
            .map_err(|e| {
                // 清理临时文件
                let _ = fs::remove_file(&temp_file_path);
                AuthlibError::WriteFailed(format!("重命名失败: {}", e))
            })?;

        reporter.add_progress(1.0);
        reporter.set_message("Authlib-Injector 下载完成".to_string());

        Ok(())
    }

    async fn install_authlib_injector(&self) -> AuthlibResult<()> {
        let minecraft_path = Path::new(&self.minecraft_path);
        let dest_path = minecraft_path.join("authlib-injector.jar");

        if !dest_path.is_file() {
            // 确保目录存在
            fs::create_dir_all(minecraft_path).await?;
            self.download_authlib_injector(&dest_path).await?;
        }

        Ok(())
    }
}

// ============================================================================
//  测试
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{components::download::Downloader, components::progress::NR};
    use tempfile::tempdir;

    // 注意：测试需要网络，可标记为 #[ignore]
    #[tokio::test]
    #[ignore]
    async fn test_download_authlib() {
        let dir = tempdir().unwrap();
        let dest = dir.path().join("authlib-injector.jar");
        let downloader = Downloader::new(".minecraft", NR, DownloadSource::Official);

        downloader.download_authlib_injector(&dest).await.unwrap();
        assert!(dest.is_file());
        assert!(dest.metadata().unwrap().len() > 0);
    }

    #[tokio::test]
    async fn test_install_authlib() {
        let dir = tempdir().unwrap();
        let mc_path = dir.path().to_str().unwrap().to_string();
        let downloader = Downloader::new(&mc_path, NR, DownloadSource::BMCLAPI);

        downloader.install_authlib_injector().await.unwrap();
        let dest = Path::new(&mc_path).join("authlib-injector.jar");
        assert!(dest.is_file());
    }
}