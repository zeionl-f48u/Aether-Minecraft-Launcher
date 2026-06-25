//! Modrinth 的模组检索和下载模块
//!
//! 提供 Modrinth API 的异步封装，支持搜索、获取模组信息、版本列表和图标下载。

use std::collections::HashMap;
use std::sync::Arc;

use image::DynamicImage;
use lazy_static::lazy_static;
use serde::Deserialize;
use thiserror::Error;
use tokio::sync::Mutex;

use crate::components::http::HttpClient;
use crate::prelude::*;

// ============================================================================
//  错误类型
// ============================================================================

#[derive(Debug, Error)]
pub enum ModrinthError {
    #[error("HTTP 请求失败: {0}")]
    Http(String),

    #[error("JSON 解析失败: {0}")]
    Json(#[from] serde_json::Error),

    #[error("模组未找到: {0}")]
    NotFound(String),

    #[error("图标解码失败: {0}")]
    ImageDecode(#[from] image::ImageError),

    #[error("无效的搜索参数: {0}")]
    InvalidParam(String),

    #[error("IO 错误: {0}")]
    Io(#[from] std::io::Error),
}

pub type ModrinthResult<T> = Result<T, ModrinthError>;

// ============================================================================
//  数据结构
// ============================================================================

/// 模组搜索结果
#[derive(Debug, Deserialize, Clone)]
pub struct ModResult {
    /// 模组 ID（通常为字符串，例如 "fabric-api"）
    pub project_id: String,
    /// 模组的短链接标识
    pub slug: String,
    /// 图标 URL（可能为空）
    pub icon_url: String,
    /// 模组标题
    pub title: String,
    /// 模组简介
    pub description: String,
}

/// 模组文件信息
#[derive(Debug, Deserialize)]
pub struct ModFile {
    pub url: String,
    pub filename: String,
    pub primary: bool,
}

/// 模组版本信息
#[derive(Debug, Deserialize)]
pub struct ModVersion {
    pub files: Vec<ModFile>,
    pub game_versions: Vec<String>,
    pub loaders: Vec<String>,
}

/// 搜索响应
#[derive(Debug, Deserialize)]
pub struct ModSearchResult {
    pub hits: Vec<ModResult>,
}

// ============================================================================
//  缓存
// ============================================================================

lazy_static! {
    static ref CACHE: Arc<Mutex<HashMap<String, ModResult>>> = Arc::new(Mutex::new(HashMap::new()));
}

/// 清除缓存
pub async fn clear_cache() {
    CACHE.lock().await.clear();
}

// ============================================================================
//  搜索参数
// ============================================================================

/// 搜索参数
#[derive(Debug, Clone)]
pub struct SearchParams {
    /// 搜索关键词
    pub search_filter: String,
    /// 偏移量（从 0 开始）
    pub offset: u64,
    /// 每页数量（最大 100）
    pub limit: u64,
}

impl Default for SearchParams {
    fn default() -> Self {
        Self {
            search_filter: String::new(),
            offset: 0,
            limit: 20,
        }
    }
}

// ============================================================================
//  API 函数
// ============================================================================

/// 搜索模组
pub async fn search_mods(params: SearchParams) -> ModrinthResult<Vec<ModResult>> {
    if params.limit == 0 || params.limit > 100 {
        return Err(ModrinthError::InvalidParam(
            "limit 必须在 1 到 100 之间".into(),
        ));
    }
    let query = urlencoding::encode(&params.search_filter);
    let url = format!(
        "https://api.modrinth.com/v2/search?offset={}&limit={}&query={}",
        params.offset, params.limit, query
    );

    let client = HttpClient::default();
    let result: ModSearchResult = client
        .get_json(&url)
        .await
        .map_err(|e| ModrinthError::Http(e.to_string()))?;

    // 清理 project_id（去除可能的 "local-" 前缀）
    let mut hits = result.hits;
    for hit in &mut hits {
        hit.project_id = hit.project_id.trim_start_matches("local-").to_string();
    }
    Ok(hits)
}

/// 获取模组信息（带缓存）
pub async fn get_mod_info(modid: &str) -> ModrinthResult<ModResult> {
    // 检查缓存
    {
        let cache = CACHE.lock().await;
        if let Some(info) = cache.get(modid) {
            return Ok(info.clone());
        }
    }

    // 从 API 获取
    let url = format!("https://api.modrinth.com/v2/project/{}", modid);
    let client = HttpClient::default();
    let mut info: ModResult = client
        .get_json(&url)
        .await
        .map_err(|e| {
            if e.to_string().contains("404") {
                ModrinthError::NotFound(modid.to_string())
            } else {
                ModrinthError::Http(e.to_string())
            }
        })?;

    // 清理 project_id
    info.project_id = info.project_id.trim_start_matches("local-").to_string();

    // 存入缓存
    {
        let mut cache = CACHE.lock().await;
        cache.insert(modid.to_string(), info.clone());
    }
    Ok(info)
}

/// 获取模组的所有版本列表
pub async fn get_mod_files(modid: &str) -> ModrinthResult<Vec<ModVersion>> {
    let url = format!("https://api.modrinth.com/v2/project/{}/version", modid);
    let client = HttpClient::default();
    let versions: Vec<ModVersion> = client
        .get_json(&url)
        .await
        .map_err(|e| ModrinthError::Http(e.to_string()))?;
    Ok(versions)
}

/// 获取模组图标
pub async fn get_mod_icon(modid: &str) -> ModrinthResult<DynamicImage> {
    let info = get_mod_info(modid).await?;
    get_mod_icon_by_url(&info.icon_url).await
}

/// 根据图标 URL 获取图标（若 URL 无效则返回透明 1x1 图片）
pub async fn get_mod_icon_by_url(url: &str) -> ModrinthResult<DynamicImage> {
    if url.is_empty() {
        return Ok(empty_image());
    }
    let client = HttpClient::default();
    let bytes = client
        .get_bytes(url)
        .await
        .map_err(|e| ModrinthError::Http(e.to_string()))?;

    // 使用 image crate 自动检测格式
    match image::load_from_memory(&bytes) {
        Ok(img) => Ok(img),
        Err(e) => {
            // 若解码失败，返回透明图片（不抛出错误）
            tracing::warn!("图标解码失败 ({}): {}", url, e);
            Ok(empty_image())
        }
    }
}

// ============================================================================
//  辅助函数
// ============================================================================

/// 生成一个 1x1 的透明像素图片
fn empty_image() -> DynamicImage {
    let mut img = image::RgbaImage::new(1, 1);
    img.put_pixel(0, 0, image::Rgba([0, 0, 0, 0]));
    DynamicImage::ImageRgba8(img)
}

// ============================================================================
//  测试
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    #[ignore] // 需要网络
    async fn test_search_mods() {
        let params = SearchParams {
            search_filter: "fabric-api".into(),
            limit: 5,
            ..Default::default()
        };
        let results = search_mods(params).await.unwrap();
        assert!(!results.is_empty());
        assert!(results[0].project_id.contains("fabric-api"));
    }

    #[tokio::test]
    #[ignore]
    async fn test_get_mod_info() {
        let info = get_mod_info("fabric-api").await.unwrap();
        assert_eq!(info.project_id, "fabric-api");
        assert!(!info.title.is_empty());
    }

    #[tokio::test]
    async fn test_empty_image() {
        let img = empty_image();
        assert_eq!(img.width(), 1);
        assert_eq!(img.height(), 1);
    }

    #[test]
    fn test_search_params_default() {
        let params = SearchParams::default();
        assert_eq!(params.limit, 20);
        assert_eq!(params.offset, 0);
    }
}