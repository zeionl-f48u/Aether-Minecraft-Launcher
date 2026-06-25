<!-- ============================================================
  MessageCenter.vue — 消息通知中心组件
  ============================================================
  功能：
  - 展示系统消息、通知和更新动态列表
  - 通过 useMessages composable 获取数据（后端优先，模拟降级）
  - 支持类型图标区分：info / success / warning / error
  - 支持标记已读、全部已读、移除消息
  - 空状态展示

  接口约定：
  - 数据层完全由 useMessages 管理，组件仅负责渲染
  - 如需替换数据源，只需修改 useMessages 而不动本组件

  使用方式：
  <MessageCenter />
  ============================================================ -->

<script setup>
import { onMounted } from "vue";
import AppIcon from "./AppIcon.vue";
import AppButton from "./AppButton.vue";
import { useMessages } from "../../composables/useMessages.js";

// ---------- 消息 API ----------
const { messages, loading, unreadCount, fetchMessages, markAsRead, markAllRead, dismissMessage } = useMessages();

// ---------- 生命周期 ----------
onMounted(() => {
  fetchMessages();
});

// ---------- 消息类型配置 ----------
/**
 * 每种消息类型对应的：图标名、边框色、背景色
 * Win11 配色风格：左侧彩色指示条 + 柔和背景
 */
const typeStyles = {
  info:    { icon: "info",    border: "border-l-blue-500/60",   bg: "bg-blue-500/8" },
  success: { icon: "success", border: "border-l-green-500/60",  bg: "bg-green-500/8" },
  warning: { icon: "warning", border: "border-l-yellow-500/60", bg: "bg-yellow-500/8" },
  error:   { icon: "error",   border: "border-l-red-500/60",    bg: "bg-red-500/8" },
};

/**
 * 格式化时间戳为友好显示
 * @param {string} iso — ISO 8601 时间戳
 * @returns {string}
 */
function formatTime(iso) {
  try {
    const date = new Date(iso);
    const now = new Date();
    const diffMs = now - date;
    const diffMin = Math.floor(diffMs / 60000);
    const diffHour = Math.floor(diffMs / 3600000);
    const diffDay = Math.floor(diffMs / 86400000);

    if (diffMin < 1) return "刚刚";
    if (diffMin < 60) return `${diffMin} 分钟前`;
    if (diffHour < 24) return `${diffHour} 小时前`;
    if (diffDay < 7) return `${diffDay} 天前`;

    return date.toLocaleDateString("zh-CN", {
      month: "short",
      day: "numeric",
    });
  } catch {
    return "";
  }
}
</script>

<template>
  <!--
    消息中心卡片容器
    根据消息数量决定最大高度，超出可滚动
  -->
  <div
    class="rounded-lg border border-[var(--card-border)] bg-[var(--card-bg)] overflow-hidden anim-slide-up"
  >
    <!-- ---- 头部：标题 + 操作按钮 ---- -->
    <div class="flex items-center justify-between px-5 py-3 border-b border-[var(--border-base)]">
      <div class="flex items-center gap-2">
        <AppIcon name="bell" size="4" />
        <span class="text-sm font-medium text-[var(--text-primary)]">消息</span>
        <span
          v-if="unreadCount > 0"
          class="inline-flex items-center justify-center min-w-[18px] h-[18px] px-1 text-[10px] font-bold text-white bg-blue-500 rounded-full"
        >
          {{ unreadCount }}
        </span>
      </div>
      <div class="flex items-center gap-2">
        <button
          v-if="unreadCount > 0"
          class="text-xs text-[var(--text-tertiary)] hover:text-[var(--text-accent)] transition-colors duration-150"
          @click="markAllRead"
        >
          全部已读
        </button>
        <button
          class="text-xs text-[var(--text-tertiary)] hover:text-[var(--text-accent)] transition-colors duration-150"
          @click="fetchMessages"
          :disabled="loading"
        >
          {{ loading ? '加载中...' : '刷新' }}
        </button>
      </div>
    </div>

    <!-- ---- 加载状态 ---- -->
    <div
      v-if="loading"
      class="flex items-center justify-center py-8 text-sm text-[var(--text-tertiary)]"
    >
      <svg class="w-4 h-4 mr-2 animate-spin" viewBox="0 0 24 24" fill="none">
        <circle class="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" stroke-width="4" />
        <path class="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4z" />
      </svg>
      加载中...
    </div>

    <!-- ---- 消息列表 ---- -->
    <div v-else-if="messages.length > 0" class="divide-y divide-[var(--border-base)] max-h-[400px] overflow-y-auto">
      <div
        v-for="msg in messages"
        :key="msg.id"
        class="relative flex items-start gap-3 px-5 py-3.5 transition-colors duration-150 cursor-pointer hover:bg-[var(--bg-hover)]"
        :class="[
          typeStyles[msg.type]?.border || 'border-l-transparent',
          'border-l-2',
          msg.read ? 'opacity-60' : 'opacity-100',
        ]"
        @click="markAsRead(msg.id)"
      >
        <!-- 图标 -->
        <span class="flex-shrink-0 mt-0.5">
          <AppIcon
            :name="typeStyles[msg.type]?.icon || 'info'"
            size="4"
          />
        </span>

        <!-- 内容 -->
        <div class="flex-1 min-w-0">
          <div class="flex items-center gap-2">
            <span
              class="text-sm font-medium truncate"
              :class="msg.read ? 'text-[var(--text-secondary)]' : 'text-[var(--text-primary)]'"
            >
              {{ msg.title }}
            </span>
            <!-- 未读小圆点 -->
            <span
              v-if="!msg.read"
              class="w-1.5 h-1.5 rounded-full bg-blue-500 flex-shrink-0"
            />
          </div>
          <p class="mt-0.5 text-xs text-[var(--text-tertiary)] line-clamp-2">{{ msg.body }}</p>
          <div class="flex items-center gap-3 mt-1">
            <span class="text-[10px] text-[var(--text-tertiary)]">{{ formatTime(msg.timestamp) }}</span>
            <button
              v-if="msg.action"
              class="text-[10px] text-[var(--text-accent)] hover:underline"
            >
              {{ msg.action }}
            </button>
          </div>
        </div>

        <!-- 关闭按钮 -->
        <button
          class="flex-shrink-0 mt-0.5 text-[var(--text-tertiary)] hover:text-[var(--text-primary)] transition-colors duration-150 opacity-0 group-hover:opacity-100"
          @click.stop="dismissMessage(msg.id)"
          title="关闭"
        >
          <AppIcon name="close" size="3" />
        </button>
      </div>
    </div>

    <!-- ---- 空状态 ---- -->
    <div
      v-else
      class="flex flex-col items-center justify-center py-10 text-sm text-[var(--text-tertiary)]"
    >
      <AppIcon name="bell" size="8" />
      <p class="mt-3">暂无消息</p>
      <p class="mt-1 text-xs">当有系统通知或更新时，将在此显示</p>
    </div>
  </div>
</template>

<style scoped>
/* 悬停时显示关闭按钮 */
.message-item:hover .dismiss-btn {
  opacity: 1;
}
</style>
