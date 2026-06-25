//! 所有的启动器元数据结构都在这里
//!
//! 定义了 Minecraft 版本元数据的完整结构，以及版本信息的加载、保存、扫描等操作。

use std::{
    collections::BTreeMap as Map,
    fmt,
    marker::PhantomData,
    path::{Path, PathBuf},
};

use serde::{
    de::{self, SeqAccess, Visitor},
    Deserialize, Deserializer, Serialize,
};
use thiserror::Error;
use tokio::fs;
use tokio::io::AsyncWriteExt;

use super::VersionType;
use crate::{
    components::package::PackageName,
    prelude::*,
    components::semver::MinecraftVersion,
    components::utils::{get_full_path, NATIVE_ARCH_LAZY, TARGET_OS},
};

// ============================================================================
//  错误类型
// ============================================================================

/// 版本管理相关错误
#[derive(Debug, Error)]
pub enum VersionError {
    #[error("版本目录不存在: {0}")]
    DirectoryNotFound(PathBuf),

    #[error("读取元数据失败: {0}")]
    ReadMetadata(#[from] std::io::Error),

    #[error("JSON 解析失败: {0}")]
    JsonParse(#[from] serde_json::Error),

    #[error("TOML 解析失败: {0}")]
    TomlParse(#[from] toml::de::Error),

    #[error("目标版本已存在: {0}")]
    VersionExists(String),

    #[error("版本元数据缺失: {0}")]
    MissingMetadata(String),

    #[error("无效的 BOM 头")]
    InvalidBom,

    #[error("异步任务失败: {0}")]
    JoinError(#[from] tokio::task::JoinError),
}

pub type VersionResult<T> = Result<T, VersionError>;

// ============================================================================
//  规则与条件判断
// ============================================================================

/// 一个针对系统的规则
#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone)]
pub struct OSRule {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub version: String,
    #[serde(default)]
    pub arch: String,
}

/// 一个规则
#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone)]
pub struct ApplyRule {
    pub action: String,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub os: Option<OSRule>,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub features: Option<Map<String, bool>>,
}

/// 判断规则是否满足
pub trait Allowed {
    fn is_allowed(&self) -> bool;
}

impl Allowed for [ApplyRule] {
    fn is_allowed(&self) -> bool {
        if self.is_empty() {
            return true;
        }

        let mut allowed = false;
        for rule in self {
            // 检查 os 条件
            let os_match = rule.os.as_ref().map_or(true, |os| {
                let name_match = os.name.is_empty() || os.name == TARGET_OS;
                let arch_match = os.arch.is_empty() || os.arch == *NATIVE_ARCH_LAZY;
                name_match && arch_match
            });

            // 如果有 features 条件且不匹配，跳过（暂未实现 features 检查）
            if let Some(features) = &rule.features {
                // 简单实现：如果 features 不为空，当作匹配（实际上需要外部传入）
                // 这里保持原有行为，即忽略 features 检查
            }

            if rule.action == "allow" && os_match {
                allowed = true;
            } else if rule.action == "disallow" && os_match {
                return false; // disallow 直接否决
            }
        }
        allowed
    }
}

// ============================================================================
//  参数与下载结构
// ============================================================================

/// 特殊指派的参数
#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone)]
pub struct SpecificalArgument {
    #[serde(default)]
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub rules: Vec<ApplyRule>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    #[serde(deserialize_with = "string_or_seq")]
    pub value: Vec<String>,
}

/// 游戏启动的一个参数
#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone)]
#[serde(untagged)]
pub enum Argument {
    Common(String),
    Specify(SpecificalArgument),
}

/// 游戏启动参数
#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone, Default)]
#[serde(default)]
pub struct Arguments {
    pub game: Vec<Argument>,
    pub jvm: Vec<Argument>,
}

/// 素材索引信息
#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone)]
pub struct AssetIndex {
    pub id: String,
    pub sha1: String,
    pub size: u32,
    #[serde(rename = "totalSize")]
    pub total_size: u32,
    pub url: String,
}

/// 一个下载项目结构
#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone)]
pub struct DownloadItem {
    #[serde(default)]
    pub path: String,
    pub sha1: String,
    pub size: usize,
    pub url: String,
}

/// 依赖库的下载信息结构
#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone)]
pub struct LibraryDownload {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub artifact: Option<DownloadItem>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub classifiers: Option<Map<String, DownloadItem>>,
}

/// 一个依赖库结构
#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone)]
pub struct Library {
    #[serde(default)]
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub rules: Vec<ApplyRule>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub downloads: Option<LibraryDownload>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub natives: Option<Map<String, String>>,
    pub name: String,
}

/// 日志配置文件的下载信息
#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone)]
pub struct LoggingFile {
    pub id: String,
    pub sha1: String,
    pub size: u32,
    pub url: String,
}

/// 日志处理方式
#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone)]
pub struct LoggingConfig {
    pub argument: String,
    #[serde(rename = "type")]
    pub logger_type: String,
    pub file: LoggingFile,
}

/// SCL 独立版本设置
#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone, Default)]
#[serde(default)]
pub struct SCLLaunchConfig {
    pub max_mem: Option<usize>,
    pub java_path: String,
    pub game_independent: bool,
    pub window_title: String,
    pub jvm_args: String,
    pub game_args: String,
    pub wrapper_path: String,
    pub wrapper_args: String,
}

/// 版本元数据里的日志方式
#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone)]
pub struct Logging {
    pub client: Option<LoggingConfig>,
}

/// 新版本的 Java 版本元数据
#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone)]
pub struct JavaVersion {
    pub component: String,
    #[serde(rename = "majorVersion")]
    pub major_version: u8,
}

/// 版本元数据
#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone)]
pub struct VersionMeta {
    #[serde(default)]
    #[serde(rename = "inheritsFrom")]
    pub inherits_from: String,
    #[serde(default)]
    #[serde(rename = "clientVersion")]
    pub client_version: String,
    #[serde(default)]
    #[serde(rename = "javaVersion")]
    pub java_version: Option<JavaVersion>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub arguments: Option<Arguments>,
    #[serde(default)]
    #[serde(rename = "minecraftArguments")]
    pub minecraft_arguments: String,
    #[serde(rename = "assetIndex")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub asset_index: Option<AssetIndex>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub downloads: Option<Map<String, DownloadItem>>,
    #[serde(default)]
    pub libraries: Vec<Library>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub logging: Option<Logging>,
    #[serde(rename = "mainClass")]
    pub main_class: String,
    #[serde(skip)]
    pub main_jars: Vec<String>,
}

impl VersionMeta {
    /// 修正 libraries 中缺失的 downloads 信息（用于兼容旧版本）
    pub(crate) fn fix_libraries(&mut self) {
        for library in &mut self.libraries {
            if library.rules.is_allowed()
                && library.downloads.is_none()
                && library.natives.is_none()
            {
                if let Ok(p) = library.name.parse::<PackageName>() {
                    let p = p.to_maven_jar_path("");
                    library.downloads = Some(LibraryDownload {
                        artifact: Some(DownloadItem {
                            path: p,
                            sha1: String::new(),
                            size: 0,
                            url: String::new(),
                        }),
                        classifiers: None,
                    });
                }
            }
        }
    }

    /// 根据元数据判断需要的 Java 版本
    pub fn required_java_version(&self) -> u8 {
        if let Some(java_version) = &self.java_version {
            java_version.major_version
        } else if let Some(assets) = &self.asset_index {
            if let Some(ver) = crate::components::semver::parse_version(&assets.id) {
                ver.required_java_version()
            } else {
                8
            }
        } else if !self.inherits_from.is_empty() {
            if let Some(ver) = crate::components::semver::parse_version(&self.inherits_from) {
                ver.required_java_version()
            } else {
                8
            }
        } else {
            8
        }
    }
}

impl std::ops::AddAssign for VersionMeta {
    fn add_assign(&mut self, data: VersionMeta) {
        self.main_class = data.main_class;
        self.minecraft_arguments = data.minecraft_arguments;
        self.libraries.extend(data.libraries);
        self.main_jars.extend(data.main_jars);

        if let Some(downloads) = data.downloads {
            if let Some(self_downloads) = &mut self.downloads {
                self_downloads.extend(downloads);
            } else {
                self.downloads = Some(downloads);
            }
        }

        if let Some(arguments) = data.arguments {
            if let Some(self_arguments) = &mut self.arguments {
                self_arguments.jvm.extend(arguments.jvm);
                self_arguments.game.extend(arguments.game);
            } else {
                self.arguments = Some(arguments);
            }
        }

        if let Some(logging) = data.logging {
            self.logging = Some(logging);
        }
    }
}

// ============================================================================
//  版本信息主结构
// ============================================================================

/// 版本信息，包含加载后的元数据和辅助功能
#[derive(Debug, Clone, Default)]
pub struct VersionInfo {
    pub version_base: String,
    pub version: String,
    pub meta: Option<VersionMeta>,
    pub scl_launch_config: Option<SCLLaunchConfig>,
    pub version_type: VersionType,
    pub minecraft_version: MinecraftVersion,
    pub required_java: u8,
}

impl VersionInfo {
    // ---------- 路径辅助 ----------
    fn version_dir(&self) -> PathBuf {
        Path::new(&self.version_base).join(&self.version)
    }

    fn meta_path(&self) -> PathBuf {
        self.version_dir().join(format!("{}.json", self.version))
    }

    fn scl_config_path(&self) -> PathBuf {
        self.version_dir().join(".scl.json")
    }

    /// 游戏主目录（根据是否独立版本）
    pub fn version_path(&self) -> PathBuf {
        if self
            .scl_launch_config
            .as_ref()
            .map(|x| x.game_independent)
            .unwrap_or(false)
        {
            self.version_dir()
        } else {
            Path::new(&self.version_base)
                .parent()
                .unwrap_or(Path::new(""))
                .to_path_buf()
        }
    }

    // ---------- 加载与保存 ----------
    /// 加载版本元数据
    pub async fn load(&mut self) -> VersionResult<()> {
        let version_dir = self.version_dir();
        if !version_dir.is_dir() {
            return Err(VersionError::DirectoryNotFound(version_dir));
        }

        let meta_path = self.meta_path();
        if !meta_path.is_file() {
            return Err(VersionError::MissingMetadata(format!(
                "版本 {} 的元数据文件不存在",
                self.version
            )));
        }

        // 加载 SCL 配置（可选）
        let scl_path = self.scl_config_path();
        if scl_path.is_file() {
            let content = fs::read_to_string(&scl_path).await?;
            let config: SCLLaunchConfig =
                serde_json::from_str(content.trim_start_matches('\u{feff}'))?;
            self.scl_launch_config = Some(config);
        }

        // 加载版本元数据
        let content = fs::read_to_string(&meta_path).await?;
        let mut meta: VersionMeta =
            serde_json::from_str(content.trim_start_matches('\u{feff}'))?;

        // 修正 libraries
        meta.fix_libraries();

        // 主 Jar 文件
        let jar_path = version_dir.join(format!("{}.jar", self.version));
        if jar_path.is_file() {
            if let Ok(full_path) = get_full_path(jar_path) {
                meta.main_jars.push(full_path);
            }
        }

        // 计算所需 Java 版本
        self.required_java = meta.required_java_version();

        // 解析 Minecraft 版本
        if let Some(assets) = &meta.asset_index {
            if let Some(ver) = crate::components::semver::parse_version(&assets.id) {
                self.minecraft_version = ver;
            }
        } else if !meta.inherits_from.is_empty() {
            if let Some(ver) = crate::components::semver::parse_version(&meta.inherits_from) {
                self.minecraft_version = ver;
            }
        }

        self.meta = Some(meta);
        self.version_type = self.guess_version_type();
        Ok(())
    }

    /// 保存元数据和 SCL 配置到文件
    pub async fn save(&self) -> VersionResult<()> {
        let version_dir = self.version_dir();
        if !version_dir.is_dir() {
            return Err(VersionError::DirectoryNotFound(version_dir));
        }

        // 保存元数据
        if let Some(meta) = &self.meta {
            let meta_path = self.meta_path();
            let json = serde_json::to_vec_pretty(meta)?;
            let mut file = fs::File::create(&meta_path).await?;
            file.write_all(&json).await?;
            file.sync_all().await?;
        }

        // 保存或删除 SCL 配置
        let scl_path = self.scl_config_path();
        if let Some(config) = &self.scl_launch_config {
            let json = serde_json::to_vec_pretty(config)?;
            let mut file = fs::File::create(&scl_path).await?;
            file.write_all(&json).await?;
            file.sync_all().await?;
        } else if scl_path.is_file() {
            fs::remove_file(&scl_path).await?;
        }

        Ok(())
    }

    /// 删除整个版本文件夹
    pub async fn delete(self) -> VersionResult<()> {
        let version_dir = self.version_dir();
        if version_dir.is_dir() {
            fs::remove_dir_all(&version_dir).await?;
        }
        Ok(())
    }

    /// 重命名版本（若目标不存在）
    pub async fn rename_version(&mut self, new_name: &str) -> VersionResult<()> {
        let old_dir = self.version_dir();
        if !old_dir.is_dir() {
            return Err(VersionError::DirectoryNotFound(old_dir));
        }

        let new_dir = Path::new(&self.version_base).join(new_name);
        if new_dir.is_dir() {
            return Err(VersionError::VersionExists(new_name.to_string()));
        }

        // 先重命名目录内文件
        let old_meta = old_dir.join(format!("{}.json", self.version));
        let new_meta = old_dir.join(format!("{new_name}.json"));
        if old_meta.is_file() {
            fs::rename(&old_meta, &new_meta).await?;
        }

        let old_jar = old_dir.join(format!("{}.jar", self.version));
        let new_jar = old_dir.join(format!("{new_name}.jar"));
        if old_jar.is_file() {
            fs::rename(&old_jar, &new_jar).await?;
        }

        // 重命名目录
        fs::rename(&old_dir, &new_dir).await?;
        self.version = new_name.to_string();
        Ok(())
    }

    // ---------- 类型推断 ----------
    /// 根据元数据猜测版本类型
    pub fn guess_version_type(&self) -> VersionType {
        let meta = match &self.meta {
            Some(m) => m,
            None => return VersionType::Unknown,
        };

        let mut has_optifine = false;
        let mut has_fabric = false;

        for lib in &meta.libraries {
            if lib.name.starts_with("net.fabricmc:") {
                has_fabric = true;
            } else if lib.name.starts_with("net.neoforged:") {
                return VersionType::NeoForge;
            } else if lib.name.starts_with("net.minecraftforge:") {
                return VersionType::Forge;
            } else if lib.name.starts_with("org.quiltmc:") {
                return VersionType::QuiltMC;
            } else if lib.name.starts_with("optifine:") {
                has_optifine = true;
            }
        }

        if has_fabric {
            VersionType::Fabric
        } else if has_optifine {
            VersionType::Optifine
        } else {
            VersionType::Vanilla
        }
    }

    // ---------- 模组扫描 ----------
    /// 获取该版本下的所有模组
    pub async fn get_mods(&self) -> VersionResult<Vec<super::mods::Mod>> {
        let mods_path = self.version_path().join("mods");
        if !mods_path.is_dir() {
            return Ok(vec![]);
        }

        let mut entries = fs::read_dir(&mods_path).await?;
        let mut mods = Vec::new();
        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            if path.is_file() {
                let name = path.file_name().unwrap().to_string_lossy();
                if name.ends_with(".jar") || name.ends_with(".jar.disabled") {
                    mods.push(super::mods::Mod::from_path(path));
                }
            }
        }
        Ok(mods)
    }

    // ---------- 自动内存计算 ----------
    /// 根据模组数量和可用内存计算推荐的最大内存（MB）
    pub async fn get_automated_maximum_memory(&self) -> u64 {
        let mods = self.get_mods().await.unwrap_or_default();
        let mod_count = mods.len();

        let mem_status = crate::components::utils::get_mem_status().unwrap_or(crate::components::utils::MemoryStatus { total: 0, free: 0 });
        let mut free = mem_status.free as i64;

        let (mem_min, mem_t1, mem_t2, mem_t3) = if mod_count > 0 {
            (
                400 + mod_count as i64 * 7,
                1500 + mod_count as i64 * 10,
                3000 + mod_count as i64 * 17,
                6000 + mod_count as i64 * 34,
            )
        } else {
            (300, 1500, 2500, 4000)
        };

        let mut result = 0;
        // 阶段1：0 ~ T1，100%
        let mut remain = free - 100;
        if remain > 0 {
            let delta = mem_t1;
            let alloc = remain.min(delta);
            result += alloc;
            remain -= alloc + 100;
        } else {
            return result.max(mem_min) as u64;
        }

        // 阶段2：T1 ~ T2，80%
        if remain > 0 {
            let delta = mem_t2 - mem_t1;
            let alloc = ((remain as f64 * 0.8) as i64).min(delta);
            result += alloc;
            remain -= ((delta as f64 / 0.8) as i64) + 100;
        } else {
            return result.max(mem_min) as u64;
        }

        // 阶段3：T2 ~ T3，60%
        if remain > 0 {
            let delta = mem_t3 - mem_t2;
            let alloc = ((remain as f64 * 0.6) as i64).min(delta);
            result += alloc;
            remain -= ((delta as f64 / 0.6) as i64) + 200;
        } else {
            return result.max(mem_min) as u64;
        }

        // 阶段4：T3 ~ T3*2，40%
        if remain > 0 {
            let delta = mem_t3;
            let alloc = ((remain as f64 * 0.4) as i64).min(delta);
            result += alloc;
            // 不再需要扣减，结束
        }

        result.max(mem_min) as u64
    }

    // ---------- 世界存档与资源包（待实现） ----------
    /// 读取该版本下的所有世界存档（目前仅占位）
    pub async fn get_saves(&self) -> VersionResult<Vec<WorldSave>> {
        // TODO: 实现解析
        Ok(vec![])
    }

    /// 读取该版本下的所有资源包（目前仅占位）
    pub async fn get_resources_packs(&self) -> VersionResult<Vec<ResourcesPack>> {
        // TODO: 实现解析
        Ok(vec![])
    }
}

// ============================================================================
//  辅助类型（占位）
// ============================================================================

/// 一个世界存档的信息结构（待完善）
#[derive(Debug)]
pub struct WorldSave;

/// 一个资源包的信息结构（待完善）
#[derive(Debug)]
pub struct ResourcesPack;

// ============================================================================
//  反序列化辅助函数
// ============================================================================

fn string_or_seq<'de, D>(deserializer: D) -> Result<Vec<String>, D::Error>
where
    D: Deserializer<'de>,
{
    struct StringOrVec(PhantomData<Vec<String>>);

    impl<'de> Visitor<'de> for StringOrVec {
        type Value = Vec<String>;

        fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
            formatter.write_str("string or list of strings")
        }

        fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            Ok(vec![value.to_string()])
        }

        fn visit_seq<S>(self, seq: S) -> Result<Self::Value, S::Error>
        where
            S: SeqAccess<'de>,
        {
            Deserialize::deserialize(de::value::SeqAccessDeserializer::new(seq))
        }
    }

    deserializer.deserialize_any(StringOrVec(PhantomData))
}

// ============================================================================
//  测试
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_load_save() {
        let dir = tempdir().unwrap();
        let version = "1.16.5";
        let base = dir.path().to_str().unwrap().to_string();
        let version_dir = Path::new(&base).join(version);
        std::fs::create_dir_all(&version_dir).unwrap();

        // 创建模拟的 version.json
        let meta = VersionMeta {
            main_class: "net.minecraft.client.main.Main".to_string(),
            minecraft_arguments: "--username ${auth_player_name}".to_string(),
            ..Default::default()
        };
        let json = serde_json::to_string_pretty(&meta).unwrap();
        std::fs::write(version_dir.join("1.16.5.json"), json).unwrap();

        // 创建模拟的 SCL 配置
        let scl_config = SCLLaunchConfig {
            game_independent: true,
            ..Default::default()
        };
        let json = serde_json::to_string_pretty(&scl_config).unwrap();
        std::fs::write(version_dir.join(".scl.json"), json).unwrap();

        let mut info = VersionInfo {
            version_base: base,
            version: version.to_string(),
            ..Default::default()
        };

        info.load().await.unwrap();
        assert!(info.meta.is_some());
        assert!(info.scl_launch_config.is_some());
        assert_eq!(info.version_type, VersionType::Vanilla);

        // 测试保存（修改配置）
        if let Some(config) = &mut info.scl_launch_config {
            config.game_independent = false;
        }
        info.save().await.unwrap();

        // 重新加载验证
        let mut info2 = info.clone();
        info2.load().await.unwrap();
        assert_eq!(
            info2.scl_launch_config.unwrap().game_independent,
            false
        );
    }

    #[test]
    fn test_rule_allowed() {
        let rules = vec![ApplyRule {
            action: "allow".to_string(),
            os: Some(OSRule {
                name: "windows".to_string(),
                version: String::new(),
                arch: String::new(),
            }),
            features: None,
        }];
        // 若当前系统为 windows，应该 allow
        if TARGET_OS == "windows" {
            assert!(rules.as_slice().is_allowed());
        } else {
            assert!(!rules.as_slice().is_allowed());
        }

        let rules_disallow = vec![ApplyRule {
            action: "disallow".to_string(),
            os: Some(OSRule {
                name: "windows".to_string(),
                version: String::new(),
                arch: String::new(),
            }),
            features: None,
        }];
        if TARGET_OS == "windows" {
            assert!(!rules_disallow.as_slice().is_allowed());
        } else {
            // 不匹配则不影响
            assert!(rules_disallow.as_slice().is_allowed());
        }
    }
}