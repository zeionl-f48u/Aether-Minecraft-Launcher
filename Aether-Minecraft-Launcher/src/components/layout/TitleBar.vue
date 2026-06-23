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

// ---------- Props 定义 ----------
/**
 * title — 标题栏显示的应用名称
 * 通过 props 传入以支持不同页面自定义标题
 */
defineProps({
  title: {
    type: String,
    default: "Aether Launcher",
  },
});

// ---------- 窗口控制方法 ----------
// 从 composable 解构出三个窗口操作方法
const { minimize, toggleMaximize, close } = useWindowControls();
</script>

<template>
  <!--
    外层容器：fixed 固定定位，悬浮于内容之上
    样式说明：
      - bg-white/[0.06] + backdrop-blur-md：极淡毛玻璃效果
      - rounded-2xl：大圆角
      - border-white/10：半透明白色描边
      - active:scale-90：按钮点击缩放反馈
  -->
  <div
    class="fixed top-0 left-1 right-1 z-50 flex items-center justify-between h-[38px] mt-1 bg-white/[0.06] backdrop-blur-md rounded-2xl select-none shadow-lg shadow-black/10 border border-white/10"
  >
    <!-- ---- 左侧：窗口拖拽区域 ----
         data-tauri-drag-region 是 Tauri 2 提供的 HTML 属性，
         标记该区域可被鼠标拖拽移动窗口。
         配合 tauri.conf.json 中 decorations: false 使用。
         flex-1 使其撑满剩余空间。 -->
    <div
      class="flex items-center h-full px-4 flex-1"
      data-tauri-drag-region
    >
      <span class="text-sm font-semibold text-white/90">{{ title }}</span>
    </div>

    <!-- ---- 右侧：窗口控制按钮 ----
         三个按钮分别绑定 minimize / toggleMaximize / close 方法。
         样式统一：38×28px 圆角按钮，hover 亮色背景，
         关闭按钮 hover 使用红色背景以符合惯例。 -->
    <div class="flex items-center h-full pr-1">
      <!-- 最小化 -->
      <button
        class="flex items-center justify-center w-7 h-7 mx-0.5 text-gray-500 hover:text-white hover:bg-white/10 transition-all duration-150 rounded-full active:scale-90"
        @click="minimize"
        title="最小化"
      >
        <AppIcon name="min" size="3.5" stroke-width="1.5" />
      </button>
      <!-- 最大化/还原 -->
      <button
        class="flex items-center justify-center w-7 h-7 mx-0.5 text-gray-500 hover:text-white hover:bg-white/10 transition-all duration-150 rounded-full active:scale-90"
        @click="toggleMaximize"
        title="最大化"
      >
        <AppIcon name="max" size="3.5" stroke-width="1.5" />
      </button>
      <!-- 关闭 -->
      <button
        class="flex items-center justify-center w-7 h-7 mx-0.5 text-gray-500 hover:text-white hover:bg-red-500/60 transition-all duration-150 rounded-full active:scale-90"
        @click="close"
        title="关闭"
      >
        <AppIcon name="close" size="3.5" stroke-width="1.5" />
      </button>
    </div>
  </div>
</template>
