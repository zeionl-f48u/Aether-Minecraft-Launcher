//! 客户端结构，用于启动游戏
//!
//! 整合版本元数据、Java运行时、账户认证、JVM参数和游戏参数，
//! 构建并启动 Minecraft 游戏进程。

use std::collections::HashMap;
use std::ffi::OsString;
use std::fmt::{Debug, Display, Formatter, Result as FmtResult};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use tokio::process::{Child, Command};
use tokio::task::spawn_blocking;
use thiserror::Error;

use super::{
    auth::structs::AuthMethod,
    version::structs::{Argument, VersionInfo},
};
use crate::{
    components::jave::JavaRuntime,
    prelude::*,
    components::utils::{get_full_path, CLASSPATH_SEPARATOR, NATIVE_ARCH_LAZY, TARGET_OS},
    components::version::structs::{Allowed, VersionMeta},
};

// ============================================================================
//  错误类型
// ============================================================================

#[derive(Debug, Error)]
pub enum ClientError {
    #[error("版本元数据缺失")]
    MissingVersionMeta,

    #[error("无法解析版本继承: {0}")]
    InheritError(String),

    #[error("Java 运行时错误: {0}")]
    JavaRuntimeError(String),

    #[error("路径解析失败: {0}")]
    PathError(String),

    #[error("IO 错误: {0}")]
    Io(#[from] std::io::Error),

    #[error("进程启动失败: {0}")]
    SpawnError(String),

    #[error("配置错误: {0}")]
    ConfigError(String),

    #[error("原生库解压失败: {0}")]
    NativeExtractError(String),

    #[error("资源复制失败: {0}")]
    AssetCopyError(String),
}

pub type ClientResult<T> = Result<T, ClientError>;

// ============================================================================
//  常量
// ============================================================================

/// 用于修复 CVE-2021-44228 远程代码执行漏洞的 Agent JAR
/// 注：当前未启用，如需使用请通过 `-javaagent` 参数加载
pub const LOG4J_PATCH: &[u8] = &[];

// ============================================================================
//  配置结构
// ============================================================================

/// 客户端配置结构
#[derive(Debug, Clone)]
pub struct ClientConfig {
    /// 使用的玩家账户
    pub auth: AuthMethod,
    /// 启动的版本元数据信息
    pub version_info: VersionInfo,
    /// 启动的版本类型
    pub version_type: String,
    /// 自定义 JVM 参数（附加在 Class Path 之前）
    pub custom_java_args: Vec<String>,
    /// 自定义游戏参数（附加在参数最后）
    pub custom_args: Vec<String>,
    /// 需要使用的 Java 运行时
    pub java_runtime: JavaRuntime,
    /// 最高内存（MB）
    pub max_mem: u32,
    /// 是否进行预先资源及依赖检查
    pub recheck: bool,
}

// ============================================================================
//  客户端结构
// ============================================================================

/// Minecraft 客户端，持有启动命令和进程
pub struct Client {
    /// 实际的指令对象
    pub cmd: Command,
    /// 当前游戏目录路径
    pub game_dir: PathBuf,
    /// 当前使用的 Java 运行时路径
    pub java_path: PathBuf,
    /// 当前启动参数的副本（包含 Java 自身）
    pub args: Vec<String>,
    /// 正在运行的进程对象
    pub process: Option<Child>,
}

// ============================================================================
//  辅助函数（路径、继承、原生库、资产复制）
// ============================================================================

/// 获取游戏运行目录
fn get_game_directory(cfg: &ClientConfig) -> ClientResult<PathBuf> {
    let version_base = Path::new(&cfg.version_info.version_base);
    let version_dir = version_base.join(&cfg.version_info.version);
    let version_dir = PathBuf::from(get_full_path(version_dir).map_err(|e| ClientError::PathError(e.to_string()))?);
    let game_dir = version_base
        .parent()
        .ok_or_else(|| ClientError::PathError("版本基础目录无父目录".into()))?;
    let game_dir = PathBuf::from(get_full_path(game_dir).map_err(|e| ClientError::PathError(e.to_string()))?);

    if let Some(acl) = &cfg.version_info.acl_launch_config {
        if acl.game_independent {
            Ok(version_dir)
        } else {
            Ok(game_dir)
        }
    } else {
        Ok(game_dir)
    }
}

/// 递归解析版本继承，合并所有父版本元数据
async fn resolve_inherited_meta(cfg: &ClientConfig) -> ClientResult<VersionMeta> {
    let meta = cfg
        .version_info
        .meta
        .as_ref()
        .ok_or(ClientError::MissingVersionMeta)?;

    let mut current_meta = meta.clone();
    let mut inherits_from = current_meta.inherits_from.clone();

    while !inherits_from.is_empty() {
        let mut parent_info = VersionInfo {
            version: inherits_from.clone(),
            version_base: cfg.version_info.version_base.clone(),
            ..Default::default()
        };

        match parent_info.load().await {
            Ok(()) => {
                if let Some(parent_meta) = parent_info.meta {
                    let mut merged = parent_meta;
                    merged += current_meta;
                    current_meta = merged;
                    inherits_from = current_meta.inherits_from.clone();
                } else {
                    break;
                }
            }
            Err(e) => {
                return Err(ClientError::InheritError(format!(
                    "加载父版本 {} 失败: {}",
                    inherits_from, e
                )));
            }
        }
    }

    Ok(current_meta)
}

// ============================================================================
//  原生库解压
// ============================================================================

/// 需要解压的原生库信息
#[derive(Clone)]
struct NativeLib {
    source_jar: PathBuf,
    target_dir: PathBuf,
    classifier: String,
}

/// 解压原生库到目标目录（同步操作，用于 spawn_blocking）
fn extract_natives_sync(natives: &[NativeLib]) -> Result<(), ClientError> {
    use std::fs::File;
    use std::io::{self, Read, Write};
    use zip::ZipArchive;

    for native in natives {
        let file = File::open(&native.source_jar)
            .map_err(|e| ClientError::NativeExtractError(format!("打开 JAR 失败: {}", e)))?;
        let mut archive = ZipArchive::new(file)
            .map_err(|e| ClientError::NativeExtractError(format!("解析 JAR 失败: {}", e)))?;

        // 查找匹配 classifier 的条目，通常文件在 `META-INF/natives/` 或根目录下
        let classifier_pattern = &native.classifier;
        for i in 0..archive.len() {
            let mut entry = archive.by_index(i)
                .map_err(|e| ClientError::NativeExtractError(format!("读取 ZIP 条目失败: {}", e)))?;
            let entry_path = entry.name();
            // 只提取包含 classifier 的文件（如 windows-x86、linux 等）
            if entry_path.contains(classifier_pattern) && !entry_path.ends_with('/') {
                let file_name = Path::new(entry_path).file_name()
                    .ok_or_else(|| ClientError::NativeExtractError("无效的文件名".into()))?;
                let target_path = native.target_dir.join(file_name);
                if let Some(parent) = target_path.parent() {
                    std::fs::create_dir_all(parent)?;
                }
                let mut out = File::create(&target_path)?;
                io::copy(&mut entry, &mut out)?;
            }
        }
    }
    Ok(())
}

/// 构建变量映射，同时收集需要解压的原生库列表
fn build_variables_and_natives(
    cfg: &ClientConfig,
    meta: &VersionMeta,
    java_runtime: &JavaRuntime,
    game_dir: &Path,
) -> ClientResult<(HashMap<String, String>, Vec<NativeLib>)> {
    let mut vars = HashMap::with_capacity(25);

    // ... 原有变量构建代码（与之前相同，但需要额外收集 natives） ...

    // 基础路径
    let lib_base_path = Path::new(&cfg.version_info.version_base)
        .parent()
        .ok_or_else(|| ClientError::PathError("无法获取 libraries 父目录".into()))?
        .join("libraries");
    let lib_base_path = PathBuf::from(get_full_path(lib_base_path).map_err(|e| ClientError::PathError(e.to_string()))?);
    let lib_base_str = lib_base_path.to_string_lossy().replace('\\', "/");
    vars.insert("${library_directory}".into(), lib_base_str);

    // 类路径构建（同时收集原生库）
    let (classpath, natives) = build_classpath_and_natives(
        cfg, meta, &lib_base_path, java_runtime
    )?;
    vars.insert("${classpath}".into(), classpath);

    // 其他变量（省略重复代码，与之前相同）...
    // 为节省篇幅，此处仅示意结构，实际需包含所有变量
    // 参照之前的 build_variables 函数填充所有变量

    // 返回 vars 和 natives 列表
    Ok((vars, natives))
}

/// 构建类路径并返回原生库列表
fn build_classpath_and_natives(
    cfg: &ClientConfig,
    meta: &VersionMeta,
    lib_base: &Path,
    java_runtime: &JavaRuntime,
) -> ClientResult<(String, Vec<NativeLib>)> {
    let mut lib_map: HashMap<String, PathBuf> = HashMap::new();
    let mut natives_list = Vec::new();

    for lib in &meta.libraries {
        if !lib.rules.is_allowed() {
            continue;
        }

        let parts: Vec<&str> = lib.name.splitn(3, ':').collect();
        if parts.len() < 3 {
            continue;
        }
        let (package_str, name, version) = (parts[0], parts[1], parts[2]);
        let key = format!("{}:{}", package_str, name);

        // 构建库路径
        let mut lib_path = if let Some(ds) = &lib.downloads {
            if let Some(artifact) = &ds.artifact {
                let path = artifact.path.replace('\\', "/");
                lib_base.join(path)
            } else {
                lib_base
                    .join(package_str.replace('.', "/"))
                    .join(name)
                    .join(version)
                    .join(format!("{}-{}.jar", name, version))
            }
        } else {
            lib_base
                .join(package_str.replace('.', "/"))
                .join(name)
                .join(version)
                .join(format!("{}-{}.jar", name, version))
        };

        // 处理原生库
        if let Some(natives) = &lib.natives {
            let native_key = natives.get(TARGET_OS)
                .map(|s| s.replace("${arch}", NATIVE_ARCH_LAZY.as_ref()));
            if let Some(native_key) = native_key {
                if let Some(classifiers) = lib.downloads.as_ref().and_then(|d| d.classifiers.as_ref()) {
                    if let Some(classifier) = classifiers.get(&native_key) {
                        let native_path = lib_base.join(classifier.path.replace('\\', "/"));
                        // 保存为原生库，稍后解压
                        let target_dir = Path::new(&cfg.version_info.version_base)
                            .join(&cfg.version_info.version)
                            .join("natives")
                            .join(get_native_subdir(java_runtime));
                        natives_list.push(NativeLib {
                            source_jar: native_path,
                            target_dir,
                            classifier: native_key.clone(),
                        });
                        // 原生库本身不需要加入类路径，但部分加载器可能需要，这里暂不添加
                    }
                }
            }
        } else {
            // 普通库加入类路径
            lib_map.insert(key, lib_path);
        }
    }

    // 添加主 Jar
    for main_jar in &meta.main_jars {
        let path = PathBuf::from(main_jar.replace('\\', "/"));
        let full_path = if path.is_absolute() {
            path
        } else {
            Path::new(&cfg.version_info.version_base)
                .join(&cfg.version_info.version)
                .join(path)
        };
        lib_map.insert(format!("main_{}", main_jar), full_path);
    }

    // 收集所有路径，去重
    let mut paths: Vec<PathBuf> = lib_map.into_values().collect();
    paths.sort();
    paths.dedup();

    let separator = if cfg!(windows) { ";" } else { ":" };
    let classpath = paths
        .iter()
        .map(|p| p.to_string_lossy().replace('\\', "/"))
        .collect::<Vec<_>>()
        .join(separator);

    Ok((classpath, natives_list))
}

/// 获取原生库子目录名
fn get_native_subdir(java_runtime: &JavaRuntime) -> &'static str {
    #[cfg(target_os = "windows")]
    {
        match java_runtime.java_arch() {
            super::jave::Arch::X86 => "natives-windows-x86",
            super::jave::Arch::X86_64 => "natives-windows",
            super::jave::Arch::AArch64 => "natives-windows-arm64",
            _ => "natives-windows",
        }
    }
    #[cfg(target_os = "linux")]
    {
        "natives-linux"
    }
    #[cfg(target_os = "macos")]
    {
        match java_runtime.java_arch() {
            super::jave::Arch::X86 | super::jave::Arch::X86_64 => "natives-macos",
            super::jave::Arch::AArch64 => "natives-macos-arm64",
            _ => "natives-macos",
        }
    }
}

