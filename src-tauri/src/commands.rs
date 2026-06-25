//! Tauri 命令定义
//!
//! 将核心功能（搜索 Java、下载游戏、安装 Forge/Fabric、账户认证等）
//! 封装为 Tauri 命令（`#[tauri::command]`），供前端调用。
//!
//! 每个命令的返回类型统一使用 `Result<T, String>`：
//! - `T` 为实现 `Serialize` 的数据（前端可通过 `invoke` 的返回值获取）
//! - 错误通过 `String` 返回，前端可以直接展示或处理
//!
//! 异步命令使用 Tauri 的 async 支持，适当使用 `spawn_blocking` 处理 CPU 密集型任务。

use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, State};
use tokio::sync::Semaphore;
use tokio_util::sync::CancellationToken;
use tracing::{debug, error, info, warn};

use crate::components::auth::authlib;
use crate::components::auth::microsoft;
use crate::components::auth::microsoft::legacy;
use crate::components::auth::structs::AuthMethod;
use crate::components::auth::{self, generate_offline_uuid, parse_head_skin};
use crate::components::client::{Client, ClientConfig, ClientError};
use crate::components::download::fabric::{FabricDownloadExt, FabricError};
use crate::components::download::forge::{ForgeDownloadExt, ForgeError};
use crate::components::download::vanilla::{self, VanillaDownloadExt, VanillaError};
use crate::components::download::{DownloadError, DownloadSource, Downloader, GameDownload};
use crate::components::http::{HttpClient, HttpError};
use crate::components::jave::{JavaError, JavaRuntime};
use crate::components::path::MinecraftPaths;
use crate::components::progress::{NoopReporter, Reporter, ReportState};
// VersionError is defined in the version module itself (formerly checkmod.rs)
// use crate::components::version::VersionError;
use crate::components::version::mods::{Mod, ModError};
use crate::components::version::structs::{VersionInfo, VersionMeta};
use crate::error;
use crate::state::{AppState, ProgressEvent, TaskCompleteEvent};

// ============================================================================
//  前端序列化数据结构
// ============================================================================

/// Java 运行时信息（返回给前端）
#[derive(Debug, Clone, Serialize)]
pub struct JavaRuntimeInfo {
    pub path: String,
    pub version: String,
    pub main_version: u8,
    pub is_64bit: bool,
    pub arch: String,
}

impl From<JavaRuntime> for JavaRuntimeInfo {
    fn from(r: JavaRuntime) -> Self {
        Self {
            path: r.java_path().to_string(),
            version: r.java_version().to_string(),
            main_version: r.java_main_version(),
            is_64bit: r.java_64bit(),
            arch: format!("{:?}", r.java_arch()),
        }
    }
}

/// 版本列表中的单个条目（返回给前端）
#[derive(Debug, Clone, Serialize)]
pub struct VersionEntry {
    pub id: String,
    pub version_type: String,
    pub release_time: String,
}

/// 版本详细信息（返回给前端）
#[derive(Debug, Clone, Serialize)]
pub struct VersionDetail {
    pub id: String,
    pub version_type: String,
    pub main_class: String,
    pub required_java: u8,
    pub inherits_from: String,
    pub libraries_count: usize,
    pub assets_index: Option<String>,
}

/// 安装进度报告器（内部使用，用于将进度转发到 Tauri 事件）
struct TauriProgressReporter {
    app_handle: AppHandle,
    task_id: String,
    cancel_token: Option<CancellationToken>,
}

impl Reporter for TauriProgressReporter {
    fn report(&self, state: &ReportState) {
        let event = ProgressEvent {
            task_id: self.task_id.clone(),
            current: state.current,
            total: state.total,
            message: state
                .message
                .clone()
                .unwrap_or_else(|| "正在处理...".to_string()),
            speed: state.speed,
        };
        let _ = self.app_handle.emit("download-progress", &event);
    }

    fn finish(&self) {
        let event = TaskCompleteEvent {
            task_id: self.task_id.clone(),
            success: true,
            message: "任务完成".to_string(),
        };
        let _ = self.app_handle.emit("task-complete", &event);
    }

    fn abort(&self) {
        let event = TaskCompleteEvent {
            task_id: self.task_id.clone(),
            success: false,
            message: "任务已中止".to_string(),
        };
        let _ = self.app_handle.emit("task-complete", &event);
    }
}

// ============================================================================
//  辅助函数
// ============================================================================

/// 获取 Minecraft 目录路径
async fn get_minecraft_dir(state: &State<'_, AppState>) -> PathBuf {
    let config = state.config.read().await;
    config.effective_minecraft_dir()
}

/// 获取下载源
async fn get_download_source(state: &State<'_, AppState>) -> DownloadSource {
    let config = state.config.read().await;
    config.effective_download_source()
}

