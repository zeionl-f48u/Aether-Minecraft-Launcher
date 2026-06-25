/**
 * useTheme.js — 主题管理组合式函数
 * =====================================================
 * 功能：
 *   提供深色/浅色/跟随系统三种主题模式
 *   使用 CSS 自定义属性实现 Win11 经典配色
 *   将主题偏好持久化到 localStorage
 *
 * 使用方式：
 *   import { useTheme } from "../composables/useTheme.js";
 *   const { themeMode, currentTheme, setTheme, toggleTheme } = useTheme();
 *
 * 主题模式：
 *   "dark"    — 强制深色模式（Win11 深色经典）
 *   "light"   — 强制浅色模式（Win11 浅色经典）
 *   "system"  — 跟随操作系统设置
 * ===================================================== */

import { ref, watch, onMounted, computed } from "vue";

/** localStorage 存储键名 */
const STORAGE_KEY = "aether-theme-mode";

/**
 * 导出组合式函数
 * 可在任意组件中调用，共享同一个响应式状态（模块级单例）
 */
const themeMode = ref(localStorage.getItem(STORAGE_KEY) || "dark");

/** 实际生效的主题（"dark" 或 "light"） */
const currentTheme = ref("dark");

/**
 * 检测系统深色模式偏好
 * 使用 window.matchMedia 监听 prefers-color-scheme
 */
function getSystemTheme() {
  if (typeof window === "undefined") return "dark";
  return window.matchMedia("(prefers-color-scheme: dark)").matches
    ? "dark"
    : "light";
}

/**
 * 解析当前应生效的主题
 */
function resolveTheme(mode) {
  if (mode === "system") return getSystemTheme();
  return mode;
}

/**
 * 应用主题到 document.documentElement
 * 设置 data-theme 属性和 CSS 变量的降级兼容
 */
function applyTheme(theme) {
  const root = document.documentElement;
  root.setAttribute("data-theme", theme);

  // 同时也设置 class 以便 Tailwind dark: 前缀使用
  if (theme === "dark") {
    root.classList.add("dark");
  } else {
    root.classList.remove("dark");
  }

  // 保存到 localStorage
  localStorage.setItem(STORAGE_KEY, themeMode.value);
}

/**
 * 设置主题模式
 * @param {"dark"|"light"|"system"} mode
 */
export function setTheme(mode) {
  themeMode.value = mode;
  currentTheme.value = resolveTheme(mode);
  applyTheme(currentTheme.value);
}

/**
 * 切换深色/浅色（仅切换强制模式，不影响 system）
 */
export function toggleTheme() {
  const next = currentTheme.value === "dark" ? "light" : "dark";
  themeMode.value = next;
  currentTheme.value = next;
  applyTheme(next);
}

/**
 * 初始化主题
 */
export function initTheme() {
  currentTheme.value = resolveTheme(themeMode.value);
  applyTheme(currentTheme.value);
}

/**
 * 监听系统主题变化（仅当 mode === "system" 时生效）
 */
let mediaQuery = null;
function watchSystemTheme() {
  if (typeof window === "undefined") return;
  mediaQuery = window.matchMedia("(prefers-color-scheme: dark)");
  const handler = () => {
    if (themeMode.value === "system") {
      currentTheme.value = getSystemTheme();
      applyTheme(currentTheme.value);
    }
  };
  mediaQuery.addEventListener("change", handler);
}

// 模块级单例导出
export function useTheme() {
  onMounted(() => {
    initTheme();
    watchSystemTheme();
  });

  // 监听 themeMode 变化
  watch(themeMode, (mode) => {
    currentTheme.value = resolveTheme(mode);
    applyTheme(currentTheme.value);
  });

  return {
    /** 当前选择的模式："dark" | "light" | "system" */
    themeMode,
    /** 当前生效的主题："dark" | "light" */
    currentTheme,
    /** 是否为深色模式 */
    isDark: computed(() => currentTheme.value === "dark"),
    /** 设置主题模式 */
    setTheme,
    /** 切换深色/浅色 */
    toggleTheme,
  };
}