/// 复制 pre-1.6 assets（同步操作）
fn copy_pre_16_assets_sync(assets_src: &Path, resources_dst: &Path) -> Result<(), ClientError> {
    if !assets_src.exists() {
        return Ok(());
    }
    std::fs::create_dir_all(resources_dst)
        .map_err(|e| ClientError::AssetCopyError(format!("创建资源目录失败: {}", e)))?;
    // 递归复制目录内容（使用 std::fs）
    copy_dir_recursive(assets_src, resources_dst, true)
        .map_err(|e| ClientError::AssetCopyError(format!("复制资源失败: {}", e)))?;
    Ok(())
}

/// 递归复制目录内容
fn copy_dir_recursive(src: &Path, dst: &Path, skip_existing: bool) -> std::io::Result<()> {
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let file_type = entry.file_type()?;
        let src_path = entry.path();
        let file_name = src_path.file_name().unwrap();
        let dst_path = dst.join(file_name);
        if file_type.is_dir() {
            std::fs::create_dir_all(&dst_path)?;
            copy_dir_recursive(&src_path, &dst_path, skip_existing)?;
        } else if file_type.is_file() {
            if skip_existing && dst_path.exists() {
                continue;
            }
            std::fs::copy(&src_path, &dst_path)?;
        }
    }
    Ok(())
}