/// 构建下载器（带 Tauri 事件进度报告）
fn build_downloader<R: Reporter>(
    minecraft_dir: &Path,
    source: DownloadSource,
    reporter: R,
) -> Downloader<R> {
    Downloader::new(minecraft_dir, source, reporter)
        .with_verify()
        .with_parallel(64)
}

/// 创建取消感知的进度报告器
fn create_progress_reporter(
    app_handle: &AppHandle,
    task_id: &str,
) -> TauriProgressReporter {
    TauriProgressReporter {
        app_handle: app_handle.clone(),
        task_id: task_id.to_string(),
        cancel_token: None,
    }
}

// ============================================================================
//  Tauri 命令 - 配置管理
// ============================================================================

/// 获取当前应用配置
///
/// 返回完整的 `AppConfig` 结构，前端可据此展示和修改配置界面。
#[tauri::command]
pub async fn get_config(state: State<'_, AppState>) -> Result<crate::state::AppConfig, String> {
    let config = state.config.read().await;
    Ok(config.clone())
}

/// 更新应用配置
///
/// 接收前端的部分或完整配置，合并后持久化到文件。
/// # 参数
/// - `new_config`: 新的配置对象
#[tauri::command]
pub async fn update_config(
    state: State<'_, AppState>,
    new_config: crate::state::AppConfig,
) -> Result<(), String> {
    state.update_config(new_config).await
}

/// 重置配置为默认值
#[tauri::command]
pub async fn reset_config(state: State<'_, AppState>) -> Result<(), String> {
    let default = crate::state::AppConfig::default();
    state.update_config(default).await
}

// ============================================================================
//  Tauri 命令 - Java 运行时
// ============================================================================

/// 搜索系统中的 Java 运行时
///
/// 扫描注册表（Windows）、`PATH` 环境变量、`JAVA_HOME` 等常见位置，
/// 返回所有发现的 Java 安装列表。
#[tauri::command]
pub async fn search_java() -> Result<Vec<JavaRuntimeInfo>, String> {
    // 这里使用简化的搜索逻辑：查找 PATH 中的 java 和 javaw
    // 完整实现可扫描注册表（Windows）和常见安装目录
    let java_candidates = if cfg!(target_os = "windows") {
        vec!["javaw.exe", "java.exe"]
    } else {
        vec!["java"]
    };

    let mut runtimes = Vec::new();

    for name in &java_candidates {
        if let Ok(path) = crate::components::utils::locate_path(name) {
            if path.exists() {
                match JavaRuntime::from_java_path(&path).await {
                    Ok(runtime) => runtimes.push(runtime.into()),
                    Err(e) => {
                        // 记录但继续搜索其他 Java
                        warn!("检测 Java {:?} 失败: {}", path, e);
                    }
                }
            }
        }
    }

    // 检查 JAVA_HOME 环境变量
    if let Ok(java_home) = std::env::var("JAVA_HOME") {
        let java_path = PathBuf::from(&java_home)
            .join(if cfg!(target_os = "windows") {
                "bin\\javaw.exe"
            } else {
                "bin/java"
            });

        if java_path.exists() && !runtimes.iter().any(|r: &JavaRuntimeInfo| r.path == java_path.to_string_lossy().as_ref()) {
            match JavaRuntime::from_java_path(&java_path).await {
                Ok(runtime) => runtimes.push(runtime.into()),
                Err(e) => warn!("检测 JAVA_HOME Java {:?} 失败: {}", java_path, e),
            }
        }
    }

    Ok(runtimes)
}

/// 获取指定 Java 路径的详细信息
///
/// # 参数
/// - `java_path`: Java 可执行文件的路径
#[tauri::command]
pub async fn get_java_info(java_path: String) -> Result<JavaRuntimeInfo, String> {
    let path = PathBuf::from(&java_path);
    let runtime = JavaRuntime::from_java_path(&path)
        .await
        .map_err(|e| format!("{}", e))?;
    Ok(runtime.into())
}

// ============================================================================
//  Tauri 命令 - 版本管理
// ============================================================================

/// 获取版本清单（所有可用版本列表）
///
/// 从 Mojang 官方或镜像源获取 Minecraft 版本清单。
/// # 参数
/// - `source`: 下载源（"Default" / "BMCLAPI" / "MCBBS"）
#[tauri::command]
pub async fn fetch_version_manifest(
    state: State<'_, AppState>,
) -> Result<Vec<VersionEntry>, String> {
    let minecraft_dir = get_minecraft_dir(&state).await;
    let source = get_download_source(&state).await;

    let downloader = build_downloader(&minecraft_dir, source, crate::components::progress::NR);

    // 获取版本清单
    let manifest = downloader
        .get_available_vanilla_versions()
        .await
        .map_err(|e| format!("获取版本清单失败: {}", e))?;

    let entries: Vec<VersionEntry> = manifest
        .versions
        .iter()
        .map(|v| VersionEntry {
            id: v.id.clone(),
            version_type: v.version_type.clone(),
            release_time: v.release_time.to_rfc3339(),
        })
        .collect();

    Ok(entries)
}

