//! Forge 模组加载器的下载与安装
//!
//! 支持旧版（覆盖 JAR）和新版（安装器）两种安装方式。

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::time::{Duration, Instant};

use futures_util::StreamExt;
use serde_json::Value;
use thiserror::Error;
use tokio::fs;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::Command;
use tokio::task::spawn_blocking;

use super::{structs::ForgeVersionsData, Downloader};
use crate::download::structs::{ForgeItemInfo, ForgePromoItem};
use crate::http::HttpClient;
use crate::prelude::*;
use crate::semver::MinecraftVersion;

// ============================================================================
//  常量
// ============================================================================

const FORGE_INSTALL_HELPER: &[u8] = include_bytes!("../../assets/forge-install-bootstrapper.jar");

#[cfg(target_os = "windows")]
const CLASS_PATH_SEPARATOR: &str = ";";
#[cfg(not(target_os = "windows"))]
const CLASS_PATH_SEPARATOR: &str = ":";

// ============================================================================
//  错误类型
// ============================================================================

#[derive(Debug, Error)]
pub enum ForgeError {
    #[error("HTTP 请求失败: {0}")]
    Http(String),

    #[error("JSON 解析失败: {0}")]
    Json(#[from] serde_json::Error),

    #[error("IO 错误: {0}")]
    Io(#[from] std::io::Error),

    #[error("无效的版本格式: {0}")]
    InvalidVersion(String),

    #[error("下载失败: {0}")]
    DownloadFailed(String),

    #[error("安装器执行失败: {0}")]
    InstallerFailed(String),

    #[error("安装器输出解析失败: {0}")]
    OutputParse(String),

    #[error("修改安装器失败: {0}")]
    ModifierFailed(String),

    #[error("版本不兼容: {0}")]
    Incompatible(String),
}

pub type ForgeResult<T> = Result<T, ForgeError>;

// ============================================================================
//  镜像 URL 辅助
// ============================================================================

/// 获取 Forge 下载镜像列表（优先使用配置的源）
fn get_forge_download_urls(
    source: DownloadSource,
    vanilla_version: &str,
    forge_version: &str,
    suffix: &str,
) -> Vec<String> {
    let base_urls = match source {
        DownloadSource::BMCLAPI => vec![
            format!("https://bmclapi2.bangbang93.com/maven/net/minecraftforge/forge/{vanilla_version}-{forge_version}/forge-{vanilla_version}-{forge_version}-{suffix}"),
            format!("https://download.mcbbs.net/maven/net/minecraftforge/forge/{vanilla_version}-{forge_version}/forge-{vanilla_version}-{forge_version}-{suffix}"),
            format!("https://maven.minecraftforge.net/net/minecraftforge/forge/{vanilla_version}-{forge_version}/forge-{vanilla_version}-{forge_version}-{suffix}"),
        ],
        DownloadSource::MCBBS => vec![
            format!("https://download.mcbbs.net/maven/net/minecraftforge/forge/{vanilla_version}-{forge_version}/forge-{vanilla_version}-{forge_version}-{suffix}"),
            format!("https://bmclapi2.bangbang93.com/maven/net/minecraftforge/forge/{vanilla_version}-{forge_version}/forge-{vanilla_version}-{forge_version}-{suffix}"),
            format!("https://maven.minecraftforge.net/net/minecraftforge/forge/{vanilla_version}-{forge_version}/forge-{vanilla_version}-{forge_version}-{suffix}"),
        ],
        _ => vec![
            format!("https://maven.minecraftforge.net/net/minecraftforge/forge/{vanilla_version}-{forge_version}/forge-{vanilla_version}-{forge_version}-{suffix}"),
            format!("https://bmclapi2.bangbang93.com/maven/net/minecraftforge/forge/{vanilla_version}-{forge_version}/forge-{vanilla_version}-{forge_version}-{suffix}"),
            format!("https://download.mcbbs.net/maven/net/minecraftforge/forge/{vanilla_version}-{forge_version}/forge-{vanilla_version}-{forge_version}-{suffix}"),
        ],
    };
    // 去重
    let mut unique = Vec::new();
    for url in base_urls {
        if !unique.contains(&url) {
            unique.push(url);
        }
    }
    unique
}

/// 获取 Forge 安装器下载 URL 列表（特殊处理旧版本）
fn get_installer_download_urls(
    source: DownloadSource,
    vanilla_version: &str,
    forge_version: &str,
) -> Vec<String> {
    let build_id = forge_version
        .rsplit('.')
        .next()
        .unwrap_or(forge_version);
    // 判断是否为三段版本号（如 1.16.5-36.2.34 有三段）
    let parts: Vec<&str> = forge_version.split('.').collect();
    if parts.len() == 3 {
        get_forge_download_urls(source, vanilla_version, forge_version, "installer.jar")
    } else {
        // 旧版本使用 BMCLAPI 特殊下载路径
        let base = match source {
            DownloadSource::BMCLAPI => format!("https://bmclapi2.bangbang93.com/forge/download/{}", build_id),
            DownloadSource::MCBBS => format!("https://download.mcbbs.net/forge/download/{}", build_id),
            _ => format!("https://maven.minecraftforge.net/net/minecraftforge/forge/{vanilla_version}-{forge_version}/forge-{vanilla_version}-{forge_version}-installer.jar"),
        };
        vec![
            base,
            format!("https://bmclapi2.bangbang93.com/forge/download/{}", build_id),
            format!("https://download.mcbbs.net/forge/download/{}", build_id),
            format!("https://maven.minecraftforge.net/net/minecraftforge/forge/{vanilla_version}-{forge_version}/forge-{vanilla_version}-{forge_version}-installer.jar"),
        ]
    }
}

// ============================================================================
//  Forge 下载特质
// ============================================================================

/// Forge 模组加载器的安装特质
pub trait ForgeDownloadExt: Sync {
    /// 获取指定原版版本可用的 Forge 版本列表
    async fn get_available_installers(
        &self,
        vanilla_version: &str,
    ) -> ForgeResult<ForgeVersionsData>;

    /// 下载 Forge 安装所需的文件（前置步骤）
    async fn install_forge_pre(
        &self,
        version_id: &str,
        vanilla_version: &str,
        forge_version: &str,
    ) -> ForgeResult<()>;

    /// 执行 Forge 安装（后置步骤）
    async fn install_forge_post(
        &self,
        version_name: &str,
        version_id: &str,
        forge_version: &str,
    ) -> ForgeResult<()>;

    /// 修改 Forge 安装器中的元数据（镜像源、版本名等）
    async fn modify_forge_installer(
        &self,
        from_reader: std::fs::File,
        to_writer: std::fs::File,
        name: &str,
    ) -> ForgeResult<()>;
}

// ============================================================================
//  为 Downloader 实现
// ============================================================================

impl<R: Reporter> ForgeDownloadExt for Downloader<R> {
    async fn get_available_installers(
        &self,
        vanilla_version: &str,
    ) -> ForgeResult<ForgeVersionsData> {
        let client = HttpClient::default();

        // 获取版本列表和 promos
        let version_url = format!(
            "https://bmclapi2.bangbang93.com/forge/minecraft/{}",
            vanilla_version
        );
        let promo_url = "https://bmclapi2.bangbang93.com/forge/promos";

        let (versions, promos) = tokio::try_join!(
            client.get_json::<Vec<ForgeItemInfo>>(&version_url),
            client.get_json::<Vec<ForgePromoItem>>(&promo_url)
        )
        .map_err(|e| ForgeError::Http(e.to_string()))?;

        // 排序版本（使用语义版本比较）
        let mut sorted = versions;
        sorted.sort_by(|a, b| {
            let a_ver = a.version.parse::<MinecraftVersion>().unwrap_or_default();
            let b_ver = b.version.parse::<MinecraftVersion>().unwrap_or_default();
            a_ver.cmp(&b_ver)
        });
        sorted.reverse(); // 最新在前

        let recommended = promos
            .iter()
            .find(|p| p.name == format!("{}-recommended", vanilla_version))
            .and_then(|p| p.build.clone());
        let latest = promos
            .iter()
            .find(|p| p.name == format!("{}-latest", vanilla_version))
            .and_then(|p| p.build.clone());

        Ok(ForgeVersionsData {
            recommended,
            latest,
            all_versions: sorted,
        })
    }

    async fn install_forge_pre(
        &self,
        version_id: &str,
        vanilla_version: &str,
        forge_version: &str,
    ) -> ForgeResult<()> {
        let mc_version = vanilla_version
            .parse::<MinecraftVersion>()
            .map_err(|_| ForgeError::InvalidVersion(vanilla_version.to_string()))?;

        let reporter = self.reporter.fork();
        reporter.add_max_progress(1.0);

        if mc_version.should_forge_use_override_installiation() {
            // 旧版覆盖安装
            let suffix = if mc_version.should_forge_use_client_or_universal() {
                "client"
            } else {
                "universal"
            };
            let urls = get_forge_download_urls(
                self.source,
                vanilla_version,
                forge_version,
                suffix,
            );
            let dest_path = self.library_path()
                .join("net/minecraftforge/forge")
                .join(format!("{}-{}", vanilla_version, forge_version))
                .join(format!("forge-{}-{}-{}.zip", vanilla_version, forge_version, suffix));
            self.download_with_progress(&urls, &dest_path, &reporter).await?;
        } else {
            // 新版本下载安装器
            let urls = get_installer_download_urls(
                self.source,
                vanilla_version,
                forge_version,
            );
            let dest_path = self.library_path()
                .join("net/minecraftforge/forge")
                .join(format!("{}-{}", vanilla_version, forge_version))
                .join(format!("forge-{}-{}-installer.jar", vanilla_version, forge_version));
            self.download_with_progress(&urls, &dest_path, &reporter).await?;
        }

        reporter.set_progress(1.0);
        Ok(())
    }

    async fn install_forge_post(
        &self,
        version_name: &str,
        version_id: &str,
        forge_version: &str,
    ) -> ForgeResult<()> {
        let reporter = self.reporter.fork();

        let mc_version = version_id
            .parse::<MinecraftVersion>()
            .map_err(|_| ForgeError::InvalidVersion(version_id.to_string()))?;

        if mc_version.should_forge_use_override_installiation() {
            // 旧版覆盖 JAR 方式
            self.install_forge_legacy(version_name, version_id, forge_version, &reporter)
                .await
        } else {
            // 新版安装器方式
            self.install_forge_modern(version_name, version_id, forge_version, &reporter)
                .await
        }
    }

    async fn modify_forge_installer(
        &self,
        from_reader: std::fs::File,
        to_writer: std::fs::File,
        name: &str,
    ) -> ForgeResult<()> {
        // 此操作耗时且同步，使用 spawn_blocking
        let source = self.source;
        let result = spawn_blocking(move || {
            modify_forge_installer_sync(from_reader, to_writer, name, source)
        })
        .await
        .map_err(|e| ForgeError::ModifierFailed(e.to_string()))?;
        result
    }
}

// ============================================================================
//  辅助实现（下载器内部）
// ============================================================================

impl<R: Reporter> Downloader<R> {
    /// 获取库文件路径（Maven 风格）
    fn library_path(&self) -> PathBuf {
        PathBuf::from(&self.minecraft_library_path)
    }

    /// 获取版本目录路径
    fn version_path(&self, version_name: &str) -> PathBuf {
        PathBuf::from(&self.minecraft_version_path).join(version_name)
    }

    /// 带进度的下载（使用镜像列表）
    async fn download_with_progress(
        &self,
        urls: &[String],
        dest: &Path,
        reporter: &impl Reporter,
    ) -> ForgeResult<()> {
        // 创建父目录
        if let Some(parent) = dest.parent() {
            fs::create_dir_all(parent).await?;
        }

        // 如果文件已存在，跳过
        if dest.is_file() {
            return Ok(());
        }

        let temp_path = dest.with_extension("tmp");
        let client = HttpClient::default();

        for (idx, url) in urls.iter().enumerate() {
            reporter.set_message(format!("尝试下载 {} ({}/{})", url, idx + 1, urls.len()));
            if let Err(e) = client.download(&[url.clone()], &temp_path).await {
                let _ = fs::remove_file(&temp_path).await;
                reporter.set_message(format!("下载失败: {}", e));
                continue;
            }
            // 原子重命名
            fs::rename(&temp_path, dest).await?;
            return Ok(());
        }
        Err(ForgeError::DownloadFailed("所有镜像下载失败".into()))
    }

    /// 旧版 Forge 覆盖安装
    async fn install_forge_legacy(
        &self,
        version_name: &str,
        version_id: &str,
        forge_version: &str,
        reporter: &impl Reporter,
    ) -> ForgeResult<()> {
        let mc_version = version_id
            .parse::<MinecraftVersion>()
            .map_err(|_| ForgeError::InvalidVersion(version_id.to_string()))?;
        let suffix = if mc_version.should_forge_use_client_or_universal() {
            "client"
        } else {
            "universal"
        };

        let source_zip = self.library_path()
            .join("net/minecraftforge/forge")
            .join(format!("{}-{}", version_id, forge_version))
            .join(format!("forge-{}-{}-{}.zip", version_id, forge_version, suffix));

        let version_dir = self.version_path(version_name);
        let target_jar = version_dir.join(format!("{}.jar", version_name));
        let temp_jar = version_dir.join(format!("{}.tmp.jar", version_name));

        // 确保目标目录存在
        fs::create_dir_all(&version_dir).await?;

        reporter.set_max_progress(2.0);
        reporter.set_message("正在合并覆盖包...".into());

        // 同步执行 JAR 合并（使用 spawn_blocking）
        let source_zip = source_zip.to_path_buf();
        let target_jar = target_jar.to_path_buf();
        let temp_jar = temp_jar.to_path_buf();

        spawn_blocking(move || {
            merge_jar_overlay(&source_zip, &target_jar, &temp_jar)
        })
        .await
        .map_err(|e| ForgeError::ModifierFailed(e.to_string()))?;

        reporter.add_progress(1.0);
        reporter.set_message("覆盖安装完成".into());
        Ok(())
    }

    /// 新版 Forge 安装器安装
    async fn install_forge_modern(
        &self,
        version_name: &str,
        version_id: &str,
        forge_version: &str,
        reporter: &impl Reporter,
    ) -> ForgeResult<()> {
        // 1. 准备安装辅助 JAR
        let helper_path = self.library_path()
            .join("com/bangbang93/forge-install-bootstrapper/0.0.0/forge-install-bootstrapper.jar");
        fs::create_dir_all(helper_path.parent().unwrap()).await?;
        fs::write(&helper_path, FORGE_INSTALL_HELPER).await?;

        // 2. 准备修改后的安装器
        let installer_source = self.library_path()
            .join("net/minecraftforge/forge")
            .join(format!("{}-{}", version_id, forge_version))
            .join(format!("forge-{}-{}-installer.jar", version_id, forge_version));

        let temp_dir = self.library_path().join("temp");
        fs::create_dir_all(&temp_dir).await?;
        let temp_installer = temp_dir.join(format!("forge-installer-{}.tmp.jar", std::time::SystemTime::now().elapsed().unwrap_or_default().as_secs()));

        // 3. 修改安装器（同步）
        reporter.set_message("正在修改安装器...".into());
        let source = self.source;
        let from_file = fs::File::open(&installer_source).await?;
        let to_file = fs::File::create(&temp_installer).await?;
        // 使用 spawn_blocking 执行修改
        spawn_blocking(move || {
            modify_forge_installer_sync(from_file, to_file, version_name, source)
        })
        .await
        .map_err(|e| ForgeError::ModifierFailed(e.to_string()))?;

        // 4. 运行安装器
        reporter.set_max_progress(2.0);
        reporter.set_message("正在运行 Forge 安装器...".into());
        self.run_forge_installer(version_name, version_id, forge_version, &helper_path, &temp_installer, reporter).await?;

        // 5. 清理临时文件
        let _ = fs::remove_file(&temp_installer).await;

        reporter.add_progress(1.0);
        reporter.set_message("Forge 安装完成".into());
        Ok(())
    }

    /// 运行 Forge 安装器并解析输出
    async fn run_forge_installer(
        &self,
        version_name: &str,
        version_id: &str,
        forge_version: &str,
        helper_path: &Path,
        installer_path: &Path,
        reporter: &impl Reporter,
    ) -> ForgeResult<()> {
        let java_path = &self.java_path;
        let mc_path = &self.minecraft_path;

        let mut cmd = Command::new(java_path);
        #[cfg(windows)]
        {
            use tokio::process::windows::CommandExt;
            cmd.creation_flags(0x08000000);
        }
        cmd.arg("-cp");
        cmd.arg(format!(
            "{}{}{}",
            helper_path.display(),
            CLASS_PATH_SEPARATOR,
            installer_path.display()
        ));
        cmd.arg("com.bangbang93.ForgeInstaller");
        cmd.arg(mc_path);
        cmd.stdout(std::process::Stdio::piped());
        cmd.stderr(std::process::Stdio::inherit());
        cmd.stdin(std::process::Stdio::null());

        let mut child = cmd.spawn()?;
        let stdout = child.stdout.take().unwrap();
        let mut reader = BufReader::new(stdout);
        let mut line = String::new();

        let install_succeed = AtomicBool::new(false);
        let mut last_progress_update = Instant::now();

        while let Ok(len) = reader.read_line(&mut line).await {
            if len == 0 {
                break;
            }
            let line = line.trim();
            tracing::trace!("[ForgeInstaller] {}", line);

            // 解析输出并更新进度
            if line.starts_with("Patching ") {
                if last_progress_update.elapsed() > Duration::from_millis(100) {
                    reporter.set_message(format!("修补类: {}", &line[8..]));
                    last_progress_update = Instant::now();
                }
            } else if line.starts_with("Downloading library from ") {
                reporter.set_message(format!("下载依赖: {}", &line[23..]));
            } else if line.starts_with("Following redirect: ") {
                reporter.set_message(format!("下载重定向: {}", &line[20..]));
            } else if line.starts_with("Reading patch ") {
                reporter.set_message(format!("读取补丁: {}", &line[14..]));
            } else if line == "Task: DOWNLOAD_MOJMAPS" {
                reporter.set_message("下载源码对照表".into());
            } else if line == "Task: MERGE_MAPPING" {
                reporter.set_message("合并源码对照表".into());
            } else if line == "Injecting profile" {
                reporter.set_message("注入版本元数据".into());
            } else if line == "true" {
                install_succeed.store(true, std::sync::atomic::Ordering::SeqCst);
            }
            line.clear();
        }

        let status = child.wait().await?;
        if !status.success() || !install_succeed.load(std::sync::atomic::Ordering::SeqCst) {
            return Err(ForgeError::InstallerFailed(format!(
                "安装器退出码: {}, 成功标志: {}",
                status.code().unwrap_or(-1),
                install_succeed.load(std::sync::atomic::Ordering::SeqCst)
            )));
        }
        Ok(())
    }
}

// ============================================================================
//  同步辅助函数（用于 spawn_blocking）
// ============================================================================

/// 合并覆盖 JAR（旧版 Forge）
fn merge_jar_overlay(source_zip: &Path, target_jar: &Path, temp_jar: &Path) -> ForgeResult<()> {
    use std::fs::File;
    use std::io::{Read, Write};
    use zip::ZipArchive;
    use zip::ZipWriter;

    let source_file = File::open(source_zip)?;
    let mut source_archive = ZipArchive::new(source_file)?;

    let target_file = File::open(target_jar)?;
    let mut target_archive = ZipArchive::new(target_file)?;

    let temp_file = File::create(temp_jar)?;
    let mut temp_writer = ZipWriter::new(temp_file);

    // 复制目标 JAR 内容（排除 META-INF）
    for i in 0..target_archive.len() {
        let entry = target_archive.by_index(i)?;
        if entry.name().starts_with("META-INF/") {
            continue;
        }
        if entry.is_file() {
            temp_writer.raw_copy_file(entry)?;
        } else if entry.is_dir() {
            temp_writer.add_directory(entry.name(), Default::default())?;
        }
    }

    // 复制源 ZIP 内容（排除 META-INF）
    for i in 0..source_archive.len() {
        let entry = source_archive.by_index(i)?;
        if entry.name().starts_with("META-INF/") {
            continue;
        }
        if entry.is_file() {
            temp_writer.raw_copy_file(entry)?;
        } else if entry.is_dir() {
            temp_writer.add_directory(entry.name(), Default::default())?;
        }
    }

    let writer = temp_writer.finish()?;
    writer.sync_all()?;

    // 原子替换
    std::fs::remove_file(target_jar)?;
    std::fs::rename(temp_jar, target_jar)?;
    Ok(())
}

/// 修改 Forge 安装器（同步）
fn modify_forge_installer_sync(
    from_reader: std::fs::File,
    to_writer: std::fs::File,
    version_name: &str,
    source: DownloadSource,
) -> ForgeResult<()> {
    use std::io::{Read, Write};
    use zip::ZipArchive;
    use zip::ZipWriter;

    let mut archive = ZipArchive::new(from_reader)?;
    let mut writer = ZipWriter::new(to_writer);

    let replace_source = match source {
        DownloadSource::BMCLAPI => "https://bmclapi2.bangbang93.com/maven",
        DownloadSource::MCBBS => "https://download.mcbbs.net/maven",
        _ => "https://files.minecraftforge.net",
    };

    for i in 0..archive.len() {
        let mut entry = archive.by_index(i)?;
        let name = entry.name().to_string();
        if name.starts_with("META-INF/") {
            continue;
        }

        if entry.is_file() {
            if name == "install_profile.json" {
                // 读取并修改 JSON
                let mut content = String::with_capacity(entry.size() as usize);
                entry.read_to_string(&mut content)?;
                let mut json: Value = serde_json::from_str(&content)?;

                // 修改 version 和 install.target
                if let Some(obj) = json.as_object_mut() {
                    if let Some(Value::String(ver)) = obj.get_mut("version") {
                        *ver = version_name.to_string();
                    }
                    if let Some(Value::Object(install)) = obj.get_mut("install") {
                        if let Some(Value::String(target)) = install.get_mut("target") {
                            *target = version_name.to_string();
                        }
                    }
                    // 修改镜像源
                    modify_libraries_urls(&mut json, replace_source);
                    if let Some(Value::Object(version_info)) = obj.get_mut("versionInfo") {
                        if let Some(Value::Array(libs)) = version_info.get_mut("libraries") {
                            for lib in libs.iter_mut() {
                                modify_library_urls(lib, replace_source);
                            }
                        }
                    }
                }

                let output = serde_json::to_vec_pretty(&json)?;
                writer.start_file(&name, Default::default())?;
                writer.write_all(&output)?;
            } else {
                // 直接复制其他文件
                writer.raw_copy_file(entry)?;
            }
        } else if entry.is_dir() {
            writer.add_directory(&name, Default::default())?;
        }
    }

    writer.finish()?.sync_all()?;
    Ok(())
}

/// 修改 library 对象的 URL
fn modify_library_urls(lib: &mut Value, replace_source: &str) {
    if let Some(Value::Object(obj)) = lib.as_object_mut() {
        if let Some(Value::Object(downloads)) = obj.get_mut("downloads") {
            if let Some(Value::Object(artifact)) = downloads.get_mut("artifact") {
                if let Some(Value::String(url)) = artifact.get_mut("url") {
                    if let Some(path) = url.strip_prefix("https://maven.minecraftforge.net") {
                        *url = format!("{}{}", replace_source, path);
                    }
                }
            }
        }
        if let Some(Value::String(url)) = obj.get_mut("url") {
            if let Some(path) = url.strip_prefix("https://maven.minecraftforge.net") {
                *url = format!("{}{}", replace_source, path);
            }
            if let Some(path) = url.strip_prefix("https://files.minecraftforge.net") {
                *url = format!("{}{}", replace_source, path);
            }
        }
    }
}

/// 修改 install_profile.json 中的 libraries
fn modify_libraries_urls(json: &mut Value, replace_source: &str) {
    if let Some(Value::Object(obj)) = json.as_object_mut() {
        if let Some(Value::Array(libs)) = obj.get_mut("libraries") {
            for lib in libs.iter_mut() {
                modify_library_urls(lib, replace_source);
            }
        }
    }
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
    async fn test_get_available_installers() {
        let downloader = Downloader::new(".minecraft", NR, DownloadSource::Official);
        let data = downloader.get_available_installers("1.16.5").await.unwrap();
        assert!(!data.all_versions.is_empty());
        assert!(data.recommended.is_some() || data.latest.is_some());
    }

    #[tokio::test]
    #[ignore]
    async fn test_download_forge_pre() {
        let dir = tempdir().unwrap();
        let mc_path = dir.path().to_str().unwrap();
        let downloader = Downloader::new(mc_path, NR, DownloadSource::Official);
        downloader.install_forge_pre("1.16.5", "1.16.5", "36.2.34").await.unwrap();
        // 检查文件是否存在
        let lib = Path::new(mc_path)
            .join("libraries/net/minecraftforge/forge/1.16.5-36.2.34/forge-1.16.5-36.2.34-installer.jar");
        assert!(lib.is_file());
    }
}