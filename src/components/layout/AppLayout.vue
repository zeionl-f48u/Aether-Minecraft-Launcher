<!-- ============================================================
  AppLayout.vue — 应用整体布局组件
  ============================================================
  职责：
  - 定义应用的整体布局结构
  - 组合 TitleBar（顶部标题栏）和 Sidebar（左侧导航）
  - 提供内容插槽（slot），供父组件注入页面内容
  - 在 mounted 时初始化 Flowbite 组件

  布局结构（从上到下，从左到右）：
  ┌─────────────────────────────────────┐
  │          TitleBar (fixed top)        │
  ├──────┬──────────────────────────────┤
  │      │                              │
  │Side- │    Main Content Area         │
  │ bar  │    (scrollable, slot)         │
  │      │                              │
  └──────┴──────────────────────────────┘

  使用方式：
  <AppLayout>
    <Home />    ← 页面内容通过 slot 插入
  </AppLayout>
  ============================================================ -->

<script setup>
// ---------- Vue 生命周期 ----------
import { onMounted } from "vue";

// ---------- Flowbite 初始化 ----------
import { initFlowbite } from "flowbite";

// ---------- 主题管理 ----------
import { useTheme } from "../../composables/useTheme.js";

// ---------- 布局子组件 ----------
import TitleBar from "./TitleBar.vue";  // 顶部标题栏（固定定位）
import Sidebar from "./Sidebar.vue";    // 左侧导航栏（固定定位）

// ---------- 主题初始化 ----------
const { toggleTheme } = useTheme();

/**
 * onMounted — 组件挂载后初始化 Flowbite
 */
onMounted(() => {
  initFlowbite();
});
</script>

<template>
  <!--
    最外层容器：
      - 使用 CSS 变量实现 Win11 经典渐变背景
      - 深色/浅色自动适配
  -->
  <div
    class="flex h-screen w-screen overflow-hidden transition-colors duration-300"
    :style="{
      background: `linear-gradient(135deg, var(--gradient-start), var(--gradient-end))`,
      color: 'var(--text-primary)',
    }"
  >
    <!-- TitleBar：fixed 固定顶部，不占文档流 -->
    <TitleBar
      title="Aether Launcher"
      @toggle-theme="toggleTheme"
    />

    <!-- Sidebar：fixed 固定左侧，在标题栏下方 -->
    <Sidebar active="home" />

    <!--
      主内容区域：
        - flex-1：撑满剩余宽度
        - ml-[60px]：为左侧固定侧边栏留出空间
        - mt-[44px]：为顶部固定标题栏留出空间
        - overflow-auto：内容超出时可滚动
        - p-6：内边距
    -->
    <main class="flex-1 ml-[60px] mt-[44px] overflow-auto p-6">
      <slot />
    </main>

  </div>
</template>