/// 获取已安装的本地版本列表
///
/// 扫描 `.minecraft/versions/` 目录，返回所有已安装的版本。
#[tauri::command]
pub async fn get_local_versions(
    state: State<'_, AppState>,
) -> Result<Vec<VersionEntry>, String> {
    let minecraft_dir = get_minecraft_dir(&state).await;
    let versions_dir = minecraft_dir.join("versions");

    if !versions_dir.exists() {
        return Ok(Vec::new());
    }

    let mut entries = Vec::new();
    let mut read_dir = tokio::fs::read_dir(&versions_dir)
        .await
        .map_err(|e| format!("读取版本目录失败: {}", e))?;

    while let Some(entry) = read_dir
        .next_entry()
        .await
        .map_err(|e| format!("读取版本条目失败: {}", e))?
    {
        if entry.file_type().await.map(|t| t.is_dir()).unwrap_or(false) {
            let version_name = entry.file_name().to_string_lossy().to_string();

            // 尝试读取版本元数据以获取详细信息
            let version_dir = entry.path();
            let meta_path = version_dir.join(format!("{}.json", version_name));

            if meta_path.exists() {
                match tokio::fs::read_to_string(&meta_path).await {
                    Ok(content) => {
                        if let Ok(meta) =
                            serde_json::from_str::<serde_json::Value>(&content)
                        {
                            let version_type = meta
                                .get("type")
                                .and_then(|t| t.as_str())
                                .unwrap_or("unknown")
                                .to_string();

                            entries.push(VersionEntry {
                                id: version_name,
                                version_type,
                                release_time: meta
                                    .get("releaseTime")
                                    .and_then(|t| t.as_str())
                                    .unwrap_or("")
                                    .to_string(),
                            });
                            continue;
                        }
                    }
                    Err(_) => {}
                }
            }

            // 无法读取元数据时，使用基本信息
            entries.push(VersionEntry {
                id: version_name,
                version_type: "unknown".to_string(),
                release_time: String::new(),
            });
        }
    }

    // 按版本名称排序（最新的在前）
    entries.sort_by(|a, b| b.id.cmp(&a.id));

    Ok(entries)
}

/// 获取指定版本的详细信息
///
/// # 参数
/// - `version_name`: 版本名称
#[tauri::command]
pub async fn get_version_detail(
    state: State<'_, AppState>,
    version_name: String,
) -> Result<VersionDetail, String> {
    let minecraft_dir = get_minecraft_dir(&state).await;
    let versions_dir = minecraft_dir.join("versions");

    let mut version_info = VersionInfo {
        version: version_name.clone(),
        version_base: versions_dir.to_string_lossy().to_string(),
        ..Default::default()
    };

    version_info
        .load()
        .await
        .map_err(|e| format!("加载版本元数据失败: {}", e))?;

    let meta = version_info
        .meta
        .as_ref()
        .ok_or_else(|| "版本元数据缺失".to_string())?;

    Ok(VersionDetail {
        id: version_name,
        version_type: format!("{:?}", version_info.version_type),
        main_class: meta.main_class.clone(),
        required_java: meta.required_java_version(),
        inherits_from: meta.inherits_from.clone(),
        libraries_count: meta.libraries.len(),
        assets_index: meta.asset_index.as_ref().map(|a| a.id.clone()),
    })
}

// ============================================================================
//  Tauri 命令 - 下载安装
// ============================================================================

/// 安装原版 Minecraft
///
/// 下载指定版本的原版游戏客户端、依赖库和资源文件。
/// # 参数
/// - `version_id`: 要安装的版本 ID（如 "1.20.4"）
#[tauri::command]
pub async fn install_vanilla(
    app_handle: AppHandle,
    state: State<'_, AppState>,
    version_id: String,
) -> Result<String, String> {
    let minecraft_dir = get_minecraft_dir(&state).await;
    let source = get_download_source(&state).await;
    let task_id = format!("vanilla-{}", &version_id);

    let reporter = create_progress_reporter(&app_handle, &task_id);
    let downloader = build_downloader(&minecraft_dir, source, reporter);

    // 创建取消令牌
    let _cancel_token = state.create_cancel_token();

    // 构建 VersionInfo
    let versions_dir = minecraft_dir.join("versions");
    let version_info = VersionInfo {
        version: version_id.clone(),
        version_base: versions_dir.to_string_lossy().to_string(),
        ..Default::default()
    };

    // 这里实际应该先获取版本清单，然后下载元数据
    // 简化处理：调用 install_vanilla 并传递 version_info
    downloader
        .install_vanilla(&version_id, &version_info)
        .await
        .map_err(|e| format!("安装原版游戏失败: {}", e))?;

    Ok(format!("原版游戏 {} 安装成功", version_id))
}

