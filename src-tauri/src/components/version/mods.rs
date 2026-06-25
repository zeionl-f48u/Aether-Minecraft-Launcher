//! 模组文件管理模块
//!
//! 提供对 Fabric、Forge 模组 JAR 文件的元数据读取、启用/禁用、删除和图标提取功能。

use std::collections::HashMap;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use image::DynamicImage;
use serde::Deserialize;
use thiserror::Error;
use tokio::task::spawn_blocking;

// ============================================================================
//  错误类型
// ============================================================================

/// 模组操作相关错误
#[derive(Debug, Error)]
pub enum ModError {
    #[error("IO 错误: {0}")]
    Io(#[from] std::io::Error),

    #[error("ZIP 解析失败: {0}")]
    Zip(#[from] zip::result::ZipError),

    #[error("JSON 解析失败: {0}")]
    Json(#[from] serde_json::Error),

    #[error("TOML 解析失败: {0}")]
    Toml(#[from] toml::de::Error),

    #[error("图片解码失败: {0}")]
    Image(#[from] image::ImageError),

    #[error("元数据解析失败: {0}")]
    MetadataParse(String),

    #[error("图标未找到: {0}")]
    IconNotFound(String),

    #[error("模组无效: {0}")]
    InvalidMod(String),

    #[error("阻塞任务失败: {0}")]
    JoinError(#[from] tokio::task::JoinError),
}

pub type ModResult<T> = Result<T, ModError>;

// ============================================================================
//  数据结构
// ============================================================================

/// 模组文件数据
#[derive(Debug, Clone)]
pub struct Mod {
    file_name: String,
    path: PathBuf,
    enabled: bool,
    /// 元数据缓存（懒加载）
    meta: std::sync::Arc<Mutex<Option<ModMeta>>>,
}

impl Mod {
    /// 从文件路径创建 `Mod` 实例（不进行 I/O）
    pub fn from_path(path: impl AsRef<Path>) -> Self {
        let path = path.as_ref().to_path_buf();
        let file_name = path.file_name().unwrap().to_string_lossy().to_string();
        let enabled = !file_name.ends_with(".disabled");
        Self {
            file_name,
            path,
            enabled,
            meta: std::sync::Arc::new(Mutex::new(None)),
        }
    }

    /// 模组是否启用
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// 模组文件名
    pub fn file_name(&self) -> &str {
        &self.file_name
    }

    /// 模组文件路径
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// 获取显示名称（优先从元数据获取，否则从文件名推断）
    pub async fn display_name(&self) -> String {
        self.try_get_mod_name()
            .await
            .unwrap_or_else(|_| {
                self.file_name
                    .trim_end_matches(".jar.disabled")
                    .trim_end_matches(".jar")
                    .to_owned()
            })
    }

    // ========================================================================
    //  元数据读取（懒加载 + 缓存）
    // ========================================================================

    /// 获取模组元数据（内部会缓存结果）
    pub async fn meta(&self) -> ModResult<ModMeta> {
        // 先检查缓存
        {
            let lock = self.meta.lock().unwrap();
            if let Some(ref meta) = *lock {
                return Ok(meta.clone());
            }
        }
        // 未命中，执行解析
        let path = self.path.clone();
        let meta = spawn_blocking(move || -> ModResult<ModMeta> {
            parse_mod_metadata(&path)
        }).await??;
        // 存入缓存
        {
            let mut lock = self.meta.lock().unwrap();
            *lock = Some(meta.clone());
        }
        Ok(meta)
    }

    /// 仅获取模组名称（便捷方法）
    pub async fn try_get_mod_name(&self) -> ModResult<String> {
        Ok(self.meta().await?.name().to_owned())
    }

    /// 获取模组图标（从元数据中提取路径并解压图片）
    pub async fn try_get_mod_icon(&self) -> ModResult<DynamicImage> {
        let meta = self.meta().await?;
        let icon_path = match meta {
            ModMeta::Fabric(ref m) => {
                match &m.icon {
                    Some(FabricModIcon::Multiply(map)) => {
                        // 取第一个图标的路径（value）
                        map.values()
                            .next()
                            .ok_or_else(|| ModError::IconNotFound("多重图标为空".into()))?
                            .clone()
                    }
                    Some(FabricModIcon::Single(path)) => path.clone(),
                    None => return Err(ModError::IconNotFound("模组未提供图标".into())),
                }
            }
            ModMeta::Forge(ref m) => m.logo_file.clone(),
        };
        // 解压图标
        let path = self.path.clone();
        let icon_path = icon_path.trim_start_matches(['/', '\\']).to_owned();
        let image = spawn_blocking(move || -> ModResult<DynamicImage> {
            let file = std::fs::File::open(&path)?;
            let mut archive = zip::ZipArchive::new(file)?;
            let mut entry = archive.by_name(&icon_path)?;
            let mut buf = Vec::with_capacity(entry.size() as usize);
            entry.read_to_end(&mut buf)?;
            Ok(image::load_from_memory(&buf)?)
        }).await??;
        Ok(image)
    }

    // ========================================================================
    //  启用/禁用/删除
    // ========================================================================

    /// 启用模组（如果已禁用）
    pub async fn enable(&mut self) -> ModResult<()> {
        if self.file_name.ends_with(".disabled") && !self.enabled {
            let old_path = self.path.clone();
            let mut new_path = self.path.clone();
            let base = self.file_name.trim_end_matches(".disabled");
            new_path.set_file_name(base);
            let new_path_clone = new_path.clone();
            spawn_blocking(move || {
                std::fs::rename(&old_path, &new_path_clone)?;
                Ok::<_, std::io::Error>(())
            }).await??;
            self.path = new_path;
            self.file_name = base.to_owned();
            self.enabled = true;
        }
        Ok(())
    }

    /// 禁用模组（如果已启用）
    pub async fn disable(&mut self) -> ModResult<()> {
        if !self.file_name.ends_with(".disabled") && self.enabled {
            let old_path = self.path.clone();
            let mut new_path = self.path.clone();
            let new_name = format!("{}.disabled", self.file_name);
            new_path.set_file_name(&new_name);
            let new_path_clone = new_path.clone();
            spawn_blocking(move || {
                std::fs::rename(&old_path, &new_path_clone)?;
                Ok::<_, std::io::Error>(())
            }).await??;
            self.path = new_path;
            self.file_name = new_name;
            self.enabled = false;
        }
        Ok(())
    }

    /// 永久删除模组文件（不可恢复）
    pub async fn remove(self) -> ModResult<()> {
        spawn_blocking(move || {
            std::fs::remove_file(&self.path)?;
            Ok::<_, std::io::Error>(())
        }).await??;
        Ok(())
    }
}

// ============================================================================
//  元数据结构定义
// ============================================================================

/// Fabric 模组图标（可为单个或多个）
#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum FabricModIcon {
    Multiply(HashMap<String, String>),
    Single(String),
}

/// Fabric 模组元数据
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct FabricModMeta {
    pub name: String,
    pub description: String,
    pub version: String,
    pub icon: Option<FabricModIcon>,
}

/// Forge 模组元数据（旧版 mcmod.info）
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct ForgeModMeta {
    pub name: String,
    pub description: String,
    pub version: String,
    #[serde(rename = "logoFile")]
    pub logo_file: String,
}

/// 新版 Forge 模组元数据（mods.toml，包含多个模组）
#[derive(Debug, Clone, Deserialize)]
pub struct NewForgeModMeta {
    pub mods: Vec<ForgeModMeta>,
}

/// 统一的模组元数据枚举
#[derive(Debug, Clone)]
pub enum ModMeta {
    Fabric(FabricModMeta),
    Forge(ForgeModMeta),
}

impl ModMeta {
    pub fn name(&self) -> &str {
        match self {
            ModMeta::Fabric(m) => &m.name,
            ModMeta::Forge(m) => &m.name,
        }
    }

    pub fn version(&self) -> &str {
        match self {
            ModMeta::Fabric(m) => &m.version,
            ModMeta::Forge(m) => &m.version,
        }
    }

    pub fn description(&self) -> &str {
        match self {
            ModMeta::Fabric(m) => &m.description,
            ModMeta::Forge(m) => &m.description,
        }
    }

    pub fn authors(&self) -> Vec<String> {
        Vec::new()
    }
}

// ============================================================================
//  内部解析函数（同步，用于 spawn_blocking）
// ============================================================================

/// 解析模组元数据（从 JAR 中读取 fabric.mod.json / mods.toml / mcmod.info）
fn parse_mod_metadata(path: &Path) -> ModResult<ModMeta> {
    let file = std::fs::File::open(path)?;
    let mut archive = zip::ZipArchive::new(file)?;

    // 尝试 Fabric
    if let Ok(entry) = archive.by_name("fabric.mod.json") {
        let meta: FabricModMeta = serde_json::from_reader(entry)?;
        return Ok(ModMeta::Fabric(meta));
    }

    // 尝试新版 Forge (mods.toml)
    if let Ok(mut entry) = archive.by_name("mods.toml") {
        let mut content = String::with_capacity(entry.size() as usize);
        entry.read_to_string(&mut content)?;
        let meta: NewForgeModMeta = toml::from_str(&content)?;
        let first = meta.mods.into_iter().next()
            .ok_or_else(|| ModError::MetadataParse("mods.toml 中没有模组定义".into()))?;
        return Ok(ModMeta::Forge(first));
    }

    // 尝试旧版 Forge (mcmod.info)
    if let Ok(entry) = archive.by_name("mcmod.info") {
        // mcmod.info 可能是数组或对象
        // 先尝试解析为 Vec<ForgeModMeta>
        let meta: Vec<ForgeModMeta> = serde_json::from_reader(entry)?;
        let first = meta.into_iter().next()
            .ok_or_else(|| ModError::MetadataParse("mcmod.info 中没有模组定义".into()))?;
        return Ok(ModMeta::Forge(first));
    }

    Err(ModError::MetadataParse("未找到已知的元数据文件".into()))
}

// ============================================================================
//  测试
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    // 辅助：生成一个测试用 Fabric 模组 JAR（内存中）
    fn create_test_fabric_jar() -> Vec<u8> {
        use zip::write::SimpleFileOptions;
        use zip::ZipWriter;

        let mut buf = std::io::Cursor::new(Vec::new());
        let mut zip = ZipWriter::new(&mut buf);
        let options = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);

        // fabric.mod.json
        let fabric_json = r#"{
            "name": "TestMod",
            "description": "A test mod",
            "version": "1.0.0",
            "icon": "icon.png"
        }"#;
        zip.start_file("fabric.mod.json", options).unwrap();
        zip.write_all(fabric_json.as_bytes()).unwrap();

        // 假图标（仅占位）
        zip.start_file("icon.png", options).unwrap();
        zip.write_all(b"fake image data").unwrap();

        zip.finish().unwrap();
        buf.into_inner()
    }

    #[tokio::test]
    async fn test_parse_fabric_mod() {
        let jar_data = create_test_fabric_jar();
        let temp_dir = tempfile::tempdir().unwrap();
        let jar_path = temp_dir.path().join("testmod.jar");
        std::fs::write(&jar_path, &jar_data).unwrap();

        let mod_obj = Mod::from_path(&jar_path);
        let meta = mod_obj.meta().await.unwrap();
        match meta {
            ModMeta::Fabric(m) => {
                assert_eq!(m.name, "TestMod");
                assert_eq!(m.version, "1.0.0");
            }
            _ => panic!("Expected Fabric mod"),
        }

        // 测试图标
        let icon = mod_obj.try_get_mod_icon().await.unwrap();
        // 因为图标数据是假的，这里应该会失败（但至少走通了流程）
        // 这里不断言，因为图片加载会失败，但我们可以测试错误
        let icon_result = mod_obj.try_get_mod_icon().await;
        assert!(icon_result.is_err()); // 图片无效
    }

    #[tokio::test]
    async fn test_enable_disable() {
        let temp_dir = tempfile::tempdir().unwrap();
        let jar_path = temp_dir.path().join("testmod.jar");
        std::fs::write(&jar_path, b"dummy content").unwrap();

        let mut mod_obj = Mod::from_path(&jar_path);
        assert!(mod_obj.is_enabled());

        // 禁用
        mod_obj.disable().await.unwrap();
        assert!(!mod_obj.is_enabled());
        assert!(mod_obj.path().file_name().unwrap().to_str().unwrap().ends_with(".disabled"));

        // 启用
        mod_obj.enable().await.unwrap();
        assert!(mod_obj.is_enabled());
        assert!(!mod_obj.path().file_name().unwrap().to_str().unwrap().ends_with(".disabled"));
    }
}