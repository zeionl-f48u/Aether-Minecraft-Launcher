//! 获取模组中文名称的模块
//!
//! 支持多种数据源，包括：
//! - Gitee 仓库 (默认)
//! - MC百科非官方 API
//! - 本地 i18n 数据库
//! - CurseForge API
//! - Modrinth API

use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

use lazy_static::lazy_static;
use serde::Deserialize;
use tokio::fs;
use tokio::sync::Mutex;

use crate::components::http::HttpClient;
use crate::prelude::*;

// ============================================================================
//  错误类型
// ============================================================================

#[derive(Debug, thiserror::Error)]
pub enum McModError {
    #[error("HTTP 请求失败: {0}")]
    Http(String),

    #[error("数据解析失败: {0}")]
    Parse(String),

    #[error("模组 '{0}' 未找到")]
    NotFound(String),

    #[error("IO 错误: {0}")]
    Io(#[from] std::io::Error),

    #[error("所有数据源均失败")]
    AllSourcesFailed,
}

pub type McModResult<T> = Result<T, McModError>;

// ============================================================================
//  数据结构
// ============================================================================

/// 模组名称信息
#[derive(Debug, Clone)]
pub struct ModNameInfo {
    /// 模组 ID (modid)
    pub modid: String,
    /// 中文名称
    pub chinese_name: String,
    /// 英文名称（可选）
    pub english_name: Option<String>,
    /// 数据源标识
    pub source: String,
}

// ============================================================================
//  缓存
// ============================================================================

lazy_static! {
    static ref CACHE: Arc<Mutex<HashMap<String, ModNameInfo>>> = Arc::new(Mutex::new(HashMap::new()));
}

/// 清除缓存
pub async fn clear_cache() {
    CACHE.lock().await.clear();
}

// ============================================================================
//  数据源配置
// ============================================================================

/// 数据源类型
#[derive(Debug, Clone)]
pub enum McModSource {
    /// 使用 Gitee 上的 scl-data 仓库 (默认)
    Gitee,
    /// 使用 MC百科的非官方 API（需要自行部署）
    McModApi { base_url: String },
    /// 使用本地 i18n 数据库（JSON 文件，格式为 { "modid": "中文名" }）
    I18nDatabase { path: String },
    /// 使用 CurseForge API (仅获取英文名)
    CurseForge,
    /// 使用 Modrinth API (仅获取英文名)
    Modrinth,
    /// 组合多个数据源，按顺序尝试（自动回退）
    Combined(Vec<McModSource>),
}

impl Default for McModSource {
    fn default() -> Self {
        McModSource::Gitee
    }
}

// ============================================================================
//  核心函数
// ============================================================================

/// 获取模组的中文名称（使用默认数据源和缓存）
pub async fn get_mod_cname(modid: &str) -> String {
    get_mod_name_info(modid, McModSource::default())
        .await
        .map(|info| info.chinese_name)
        .unwrap_or_else(|_| String::new())
}

/// 获取模组的名称信息（支持自定义数据源，带缓存）
pub async fn get_mod_name_info(modid: &str, source: McModSource) -> McModResult<ModNameInfo> {
    // 1. 检查缓存
    {
        let cache = CACHE.lock().await;
        if let Some(info) = cache.get(modid) {
            return Ok(info.clone());
        }
    }

    // 2. 从数据源获取
    let info = match source {
        McModSource::Gitee => fetch_from_gitee(modid).await?,
        McModSource::McModApi { base_url } => fetch_from_mcmod_api(modid, &base_url).await?,
        McModSource::I18nDatabase { path } => fetch_from_i18n_db(modid, &path).await?,
        McModSource::CurseForge => fetch_from_curseforge(modid).await?,
        McModSource::Modrinth => fetch_from_modrinth(modid).await?,
        McModSource::Combined(sources) => {
            let mut last_err = None;
            for src in sources {
                match fetch_from_source(modid, src).await {
                    Ok(info) => {
                        // 存入缓存
                        let mut cache = CACHE.lock().await;
                        cache.insert(modid.to_string(), info.clone());
                        return Ok(info);
                    }
                    Err(e) => last_err = Some(e),
                }
            }
            return Err(last_err.unwrap_or(McModError::AllSourcesFailed));
        }
    };

    // 3. 存入缓存
    {
        let mut cache = CACHE.lock().await;
        cache.insert(modid.to_string(), info.clone());
    }

    Ok(info)
}

/// 非递归的获取模组名称（避免 async 递归导致的无限 Future 大小问题）
async fn fetch_from_source(modid: &str, source: McModSource) -> McModResult<ModNameInfo> {
    match source {
        McModSource::Gitee => fetch_from_gitee(modid).await,
        McModSource::McModApi { base_url } => fetch_from_mcmod_api(modid, &base_url).await,
        McModSource::I18nDatabase { path } => fetch_from_i18n_db(modid, &path).await,
        McModSource::CurseForge => fetch_from_curseforge(modid).await,
        McModSource::Modrinth => fetch_from_modrinth(modid).await,
        McModSource::Combined(sources) => {
            let mut last_err = None;
            for src in sources {
                match dispatch_source(modid, src).await {
                    Ok(info) => return Ok(info),
                    Err(e) => last_err = Some(e),
                }
            }
            Err(last_err.unwrap_or(McModError::AllSourcesFailed))
        }
    }
}

/// 分发到单个数据源（非递归）
async fn dispatch_source(modid: &str, source: McModSource) -> McModResult<ModNameInfo> {
    match source {
        McModSource::Gitee => fetch_from_gitee(modid).await,
        McModSource::McModApi { base_url } => fetch_from_mcmod_api(modid, &base_url).await,
        McModSource::I18nDatabase { path } => fetch_from_i18n_db(modid, &path).await,
        McModSource::CurseForge => fetch_from_curseforge(modid).await,
        McModSource::Modrinth => fetch_from_modrinth(modid).await,
        McModSource::Combined(_) => Err(McModError::AllSourcesFailed), // 不应嵌套 Combined
    }
}

// ============================================================================
//  数据源实现
// ============================================================================

// ---------- Gitee ----------
async fn fetch_from_gitee(modid: &str) -> McModResult<ModNameInfo> {
    let encoded = urlencoding::encode(modid);
    let url = format!("https://gitee.com/SteveXMH/scl-data/raw/master/mcmod/cname/{}", encoded);

    let client = HttpClient::default();
    let chinese_name = client
        .get_string(&url)
        .await
        .map_err(|e| McModError::Http(e.to_string()))?;

    if chinese_name.trim().is_empty() {
        return Err(McModError::NotFound(modid.to_string()));
    }

    Ok(ModNameInfo {
        modid: modid.to_string(),
        chinese_name: chinese_name.trim().to_string(),
        english_name: None,
        source: "Gitee".to_string(),
    })
}

// ---------- MC百科 API ----------
async fn fetch_from_mcmod_api(modid: &str, base_url: &str) -> McModResult<ModNameInfo> {
    let url = format!("{}/search_api.php?key={}", base_url, modid);
    let client = HttpClient::default();

    #[derive(Deserialize)]
    struct ApiResponse {
        success: bool,
        best_result: Option<BestResult>,
    }

    #[derive(Deserialize)]
    struct BestResult {
        data: ModData,
    }

    #[derive(Deserialize)]
    struct ModData {
        #[serde(rename = "chinese_name")]
        chinese_name: Option<String>,
        #[serde(rename = "sub_name")]
        sub_name: Option<String>,
    }

    let resp: ApiResponse = client
        .get_json(&url)
        .await
        .map_err(|e| McModError::Http(e.to_string()))?;

    if !resp.success {
        return Err(McModError::Parse("API 返回失败".to_string()));
    }

    let best = resp
        .best_result
        .ok_or_else(|| McModError::NotFound(modid.to_string()))?;
    let chinese_name = best
        .data
        .chinese_name
        .ok_or_else(|| McModError::NotFound(modid.to_string()))?;

    Ok(ModNameInfo {
        modid: modid.to_string(),
        chinese_name,
        english_name: best.data.sub_name,
        source: "McModApi".to_string(),
    })
}

// ---------- i18n 本地数据库 ----------
async fn fetch_from_i18n_db(modid: &str, path: &str) -> McModResult<ModNameInfo> {
    // 从本地 JSON 文件加载数据库（若文件不存在或解析失败则报错）
    let content = fs::read_to_string(path)
        .await
        .map_err(|e| McModError::Io(e))?;
    let db: HashMap<String, String> = serde_json::from_str(&content)
        .map_err(|e| McModError::Parse(e.to_string()))?;

    let chinese_name = db
        .get(modid)
        .ok_or_else(|| McModError::NotFound(modid.to_string()))?
        .clone();

    Ok(ModNameInfo {
        modid: modid.to_string(),
        chinese_name,
        english_name: None,
        source: "I18nDB".to_string(),
    })
}

// ---------- CurseForge ----------
async fn fetch_from_curseforge(modid: &str) -> McModResult<ModNameInfo> {
    // CurseForge 通常使用数字 ID 或 slug，这里简化，假设输入为 slug
    // 使用搜索 API 获取模组
    let url = format!(
        "https://api.curseforge.com/v1/mods/search?gameId=432&classId=6&slug={}",
        modid
    );
    let client = HttpClient::default();

    // 需要 API 密钥，此处从环境变量获取
    let api_key = std::env::var("CURSEFORGE_API_KEY")
        .map_err(|_| McModError::Http("缺少 CURSEFORGE_API_KEY 环境变量".to_string()))?;

    let response = client
        .inner()
        .get(&url)
        .header("x-api-key", api_key)
        .send()
        .await
        .map_err(|e| McModError::Http(e.to_string()))?;

    if !response.status().is_success() {
        return Err(McModError::NotFound(modid.to_string()));
    }

    #[derive(Deserialize)]
    struct CfResponse {
        data: Vec<CfMod>,
    }

    #[derive(Deserialize)]
    struct CfMod {
        name: String,
    }

    let resp: CfResponse = response
        .json()
        .await
        .map_err(|e| McModError::Parse(e.to_string()))?;

    let first = resp.data.into_iter().next()
        .ok_or_else(|| McModError::NotFound(modid.to_string()))?;

    // CurseForge 不提供中文名，只返回英文名
    Ok(ModNameInfo {
        modid: modid.to_string(),
        chinese_name: first.name.clone(), // 作为 fallback
        english_name: Some(first.name),
        source: "CurseForge".to_string(),
    })
}

// ---------- Modrinth ----------
async fn fetch_from_modrinth(modid: &str) -> McModResult<ModNameInfo> {
    let url = format!("https://api.modrinth.com/v2/project/{}", modid);
    let client = HttpClient::default();

    #[derive(Deserialize)]
    struct ModrinthProject {
        title: String,
        description: Option<String>,
    }

    let project: ModrinthProject = client
        .get_json(&url)
        .await
        .map_err(|e| McModError::Http(e.to_string()))?;

    // Modrinth 返回的 title 通常是英文名（但可能有社区翻译），作为 fallback
    Ok(ModNameInfo {
        modid: modid.to_string(),
        chinese_name: project.title.clone(),
        english_name: Some(project.title),
        source: "Modrinth".to_string(),
    })
}

// ============================================================================
//  预定义组合数据源
// ============================================================================

/// 返回一个组合数据源，按推荐优先级顺序尝试：
/// 1. Gitee（作者维护的中文名）
/// 2. i18n 本地数据库（如果提供了路径）
/// 3. MC百科 API（如果提供了 base_url）
/// 4. Modrinth API（fallback 英文名）
/// 5. CurseForge API（fallback 英文名）
pub fn default_combined_source(
    i18n_path: Option<String>,
    mcmod_api_url: Option<String>,
) -> McModSource {
    let mut sources = Vec::new();

    // 优先 Gitee
    sources.push(McModSource::Gitee);

    // 其次 i18n 本地
    if let Some(path) = i18n_path {
        sources.push(McModSource::I18nDatabase { path });
    }

    // 再 MC百科 API
    if let Some(base_url) = mcmod_api_url {
        sources.push(McModSource::McModApi { base_url });
    }

    // 最后英文名 fallback
    sources.push(McModSource::Modrinth);
    sources.push(McModSource::CurseForge);

    McModSource::Combined(sources)
}

// ============================================================================
//  测试
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_get_mod_cname_gitee() {
        let name = get_mod_cname("jei").await;
        // 测试需要网络
        if !name.is_empty() {
            println!("JEI 中文名: {}", name);
        }
    }

    #[tokio::test]
    async fn test_get_mod_name_info_combined() {
        let source = default_combined_source(None, None);
        let info = get_mod_name_info("chisel", source).await;
        if let Ok(info) = info {
            println!("Chisel 名称: {:?}", info);
        }
    }

    #[tokio::test]
    async fn test_cache() {
        let _ = get_mod_cname("jei").await;
        // 第二次应该命中缓存
        let start = std::time::Instant::now();
        let _ = get_mod_cname("jei").await;
        let elapsed = start.elapsed();
        assert!(elapsed < std::time::Duration::from_millis(50));
    }
}