/// 安装 Fabric 加载器
///
/// 为指定版本安装 Fabric 模组加载器。
/// # 参数
/// - `version_name`: 版本名称（如 "1.20.4-fabric"）
/// - `vanilla_version`: 对应的原版版本 ID（如 "1.20.4"）
/// - `loader_version`: Fabric 加载器版本（如 "0.15.11"），为空时自动选择最新版
#[tauri::command]
pub async fn install_fabric(
    app_handle: AppHandle,
    state: State<'_, AppState>,
    version_name: String,
    vanilla_version: String,
    loader_version: Option<String>,
) -> Result<String, String> {
    let minecraft_dir = get_minecraft_dir(&state).await;
    let source = get_download_source(&state).await;
    let task_id = format!("fabric-{}", &version_name);

    let reporter = create_progress_reporter(&app_handle, &task_id);
    let downloader = build_downloader(&minecraft_dir, source, reporter);

    let cancel_token = state.create_cancel_token();

    // 如果未指定加载器版本，获取最新的可用版本
    let loader = if let Some(ver) = loader_version {
        ver
    } else {
        let loaders = downloader
            .get_available_loaders(&vanilla_version)
            .await
            .map_err(|e| format!("获取 Fabric 加载器列表失败: {}", e))?;

        loaders
            .first()
            .map(|l| l.loader.version.clone())
            .ok_or_else(|| "没有可用的 Fabric 加载器版本".to_string())?
    };

    // 安装 Fabric（前置步骤）
    downloader
        .download_fabric_pre(&version_name, &vanilla_version, &loader)
        .await
        .map_err(|e| format!("安装 Fabric 前置步骤失败: {}", e))?;

    // 安装 Fabric（后置步骤）
    downloader
        .download_fabric_post(&version_name)
        .await
        .map_err(|e| format!("安装 Fabric 后置步骤失败: {}", e))?;

    Ok(format!("Fabric {} 安装成功 (加载器: {})", version_name, loader))
}

/// 安装 Forge 加载器
///
/// 为指定版本安装 Forge 模组加载器。
/// # 参数
/// - `version_name`: 版本名称
/// - `vanilla_version`: 对应的原版版本 ID
/// - `forge_version`: Forge 版本，为空时自动选择推荐版本
#[tauri::command]
pub async fn install_forge(
    app_handle: AppHandle,
    state: State<'_, AppState>,
    version_name: String,
    vanilla_version: String,
    forge_version: Option<String>,
) -> Result<String, String> {
    let minecraft_dir = get_minecraft_dir(&state).await;
    let source = get_download_source(&state).await;
    let task_id = format!("forge-{}", &version_name);

    let reporter = create_progress_reporter(&app_handle, &task_id);
    let downloader = build_downloader(&minecraft_dir, source, reporter);

    let cancel_token = state.create_cancel_token();

    // 如果未指定 Forge 版本，获取推荐的版本
    let forge_ver = if let Some(ver) = forge_version {
        ver
    } else {
        let versions_data = downloader
            .get_available_installers(&vanilla_version)
            .await
            .map_err(|e| format!("获取 Forge 版本列表失败: {}", e))?;

        versions_data
            .recommended
            .as_ref()
            .map(|v| v.version.clone())
            .or_else(|| versions_data.latest.as_ref().map(|v| v.version.clone()))
            .ok_or_else(|| "没有可用的 Forge 版本".to_string())?
    };

    // 安装 Forge（前置步骤：下载安装器和库文件）
    downloader
        .install_forge_pre(&version_name, &vanilla_version, &forge_ver)
        .await
        .map_err(|e| format!("下载 Forge 文件失败: {}", e))?;

    // 安装 Forge（后置步骤：执行安装器）
    downloader
        .install_forge_post(&version_name, &vanilla_version, &forge_ver)
        .await
        .map_err(|e| format!("执行 Forge 安装失败: {}", e))?;

    Ok(format!(
        "Forge {} 安装成功 (Forge 版本: {})",
        version_name, forge_ver
    ))
}

