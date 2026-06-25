// ============================================================================
//  模块声明
// ============================================================================

use tauri::Manager;

/// 核心功能模块（Minecraft 启动器逻辑）
pub mod components;

/// 预导入模块，提供常用的类型和 trait 别名
pub mod prelude;

/// 全局配置和状态管理
pub mod state;

/// Tauri 命令定义
pub mod commands;

/// 错误映射工具
pub mod error;

// ============================================================================
//  应用入口
// ============================================================================

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        // 注册全局状态（在 setup 中初始化）
        .setup(|app| {
            // 获取应用配置目录
            let config_dir = app
                .path()
                .app_config_dir()
                .expect("无法获取应用配置目录");

            // 初始化全局状态
            let app_state = tauri::async_runtime::block_on(state::AppState::new(&config_dir));
            app.manage(app_state);

            // 设置事件监听（日志等）
            commands::setup_event_listeners(&app.handle());

            Ok(())
        })
        // 注册所有 Tauri 命令
        .invoke_handler(tauri::generate_handler![
            // 配置管理
            commands::get_config,
            commands::update_config,
            commands::reset_config,
            // Java 运行时
            commands::search_java,
            commands::get_java_info,
            // 版本管理
            commands::fetch_version_manifest,
            commands::get_local_versions,
            commands::get_version_detail,
            // 下载安装
            commands::install_vanilla,
            commands::install_fabric,
            commands::install_forge,
            commands::install_game,
            // 账户认证
            commands::offline_login,
            commands::microsoft_device_login_start,
            commands::microsoft_device_login_poll,
            commands::microsoft_device_login_complete,
            commands::microsoft_login,
            commands::microsoft_refresh_token,
            commands::authlib_login,
            commands::authlib_refresh_token,
            commands::validate_token,
            // 游戏启动
            commands::launch_game,
            // 模组管理
            commands::list_mods,
            commands::toggle_mod,
            commands::delete_mod,
            // 系统工具
            commands::get_system_memory,
            commands::get_minecraft_dir_size,
            commands::get_logs,
            commands::cancel_operation,
            // 额外加载器安装
            commands::install_neoforge,
            commands::install_optifine,
            commands::install_quiltmc,
            // 获取加载器版本列表
            commands::get_fabric_loaders,
            commands::get_forge_versions,
            commands::get_neoforge_versions,
            commands::get_optifine_versions,
            commands::get_quiltmc_loaders,
            // 模组市场
            commands::search_modrinth,
            commands::get_modrinth_versions,
            commands::search_curseforge,
            // 游戏启动进程
            commands::start_game_process,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
