//! 游戏版本管理模块
//!
//! 负责扫描和解析 Minecraft 版本元数据，支持传统版本（1.x）和年度版本（YY.R.P）。

use std::path::{Path, PathBuf};
use std::time::SystemTime;

use serde::{Deserialize, Serialize};
use tokio::fs;
use tokio::task::spawn_blocking;
use thiserror::Error;

pub mod mods;
pub mod structs;

use self::structs::VersionInfo;
use crate::prelude::*;

pub type DynResult<T> = Result<T, Box<dyn std::error::Error + Send + Sync>>;

// ============================================================================
//  错误类型
// ============================================================================

/// 版本管理相关错误
#[derive(Debug, Error)]
pub enum VersionError {
    #[error("版本目录不存在: {0}")]
    DirectoryNotFound(String),

    #[error("读取目录失败: {0}")]
    ReadDirFailed(String),

    #[error("解析版本元数据失败: {0}")]
    ParseMetadata(String),

    #[error("IO 错误: {0}")]
    Io(#[from] std::io::Error),

    #[error("无效的版本号格式: {0}")]
    InvalidVersionFormat(String),
}

pub type VersionResult<T> = Result<T, VersionError>;

// ============================================================================
//  版本号解析（支持新旧格式）
// ============================================================================

/// 解析后的版本号结构，用于排序和比较
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum ParsedVersion {
    /// 传统版本，如 1.16.5, 1.21.4
    Legacy { major: u32, minor: u32, patch: u32 },
    /// 年度版本，如 26.1, 26.1.2
    Yearly { year: u32, release: u32, patch: u32 },
    /// 快照（旧格式 25w41a 或新格式 26.1-snapshot-1）
    Snapshot { year: u32, identifier: String },
    /// 其他自定义格式（降级为字符串）
    Custom(String),
}

impl ParsedVersion {
    /// 解析版本字符串
    pub fn parse(version_str: &str) -> Self {
        use regex::Regex;
        lazy_static::lazy_static! {
            static ref LEGACY_RE: Regex = Regex::new(r"^(\d+)\.(\d+)(?:\.(\d+))?$").unwrap();
            static ref YEARLY_RE: Regex = Regex::new(r"^(\d{2,4})\.(\d+)(?:\.(\d+))?$").unwrap();
            static ref SNAPSHOT_OLD_RE: Regex = Regex::new(r"^(\d{2})w(\d{2})([a-z])$").unwrap();
            static ref SNAPSHOT_NEW_RE: Regex = Regex::new(r"^(\d{2,4})\.(\d+)-snapshot-(\d+)$").unwrap();
        }

        let version = version_str.trim();

        // 1. 传统格式
        if let Some(caps) = LEGACY_RE.captures(version) {
            let major: u32 = caps[1].parse().unwrap_or(0);
            let minor: u32 = caps[2].parse().unwrap_or(0);
            let patch: u32 = caps.get(3).map_or(0, |m| m.as_str().parse().unwrap_or(0));
            if major == 1 {
                return ParsedVersion::Legacy { major, minor, patch };
            }
        }

        // 2. 年度格式
        if let Some(caps) = YEARLY_RE.captures(version) {
            let year: u32 = caps[1].parse().unwrap_or(0);
            let release: u32 = caps[2].parse().unwrap_or(0);
            let patch: u32 = caps.get(3).map_or(0, |m| m.as_str().parse().unwrap_or(0));
            if year >= 26 {
                return ParsedVersion::Yearly { year, release, patch };
            }
        }

        // 3. 旧快照
        if let Some(caps) = SNAPSHOT_OLD_RE.captures(version) {
            let year: u32 = caps[1].parse().unwrap_or(0);
            let week: u32 = caps[2].parse().unwrap_or(0);
            let letter = caps[3].to_string();
            if year >= 10 && year <= 25 && week >= 1 && week <= 52 {
                return ParsedVersion::Snapshot {
                    year,
                    identifier: format!("w{:02}{}", week, letter),
                };
            }
        }

        // 4. 新快照
        if let Some(caps) = SNAPSHOT_NEW_RE.captures(version) {
            let year: u32 = caps[1].parse().unwrap_or(0);
            let release: u32 = caps[2].parse().unwrap_or(0);
            let snapshot_num: u32 = caps[3].parse().unwrap_or(0);
            if year >= 26 {
                return ParsedVersion::Snapshot {
                    year,
                    identifier: format!(".{}-snapshot-{}", release, snapshot_num),
                };
            }
        }

        // 5. 自定义
        ParsedVersion::Custom(version.to_string())
    }