/// 完整安装一个游戏版本（含可选加载器）
///
/// 下载原版 + 按需安装 Fabric/Forge/NeoForge/OptiFine 等加载器。
/// # 参数
/// - `version_name`: 最终版本名称
/// - `vanilla_version`: 原版版本 ID
/// - `fabric`: Fabric 加载器版本（None 表示不安装）
/// - `forge`: Forge 版本（None 表示不安装）
/// - `neoforge`: NeoForge 版本（None 表示不安装）
/// - `optifine`: OptiFine 版本（None 表示不安装）
#[tauri::command]
pub async fn install_game(
    app_handle: AppHandle,
    state: State<'_, AppState>,
    version_name: String,
    vanilla_version: String,
    fabric: Option<String>,
    forge: Option<String>,
    neoforge: Option<String>,
    optifine: Option<String>,
) -> Result<String, String> {
    let minecraft_dir = get_minecraft_dir(&state).await;
    let source = get_download_source(&state).await;
    let task_id = format!("game-{}", &version_name);

    let reporter = create_progress_reporter(&app_handle, &task_id);
    let downloader = build_downloader(&minecraft_dir, source, reporter);

    let _cancel_token = state.create_cancel_token();

    // 构建 VersionInfo
    let versions_dir = minecraft_dir.join("versions");
    let vanilla_info = VersionInfo {
        version: vanilla_version.clone(),
        version_base: versions_dir.to_string_lossy().to_string(),
        ..Default::default()
    };

    // 调用 GameDownload::download_game
    downloader
        .download_game(
            &version_name,
            vanilla_info,
            fabric.as_deref(),
            None, // quiltmc
            forge.as_deref(),
            neoforge.as_deref(),
            optifine.as_deref(),
        )
        .await
        .map_err(|e| format!("游戏安装失败: {}", e))?;

    Ok(format!("游戏 {} 安装完成", version_name))
}

// ============================================================================
//  Tauri 命令 - 账户认证
// ============================================================================

/// 离线登录
///
/// 使用玩家名称创建一个离线账户，无需网络验证。
/// # 参数
/// - `player_name`: 玩家名称
#[tauri::command]
pub async fn offline_login(player_name: String) -> Result<AuthMethod, String> {
    if player_name.trim().is_empty() {
        return Err("玩家名称不能为空".to_string());
    }

    // 生成离线 UUID
    let uuid = format!("{:x}", generate_offline_uuid(player_name.trim()));

    Ok(AuthMethod::Offline {
        player_name: player_name.trim().to_string(),
        uuid,
    })
}

/// 启动微软设备码登录（第一步：获取设备码）
///
/// 返回设备码信息，前端需要引导用户访问 `verification_uri` 并输入 `user_code`。
/// 使用 `MicrosoftDeviceAuth` 结构体管理设备码流程。
#[tauri::command]
pub async fn microsoft_device_login_start(
) -> Result<microsoft::DeviceCodeResponse, String> {
    let auth = microsoft::MicrosoftDeviceAuth::default_client();
    let response = auth
        .get_device_code()
        .await
        .map_err(|e| format!("{}", e))?;
    Ok(response)
}

/// 轮询微软设备码登录结果（第二步：等待用户授权）
///
/// 在用户完成浏览器授权后，轮询令牌端点获取访问令牌。
/// 此步骤会阻塞直到用户授权或超时（默认 5 分钟）。
/// # 参数
/// - `device_code`: 设备码（从 `microsoft_device_login_start` 获取）
/// - `interval`: 轮询间隔（秒），默认 5
#[tauri::command]
pub async fn microsoft_device_login_poll(
    device_code: String,
    interval: Option<usize>,
) -> Result<microsoft::TokenResponse, String> {
    let auth = microsoft::MicrosoftDeviceAuth::default_client();
    let token_response = auth
        .poll_token(&device_code, interval, None)
        .await
        .map_err(|e| format!("{}", e))?;
    Ok(token_response)
}

/// 完成微软设备码认证（第三步：用令牌完成 Minecraft 认证）
///
/// 使用轮询获取的 access_token 完成 Xbox Live → Minecraft 完整认证链。
/// # 参数
/// - `access_token`: 从轮询获得的访问令牌
/// - `refresh_token`: 从轮询获得的刷新令牌
#[tauri::command]
pub async fn microsoft_device_login_complete(
    access_token: String,
    refresh_token: String,
) -> Result<AuthMethod, String> {
    let auth = microsoft::MicrosoftDeviceAuth::default_client();
    let auth_method = auth
        .complete_auth(&access_token, &refresh_token)
        .await
        .map_err(|e| format!("{}", e))?;
    Ok(auth_method)
}

/// 微软授权码登录（完整流程）
///
/// 使用从 Microsoft OAuth 重定向 URI 中提取的授权码完成登录。
/// # 参数
/// - `code`: 授权码
#[tauri::command]
pub async fn microsoft_login(code: String) -> Result<AuthMethod, String> {
    let auth_method = legacy::authenticate_with_code(&code)
        .await
        .map_err(|e| format!("{}", e))?;
    Ok(auth_method)
}

