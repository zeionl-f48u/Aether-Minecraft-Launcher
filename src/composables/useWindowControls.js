/**
 * useWindowControls.js — 窗口控制组合式函数
 * =====================================================
 * 功能：
 *   封装 Tauri 窗口的最小化、最大化/还原、关闭操作。
 *   供 TitleBar.vue 调用，使窗口控制逻辑与视图解耦。
 *
 * 使用方式：
 *   import { useWindowControls } from ".../composables/useWindowControls.js";
 *   const { minimize, toggleMaximize, close } = useWindowControls();
 *
 * 依赖：
 *   @tauri-apps/api/window — Tauri 2 窗口 API
 *   需在 src-tauri/capabilities/default.json 中声明以下权限：
 *   - core:window:allow-minimize
 *   - core:window:allow-toggle-maximize
 *   - core:window:allow-close
 * ===================================================== */

import { getCurrentWindow } from "@tauri-apps/api/window";

/**
 * 导出组合式函数 useWindowControls
 * 返回三个窗口控制方法，均为 async 函数
 */
export function useWindowControls() {
  // 获取当前 Tauri 窗口实例
  const appWindow = getCurrentWindow();

  /**
   * minimize — 最小化窗口
   * 调用 Tauri 窗口 API 将应用最小化到任务栏
   */
  async function minimize() {
    await appWindow.minimize();
  }

  /**
   * toggleMaximize — 切换最大化/还原
   * 如果窗口当前是最大化状态则还原，否则最大化
   */
  async function toggleMaximize() {
    await appWindow.toggleMaximize();
  }

  /**
   * close — 关闭窗口
   * 调用 Tauri 窗口 API 关闭应用
   */
  async function close() {
    await appWindow.close();
  }

  // 暴露方法供组件调用
  return {
    minimize,
    toggleMaximize,
    close,
  };
}
