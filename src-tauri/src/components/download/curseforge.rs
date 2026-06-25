//! CurseForge 模组搜索、信息获取与下载
//!
//! 本模块提供 CurseForge API 的异步封装，支持模组搜索、详情查询、文件列表及下载。
//!
//! **注意**：使用前需设置环境变量 `CURSEFORGE_API_KEY` 或通过 [`set_api_key`] 设置 API 密钥。

use std::fmt::Write;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use futures_util::StreamExt;
use serde::Deserialize;
use thiserror::Error;
use tokio::fs::{self, File};
use tokio::io::AsyncWriteExt;

use crate::components::http::{HttpClient, HttpClientConfig};
use crate::components::progress::ReporterExt;
use crate::prelude::*;

// ============================================================================
//  错误类型
// ============================================================================

#[derive(Debug, Error)]
pub enum CurseForgeError {
    #[error("API 密钥未设置，请设置 CURSEFORGE_API_KEY 环境变量")]
    MissingApiKey,

    #[error("HTTP 请求失败: {0}")]
    Http(String),

    #[error("JSON 解析失败: {0}")]
    Json(#[from] serde_json::Error),

    #[error("IO 错误: {0}")]
    Io(#[from] std::io::Error),

    #[error("API 返回错误: status={status}, message={message}")]
    ApiError { status: u16, message: String },

    #[error("无效的响应数据: {0}")]
    InvalidResponse(String),

    #[error("模组图标不存在或无法解析")]
    IconNotFound,

    #[error("下载失败: {0}")]
    DownloadFailed(String),
}

pub type CurseForgeResult<T> = Result<T, CurseForgeError>;

// ============================================================================
//  API 密钥管理
// ============================================================================

static API_KEY: OnceLock<String> = OnceLock::new();

/// 设置 CurseForge API 密钥（优先于环境变量）
pub fn set_api_key(key: impl Into<String>) {
    let _ = API_KEY.set(key.into());
}

/// 获取当前 API 密钥（从环境变量或已设置值）
fn get_api_key() -> CurseForgeResult<String> {
    if let Some(key) = API_KEY.get() {
        return Ok(key.clone());
    }
    std::env::var("CURSEFORGE_API_KEY")
        .map_err(|_| CurseForgeError::MissingApiKey)
}

// ============================================================================
//  常量与客户端
// ============================================================================

const BASE_URL: &str = "https://api.curseforge.com/v1/";

/// 获取经过认证的 HTTP 客户端
fn auth_client() -> CurseForgeResult<HttpClient> {
    let key = get_api_key()?;
    let mut config = HttpClientConfig::default();
    // 添加自定义 header
    // 由于 HttpClient 不直接支持额外 header，我们使用 reqwest 的 builder 方式
    // 为了简单，我们直接使用 reqwest::Client 并封装一层
    // 但我们可以修改 HttpClient 支持额外 headers，或直接使用 reqwest 实例。
    // 这里采用直接使用 reqwest 实例（简化）
    // 更好的做法：扩展 HttpClient 允许添加自定义 headers
    // 为了演示，我们构建一个 reqwest::Client 并存入 static
    Ok(HttpClient::default())
}

/// 发送带认证的 GET 请求并返回 JSON 响应
async fn get_json<T: serde::de::DeserializeOwned>(
    url: &str,
) -> CurseForgeResult<T> {
    let key = get_api_key()?;
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .default_headers({
            let mut headers = reqwest::header::HeaderMap::new();
            headers.insert(
                "x-api-key",
                reqwest::header::HeaderValue::from_str(&key)
                    .map_err(|e| CurseForgeError::Http(e.to_string()))?,
            );
            headers.insert(
                reqwest::header::ACCEPT,
                reqwest::header::HeaderValue::from_static("application/json"),
            );
            headers
        })
        .build()
        .map_err(|e| CurseForgeError::Http(e.to_string()))?;

    let response = client.get(url)
        .send()
        .await
        .map_err(|e| CurseForgeError::Http(e.to_string()))?;

    let status = response.status();
    if !status.is_success() {
        let error_text = response.text().await.unwrap_or_default();
        return Err(CurseForgeError::ApiError {
            status: status.as_u16(),
            message: error_text,
        });
    }

    let data = response.json::<T>()
        .await
        .map_err(|e| CurseForgeError::Http(e.to_string()))?;
    Ok(data)
}

// ============================================================================
//  数据结构（API 响应包装）
// ============================================================================

/// CurseForge API 通用响应包装
#[derive(Debug, Deserialize)]
struct ApiResponse<T> {
    pub data: T,
}

/// 模组资源（图标等）
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModAsset {
    pub id: i32,
    pub mod_id: i32,
    pub title: String,
    pub description: String,
    pub thumbnail_url: String,
    pub url: String,
}

/// 模组基本信息
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModInfo {
    pub id: u64,
    pub name: String,
    pub summary: String,
    pub slug: String,
    pub logo: Option<ModAsset>,
    // 可选：其他字段
}

/// 模组依赖（暂未完全实现）
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Dependency {
    // pub mod_id: i32,
    // pub relation_type: u8,
}

/// 模组文件信息
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModFile {
    pub file_name: String,
    pub download_url: String,
    pub dependencies: Vec<Dependency>,
    pub game_versions: Vec<String>,
}

// ============================================================================
//  搜索参数
// ============================================================================

/// 搜索排序方式
#[derive(Debug, Clone, Copy, Default)]
pub enum SearchSortMethod {
    #[default]
    Featured = 0,
    Popularity = 1,
    LastUpdate = 2,
    Name = 3,
    Author = 4,
    TotalDownloads = 5,
}

/// 搜索参数
#[derive(Debug, Clone)]
pub struct SearchParams {
    pub game_version: String,
    pub index: u32,
    pub page_size: u32,
    pub category_id: u32,
    pub search_filter: String,
    pub sort: SearchSortMethod,
}

impl Default for SearchParams {
    fn default() -> Self {
        Self {
            game_version: String::new(),
            index: 0,
            page_size: 20,
            category_id: 0,
            search_filter: String::new(),
            sort: SearchSortMethod::Featured,
        }
    }
}

// ============================================================================
//  公开 API 函数
// ============================================================================

/// 搜索模组
pub async fn search_mods(params: SearchParams) -> CurseForgeResult<Vec<ModInfo>> {
    let mut url = format!("{}mods/search?gameId=432&classId=6", BASE_URL);
    let _ = write!(&mut url, "&sort={}", params.sort as u8);
    if !params.search_filter.is_empty() {
        let encoded = urlencoding::encode(&params.search_filter);
        let _ = write!(&mut url, "&searchFilter={}", encoded);
    }
    if !params.game_version.is_empty() {
        let _ = write!(&mut url, "&gameVersion={}", params.game_version);
    }
    if params.index > 0 {
        let _ = write!(&mut url, "&index={}", params.index);
    }
    let page_size = if params.page_size > 0 && params.page_size <= 50 {
        params.page_size
    } else {
        20
    };
    let _ = write!(&mut url, "&pageSize={}", page_size);
    if params.category_id > 0 {
        let _ = write!(&mut url, "&categoryID={}", params.category_id);
    }

    let resp: ApiResponse<Vec<ModInfo>> = get_json(&url).await?;
    Ok(resp.data)
}

/// 获取模组详情
pub async fn get_mod_info(mod_id: u64) -> CurseForgeResult<ModInfo> {
    let url = format!("{}mods/{}", BASE_URL, mod_id);
    let resp: ApiResponse<ModInfo> = get_json(&url).await?;
    Ok(resp.data)
}

/// 获取模组文件列表
pub async fn get_mod_files(mod_id: u64) -> CurseForgeResult<Vec<ModFile>> {
    let url = format!("{}mods/{}/files", BASE_URL, mod_id);
    let resp: ApiResponse<Vec<ModFile>> = get_json(&url).await?;
    Ok(resp.data)
}

/// 获取模组图标（返回 image::DynamicImage）
pub async fn get_mod_icon(mod_info: &ModInfo) -> CurseForgeResult<image::DynamicImage> {
    let logo = mod_info.logo.as_ref()
        .ok_or(CurseForgeError::IconNotFound)?;
    let client = HttpClient::default();
    let bytes = client.get_bytes(&logo.thumbnail_url).await
        .map_err(|e| CurseForgeError::Http(e.to_string()))?;
    image::load_from_memory(&bytes)
        .map_err(|_| CurseForgeError::IconNotFound)
}

/// 通过模组 ID 获取图标（便捷方法）
pub async fn get_mod_icon_by_id(mod_id: u64) -> CurseForgeResult<image::DynamicImage> {
    let info = get_mod_info(mod_id).await?;
    get_mod_icon(&info).await
}

/// 下载模组文件（支持进度报告）
pub async fn download_mod<R: Reporter + ReporterExt>(
    url: &str,
    dest: &Path,
    reporter: Option<&R>,
) -> CurseForgeResult<()> {
    let client = HttpClient::default();
    let response = client.get_with_retry(url).await
        .map_err(|e| CurseForgeError::Http(e.to_string()))?;

    let total_size = response.content_length().unwrap_or(0);
    if let Some(r) = reporter {
        r.add_max_progress(1.0);
        r.set_message(format!("下载模组到 {}", dest.display()));
    }

    // 创建临时文件
    let temp_dir = dest.parent()
        .ok_or_else(|| CurseForgeError::DownloadFailed("无效的目标路径".into()))?;
    fs::create_dir_all(temp_dir).await?;
    let temp_path = temp_dir.join(format!(".tmp_{}", dest.file_name().unwrap_or_default().to_string_lossy()));

    let mut file = File::create(&temp_path).await?;
    let mut stream = response.bytes_stream();
    let mut downloaded = 0u64;

    while let Some(chunk_result) = stream.next().await {
        let chunk = chunk_result.map_err(|e| CurseForgeError::Http(e.to_string()))?;
        file.write_all(&chunk).await?;
        downloaded += chunk.len() as u64;

        if let Some(r) = reporter {
            if total_size > 0 {
                let progress = downloaded as f64 / total_size as f64;
                r.set_progress(progress.min(1.0));
            }
        }
    }
    file.flush().await?;
    drop(file);

    // 原子重命名
    fs::rename(&temp_path, dest).await
        .map_err(|e| {
            let _ = fs::remove_file(&temp_path);
            CurseForgeError::DownloadFailed(format!("重命名失败: {}", e))
        })?;

    if let Some(r) = reporter {
        r.set_progress(1.0);
        r.set_message("下载完成".to_string());
    }
    Ok(())
}

// ============================================================================
//  测试
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    // 注意：需要设置环境变量 CURSEFORGE_API_KEY 才能运行实际测试
    // 可以使用 mock 服务器进行单元测试

    #[tokio::test]
    #[ignore]
    async fn test_search_mods() {
        let params = SearchParams {
            search_filter: "jei".to_string(),
            game_version: "1.16.5".to_string(),
            ..Default::default()
        };
        let mods = search_mods(params).await.unwrap();
        assert!(!mods.is_empty());
    }

    #[tokio::test]
    #[ignore]
    async fn test_download_mod() {
        let dir = tempdir().unwrap();
        let dest = dir.path().join("testmod.jar");
        let url = "https://example.com/test.jar"; // 替换为实际 URL
        let result = download_mod(url, &dest, Option::<&NoopReporter>::None).await;
        // 此处仅测试逻辑，实际需要有效的 URL
    }
}