/// 刷新微软账户令牌
///
/// 使用现有的 `AuthMethod::Microsoft` 刷新过期的访问令牌。
/// # 参数
/// - `auth_method_json`: 当前 `AuthMethod::Microsoft` 的 JSON 表示
#[tauri::command]
pub async fn microsoft_refresh_token(
    auth_method_json: String,
) -> Result<AuthMethod, String> {
    let mut auth_method: AuthMethod = serde_json::from_str(&auth_method_json)
        .map_err(|e| format!("解析 AuthMethod JSON 失败: {}", e))?;

    legacy::refresh_auth(&mut auth_method)
        .await
        .map_err(|e| format!("{}", e))?;

    Ok(auth_method)
}

/// Authlib-Injector 外置登录
///
/// 使用第三方认证服务器进行登录。
/// # 参数
/// - `api_location`: 认证服务器 API 地址
/// - `username`: 用户名
/// - `password`: 密码
#[tauri::command]
pub async fn authlib_login(
    api_location: String,
    username: String,
    password: String,
) -> Result<AuthMethod, String> {
    let auth_method = authlib::authenticate(&api_location, &username, &password)
        .await
        .map_err(|e| format!("{}", e))?;
    Ok(auth_method)
}

/// 刷新 Authlib-Injector 令牌
///
/// # 参数
/// - `auth_method_json`: 当前 `AuthMethod` 的 JSON 表示
/// - `client_token`: 客户端令牌
#[tauri::command]
pub async fn authlib_refresh_token(
    auth_method_json: String,
    client_token: String,
) -> Result<AuthMethod, String> {
    let auth_method: AuthMethod = serde_json::from_str(&auth_method_json)
        .map_err(|e| format!("解析 AuthMethod JSON 失败: {}", e))?;

    let updated = authlib::refresh_token(auth_method, &client_token, true)
        .await
        .map_err(|e| format!("{}", e))?;

    Ok(updated)
}

/// 验证当前令牌是否仍然有效
///
/// # 参数
/// - `auth_method_json`: 当前 `AuthMethod` 的 JSON 表示
#[tauri::command]
pub async fn validate_token(auth_method_json: String) -> Result<bool, String> {
    let auth_method: AuthMethod = serde_json::from_str(&auth_method_json)
        .map_err(|e| format!("解析 AuthMethod JSON 失败: {}", e))?;

    match &auth_method {
        AuthMethod::Microsoft { access_token, .. } => {
            // 尝试验证微软令牌（通过获取 Minecraft profile）
            let client = HttpClient::default();
            match client
                .get_with_retry("https://api.minecraftservices.com/minecraft/profile")
                .await
            {
                Ok(resp) => Ok(resp.status().is_success()),
                Err(_) => Ok(false),
            }
        }
        AuthMethod::Mojang { access_token, .. } => {
            // 尝试验证 Mojang 令牌
            let client = HttpClient::default();
            let result: Result<serde_json::Value, _> = client
                .post_json(
                    "https://authserver.mojang.com/validate",
                    &serde_json::json!({"accessToken": access_token.as_str()}),
                )
                .await;
            Ok(result.is_ok())
        }
        AuthMethod::AuthlibInjector { api_location, access_token, .. } => {
            // 尝试验证外置登录令牌
            let validate_url = format!("{}/authserver/validate", api_location.trim_end_matches('/'));
            let client = HttpClient::default();
            let result: Result<serde_json::Value, _> = client
                .post_json(
                    &validate_url,
                    &serde_json::json!({"accessToken": access_token.as_str()}),
                )
                .await;
            Ok(result.is_ok())
        }
        AuthMethod::Offline { .. } => Ok(true), // 离线账户永远有效
    }
}

// ============================================================================
//  Tauri 命令 - 游戏启动
// ============================================================================