    /// 是否为正式版（非快照）
    pub fn is_release(&self) -> bool {
        matches!(self, ParsedVersion::Legacy { .. } | ParsedVersion::Yearly { .. })
    }

    /// 获取主版本号（用于比较）
    pub fn major_version(&self) -> Option<u32> {
        match self {
            ParsedVersion::Legacy { major, .. } => Some(*major),
            ParsedVersion::Yearly { year, .. } => Some(*year),
            ParsedVersion::Snapshot { year, .. } => Some(*year),
            ParsedVersion::Custom(_) => None,
        }
    }
}

// ============================================================================
//  版本类型定义（增强推断）
// ============================================================================

/// 版本类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum VersionType {
    /// 纯净版本
    Vanilla,
    /// Forge 版本
    Forge,
    /// NeoForge 版本
    NeoForge,
    /// Fabric 版本
    Fabric,
    /// QuiltMC 版本
    QuiltMC,
    /// Optifine 画质增强版本
    Optifine,
    /// 未知版本
    Unknown,
}

impl Default for VersionType {
    fn default() -> Self {
        Self::Unknown
    }
}

impl VersionType {
    /// 从版本名称和元数据推断类型
    pub fn infer_from(name: &str, meta: &VersionInfo) -> Self {
        let name_lower = name.to_lowercase();
        // 优先从元数据中读取类型（VersionMeta does not have version_type, skip）
        let _ = meta;
        // 启发式推断
        if name_lower.contains("forge") && !name_lower.contains("neoforge") {
            VersionType::Forge
        } else if name_lower.contains("neoforge") {
            VersionType::NeoForge
        } else if name_lower.contains("fabric") {
            VersionType::Fabric
        } else if name_lower.contains("quilt") {
            VersionType::QuiltMC
        } else if name_lower.contains("optifine") {
            VersionType::Optifine
        } else {
            VersionType::Vanilla // 默认纯净
        }
    }
}

// ============================================================================
//  版本信息结构
// ============================================================================

/// 一个游戏版本的信息
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Version {
    /// 版本名称，通常和其文件夹同名
    pub name: String,
    /// 版本类型
    pub version_type: VersionType,
    /// 版本的创建日期
    pub created_date: SystemTime,
    /// 版本的上一次游玩日期
    pub access_date: SystemTime,
}

impl Default for Version {
    fn default() -> Self {
        Self {
            name: String::new(),
            version_type: VersionType::Unknown,
            created_date: SystemTime::UNIX_EPOCH,
            access_date: SystemTime::UNIX_EPOCH,
        }
    }
}

// ============================================================================
//  核心函数：扫描可用版本
// ============================================================================

