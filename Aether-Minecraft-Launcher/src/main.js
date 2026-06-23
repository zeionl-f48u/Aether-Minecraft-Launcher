/**
 * main.js — Vue 应用入口文件
 * =====================================================
 * 职责：
 *   1. 创建 Vue 应用实例
 *   2. 导入全局样式（style.css，含 Tailwind + Flowbite）
 *   3. 挂载应用到 index.html 的 #app DOM 元素
 *
 * 后续扩展：
 *   - 如需注册全局组件，在此使用 app.component()
 *   - 如需路由，在此 app.use(router)
 *   - 如需状态管理，在此 app.use(pinia)
 * ===================================================== */

import { createApp } from "vue";          // Vue 3 应用创建函数
import App from "./App.vue";              // 根组件
import "./style.css";                      // 全局样式（Tailwind + Flowbite + 动画）

// 创建 Vue 应用实例并挂载到 #app
createApp(App).mount("#app");