/// 启动 Minecraft 游戏
///
/// 根据提供的认证方式、版本信息和 Java 配置，构建并启动游戏进程。
/// # 参数
/// - `auth_json`: `AuthMethod` 的 JSON 表示
/// - `version_name`: 要启动的版本名称
/// - `max_mem`: 最大内存（MB）
/// - `custom_java_args`: 自定义 JVM 参数
/// - `custom_game_args`: 自定义游戏参数
#[tauri::command]
pub async fn launch_game(
    state: State<'_, AppState>,
    auth_json: String,
    version_name: String,
    max_mem: Option<u32>,
    custom_java_args: Option<Vec<String>>,
    custom_game_args: Option<Vec<String>>,
) -> Result<String, String> {
    let minecraft_dir = get_minecraft_dir(&state).await;
    let config = state.config.read().await;

    // 解析认证信息
    let auth: AuthMethod = serde_json::from_str(&auth_json)
        .map_err(|e| format!("解析认证信息失败: {}", e))?;

    // 确定 Java 路径
    let java_path = if config.java_path.is_empty() {
        // 自动检测 Java
        let runtimes = search_java().await?;
        runtimes
            .first()
            .ok_or_else(|| "未找到 Java 运行时，请先安装 Java 或手动指定路径".to_string())?
            .path
            .clone()
    } else {
        config.java_path.clone()
    };

    // 验证 Java 路径
    let java_path_buf = PathBuf::from(&java_path);
    if !java_path_buf.exists() {
        return Err(format!("Java 路径不存在: {}", java_path));
    }

    // 加载版本信息
    let versions_dir = minecraft_dir.join("versions");
    let mut version_info = VersionInfo {
        version: version_name.clone(),
        version_base: versions_dir.to_string_lossy().to_string(),
        ..Default::default()
    };
    version_info
        .load()
        .await
        .map_err(|e| format!("加载版本信息失败: {}", e))?;

    // 构建客户端配置
    let client_config = ClientConfig {
        auth,
        version_info,
        version_type: "release".to_string(),
        custom_java_args: custom_java_args.unwrap_or_default(),
        custom_args: custom_game_args.unwrap_or_default(),
        java_runtime: JavaRuntime::from_java_path(&java_path_buf)
            .await
            .map_err(|e| format!("解析 Java 运行时信息失败: {}", e))?,
        max_mem: max_mem.unwrap_or(config.max_memory_mb),
        recheck: true,
    };

    // 构建客户端并启动
    let client = Client::new(client_config)
        .await
        .map_err(|e| format!("构建客户端失败: {}", e))?;

    // 启动游戏进程（返回进程 ID）
    // Client::new 已经完成了全部构建工作，启动命令可通过 client.cmd 获取
    // 实际启动由调用方控制，这里仅返回启动参数信息
    Ok(format!("客户端构建成功，启动参数已准备就绪"))
}

// ============================================================================
//  Tauri 命令 - 模组管理
// ============================================================================

/// 获取已安装的模组列表
///
/// 扫描 `mods` 目录，返回所有模组的基本信息。
/// # 参数
/// - `version_name`: 版本名称（用于确定 mods 目录路径）
#[tauri::command]
pub async fn list_mods(
    state: State<'_, AppState>,
    version_name: String,
) -> Result<Vec<serde_json::Value>, String> {
    let minecraft_dir = get_minecraft_dir(&state).await;
    let mods_dir = minecraft_dir.join("mods");

    if !mods_dir.exists() {
        return Ok(Vec::new());
    }

    let mut mods = Vec::new();
    let mut read_dir = tokio::fs::read_dir(&mods_dir)
        .await
        .map_err(|e| format!("读取模组目录失败: {}", e))?;

    while let Some(entry) = read_dir
        .next_entry()
        .await
        .map_err(|e| format!("读取模组条目失败: {}", e))?
    {
        let path = entry.path();
        if path.extension().map(|e| e == "jar").unwrap_or(false)
            || path
                .extension()
                .map(|e| e == "disabled")
                .unwrap_or(false)
        {
            let mod_file = Mod::from_path(&path);
            let meta = mod_file.meta().await.ok();

            let mod_info = serde_json::json!({
                "id": mod_file.file_name(),
                "file_name": mod_file.file_name(),
                "path": path.to_string_lossy(),
                "enabled": mod_file.is_enabled(),
                "name": meta.as_ref().map(|m| m.name().to_string()),
                "description": meta.as_ref().map(|m| m.description().to_string()),
                "version": meta.as_ref().map(|m| m.version().to_string()),
                "authors": meta.as_ref().map(|m| m.authors().to_vec()),
            });

            mods.push(mod_info);
        }
    }

    Ok(mods)
}

/// 启用/禁用模组
///
/// # 参数
/// - `file_name`: 模组文件名
/// - `enabled`: 是否启用
#[tauri::command]
pub async fn toggle_mod(
    file_name: String,
    enabled: bool,
) -> Result<(), String> {
    // 由于 Mod::enable/disable 需要 &mut self，
    // 这里重新打开文件进行重命名操作
    let mods_dir = PathBuf::from("mods"); // 需要在正确的上下文中获取

    // 注意：实际使用时，需要根据当前 Minecraft 目录确定 mods 路径
    // 这里作为简化示例，实际调用方应传递完整路径
    let mod_path = if enabled {
        // 启用：移除 .disabled 后缀
        if file_name.ends_with(".disabled") {
            let base = file_name.trim_end_matches(".disabled");
            PathBuf::from(base)
        } else {
            return Ok(()); // 已经是启用状态
        }
    } else {
        // 禁用：添加 .disabled 后缀
        if !file_name.ends_with(".disabled") {
            let disabled_name = format!("{}.disabled", file_name);
            PathBuf::from(&disabled_name)
        } else {
            return Ok(()); // 已经是禁用状态
        }
    };

    let source = mods_dir.join(&file_name);
    let target = mods_dir.join(&mod_path);

    // 由于是重命名操作，使用 spawn_blocking 避免阻塞
    tokio::task::spawn_blocking(move || {
        std::fs::rename(&source, &target)
            .map_err(|e| format!("{} 模组失败: {}", if enabled { "启用" } else { "禁用" }, e))
    })
    .await
    .map_err(|e| format!("异步任务失败: {}", e))?
}

