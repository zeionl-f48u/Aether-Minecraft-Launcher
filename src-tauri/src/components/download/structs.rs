//! 下载模块的数据结构
//!
//! 定义了版本清单、Forge、NeoForge、Optifine 等外部 API 的响应结构。

use std::collections::BTreeMap as Map;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use url::Url;

// ============================================================================
//  版本清单（Version Manifest）
// ============================================================================

/// 版本清单，包含所有可用的 Minecraft 版本
#[derive(Debug, Deserialize, Serialize, PartialEq, Eq, Clone)]
pub struct VersionManifest {
    /// 最新的正式版和快照版
    pub latest: LatestVersion,
    /// 所有版本列表
    pub versions: Vec<ManifestVersion>,
}

/// 最新的正式版和快照版本
#[derive(Debug, Deserialize, Serialize, PartialEq, Eq, Clone)]
pub struct LatestVersion {
    /// 最新的正式版本号（如 "1.20.4"）
    pub release: String,
    /// 最新的快照版本号（如 "24w12a"）
    pub snapshot: String,
}

/// 清单中的一个版本条目
#[derive(Debug, Deserialize, Serialize, PartialEq, Eq, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ManifestVersion {
    /// 版本 ID（如 "1.20.4"）
    pub id: String,
    /// 版本类型（"release" 或 "snapshot"）
    #[serde(rename = "type")]
    pub version_type: String,
    /// 该版本元数据 JSON 的下载 URL
    pub url: Url,
    /// 更新日期
    pub time: DateTime<Utc>,
    /// 发布日期
    pub release_time: DateTime<Utc>,
}

// ============================================================================
//  资源索引（Assets Index）
// ============================================================================

/// 资源索引文件结构
#[derive(Debug, Deserialize, Serialize, PartialEq, Eq, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct AssetIndexes {
    /// 是否需要在启动前将资源映射到 `game_dir/resources`（旧版特性）
    #[serde(default)]
    pub map_to_resources: bool,
    /// 资源文件哈希表，键为相对路径，值为资源项
    pub objects: Map<String, AssetItem>,
}

/// 单个资源项
#[derive(Debug, Deserialize, Serialize, PartialEq, Eq, Clone)]
pub struct AssetItem {
    /// 文件的 SHA1 哈希值（用于生成下载路径和校验）
    pub hash: String,
    /// 文件大小（字节）
    pub size: usize,
}

// ============================================================================
//  Forge
// ============================================================================

/// Forge 版本数据（推荐、最新、全部）
pub type ForgeVersionsData = VersionsData<ForgeItemInfo>;

/// 单个 Forge 版本信息
#[derive(Debug, Deserialize, Serialize, PartialEq, Eq, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ForgeItemInfo {
    /// 版本号（如 "36.2.34"）
    pub version: String,
    /// 对应的 Minecraft 版本
    pub mcversion: String,
    /// 该版本的文件列表（安装器、通用、MDK 等）
    pub files: Vec<ForgeFile>,
}

/// Forge 文件信息
#[derive(Debug, Deserialize, Serialize, PartialEq, Eq, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ForgeFile {
    /// 文件类别（"installer", "universal", "mdk" 等）
    pub category: String,
    /// 文件格式（"jar", "exe", "zip" 等）
    pub format: String,
    /// 下载 URL
    pub url: Url,
}

/// Forge 推广版本（由 promo 接口返回）
#[derive(Debug, Deserialize, Serialize, PartialEq, Eq, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ForgePromoItem {
    /// 版本名称（如 "1.20.4-recommended"）
    pub name: String,
    /// 对应的版本号（字符串，如 "36.2.34"）
    pub build: Option<String>,
}

// ============================================================================
//  NeoForge
// ============================================================================

pub type NeoForgeVersionsData = VersionsData<NeoForgeItemInfo>;

/// 单个 NeoForge 版本信息
#[derive(Debug, Deserialize, Serialize, PartialEq, Eq, Clone)]
#[serde(rename_all = "camelCase")]
pub struct NeoForgeItemInfo {
    /// 版本号（如 "47.1.99"）
    pub version: String,
    /// 原始版本号（可能包含前缀，如 "neoforge-20.4.0-beta"）
    #[serde(default)]
    pub raw_version: String,
    /// 安装器下载路径（相对于 Maven 仓库）
    #[serde(default)]
    pub installer_path: String,
    /// 对应的 Minecraft 版本
    pub mcversion: String,
}

// ============================================================================
//  通用版本数据（Forge / NeoForge 共用）
// ============================================================================

/// 通用版本数据容器
#[derive(Debug, Clone, Default)]
pub struct VersionsData<T> {
    /// 推荐的版本（可能为 None）
    pub recommended: Option<T>,
    /// 最新的版本（可能为 None）
    pub latest: Option<T>,
    /// 所有版本列表
    pub all_versions: Vec<T>,
}

// ============================================================================
//  Optifine
// ============================================================================

/// Optifine 版本元数据
#[derive(Debug, Deserialize, Serialize, PartialEq, Eq, Clone)]
#[serde(rename_all = "camelCase")]
pub struct OptifineVersionMeta {
    /// 对应的 Minecraft 版本
    pub mcversion: String,
    /// Optifine 类型（通常为 "HD_U"）
    #[serde(rename = "type")]
    pub version_type: String,
    /// 补丁版本（如 "G5"）
    pub patch: String,
    /// 文件名（如 "OptiFine_1.16.5_HD_U_G5.jar"）
    pub filename: String,
}

// ============================================================================
//  测试
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_deserialize_version_manifest() {
        // 示例 JSON（仅作验证）
        let json = r#"{
            "latest": {
                "release": "1.20.4",
                "snapshot": "24w12a"
            },
            "versions": [
                {
                    "id": "1.20.4",
                    "type": "release",
                    "url": "https://launchermeta.mojang.com/v1/packages/.../1.20.4.json",
                    "time": "2023-12-07T15:00:00+00:00",
                    "releaseTime": "2023-12-07T15:00:00+00:00"
                }
            ]
        }"#;
        let manifest: VersionManifest = serde_json::from_str(json).unwrap();
        assert_eq!(manifest.latest.release, "1.20.4");
        assert_eq!(manifest.versions.len(), 1);
        assert_eq!(manifest.versions[0].id, "1.20.4");
    }

    #[test]
    fn test_deserialize_forge_promo() {
        let json = r#"{
            "name": "1.20.4-recommended",
            "build": "36.2.34"
        }"#;
        let promo: ForgePromoItem = serde_json::from_str(json).unwrap();
        assert_eq!(promo.build, Some("36.2.34".to_string()));
    }
}