//! 全局配置与状态管理
//!
//! 使用 Tauri 的 `State` 管理全局配置（如下载源、Minecraft 目录、代理设置等）。
//! 支持从 `Config.toml` 文件持久化读取和更新用户偏好。
//! 提供下载进度事件的发送通道，用于向前端推送实时进度。

use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tokio::sync::{Mutex, RwLock, watch};
use tokio_util::sync::CancellationToken;
use tracing::{debug, error, info, warn};

use crate::components::download::DownloadSource;
use crate::components::http::{HttpClient, HttpClientConfig, ProxyConfig};
use crate::components::path::MinecraftPaths;

// ============================================================================
//  配置文件路径
// ============================================================================

/// 配置文件名
const CONFIG_FILE_NAME: &str = "Config.toml";

// ============================================================================
//  配置数据结构
// ============================================================================

/// 用户偏好配置，存储在 `Config.toml` 文件中
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AppConfig {
    /// Minecraft 游戏目录（默认通过 `MinecraftPaths` 检测）
    pub minecraft_dir: String,
    /// 下载源（Official / BMCLAPI / MCBBS / Custom URL）
    pub download_source: String,
    /// Java 运行时路径（为空时自动检测）
    pub java_path: String,
    /// 最大内存分配（MB）
    pub max_memory_mb: u32,
    /// JVM 额外参数
    pub jvm_args: String,
    /// 游戏额外参数
    pub game_args: String,
    /// 窗口宽度
    pub window_width: u32,
    /// 窗口高度
    pub window_height: u32,
    /// 是否启用代理
    pub proxy_enabled: bool,
    /// 代理主机地址
    pub proxy_host: String,
    /// 代理端口
    pub proxy_port: u16,
    /// 代理用户名（可选）
    pub proxy_username: String,
    /// 代理密码（可选）
    pub proxy_password: String,
    /// 下载并发数
    pub download_parallel: usize,
    /// 是否启用文件校验
    pub verify_files: bool,
    /// 是否版本独立
    pub game_independent: bool,
    /// 主题（"light" / "dark" / "system"）
    pub theme: String,
    /// 语言（"zh-CN" / "en-US" 等）
    pub language: String,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            minecraft_dir: String::new(), // 空字符串表示使用默认路径
            download_source: "Default".to_string(),
            java_path: String::new(),
            max_memory_mb: 2048,
            jvm_args: String::new(),
            game_args: String::new(),
            window_width: 1000,
            window_height: 700,
            proxy_enabled: false,
            proxy_host: String::new(),
            proxy_port: 1080,
            proxy_username: String::new(),
            proxy_password: String::new(),
            download_parallel: 64,
            verify_files: true,
            game_independent: false,
            theme: "dark".to_string(),
            language: "zh-CN".to_string(),
        }
    }
}

impl AppConfig {
    /// 获取 Minecraft 目录，如果配置为空则使用默认路径
    pub fn effective_minecraft_dir(&self) -> PathBuf {
        if self.minecraft_dir.is_empty() {
            MinecraftPaths::new()
                .map(|p| p.root().to_path_buf())
                .unwrap_or_else(|_| PathBuf::from(".minecraft"))
        } else {
            PathBuf::from(&self.minecraft_dir)
        }
    }

    /// 解析下载源配置
    pub fn effective_download_source(&self) -> DownloadSource {
        self.download_source
            .parse::<DownloadSource>()
            .unwrap_or(DownloadSource::Default)
    }

    /// 构建代理配置（如果启用）
    pub fn effective_proxy(&self) -> Option<ProxyConfig> {
        if self.proxy_enabled && !self.proxy_host.is_empty() {
            let mut proxy = ProxyConfig::new(
                crate::components::http::ProxyType::Http,
                &self.proxy_host,
                self.proxy_port,
            );
            if !self.proxy_username.is_empty() {
                proxy = proxy.with_auth(&self.proxy_username, &self.proxy_password);
            }
            Some(proxy)
        } else {
            None
        }
    }
}

// ============================================================================
//  进度事件
// ============================================================================

/// 进度更新事件，通过 Tauri 事件系统发送给前端
#[derive(Debug, Clone, Serialize)]
pub struct ProgressEvent {
    /// 任务 ID，用于前端区分不同任务
    pub task_id: String,
    /// 当前进度值（已下载字节数 / 已处理文件数）
    pub current: u64,
    /// 总进度值（总字节数 / 总文件数），None 表示未知
    pub total: Option<u64>,
    /// 当前阶段描述（如 "正在下载资源文件..."）
    pub message: String,
    /// 速度（字节/秒）
    pub speed: Option<f64>,
}

/// 任务完成事件
#[derive(Debug, Clone, Serialize)]
pub struct TaskCompleteEvent {
    pub task_id: String,
    pub success: bool,
    pub message: String,
}

// ============================================================================
//  共享应用状态
// ============================================================================

