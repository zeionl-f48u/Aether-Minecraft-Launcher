<!-- ============================================================
  App.vue — 应用根组件（入口组件）
  ============================================================
  职责：
  - 作为 Vue 应用的根组件，挂载到 index.html 的 #app 元素
  - 仅引入布局组件 AppLayout 和当前页面 Home
  - 不包含任何样式或逻辑，保持极简
  
  后续扩展：
  - 如需路由，可将 <Home /> 替换为 <router-view />
  - 如需全局状态管理（Pinia），在此注入
  ============================================================ -->

<script setup>
// ---------- 组件导入 ----------
// AppLayout：整体布局框架（标题栏 + 侧边栏 + 内容区）
import AppLayout from "./components/layout/AppLayout.vue";
// Home：首页内容
import Home from "./views/Home.vue";
</script>

<template>
  <!-- 将 Home 作为默认插槽内容传入 AppLayout -->
  <AppLayout>
    <Home />
  </AppLayout>
</template>
