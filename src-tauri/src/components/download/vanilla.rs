//! 原版游戏的下载模块
//!
//! 提供原版游戏客户端、依赖库和资源文件的下载与安装功能。

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde::Deserialize;
use thiserror::Error;
use tokio::fs;
use tokio::sync::Semaphore;
use tokio::task::spawn_blocking;
use url::Url;

use super::{
    structs::{AssetIndexes, VersionManifest},
    DownloadSource, Downloader,
};
use crate::{
    components::version::structs::VersionInfo,
    prelude::*,
    components::progress::{Reporter, ReporterExt},
    components::version::structs::{Allowed, Library, VersionMeta},
};

// ============================================================================
//  错误类型
// ============================================================================

#[derive(Debug, Error)]
pub enum VanillaError {
    #[error("HTTP 请求失败: {0}")]
    Http(String),

    #[error("JSON 解析失败: {0}")]
    Json(#[from] serde_json::Error),

    #[error("IO 错误: {0}")]
    Io(#[from] std::io::Error),

    #[error("URL 解析错误: {0}")]
    Url(#[from] url::ParseError),

    #[error("文件校验失败: {0} (期望: {expected}, 实际: {actual})")]
    ChecksumMismatch {
        path: String,
        expected: String,
        actual: String,
    },

    #[error("下载失败: {0}")]
    DownloadFailed(String),

    #[error("元数据缺失: {0}")]
    MissingMetadata(String),

    #[error("解压失败: {0}")]
    UnzipFailed(String),

    #[error("信号量获取失败: {0}")]
    Semaphore(#[from] tokio::sync::AcquireError),

    #[error("阻塞任务失败: {0}")]
    Join(#[from] tokio::task::JoinError),
}

pub type VanillaResult<T> = Result<T, VanillaError>;

// ============================================================================
//  镜像源管理
// ============================================================================

/// 镜像源配置
#[derive(Debug, Clone)]
pub struct MirrorSource {
    pub name: &'static str,
    pub base_url: Url,
    pub priority: u8,
    pub enabled: bool,
}

impl MirrorSource {
    pub fn bmclapi() -> Self {
        Self {
            name: "BMCLAPI",
            base_url: Url::parse("https://bmclapi2.bangbang93.com").unwrap(),
            priority: 1,
            enabled: true,
        }
    }

    pub fn sjtu() -> Self {
        Self {
            name: "SJTU Mirror",
            base_url: Url::parse("https://mirrors.cernet.edu.cn/bmclapi").unwrap(),
            priority: 2,
            enabled: true,
        }
    }

    pub fn lss233() -> Self {
        Self {
            name: "Lss233 Mirror",
            base_url: Url::parse("https://lss233.com/mirror").unwrap(),
            priority: 3,
            enabled: true,
        }
    }

    pub fn official() -> Self {
        Self {
            name: "Official",
            base_url: Url::parse("https://launcher.mojang.com").unwrap(),
            priority: 4,
            enabled: true,
        }
    }

    pub fn default_sources() -> Vec<Self> {
        vec![
            Self::bmclapi(),
            Self::sjtu(),
            Self::lss233(),
            Self::official(),
        ]
    }
}

/// 镜像源管理器
pub struct MirrorManager {
    sources: Vec<MirrorSource>,
    client: HttpClient,
}

impl MirrorManager {
    pub fn new(sources: Vec<MirrorSource>) -> Self {
        Self {
            sources,
            client: HttpClient::default(),
        }
    }

    pub fn default() -> Self {
        Self::new(MirrorSource::default_sources())
    }

    pub fn available_sources(&self) -> Vec<&MirrorSource> {
        self.sources.iter().filter(|s| s.enabled).collect()
    }

    /// 从镜像源下载文件，自动尝试所有可用源
    pub async fn download_with_fallback(&self, path: &str, dest: &Path) -> VanillaResult<()> {
        let sources = self.available_sources();
        let mut last_err = None;

        for source in sources {
            let url = source
                .base_url
                .join(path)
                .map_err(|e| VanillaError::Url(e))?;
            tracing::debug!("尝试从 {} 下载: {}", source.name, url);

            match self.client.download(&[url.to_string()], dest).await {
                Ok(_) => return Ok(()),
                Err(e) => {
                    tracing::warn!("从 {} 下载失败: {}", source.name, e);
                    last_err = Some(e);
                }
            }
        }

        Err(VanillaError::DownloadFailed(
            last_err
                .map(|e| e.to_string())
                .unwrap_or_else(|| "所有镜像源均失败".into()),
        ))
    }
}

// ============================================================================
//  校验工具
// ============================================================================

/// 校验文件 SHA1
pub async fn verify_file_sha1(path: &Path, expected_sha1: &str) -> VanillaResult<bool> {
    if !path.is_file() {
        return Ok(false);
    }
    let mut file = fs::File::open(path).await?;
    let actual_sha1 = crate::components::utils::get_data_sha1_async(&mut file).await
        .map_err(|e| VanillaError::DownloadFailed(e.to_string()))?;
    Ok(actual_sha1 == expected_sha1)
}

/// 检查文件是否存在且校验通过
pub async fn file_exists_and_valid(path: &Path, sha1: &str) -> VanillaResult<bool> {
    if !path.is_file() {
        return Ok(false);
    }
    verify_file_sha1(path, sha1).await
}

// ============================================================================
//  VanillaDownloadExt 特质
// ============================================================================

/// 原版下载扩展特质
pub trait VanillaDownloadExt: Sync {
    async fn get_available_vanilla_versions(&self) -> VanillaResult<VersionManifest>;

    async fn download_vanilla_jar(&self, path: &str, save_path: &Path, sha1: &str) -> VanillaResult<()>;

    async fn download_library_file(
        &self,
        sha1: &str,
        maven_path: &str,
        save_path: &Path,
    ) -> VanillaResult<()>;

    async fn download_libraries(
        &self,
        libraries: &[Library],
    ) -> VanillaResult<HashMap<String, Vec<String>>>;

    async fn download_asset_index(
        &self,
        name: &str,
        url: &str,
        save_path: &Path,
    ) -> VanillaResult<AssetIndexes>;

    async fn download_single_asset(
        &self,
        sha1: &str,
        name: &str,
        save_path: &Path,
        is_pre: bool,
    ) -> VanillaResult<()>;

    async fn download_vanilla(
        &self,
        version_name: &str,
        version_meta: &VersionMeta,
        is_repair: bool,
    ) -> VanillaResult<()>;

    async fn install_vanilla(&self, version_name: &str, version_info: &VersionInfo) -> VanillaResult<()>;
}

// ============================================================================
//  VanillaDownloadExt 实现
// ============================================================================

impl<R: Reporter + ReporterExt> VanillaDownloadExt for Downloader<R> {
    async fn get_available_vanilla_versions(&self) -> VanillaResult<VersionManifest> {
        let url = match self.source {
            DownloadSource::Default => "https://piston-meta.mojang.com/mc/game/version_manifest.json",
            DownloadSource::BMCLAPI => "https://bmclapi2.bangbang93.com/mc/game/version_manifest.json",
            DownloadSource::MCBBS => "https://download.mcbbs.net/mc/game/version_manifest.json",
            _ => "https://piston-meta.mojang.com/mc/game/version_manifest.json",
        };
        let client = HttpClient::default();
        let manifest: VersionManifest = client
            .get_json(url)
            .await
            .map_err(|e| VanillaError::Http(e.to_string()))?;
        Ok(manifest)
    }

    async fn download_vanilla_jar(&self, path: &str, save_path: &Path, sha1: &str) -> VanillaResult<()> {
        let reporter = self.reporter.fork();
        reporter.set_max_progress(1.0);
        let name = save_path.file_name().unwrap_or_default().to_string_lossy();
        reporter.set_message(format!("正在下载原版 {}", name));

        // 检查是否已存在且有效
        if file_exists_and_valid(save_path, sha1).await? {
            reporter.set_progress(1.0);
            return Ok(());
        }

        // 确保目录存在
        if let Some(parent) = save_path.parent() {
            fs::create_dir_all(parent).await?;
        }

        let temp_path = save_path.with_extension("tmp");
        let mirror = MirrorManager::default();
        mirror.download_with_fallback(path, &temp_path).await?;

        // 校验
        if !verify_file_sha1(&temp_path, sha1).await? {
            let _ = fs::remove_file(&temp_path).await;
            return Err(VanillaError::ChecksumMismatch {
                path: save_path.to_string_lossy().into_owned(),
                expected: sha1.to_string(),
                actual: "校验失败".into(),
            });
        }

        fs::rename(&temp_path, save_path).await?;
        reporter.set_progress(1.0);
        Ok(())
    }

    async fn download_library_file(
        &self,
        sha1: &str,
        maven_path: &str,
        save_path: &Path,
    ) -> VanillaResult<()> {
        let reporter = self.reporter.fork();
        reporter.set_max_progress(1.0);
        reporter.set_message(format!("正在下载库文件 {}", maven_path));

        if file_exists_and_valid(save_path, sha1).await? {
            reporter.set_progress(1.0);
            return Ok(());
        }

        if let Some(parent) = save_path.parent() {
            fs::create_dir_all(parent).await?;
        }

        let temp_path = save_path.with_extension("tmp");
        let mirror = MirrorManager::default();
        mirror.download_with_fallback(maven_path, &temp_path).await?;

        if !verify_file_sha1(&temp_path, sha1).await? {
            let _ = fs::remove_file(&temp_path).await;
            return Err(VanillaError::ChecksumMismatch {
                path: save_path.to_string_lossy().into_owned(),
                expected: sha1.to_string(),
                actual: "校验失败".into(),
            });
        }

        fs::rename(&temp_path, save_path).await?;
        reporter.set_progress(1.0);
        Ok(())
    }

    async fn download_libraries(
        &self,
        libraries: &[Library],
    ) -> VanillaResult<HashMap<String, Vec<String>>> {
        let reporter = self.reporter.fork();
        reporter.set_message("正在检索并下载依赖库");

        let mut tasks = Vec::new();
        let mut native_jars_mapping: HashMap<String, Vec<String>> = HashMap::new();

        for lib in libraries {
            if let Some(downloads) = &lib.downloads {
                // 处理原生库 (classifiers)
                if let Some(classifiers) = &downloads.classifiers {
                    for (platform, meta) in classifiers {
                        let target_platform = if platform == "natives-osx" {
                            "natives-macos"
                        } else {
                            platform.as_str()
                        };
                        tasks.push(DownloadTask::Native {
                            platform: target_platform.to_string(),
                            sha1: meta.sha1.clone(),
                            path: meta.path.clone(),
                        });
                    }
                }
                // 处理普通库和带 natives 后缀的库
                if let Some(artifact) = &downloads.artifact {
                    if let Some(idx) = lib.name.find(":natives-") {
                        let (_, platform) = lib.name.split_at(idx + 1);
                        tasks.push(DownloadTask::Native {
                            platform: platform.to_string(),
                            sha1: artifact.sha1.clone(),
                            path: artifact.path.clone(),
                        });
                    } else if lib.rules.is_allowed() {
                        tasks.push(DownloadTask::Common {
                            sha1: artifact.sha1.clone(),
                            path: artifact.path.clone(),
                        });
                    }
                }
            }
        }

        // 收集原生库路径
        for task in &tasks {
            if let DownloadTask::Native { platform, path, .. } = task {
                let full_path = self.libraries_dir().join(path);
                native_jars_mapping
                    .entry(platform.clone())
                    .or_insert_with(Vec::new)
                    .push(full_path.to_string_lossy().into_owned());
            }
        }

        // 并发下载（使用信号量限制并发数）
        let semaphore = self.semaphore().clone();
        let libraries_dir = self.libraries_dir().to_path_buf();

        let download_futures = tasks.into_iter().map(|task| {
            let sem = semaphore.clone();
            let libraries_dir = libraries_dir.clone();
            let downloader = &self;
            async move {
                let _permit = sem.acquire().await?;
                match task {
                    DownloadTask::Common { sha1, path } => {
                        let save_path = libraries_dir.join(&path);
                        downloader.download_library_file(&sha1, &path, &save_path).await
                    }
                    DownloadTask::Native { sha1, path, .. } => {
                        let save_path = libraries_dir.join(&path);
                        downloader.download_library_file(&sha1, &path, &save_path).await
                    }
                }
            }
        });

        use futures::stream::{self, StreamExt};
        let results: Vec<VanillaResult<()>> = stream::iter(download_futures)
            .buffer_unordered(16)
            .collect()
            .await;

        for result in results {
            result?;
        }

        reporter.remove_progress();
        Ok(native_jars_mapping)
    }

    async fn download_asset_index(
        &self,
        name: &str,
        url: &str,
        save_path: &Path,
    ) -> VanillaResult<AssetIndexes> {
        let reporter = self.reporter.fork();
        reporter.set_message(format!("正在下载资源索引 {}", name));

        let index_path = save_path.join("indexes").join(format!("{}.json", name));
        if let Some(parent) = index_path.parent() {
            fs::create_dir_all(parent).await?;
        }

        // 解析 URL 获取路径
        let url_obj = Url::parse(url)?;
        let path = url_obj.path();

        let mirror = MirrorManager::default();
        let temp_path = index_path.with_extension("tmp");
        mirror.download_with_fallback(path, &temp_path).await?;

        // 读取并解析
        let content = fs::read(&temp_path).await?;
        let indexes: AssetIndexes = serde_json::from_slice(&content)?;
        fs::rename(&temp_path, &index_path).await?;

        reporter.set_progress(1.0);
        Ok(indexes)
    }

    async fn download_single_asset(
        &self,
        sha1: &str,
        name: &str,
        save_path: &Path,
        is_pre: bool,
    ) -> VanillaResult<()> {
        let sub_hash = &sha1[..2];
        let full_path = if is_pre {
            save_path
                .parent()
                .unwrap()
                .join("virtual")
                .join("pre-1.6")
                .join(name)
        } else {
            save_path.join(sub_hash).join(sha1)
        };

        // 检查是否已存在且有效
        if file_exists_and_valid(&full_path, sha1).await? {
            return Ok(());
        }

        if let Some(parent) = full_path.parent() {
            fs::create_dir_all(parent).await?;
        }

        let download_path = format!("assets/{}/{}", sub_hash, sha1);
        let temp_path = full_path.with_extension("tmp");
        let mirror = MirrorManager::default();
        mirror.download_with_fallback(&download_path, &temp_path).await?;

        if !verify_file_sha1(&temp_path, sha1).await? {
            let _ = fs::remove_file(&temp_path).await;
            return Err(VanillaError::ChecksumMismatch {
                path: full_path.to_string_lossy().into_owned(),
                expected: sha1.to_string(),
                actual: "校验失败".into(),
            });
        }

        fs::rename(&temp_path, &full_path).await?;
        Ok(())
    }

    async fn download_vanilla(
        &self,
        version_name: &str,
        version_meta: &VersionMeta,
        is_repair: bool,
    ) -> VanillaResult<()> {
        tracing::info!("开始下载原版游戏 {}", version_name);
        let reporter = self.reporter.fork();
        reporter.set_message(format!("正在下载原版游戏 {}", version_name));

        // 下载客户端 JAR
        let game_file = self.versions_dir().join(version_name).join(format!("{}.jar", version_name));
        if let Some(downloads) = &version_meta.downloads {
            if let Some(client) = downloads.get("client") {
                self.download_vanilla_jar(&client.url, &game_file, &client.sha1)
                    .await?;
            } else {
                return Err(VanillaError::MissingMetadata("缺少 client 下载信息".into()));
            }
        } else {
            return Err(VanillaError::MissingMetadata("缺少 downloads 字段".into()));
        }

        // 下载库文件
        let native_jars = self.download_libraries(&version_meta.libraries).await?;

        // 处理 log4j 补丁（修复 CVE-2021-44228）
        if is_repair {
            let log4j_path = self.libraries_dir()
                .join("org/glavo/1.0/log4j-patch/log4j-patch-agent-1.0.jar");
            if let Some(parent) = log4j_path.parent() {
                fs::create_dir_all(parent).await?;
            }
            fs::write(&log4j_path, crate::components::client::LOG4J_PATCH).await?;
        }

        // 下载资源文件
        let asset_index = version_meta
            .asset_index
            .as_ref()
            .ok_or_else(|| VanillaError::MissingMetadata("缺少 assetIndex".into()))?;
        let is_pre = asset_index.id == "pre-1.6";
        let assets_path = self.assets_dir();

        let indexes = self
            .download_asset_index(&asset_index.id, &asset_index.url, &assets_path)
            .await?;

        // 下载资源文件（使用 HashSet 去重，并发下载）
        let mut processed_hashes = HashSet::new();
        let mut download_tasks = Vec::new();

        for (path, obj) in &indexes.objects {
            if processed_hashes.contains(&obj.hash) {
                continue;
            }
            processed_hashes.insert(obj.hash.clone());

            let full_path = if is_pre {
                assets_path
                    .join("virtual")
                    .join("pre-1.6")
                    .join(path)
            } else {
                let sub_hash = &obj.hash[..2];
                assets_path.join("objects").join(sub_hash).join(&obj.hash)
            };

            // 检查文件是否已存在且有效
            if !file_exists_and_valid(&full_path, &obj.hash).await? {
                download_tasks.push((obj.hash.clone(), path.clone(), full_path));
            }
        }

        let total = download_tasks.len();
        reporter.set_max_progress(total as f64);
        reporter.set_message("下载资源文件");

        use futures::stream::{self, StreamExt};
        let semaphore = self.semaphore().clone();

        let results: Vec<VanillaResult<()>> = stream::iter(download_tasks)
            .map(|(sha1, name, full_path)| {
                let downloader = &self;
                let sem = semaphore.clone();
                async move {
                    let _permit = sem.acquire().await?;
                    downloader.download_single_asset(&sha1, &name, &full_path, is_pre).await
                }
            })
            .buffer_unordered(32)
            .collect()
            .await;

        for result in results {
            result?;
        }

        // 解压原生库
        let native_dir = self.versions_dir().join(version_name).join("natives");
        let total_natives: usize = native_jars.values().map(|v| v.len()).sum();
        reporter.set_message("正在解压原生库");
        reporter.set_max_progress(total_natives as f64);

        for (platform, jars) in &native_jars {
            let platform_dir = native_dir.join(platform);
            for jar_path in jars {
                unzip_natives(Path::new(jar_path), &platform_dir).await?;
                reporter.add_progress(1.0);
            }
        }

        reporter.remove_progress();
        tracing::info!("原版游戏 {} 下载完成", version_name);
        Ok(())
    }

    async fn install_vanilla(&self, version_name: &str, version_info: &VersionInfo) -> VanillaResult<()> {
        let reporter = self.reporter.fork();
        reporter.set_max_progress(4.0);
        reporter.set_message(format!("正在安装原版 {}", version_name));

        // 创建必要的目录
        let assets_path = self.assets_dir();
        let libraries_path = self.libraries_dir();
        let versions_path = self.versions_dir();
        fs::create_dir_all(assets_path.join("indexes")).await?;
        fs::create_dir_all(assets_path.join("objects")).await?;
        fs::create_dir_all(&libraries_path).await?;
        fs::create_dir_all(versions_path.join(version_name)).await?;

        // 下载版本元数据 JSON
        let version_file = versions_path.join(version_name).join(format!("{}.json", version_name));
        // 从版本清单中查找版本 URL
        let manifest = self.get_available_vanilla_versions().await?;
        let version_entry = manifest.versions.iter()
            .find(|v| v.id == version_name)
            .ok_or_else(|| VanillaError::MissingMetadata(format!("版本 {} 未在清单中找到", version_name)))?;
        let path = version_entry.url.path();

        let mirror = MirrorManager::default();
        let temp_path = version_file.with_extension("tmp");
        mirror.download_with_fallback(path, &temp_path).await?;
        fs::rename(&temp_path, &version_file).await?;

        // 读取并解析元数据
        let content = fs::read(&version_file).await?;
        let mut version_meta: VersionMeta = serde_json::from_slice(&content)?;
        version_meta.fix_libraries();

        // 下载游戏文件
        self.download_vanilla(version_name, &version_meta, false).await?;

        reporter.set_progress(4.0);
        Ok(())
    }
}

// ============================================================================
//  辅助类型
// ============================================================================

enum DownloadTask {
    Common { sha1: String, path: String },
    Native { platform: String, sha1: String, path: String },
}

// ============================================================================
//  原生库解压
// ============================================================================

const NATIVE_EXTS: &[&str] = &["dll", "so", "dylib", "jnilib", "pdb"];

/// 解压原生库到指定目录（使用 spawn_blocking 避免阻塞）
pub async fn unzip_natives(unzip_file: &Path, unzip_dir: &Path) -> VanillaResult<()> {
    let unzip_file = unzip_file.to_path_buf();
    let unzip_dir = unzip_dir.to_path_buf();

    spawn_blocking(move || {
        let file = std::fs::File::open(&unzip_file)
            .map_err(|e| VanillaError::UnzipFailed(format!("打开文件失败: {}", e)))?;
        let mut archive = zip::ZipArchive::new(file)
            .map_err(|e| VanillaError::UnzipFailed(format!("解析 ZIP 失败: {}", e)))?;

        std::fs::create_dir_all(&unzip_dir)
            .map_err(|e| VanillaError::UnzipFailed(format!("创建目录失败: {}", e)))?;

        for i in 0..archive.len() {
            let mut file = archive.by_index(i)
                .map_err(|e| VanillaError::UnzipFailed(format!("读取条目失败: {}", e)))?;

            let file_name = match file.enclosed_name().and_then(|p| p.file_name().map(|n| n.to_owned())) {
                Some(name) => name,
                None => continue,
            };

            let ext = Path::new(&file_name).extension()
                .and_then(|e| e.to_str())
                .unwrap_or_default();
            if !NATIVE_EXTS.contains(&ext) {
                continue;
            }

            let save_path = unzip_dir.join(&file_name);
            if let Some(parent) = save_path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }

            let mut output = std::fs::File::create(&save_path)
                .map_err(|e| VanillaError::UnzipFailed(format!("创建文件失败: {}", e)))?;
            std::io::copy(&mut file, &mut output)
                .map_err(|e| VanillaError::UnzipFailed(format!("写入文件失败: {}", e)))?;
        }
        Ok(())
    })
    .await
    .map_err(|e| VanillaError::UnzipFailed(format!("解压任务失败: {}", e)))?
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
    async fn test_download_vanilla_jar() {
        let dir = tempdir().unwrap();
        let mc_path = dir.path().to_str().unwrap();
        let downloader = Downloader::new(mc_path, DownloadSource::default(), NoopReporter);
        // 使用一个小文件测试
        let dest = dir.path().join("test.jar");
        downloader
            .download_vanilla_jar(
                "https://launcher.mojang.com/v1/objects/...", // 实际测试应使用有效URL
                &dest,
                "dummy-sha1",
            )
            .await
            .unwrap_or(());
    }
}