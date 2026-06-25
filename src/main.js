/**
 * main.js — Vue 应用入口文件
 * =====================================================
 * 职责：
 *   1. 创建 Vue 应用实例
 *   2. 注册 Vue Router
 *   3. 导入全局样式
 *   4. 挂载应用到 #app
 * ===================================================== */

import { createApp } from "vue";
import App from "./App.vue";
import router from "./router/index.js";
import "./style.css";

const app = createApp(App);
app.use(router);
app.mount("#app");