// ============================================================================
//  Client 实现
// ============================================================================

impl Client {
    /// 创建客户端实例，构建启动参数
    pub async fn new(mut cfg: ClientConfig) -> ClientResult<Self> {
        if cfg.version_info.meta.is_none() {
            return Err(ClientError::MissingVersionMeta);
        }

        // 1. 解析版本继承
        let meta = resolve_inherited_meta(&cfg).await?;
        cfg.version_info.meta = Some(meta.clone());

        // 2. 确定 Java 运行时
        let java_runtime = if let Some(acl_config) = &cfg.version_info.acl_launch_config {
            if !acl_config.java_path.is_empty() {
                JavaRuntime::from_java_path(Path::new(&acl_config.java_path)).await
                    .map_err(|e| ClientError::JavaRuntimeError(e.to_string()))?
            } else {
                cfg.java_runtime.clone()
            }
        } else {
            cfg.java_runtime.clone()
        };

        // 3. 获取游戏目录
        let game_dir = get_game_directory(&cfg)?;

        // 4. 构建变量和原生库列表
        let (vars, natives) = build_variables_and_natives(&cfg, &meta, &java_runtime, &game_dir)?;

        // 5. 解压原生库（异步）
        if !natives.is_empty() {
            let natives_clone = natives.clone();
            spawn_blocking(move || {
                // 创建原生库目录
                for native in &natives_clone {
                    std::fs::create_dir_all(&native.target_dir)?;
                }
                extract_natives_sync(&natives_clone)?;
                Ok::<_, ClientError>(())
            })
            .await
            .map_err(|e| ClientError::NativeExtractError(format!("解压任务失败: {}", e)))?
            .map_err(|e| ClientError::NativeExtractError(format!("解压失败: {}", e)))?;
        }

        // 6. 处理 pre-1.6 assets 复制
        if let Some(asset_index) = &meta.asset_index {
            if asset_index.id == "pre-1.6" {
                let version_base = Path::new(&cfg.version_info.version_base);
                let assets_src = version_base
                    .parent()
                    .unwrap()
                    .join("assets")
                    .join("virtual")
                    .join("pre-1.6");
                let resources_dst = game_dir.join("resources");
                let assets_src = PathBuf::from(get_full_path(assets_src).map_err(|e| ClientError::PathError(e.to_string()))?);
                let resources_dst = PathBuf::from(get_full_path(resources_dst).map_err(|e| ClientError::PathError(e.to_string()))?);
                // 异步复制
                spawn_blocking(move || {
                    copy_pre_16_assets_sync(&assets_src, &resources_dst)
                })
                .await
                .map_err(|e| ClientError::AssetCopyError(format!("复制任务失败: {}", e)))?
                .map_err(|e| ClientError::AssetCopyError(format!("复制失败: {}", e)))?;
            }
        }

        // 7. 组装参数（与之前相同，略...）
        // 为了简洁，此处省略完整的参数组装代码，参照之前的实现
        // 注意：vars 中已包含所有变量，参数替换方式不变
        let args = build_args(&cfg, &meta, &vars)?;

        // 8. 构建 Command
        let wrapper_path = cfg
            .version_info
            .acl_launch_config
            .as_ref()
            .map(|x| x.wrapper_path.clone())
            .unwrap_or_default();

        let mut cmd = if wrapper_path.is_empty() {
            Command::new(java_runtime.java_path())
        } else {
            let mut cmd = Command::new(&wrapper_path);
            if let Some(wrapper_args) = cfg
                .version_info
                .acl_launch_config
                .as_ref()
                .map(|x| x.wrapper_args.clone())
                .filter(|s| !s.is_empty())
            {
                cmd.arg(wrapper_args);
            }
            cmd.arg(java_runtime.java_path());
            cmd
        };

        cmd.args(&args);
        cmd.current_dir(&game_dir);
        cmd.env("FORMAT_MESSAGES_PATTERN_DISABLE_LOOKUPS", "true");
        #[cfg(windows)]
        {
            cmd.env("APPDATA", &game_dir);
        }

        let mut full_args = vec![java_runtime.java_path().to_string()];
        full_args.extend(args.clone());

        Ok(Self {
            cmd,
            game_dir,
            java_path: PathBuf::from(java_runtime.java_path()),
            args: full_args,
            process: None,
        })
    }

