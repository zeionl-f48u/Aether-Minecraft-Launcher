/**
 * useTauriApi.js — Tauri 后端命令调用封装
 * =====================================================
 * 功能：
 *   将后端 Rust 命令封装为前端可调用的异步函数。
 *   所有调用通过 @tauri-apps/api/core 的 invoke 完成。
 *
 * 对应后端文件：src-tauri/src/commands.rs
 *
 * 使用方式：
 *   import { api } from "../composables/useTauriApi.js";
 *   const config = await api.getConfig();
 * ===================================================== */

import { invoke } from "@tauri-apps/api/core";

// =========================================================================
//  API 调用封装
// =========================================================================

async function call(cmd, args = {}) {
  try {
    return await invoke(cmd, args);
  } catch (err) {
    console.error(`[Tauri] ${cmd} 失败:`, err);
    throw err;
  }
}

// =========================================================================
//  配置管理
// =========================================================================

async function getConfig() { return call("get_config"); }
async function updateConfig(config) { return call("update_config", { newConfig: config }); }
async function resetConfig() { return call("reset_config"); }

// =========================================================================
//  Java 运行时
// =========================================================================

async function searchJava() { return call("search_java"); }
async function getJavaInfo(javaPath) { return call("get_java_info", { javaPath }); }

// =========================================================================
//  版本管理
// =========================================================================

async function fetchVersionManifest() { return call("fetch_version_manifest"); }
async function getLocalVersions() { return call("get_local_versions"); }
async function getVersionDetail(versionName) { return call("get_version_detail", { versionName }); }

// =========================================================================
//  下载安装
// =========================================================================

async function installVanilla(versionId) { return call("install_vanilla", { versionId }); }
async function installFabric({ versionName, vanillaVersion, loaderVersion }) {
  return call("install_fabric", { versionName, vanillaVersion, loaderVersion });
}
async function installForge({ versionName, vanillaVersion, forgeVersion }) {
  return call("install_forge", { versionName, vanillaVersion, forgeVersion });
}
async function installNeoForge({ versionName, vanillaVersion, neoforgeVersion }) {
  return call("install_neoforge", { versionName, vanillaVersion, neoforgeVersion });
}
async function installOptiFine({ versionName, vanillaVersion, optifineType, optifinePatch, asMod }) {
  return call("install_optifine", { versionName, vanillaVersion, optifineType, optifinePatch, asMod });
}
async function installQuiltMC({ versionName, vanillaVersion, loaderVersion }) {
  return call("install_quiltmc", { versionName, vanillaVersion, loaderVersion });
}
async function installGame({ versionName, vanillaVersion, fabric, forge, neoforge, optifine }) {
  return call("install_game", { versionName, vanillaVersion, fabric, forge, neoforge, optifine });
}

// =========================================================================
//  获取加载器版本列表
// =========================================================================

async function getFabricLoaders(vanillaVersion) {
  return call("get_fabric_loaders", { vanillaVersion });
}
async function getForgeVersions(vanillaVersion) {
  return call("get_forge_versions", { vanillaVersion });
}
async function getNeoForgeVersions(vanillaVersion) {
  return call("get_neoforge_versions", { vanillaVersion });
}
async function getOptiFineVersions(vanillaVersion) {
  return call("get_optifine_versions", { vanillaVersion });
}
async function getQuiltMCLoaders(vanillaVersion) {
  return call("get_quiltmc_loaders", { vanillaVersion });
}

// =========================================================================
//  模组市场
// =========================================================================

async function searchModrinth({ query, offset, limit }) {
  return call("search_modrinth", { query, offset, limit });
}
async function getModrinthVersions(modId) {
  return call("get_modrinth_versions", { modId });
}
async function searchCurseForge({ query, gameVersion, offset, limit }) {
  return call("search_curseforge", { query, gameVersion, offset, limit });
}

// =========================================================================
//  模组管理
// =========================================================================

async function listMods(versionName) { return call("list_mods", { versionName }); }
async function toggleMod(fileName, enabled) { return call("toggle_mod", { fileName, enabled }); }
async function deleteMod(fileName) { return call("delete_mod", { fileName }); }

// =========================================================================
//  游戏启动
// =========================================================================

async function launchGame({ authJson, versionName, maxMem, customJavaArgs, customGameArgs }) {
  return call("launch_game", { authJson, versionName, maxMem, customJavaArgs, customGameArgs });
}
async function startGameProcess({ authJson, versionName, maxMem, customJavaArgs, customGameArgs }) {
  return call("start_game_process", { authJson, versionName, maxMem, customJavaArgs, customGameArgs });
}

// =========================================================================
//  账户认证
// =========================================================================

async function offlineLogin(playerName) { return call("offline_login", { playerName }); }
async function microsoftDeviceLoginStart() { return call("microsoft_device_login_start"); }
async function microsoftDeviceLoginPoll(deviceCode, interval) {
  return call("microsoft_device_login_poll", { deviceCode, interval });
}
async function microsoftDeviceLoginComplete(accessToken, refreshToken) {
  return call("microsoft_device_login_complete", { accessToken, refreshToken });
}
async function microsoftLogin(code) { return call("microsoft_login", { code }); }
async function microsoftRefreshToken(authMethodJson) {
  return call("microsoft_refresh_token", { authMethodJson });
}
async function authlibLogin(apiLocation, username, password) {
  return call("authlib_login", { apiLocation, username, password });
}
async function authlibRefreshToken(authMethodJson, clientToken) {
  return call("authlib_refresh_token", { authMethodJson, clientToken });
}
async function validateToken(authMethodJson) {
  return call("validate_token", { authMethodJson });
}

// =========================================================================
//  工具
// =========================================================================

async function getLogs(lines = 100) { return call("get_logs", { lines }); }
async function getSystemMemory() { return call("get_system_memory"); }
async function getMinecraftDirSize() { return call("get_minecraft_dir_size"); }
async function cancelOperation() { return call("cancel_operation"); }

// =========================================================================
//  导出 API 对象
// =========================================================================

export const api = {
  getConfig, updateConfig, resetConfig,
  searchJava, getJavaInfo,
  fetchVersionManifest, getLocalVersions, getVersionDetail,
  installVanilla, installFabric, installForge,
  installNeoForge, installOptiFine, installQuiltMC, installGame,
  getFabricLoaders, getForgeVersions, getNeoForgeVersions, getOptiFineVersions, getQuiltMCLoaders,
  searchModrinth, getModrinthVersions, searchCurseForge,
  listMods, toggleMod, deleteMod,
  launchGame, startGameProcess,
  offlineLogin,
  microsoftDeviceLoginStart, microsoftDeviceLoginPoll, microsoftDeviceLoginComplete,
  microsoftLogin, microsoftRefreshToken,
  authlibLogin, authlibRefreshToken, validateToken,
  getLogs, getSystemMemory, getMinecraftDirSize, cancelOperation,
};
