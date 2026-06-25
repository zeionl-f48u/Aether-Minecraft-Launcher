//! 跨平台 Minecraft 目录路径管理
//!
//! 提供默认的 .minecraft 目录位置及其子目录（assets、libraries、versions）的获取。
//! Windows 上默认使用当前目录下的 .minecraft，其他系统使用用户主目录下的 .minecraft。

use std::env;
use std::path::{Path, PathBuf};
use thiserror::Error;

// ============================================================================
//  错误类型
// ============================================================================

#[derive(Debug, Error)]
pub enum PathError {
    #[error("无法确定主目录: {0}")]
    HomeDirNotFound(String),
    #[error("IO 错误: {0}")]
    Io(#[from] std::io::Error),
}

pub type PathResult<T> = Result<T, PathError>;

// ============================================================================
//  核心路径获取函数
// ============================================================================

/// 获取 Minecraft 主目录（默认位置）
///
/// - Windows: 当前工作目录下的 `.minecraft`
/// - macOS: `~/Library/Application Support/minecraft`
/// - Linux: `~/.minecraft`
/// - 其他平台: 当前目录下的 `.minecraft`
pub fn default_minecraft_dir() -> PathResult<PathBuf> {
    #[cfg(target_os = "windows")]
    {
        // 按原设计：使用当前目录下的 .minecraft
        Ok(PathBuf::from(".minecraft"))
    }

    #[cfg(target_os = "macos")]
    {
        let home = dirs::home_dir()
            .ok_or_else(|| PathError::HomeDirNotFound("无法获取家目录".into()))?;
        Ok(home.join("Library/Application Support/minecraft"))
    }

    #[cfg(target_os = "linux")]
    {
        let home = dirs::home_dir()
            .ok_or_else(|| PathError::HomeDirNotFound("无法获取家目录".into()))?;
        Ok(home.join(".minecraft"))
    }

    #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
    {
        // 其他平台：使用当前目录下的 .minecraft
        Ok(PathBuf::from(".minecraft"))
    }
}

/// 获取 Minecraft assets 目录的默认路径
pub fn default_assets_dir() -> PathResult<PathBuf> {
    Ok(default_minecraft_dir()?.join("assets"))
}

/// 获取 Minecraft libraries 目录的默认路径
pub fn default_libraries_dir() -> PathResult<PathBuf> {
    Ok(default_minecraft_dir()?.join("libraries"))
}

/// 获取 Minecraft versions 目录的默认路径
pub fn default_versions_dir() -> PathResult<PathBuf> {
    Ok(default_minecraft_dir()?.join("versions"))
}

// ============================================================================
//  可配置的路径结构体
// ============================================================================

/// Minecraft 目录路径管理器，支持自定义根目录
pub struct MinecraftPaths {
    root: PathBuf,
}

impl MinecraftPaths {
    /// 使用默认的 Minecraft 目录
    pub fn new() -> PathResult<Self> {
        Ok(Self {
            root: default_minecraft_dir()?,
        })
    }

    /// 使用自定义根目录
    pub fn with_root(root: impl AsRef<Path>) -> Self {
        Self {
            root: root.as_ref().to_path_buf(),
        }
    }

    /// 获取根目录
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// 获取 assets 目录
    pub fn assets(&self) -> PathBuf {
        self.root.join("assets")
    }

    /// 获取 libraries 目录
    pub fn libraries(&self) -> PathBuf {
        self.root.join("libraries")
    }

    /// 获取 versions 目录
    pub fn versions(&self) -> PathBuf {
        self.root.join("versions")
    }

    /// 确保所有必要的目录存在
    pub fn ensure_dirs(&self) -> PathResult<()> {
        std::fs::create_dir_all(self.assets())?;
        std::fs::create_dir_all(self.libraries())?;
        std::fs::create_dir_all(self.versions())?;
        Ok(())
    }
}

// ============================================================================
//  向后兼容的静态常量（已弃用，建议使用函数式 API）
// ============================================================================

/// 已弃用：请使用 `default_minecraft_dir()` 或 `MinecraftPaths`
pub static MINECRAFT_PATH: once_cell::sync::Lazy<String> =
    once_cell::sync::Lazy::new(|| {
        default_minecraft_dir()
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or_else(|_| ".minecraft".into())
    });

pub static MINECRAFT_ASSETS_PATH: once_cell::sync::Lazy<String> =
    once_cell::sync::Lazy::new(|| {
        default_assets_dir()
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or_else(|_| format!("{}/assets", *MINECRAFT_PATH))
    });

pub static MINECRAFT_LIBRARIES_PATH: once_cell::sync::Lazy<String> =
    once_cell::sync::Lazy::new(|| {
        default_libraries_dir()
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or_else(|_| format!("{}/libraries", *MINECRAFT_PATH))
    });

pub static MINECRAFT_VERSIONS_PATH: once_cell::sync::Lazy<String> =
    once_cell::sync::Lazy::new(|| {
        default_versions_dir()
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or_else(|_| format!("{}/versions", *MINECRAFT_PATH))
    });

// ============================================================================
//  测试
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_minecraft_dir() {
        let dir = default_minecraft_dir().unwrap();
        #[cfg(target_os = "windows")]
        assert_eq!(dir, PathBuf::from(".minecraft"));
        #[cfg(target_os = "macos")]
        assert!(dir.to_str().unwrap().contains("Library/Application Support/minecraft"));
        #[cfg(target_os = "linux")]
        assert!(dir.to_str().unwrap().ends_with(".minecraft"));
    }

    #[test]
    fn test_minecraft_paths() {
        let paths = MinecraftPaths::new().unwrap();
        let assets = paths.assets();
        assert!(assets.to_str().unwrap().ends_with("assets"));
        let libraries = paths.libraries();
        assert!(libraries.to_str().unwrap().ends_with("libraries"));
        let versions = paths.versions();
        assert!(versions.to_str().unwrap().ends_with("versions"));
    }

    #[test]
    fn test_custom_root() {
        let paths = MinecraftPaths::with_root("/custom/path");
        assert_eq!(paths.root(), Path::new("/custom/path"));
        assert_eq!(paths.assets(), Path::new("/custom/path/assets"));
    }

    #[test]
    fn test_ensure_dirs() {
        let temp = tempfile::tempdir().unwrap();
        let paths = MinecraftPaths::with_root(temp.path());
        paths.ensure_dirs().unwrap();
        assert!(paths.assets().exists());
        assert!(paths.libraries().exists());
        assert!(paths.versions().exists());
    }
}