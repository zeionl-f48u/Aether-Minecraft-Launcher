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
import { onMounted, ref, watch } from "vue";
import { useRoute, useRouter } from "vue-router";

// ---------- Flowbite 初始化 ----------
import { initFlowbite } from "flowbite";

// ---------- 主题管理 ----------
import { useTheme } from "../../composables/useTheme.js";

// ---------- 布局子组件 ----------
import TitleBar from "./TitleBar.vue";
import Sidebar from "./Sidebar.vue";

// ---------- 主题与路由 ----------
const { toggleTheme } = useTheme();
const route = useRoute();
const router = useRouter();

/** 当前激活的页面 ID，从路由名推断 */
const activePage = ref(route.name || "home");

/** 监听路由变化，同步激活状态 */
watch(
  () => route.name,
  (name) => {
    activePage.value = name || "home";
  }
);

/**
 * 处理侧边栏导航点击
 * @param {string} page - 目标页面名称
 */
function onNavigate(page) {
  activePage.value = page;
  router.push({ name: page });
}

onMounted(() => {
  initFlowbite();
});
</script>

<template>
  <div
    class="flex h-screen w-screen overflow-hidden transition-colors duration-300"
    :style="{
      background: `linear-gradient(135deg, var(--gradient-start), var(--gradient-end))`,
      color: 'var(--text-primary)',
    }"
  >
    <TitleBar
      title="Aether Launcher"
      @toggle-theme="toggleTheme"
    />

    <Sidebar
      :active="activePage"
      @navigate="onNavigate"
    />

    <!-- 主内容区域 -->
    <main class="flex-1 ml-[60px] mt-[44px] overflow-auto p-6">
      <slot />
    </main>

  </div>
</template>