/// Tauri 全局共享状态
pub struct AppState {
    /// 应用配置（支持运行时修改）
    pub config: RwLock<AppConfig>,
    /// 配置文件路径（在初始化时确定）
    pub config_path: PathBuf,
    /// 全局 HTTP 客户端（根据代理配置创建）
    pub http_client: RwLock<HttpClient>,
    /// 全局取消令牌，用于中止所有正在进行的操作
    pub cancel_token: RwLock<Option<CancellationToken>>,
    /// 进度事件发送通道（Tauri 事件通过 AppHandle 发送）
    pub progress_tx: watch::Sender<Option<ProgressEvent>>,
    /// 进度事件接收通道（可用于内部订阅）
    pub progress_rx: watch::Receiver<Option<ProgressEvent>>,
    /// 任务完成事件发送通道
    pub task_complete_tx: watch::Sender<Option<TaskCompleteEvent>>,
    /// 任务完成事件接收通道
    pub task_complete_rx: watch::Receiver<Option<TaskCompleteEvent>>,
    /// 当前活跃的取消令牌
    pub active_cancel_token: RwLock<Option<CancellationToken>>,
}

impl AppState {
    /// 创建新的应用状态
    pub async fn new(config_dir: &Path) -> Self {
        // 确保配置目录存在
        let config_path = config_dir.join(CONFIG_FILE_NAME);
        if let Some(parent) = config_path.parent() {
            let _ = tokio::fs::create_dir_all(parent).await;
        }

        // 加载配置
        let config = Self::load_config(&config_path).await;

        // 创建 HTTP 客户端
        let http_client = Self::build_http_client(&config);

        let (progress_tx, progress_rx) = watch::channel(None);
        let (task_complete_tx, task_complete_rx) = watch::channel(None);

        Self {
            config: RwLock::new(config),
            config_path,
            http_client: RwLock::new(http_client),
            cancel_token: RwLock::new(None),
            progress_tx,
            progress_rx,
            task_complete_tx,
            task_complete_rx,
            active_cancel_token: RwLock::new(None),
        }
    }

    /// 从文件加载配置，如果失败则使用默认配置
    async fn load_config(path: &Path) -> AppConfig {
        match tokio::fs::read_to_string(path).await {
            Ok(content) => {
                match toml::from_str::<AppConfig>(&content) {
                    Ok(config) => {
                        debug!("配置已从 {:?} 加载", path);
                        config
                    }
                    Err(e) => {
                        warn!("解析配置文件失败: {}，将使用默认配置", e);
                        AppConfig::default()
                    }
                }
            }
            Err(_) => {
                debug!("未找到配置文件 {:?}，将使用默认配置", path);
                AppConfig::default()
            }
        }
    }

    /// 构建 HTTP 客户端
    fn build_http_client(config: &AppConfig) -> HttpClient {
        let mut http_config = HttpClientConfig::default();
        if let Some(proxy) = config.effective_proxy() {
            http_config.proxy = Some(proxy);
        }
        HttpClient::new(http_config)
            .unwrap_or_else(|_| HttpClient::default())
    }

    /// 保存配置到文件
    pub async fn save_config(&self) -> Result<(), String> {
        let config = self.config.read().await;
        let content = toml::to_string_pretty(&*config)
            .map_err(|e| format!("序列化配置失败: {}", e))?;
        tokio::fs::write(&self.config_path, content)
            .await
            .map_err(|e| format!("写入配置文件失败: {}", e))?;
        debug!("配置已保存到 {:?}", self.config_path);
        Ok(())
    }

    /// 更新配置并保存
    pub async fn update_config(&self, new_config: AppConfig) -> Result<(), String> {
        // 更新 HTTP 客户端（如果代理配置变化）
        let old_source = {
            let config = self.config.read().await;
            config.download_source.clone()
        };
        let new_source = new_config.download_source.clone();

        let mut config = self.config.write().await;
        *config = new_config;
        drop(config);

        // 如果下载源或代理变化，重建 HTTP 客户端
        if old_source != new_source {
            let config = self.config.read().await;
            let mut http = self.http_client.write().await;
            *http = Self::build_http_client(&config);
        }

        self.save_config().await
    }

    /// 创建新的取消令牌并返回（旧令牌会被取消）
    pub fn create_cancel_token(&self) -> CancellationToken {
        let new_token = CancellationToken::new();
        let mut active = self.active_cancel_token.blocking_write();
        if let Some(old_token) = active.take() {
            old_token.cancel();
        }
        *active = Some(new_token.clone());
        new_token
    }

    /// 取消所有正在进行的操作
    pub fn cancel_all(&self) {
        let mut active = self.active_cancel_token.blocking_write();
        if let Some(token) = active.take() {
            token.cancel();
        }
    }

    /// 发送进度事件
    pub fn send_progress(&self, event: ProgressEvent) {
        let _ = self.progress_tx.send(Some(event));
    }

    /// 发送任务完成事件
    pub fn send_task_complete(&self, event: TaskCompleteEvent) {
        let _ = self.task_complete_tx.send(Some(event));
    }
}