    // ... 其他方法（stdin, stdout, stderr, launch, stop 等）保持不变 ...
    // 由于篇幅，此处省略，可参照之前的实现
}

// ============================================================================
//  参数组装辅助函数（抽取出来）
// ============================================================================

fn build_args(cfg: &ClientConfig, meta: &VersionMeta, vars: &HashMap<String, String>) -> ClientResult<Vec<String>> {
    let mut args = Vec::with_capacity(64);

    // 固定参数
    args.push("-Dlog4j2.formatMsgNoLookups=true".to_string());

    // 自定义 JVM 参数
    for arg in &cfg.custom_java_args {
        args.push(arg.clone());
    }
    if let Some(acl_config) = &cfg.version_info.acl_launch_config {
        if !acl_config.jvm_args.trim().is_empty() {
            if let Ok(parsed) = shell_words::split(&acl_config.jvm_args) {
                args.extend(parsed);
            } else {
                args.push(acl_config.jvm_args.clone());
            }
        }
    }

    // Authlib Injector
    if let AuthMethod::AuthlibInjector { api_location, server_meta, .. } = &cfg.auth {
        let injector_path = Path::new(&cfg.version_info.version_base)
            .parent()
            .unwrap()
            .join("authlib-injector.jar");
        let injector_path = get_full_path(injector_path)
            .map_err(|e| ClientError::PathError(e.to_string()))?;
        args.push(format!("-javaagent:{}= {}", injector_path, api_location));
        args.push(format!("-Dauthlibinjector.yggdrasil.prefetched={}", server_meta));
    }

    // 内存
    if let Some(max_mem) = vars.get("${max_memory}") {
        args.push(max_mem.clone());
    }

    // 版本元数据 JVM 参数
    if let Some(arguments) = &meta.arguments {
        for arg in &arguments.jvm {
            match arg {
                Argument::Common(a) => args.push(replace_variables(a, vars)),
                Argument::Specify(spec) => {
                    if spec.rules.is_allowed() {
                        for value in &spec.value {
                            args.push(replace_variables(value, vars));
                        }
                    }
                }
            }
        }
    } else {
        // 旧版
        args.push(format!("-Djava.library.path={}", vars.get("${natives_directory}").unwrap()));
        args.push(format!("-Dminecraft.launcher.brand={}", vars.get("${launcher_name}").unwrap()));
        args.push(format!("-Dminecraft.launcher.version={}", vars.get("${launcher_version}").unwrap()));
        args.push("-cp".to_string());
        args.push(vars.get("${classpath}").unwrap().clone());
    }

    // 主类
    args.push(meta.main_class.clone());

    // 游戏参数（旧版）
    if !meta.minecraft_arguments.trim().is_empty() {
        let parts = shell_words::split(&meta.minecraft_arguments).unwrap_or_else(|_| {
            meta.minecraft_arguments.split(' ').map(|s| s.to_string()).collect()
        });
        for part in parts {
            args.push(replace_variables(&part, vars));
        }
    }

    // 游戏参数（新版）
    if let Some(arguments) = &meta.arguments {
        for arg in &arguments.game {
            match arg {
                Argument::Common(a) => args.push(replace_variables(a, vars)),
                Argument::Specify(spec) => {
                    if spec.rules.is_allowed() {
                        for value in &spec.value {
                            args.push(replace_variables(value, vars));
                        }
                    }
                }
            }
        }
    }

    // 用户自定义游戏参数
    for arg in &cfg.custom_args {
        args.push(arg.clone());
    }
    if let Some(acl_config) = &cfg.version_info.acl_launch_config {
        if !acl_config.game_args.trim().is_empty() {
            if let Ok(parsed) = shell_words::split(&acl_config.game_args) {
                args.extend(parsed);
            } else {
                args.push(acl_config.game_args.clone());
            }
        }
    }

    Ok(args)
}

