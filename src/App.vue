<!-- ============================================================
  App.vue — 应用根组件（入口组件）
  ============================================================
  职责：
  - 作为 Vue 应用的根组件
  - 提供 AppLayout 布局外壳
  - 内部使用 router-view 切换页面
  ============================================================ -->

<script setup>
import { onMounted } from "vue";
import AppLayout from "./components/layout/AppLayout.vue";
import { initTheme } from "./composables/useTheme.js";

onMounted(() => {
  initTheme();
});
</script>

<template>
  <AppLayout>
    <router-view />
  </AppLayout>
</template>