/// 通过指定的版本文件夹，搜索所有可启动的游戏版本
///
/// # 参数
/// - `version_directory_path`: 版本文件夹路径（通常是 `.minecraft/versions`）
///
/// # 返回
/// 成功返回 `Vec<Version>`，包含所有可启动版本的信息。
pub async fn get_avaliable_versions(
    version_directory_path: impl AsRef<Path>,
) -> DynResult<Vec<Version>> {
    let dir = version_directory_path.as_ref();
    if !dir.is_dir() {
        return Ok(vec![]);
    }

    // 使用 spawn_blocking 进行同步目录读取，避免阻塞异步运行时
    let dir_path = dir.to_path_buf();
    let entries = spawn_blocking(move || {
        let mut result = Vec::new();
        let read_dir = std::fs::read_dir(&dir_path)
            .map_err(|e| VersionError::ReadDirFailed(e.to_string()))?;
        for entry in read_dir {
            let entry = entry.map_err(|e| VersionError::ReadDirFailed(e.to_string()))?;
            let metadata = entry.metadata().map_err(|e| VersionError::Io(e))?;
            let created = metadata.created().unwrap_or(SystemTime::UNIX_EPOCH);
            let accessed = metadata.accessed().unwrap_or(SystemTime::UNIX_EPOCH);
            let path = entry.path();
            if path.is_dir() {
                let name = path.file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("")
                    .to_string();
                if name.is_empty() {
                    continue;
                }
                // 尝试加载版本元数据
                let meta_path = path.join(format!("{}.json", name));
                let version_type = if meta_path.exists() {
                    if let Ok(content) = std::fs::read_to_string(&meta_path) {
                        if let Ok(meta) = serde_json::from_str::<structs::VersionMeta>(&content) {
                            let mut info = VersionInfo {
                                version: name.clone(),
                                version_base: dir_path.to_string_lossy().to_string(),
                                meta: Some(meta),
                                ..Default::default()
                            };
                            // 使用推断函数
                            VersionType::infer_from(&name, &info)
                        } else {
                            VersionType::Unknown
                        }
                    } else {
                        VersionType::Unknown
                    }
                } else {
                    VersionType::Unknown
                };
                result.push(Version {
                    name,
                    version_type,
                    created_date: created,
                    access_date: accessed,
                });
            }
        }
        Ok::<_, VersionError>(result)
    }).await
        .map_err(|e| anyhow::anyhow!("阻塞任务失败: {}", e))?;

    let mut versions = entries?;
    // 按解析后的版本号排序
    versions.sort_by(|a, b| {
        let va = ParsedVersion::parse(&a.name);
        let vb = ParsedVersion::parse(&b.name);
        va.cmp(&vb)
    });

    Ok(versions)
}

// ============================================================================
//  兼容旧 API 的辅助函数
// ============================================================================

/// 加载指定版本的完整元数据（保留原接口）
pub async fn load_version_info(version: &str, versions_dir: &Path) -> DynResult<VersionInfo> {
    let mut info = VersionInfo {
        version: version.to_string(),
        version_base: versions_dir.to_string_lossy().to_string(),
        ..Default::default()
    };
    info.load().await?;
    Ok(info)
}

// ============================================================================
//  测试
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_version() {
        assert_eq!(
            ParsedVersion::parse("1.16.5"),
            ParsedVersion::Legacy { major: 1, minor: 16, patch: 5 }
        );
        assert_eq!(
            ParsedVersion::parse("26.1"),
            ParsedVersion::Yearly { year: 26, release: 1, patch: 0 }
        );
        assert_eq!(
            ParsedVersion::parse("26.1.2"),
            ParsedVersion::Yearly { year: 26, release: 1, patch: 2 }
        );
        assert_eq!(
            ParsedVersion::parse("25w41a"),
            ParsedVersion::Snapshot { year: 25, identifier: "w41a".into() }
        );
        assert_eq!(
            ParsedVersion::parse("26.1-snapshot-1"),
            ParsedVersion::Snapshot { year: 26, identifier: ".1-snapshot-1".into() }
        );
        assert!(ParsedVersion::parse("custom").is_custom());
    }

    #[test]
    fn test_version_type_infer() {
        let mut info = VersionInfo::default();
        assert_eq!(VersionType::infer_from("1.16.5", &info), VersionType::Vanilla);
        assert_eq!(VersionType::infer_from("forge-1.16.5", &info), VersionType::Forge);
        assert_eq!(VersionType::infer_from("fabric-1.17", &info), VersionType::Fabric);
        assert_eq!(VersionType::infer_from("quilt-1.18", &info), VersionType::QuiltMC);
        assert_eq!(VersionType::infer_from("neoforge-1.19", &info), VersionType::NeoForge);
        assert_eq!(VersionType::infer_from("optifine-1.16.5", &info), VersionType::Optifine);
    }
}