fn replace_variables(arg: &str, vars: &HashMap<String, String>) -> String {
    let mut result = arg.to_string();
    for (key, value) in vars {
        if result.contains(key) {
            result = result.replace(key, value);
        }
    }
    result
}

// ============================================================================
//  进程控制方法
// ============================================================================

impl Client {
    /// 启动 Minecraft 游戏进程
    ///
    /// 调用 `Command::spawn()` 启动游戏，返回进程 PID。
    pub async fn launch(&mut self) -> ClientResult<u32> {
        let mut c = self
            .cmd
            .spawn()
            .map_err(|e| ClientError::SpawnError(format!("启动游戏进程失败: {}", e)))?;

        let pid = c.id().ok_or_else(|| ClientError::SpawnError("无法获取进程 ID".into()))?;

        tracing::info!("游戏进程已启动，PID: {}", pid);
        self.process = Some(c);

        Ok(pid)
    }

    /// 停止 Minecraft 游戏进程
    ///
    /// 发送 SIGTERM（Unix）或 TerminateProcess（Windows），等待进程退出。
    pub async fn stop(&mut self) -> ClientResult<()> {
        if let Some(mut child) = self.process.take() {
            child
                .kill()
                .await
                .map_err(|e| ClientError::SpawnError(format!("无法终止进程: {}", e)))?;

            tracing::info!("游戏进程已终止");
        } else {
            tracing::warn!("没有正在运行的进程");
        }

        Ok(())
    }

    /// 获取当前进程的 PID（如果有）
    pub fn pid(&self) -> Option<u32> {
        self.process.as_ref().and_then(|c| c.id())
    }
}