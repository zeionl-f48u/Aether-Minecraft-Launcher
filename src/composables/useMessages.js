/**
 * useMessages.js — 消息通知 API 接口组合式函数
 * =====================================================
 * 功能：
 *   提供统一的消息通知数据接口。
 *   优先调用 Tauri 后端命令获取消息，
 *   后端不可用时回退到模拟数据以确保开发体验。
 *
 * 接口设计（模块化）：
 *   fetchMessages()   — 拉取消息列表
 *   markAsRead(id)    — 标记单条消息为已读
 *   markAllRead()     — 标记全部为已读
 *   dismissMessage(id)— 移除单条消息
 *
 * 使用方式：
 *   import { useMessages } from "../composables/useMessages.js";
 *   const { messages, loading, fetchMessages, markAsRead } = useMessages();
 * ===================================================== */

import { ref, computed } from "vue";

// =========================================================================
//  消息数据结构（与后端约定接口）
// =========================================================================

/**
 * @typedef {Object} MessageItem
 * @property {string}  id          — 消息唯一标识
 * @property {string}  type        — 类型："info" | "success" | "warning" | "error"
 * @property {string}  title       — 消息标题
 * @property {string}  body        — 消息正文
 * @property {string}  timestamp   — ISO 8601 时间戳
 * @property {boolean} read        — 是否已读
 * @property {string}  [action]    — 可选的操作按钮文字
 */

// =========================================================================
//  模拟数据（后端不可用时的降级方案）
// =========================================================================

/** 模拟消息列表 */
const MOCK_MESSAGES = [
  {
    id: "1",
    type: "info",
    title: "欢迎使用 Aether Launcher",
    body: "感谢选择 Aether Minecraft Launcher！你可以通过左侧导航浏览版本、管理模组和配置启动器设置。",
    timestamp: new Date().toISOString(),
    read: false,
    action: "查看指南",
  },
  {
    id: "2",
    type: "success",
    title: "Java 运行时检测完成",
    body: "系统已自动检测到 Java 17.0.9，位于 C:\\Program Files\\Java\\jdk-17\\bin\\java.exe",
    timestamp: new Date(Date.now() - 3600000).toISOString(),
    read: false,
  },
  {
    id: "3",
    type: "warning",
    title: "Minecraft 目录未设置",
    body: "尚未设置 .minecraft 目录路径，部分功能可能受限。请在设置中指定游戏目录。",
    timestamp: new Date(Date.now() - 7200000).toISOString(),
    read: false,
    action: "前往设置",
  },
  {
    id: "4",
    type: "error",
    title: "网络连接异常",
    body: "无法连接到 Mojang 版本清单服务，版本列表可能不完整。请检查网络连接后重试。",
    timestamp: new Date(Date.now() - 86400000).toISOString(),
    read: true,
    action: "重试",
  },
  {
    id: "5",
    type: "info",
    title: "新版本可用：Fabric 0.16.0",
    body: "Fabric Loader 0.16.0 已发布，支持 Minecraft 1.21.4。可在版本管理中安装。",
    timestamp: new Date(Date.now() - 172800000).toISOString(),
    read: true,
  },
];

// =========================================================================
//  消息 API 接口（模块化单例）
// =========================================================================

/** 响应式消息列表 */
const messages = ref([]);
/** 加载状态 */
const loading = ref(false);
/** 错误信息 */
const error = ref(null);

/** 未读消息数（计算属性） */
const unreadCount = computed(() => messages.value.filter((m) => !m.read).length);

/**
 * 尝试调用 Tauri 后端获取消息
 * 如果后端命令不可用，回退到模拟数据
 * @returns {Promise<Array>} 消息列表
 */
async function fetchFromBackend() {
  let messages = null;

  try {
    // 动态导入 Tauri invoke，避免前端构建时报错
    const { invoke } = await import("@tauri-apps/api/core");

    // 调用后端的 get_logs 命令（或未来专用的 get_messages 命令）
    // 后端返回格式：string[]（日志行）
    // 我们将其转换为 MessageItem 格式
    const logs = await invoke("get_logs", { lines: 50 });
    if (Array.isArray(logs) && logs.length > 0) {
      // 将日志行转换为消息格式
      messages = logs.slice(0, 10).map((line, index) => ({
        id: `log-${Date.now()}-${index}`,
        type: line.toLowerCase().includes("error") ? "error"
             : line.toLowerCase().includes("warn") ? "warning"
             : line.toLowerCase().includes("success") ? "success"
             : "info",
        title: `日志 #${index + 1}`,
        body: line,
        timestamp: new Date().toISOString(),
        read: false,
      }));
    }
  } catch {
    // 后端不可用，返回 null 表示使用模拟数据
    messages = null;
  }

  return messages;
}

/**
 * 拉取消息列表
 * 优先从后端获取，失败时使用模拟数据
 */
async function fetchMessages() {
  loading.value = true;
  error.value = null;

  try {
    const backendMessages = await fetchFromBackend();
    if (backendMessages && backendMessages.length > 0) {
      messages.value = backendMessages;
    } else {
      // 后端无数据，使用模拟数据
      messages.value = MOCK_MESSAGES.map((m) => ({ ...m }));
    }
  } catch (e) {
    error.value = "获取消息失败，使用本地数据";
    messages.value = MOCK_MESSAGES.map((m) => ({ ...m }));
  } finally {
    loading.value = false;
  }
}

/**
 * 标记单条消息为已读
 * @param {string} id — 消息 ID
 */
function markAsRead(id) {
  const msg = messages.value.find((m) => m.id === id);
  if (msg) msg.read = true;

  // 未来可调用后端命令持久化状态：
  // import { invoke } from "@tauri-apps/api/core";
  // invoke("mark_message_read", { id });
}

/**
 * 标记所有消息为已读
 */
function markAllRead() {
  messages.value.forEach((m) => { m.read = true; });
}

/**
 * 移除单条消息
 * @param {string} id — 消息 ID
 */
function dismissMessage(id) {
  messages.value = messages.value.filter((m) => m.id !== id);
}

// =========================================================================
//  导出组合式函数
// =========================================================================

export function useMessages() {
  return {
    /** 消息列表（响应式） */
    messages,
    /** 是否正在加载 */
    loading,
    /** 错误信息 */
    error,
    /** 未读消息数（计算属性） */
    unreadCount,
    /** 拉取消息列表 */
    fetchMessages,
    /** 标记单条为已读 */
    markAsRead,
    /** 标记全部为已读 */
    markAllRead,
    /** 移除消息 */
    dismissMessage,
  };
}
