<!-- ============================================================
  TitleBar.vue — 顶部标题栏组件
  ============================================================
  职责：
  - 固定在窗口顶部，模拟原生窗口标题栏
  - 左侧为窗口拖拽区域（data-tauri-drag-region）
  - 右侧提供最小化、最大化/还原、关闭三个控制按钮
  - 使用毛玻璃半透明效果，适应无边框窗口设计

  接收 Props：
  - title: String — 窗口标题（默认 "Aether Launcher"）

  依赖：
  - AppIcon.vue     — 通用图标组件（渲染 min/max/close 图标）
  - useWindowControls — 窗口控制组合式函数
  ============================================================ -->

<script setup>
// ---------- 组件与 composable 导入 ----------
import AppIcon from "../common/AppIcon.vue";                      // 通用图标
import { useWindowControls } from "../../composables/useWindowControls.js"; // 窗口控制

// ---------- Props & Emits 定义 ----------
defineProps({
  title: {
    type: String,
    default: "Aether Minecraft Launcher",
  },
});

/** 向父组件发送切换主题事件 */
defineEmits(["toggle-theme"]);

// ---------- 窗口控制方法 ----------
// 从 composable 解构出三个窗口操作方法
const { minimize, toggleMaximize, close } = useWindowControls();
</script>

<template>
  <!--
    外层容器：Win11 经典毛玻璃标题栏
    样式说明：
      - 使用 CSS 变量实现深色/浅色自适应
      - 毛玻璃效果（backdrop-blur-md）
      - 大圆角（rounded-2xl）
      - 窗口控制按钮居中对齐
  -->
  <div
    class="fixed top-0 left-1 right-1 z-50 flex items-center justify-between h-[38px] mt-1 backdrop-blur-md rounded-2xl select-none"
    :class="[
      'bg-[var(--glass-bg)] border border-[var(--glass-border)] shadow-lg shadow-[var(--glass-shadow)]',
      'transition-colors duration-300'
    ]"
  >
    <!-- ---- 左侧：窗口拖拽区域 ---- -->
    <div
      class="flex items-center h-full px-4 flex-1"
      data-tauri-drag-region
    >
      <span class="text-sm font-semibold text-[var(--text-primary)]">{{ title }}</span>
    </div>

    <!-- ---- 右侧：主题切换 + 窗口控制按钮 ---- -->
    <div class="flex items-center h-full pr-1 gap-0.5">
      <!-- 主题切换按钮 -->
      <button
        class="flex items-center justify-center w-7 h-7 text-[var(--text-tertiary)] hover:text-[var(--text-primary)] hover:bg-[var(--bg-hover)] transition-all duration-150 rounded-full active:scale-90"
        @click="$emit('toggle-theme')"
        title="切换主题"
      >
        <AppIcon name="theme" size="3.5" stroke-width="1.5" />
      </button>

      <!-- 分隔线 -->
      <span class="w-px h-4 mx-0.5 bg-[var(--border-base)]" />

      <!-- 最小化 -->
      <button
        class="flex items-center justify-center w-7 h-7 text-[var(--text-tertiary)] hover:text-[var(--text-primary)] hover:bg-[var(--bg-hover)] transition-all duration-150 rounded-full active:scale-90"
        @click="minimize"
        title="最小化"
      >
        <AppIcon name="min" size="3.5" stroke-width="1.5" />
      </button>

      <!-- 最大化/还原 -->
      <button
        class="flex items-center justify-center w-7 h-7 text-[var(--text-tertiary)] hover:text-[var(--text-primary)] hover:bg-[var(--bg-hover)] transition-all duration-150 rounded-full active:scale-90"
        @click="toggleMaximize"
        title="最大化"
      >
        <AppIcon name="max" size="3.5" stroke-width="1.5" />
      </button>

      <!-- 关闭 -->
      <button
        class="flex items-center justify-center w-7 h-7 text-[var(--text-tertiary)] hover:text-white hover:bg-red-500/70 transition-all duration-150 rounded-full active:scale-90"
        @click="close"
        title="关闭"
      >
        <AppIcon name="close" size="3.5" stroke-width="1.5" />
      </button>
    </div>
  </div>
</template>
