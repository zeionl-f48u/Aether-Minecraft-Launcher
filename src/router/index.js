/**
 * router/index.js — Vue Router 路由配置
 * =====================================================
 * 定义应用的所有路由页面及其对应的组件。
 * ===================================================== */

import { createRouter, createWebHistory } from "vue-router";

// ---------- 页面视图组件 ----------
import Home from "../views/Home.vue";
import Versions from "../views/Versions.vue";
import Download from "../views/Download.vue";
import Settings from "../views/Settings.vue";
import UserProfile from "../views/UserProfile.vue";

const routes = [
  {
    path: "/",
    name: "home",
    component: Home,
    meta: { title: "首页", icon: "home" },
  },
  {
    path: "/versions",
    name: "versions",
    component: Versions,
    meta: { title: "版本管理", icon: "versions" },
  },
  {
    path: "/download",
    name: "download",
    component: Download,
    meta: { title: "下载", icon: "download" },
  },
  {
    path: "/settings",
    name: "settings",
    component: Settings,
    meta: { title: "设置", icon: "settings" },
  },
  {
    path: "/user",
    name: "user",
    component: UserProfile,
    meta: { title: "用户中心", icon: "user" },
  },
];

const router = createRouter({
  history: createWebHistory(),
  routes,
});

export default router;