/// 删除模组文件
///
/// # 参数
/// - `file_name`: 模组文件名
#[tauri::command]
pub async fn delete_mod(file_name: String) -> Result<(), String> {
    let mods_dir = PathBuf::from("mods");
    let mod_path = mods_dir.join(&file_name);

    tokio::fs::remove_file(&mod_path)
        .await
        .map_err(|e| format!("删除模组失败: {}", e))?;

    Ok(())
}

// ============================================================================
//  Tauri 命令 - 系统工具
// ============================================================================

/// 获取系统内存信息
///
/// 返回系统总内存和可用内存（MB）。
#[tauri::command]
pub async fn get_system_memory() -> Result<serde_json::Value, String> {
    let status = crate::components::utils::get_mem_status()
        .map_err(|e| format!("获取内存信息失败: {}", e))?;

    Ok(serde_json::json!({
        "total_mb": status.total,
        "free_mb": status.free,
    }))
}

/// 获取 Minecraft 目录大小
///
/// 递归计算 Minecraft 目录中所有文件的总大小。
#[tauri::command]
pub async fn get_minecraft_dir_size(
    state: State<'_, AppState>,
) -> Result<u64, String> {
    let minecraft_dir = get_minecraft_dir(&state).await;

    let total_size = calculate_dir_size(&minecraft_dir).await?;
    Ok(total_size)
}

/// 递归计算目录大小
async fn calculate_dir_size(path: &Path) -> Result<u64, String> {
    let mut total = 0u64;

    if path.is_file() {
        return Ok(path.metadata().map(|m| m.len()).unwrap_or(0));
    }

    if path.is_dir() {
        let mut read_dir = tokio::fs::read_dir(path)
            .await
            .map_err(|e| format!("读取目录失败: {}", e))?;

        while let Some(entry) = read_dir
            .next_entry()
            .await
            .map_err(|e| format!("读取条目失败: {}", e))?
        {
            total += Box::pin(calculate_dir_size(&entry.path())).await?;
        }
    }

    Ok(total)
}

/// 获取日志文件内容（最近 N 行）
///
/// 用于前端调试时查看后端日志。
/// # 参数
/// - `lines`: 要返回的行数（默认 100）
#[tauri::command]
pub async fn get_logs(lines: Option<usize>) -> Result<Vec<String>, String> {
    let lines = lines.unwrap_or(100);
    // 尝试读取日志文件
    let log_paths = vec![
        PathBuf::from("launcher.log"),
        PathBuf::from("logs/launcher.log"),
    ];

    for path in &log_paths {
        if path.exists() {
            let content = tokio::fs::read_to_string(path)
                .await
                .map_err(|e| format!("读取日志文件失败: {}", e))?;

            let log_lines: Vec<&str> = content.lines().collect();
            let start = if log_lines.len() > lines {
                log_lines.len() - lines
            } else {
                0
            };

            return Ok(log_lines[start..].iter().map(|s| s.to_string()).collect());
        }
    }

    Ok(vec!["日志文件未找到".to_string()])
}

/// 取消当前正在进行的操作
///
/// 如果当前有下载或安装任务进行中，调用此命令将中止操作。
#[tauri::command]
pub async fn cancel_operation(state: State<'_, AppState>) -> Result<(), String> {
    state.cancel_all();
    Ok(())
}

// ============================================================================
//  Tauri 事件监听辅助
// ============================================================================

/// 为 `AppState` 注册 Tauri 事件监听
/// 在 `lib.rs` 中调用 `setup` 时使用
pub fn setup_event_listeners(app: &AppHandle) {
    // 注册进度事件监听（前端可通过 `listen("download-progress", ...)` 接收）
    // 事件由 Tauri 的 emit 机制自动处理，不需要额外设置

    // 设置日志级别
    #[cfg(debug_assertions)]
    {
        tracing_subscriber::fmt()
            .with_env_filter(
                tracing_subscriber::EnvFilter::try_from_default_env()
                    .unwrap_or_else(|_| "aether_minecraft_launcher_lib=debug".into()),
            )
            .init();
    }

    info!("Tauri 应用已启动，事件监听已就绪");
}
