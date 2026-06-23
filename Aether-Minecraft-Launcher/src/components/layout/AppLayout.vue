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

// ---------- 布局子组件 ----------
import TitleBar from "./TitleBar.vue";  // 顶部标题栏（固定定位）
import Sidebar from "./Sidebar.vue";    // 左侧导航栏（固定定位）

/**
 * onMounted — 组件挂载后初始化 Flowbite
 * Flowbite 需要手动调用 initFlowbite() 来激活
 * 其 JavaScript 交互组件（如 dropdown、modal 等）。
 */
onMounted(() => {
  initFlowbite();
});
</script>

<template>
  <!--
    最外层容器：
      - flex 布局，子元素横向排列
      - h-screen w-screen：占满整个视口
      - overflow-hidden：防止整体滚动
      - 深色渐变背景（bg-gradient-to-br from-[#12121a] to-[#0d0d14]）
  -->
  <div class="flex h-screen w-screen overflow-hidden bg-gradient-to-br from-[#12121a] to-[#0d0d14] text-white">

    <!-- TitleBar：fixed 固定顶部，不占文档流 -->
    <TitleBar title="Aether Launcher" />

    <!-- Sidebar：fixed 固定左侧，在标题栏下方 -->
    <Sidebar active="home" />

    <!--
      主内容区域：
        - flex-1：撑满剩余宽度
        - ml-[60px]：为左侧固定侧边栏留出空间
        - mt-[44px]：为顶部固定标题栏留出空间
        - overflow-auto：内容超出时可滚动
        - p-6：内边距
        - <slot />：接收父组件传入的页面内容
    -->
    <main class="flex-1 ml-[60px] mt-[44px] overflow-auto p-6">
      <slot />
    </main>

  </div>
</template>

<style scoped>
/* 页面过渡动画预留 —— 启用 vue-router 后可在此添加 Transition 样式 */
</style>
