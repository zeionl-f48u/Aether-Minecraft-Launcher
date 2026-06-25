<!-- ============================================================
  AppIcon.vue — 通用 SVG 图标组件
  ============================================================
  功能：
  - 集中管理应用中所有 SVG 图标
  - 通过 name 属性切换图标，避免每个组件重复写 SVG
  - 支持动态 size（尺寸）和 strokeWidth（描边宽度）
  - 图标列表：home / versions / download / settings / user / launch / close / min / max / theme / bell / info / success / warning / error

  使用方式：
  <AppIcon name="home" size="5" stroke-width="2" />
  ============================================================ -->

<script setup>
/**
 * Props 定义
 *
 * @prop {string} name        - 图标名称（必填）
 *   可用值：home | versions | download | settings | user | launch | close | min | max | theme | bell | info | success | warning | error
 * @prop {string} size        - 图标尺寸（对应 Tailwind 的 w-* h-* 数字，默认 "5" = 20px）
 * @prop {string} strokeWidth - SVG 描边宽度（默认 "2"）
 */
defineProps({
  name: {
    type: String,
    required: true,
  },
  size: {
    type: String,
    default: "5",
  },
  strokeWidth: {
    type: String,
    default: "2",
  },
});
</script>

<template>
  <!--
    外层 span：固定容器，确保图标水平垂直居中
    尺寸由 :class="`w-${size} h-${size}`" 控制
  -->
  <span class="inline-flex items-center justify-center" :class="`w-${size} h-${size}`">

    <!-- ===== 首页（房子图标）===== -->
    <svg v-if="name === 'home'" :class="`w-${size} h-${size}`" viewBox="0 0 24 24" fill="none" stroke="currentColor" :stroke-width="strokeWidth" stroke-linecap="round" stroke-linejoin="round">
      <path d="M3 9l9-7 9 7v11a2 2 0 01-2 2H5a2 2 0 01-2-2z" />
      <polyline points="9 22 9 12 15 12 15 22" />
    </svg>

    <!-- ===== 版本管理（带标签的文件夹/网格）===== -->
    <svg v-else-if="name === 'versions'" :class="`w-${size} h-${size}`" viewBox="0 0 24 24" fill="none" stroke="currentColor" :stroke-width="strokeWidth" stroke-linecap="round" stroke-linejoin="round">
      <rect x="3" y="3" width="18" height="18" rx="2" ry="2" />
      <line x1="3" y1="9" x2="21" y2="9" />
      <line x1="9" y1="21" x2="9" y2="9" />
    </svg>

    <!-- ===== 下载（向下箭头+横线）===== -->
    <svg v-else-if="name === 'download'" :class="`w-${size} h-${size}`" viewBox="0 0 24 24" fill="none" stroke="currentColor" :stroke-width="strokeWidth" stroke-linecap="round" stroke-linejoin="round">
      <path d="M21 15v4a2 2 0 01-2 2H5a2 2 0 01-2-2v-4" />
      <polyline points="7 10 12 15 17 10" />
      <line x1="12" y1="15" x2="12" y2="3" />
    </svg>

    <!-- ===== 设置（齿轮）===== -->
    <svg v-else-if="name === 'settings'" :class="`w-${size} h-${size}`" viewBox="0 0 24 24" fill="none" stroke="currentColor" :stroke-width="strokeWidth" stroke-linecap="round" stroke-linejoin="round">
      <circle cx="12" cy="12" r="3" />
      <path d="M19.4 15a1.65 1.65 0 00.33 1.82l.06.06a2 2 0 010 2.83 2 2 0 01-2.83 0l-.06-.06a1.65 1.65 0 00-1.82-.33 1.65 1.65 0 00-1 1.51V21a2 2 0 01-2 2 2 2 0 01-2-2v-.09A1.65 1.65 0 009 19.4a1.65 1.65 0 00-1.82.33l-.06.06a2 2 0 01-2.83 0 2 2 0 010-2.83l.06-.06A1.65 1.65 0 004.68 15a1.65 1.65 0 00-1.51-1H3a2 2 0 01-2-2 2 2 0 012-2h.09A1.65 1.65 0 004.6 9a1.65 1.65 0 00-.33-1.82l-.06-.06a2 2 0 010-2.83 2 2 0 012.83 0l.06.06A1.65 1.65 0 009 4.68a1.65 1.65 0 001-1.51V3a2 2 0 012-2 2 2 0 012 2v.09a1.65 1.65 0 001 1.51 1.65 1.65 0 001.82-.33l.06-.06a2 2 0 012.83 0 2 2 0 010 2.83l-.06.06A1.65 1.65 0 0019.32 9a1.65 1.65 0 001.51 1H21a2 2 0 012 2 2 2 0 01-2 2h-.09a1.65 1.65 0 00-1.51 1z" />
    </svg>

    <!-- ===== 用户（人头+上半身）===== -->
    <svg v-else-if="name === 'user'" :class="`w-${size} h-${size}`" viewBox="0 0 24 24" fill="none" stroke="currentColor" :stroke-width="strokeWidth" stroke-linecap="round" stroke-linejoin="round">
      <path d="M20 21v-2a4 4 0 00-4-4H8a4 4 0 00-4 4v2" />
      <circle cx="12" cy="7" r="4" />
    </svg>

    <!-- ===== 启动/播放（三角形播放按钮）===== -->
    <svg v-else-if="name === 'launch'" :class="`w-${size} h-${size}`" viewBox="0 0 24 24" fill="currentColor">
      <path d="M8 5v14l11-7z" />
    </svg>

    <!-- ===== 关闭（X 符号）===== -->
    <svg v-else-if="name === 'close'" :class="`w-${size} h-${size}`" viewBox="0 0 12 12" fill="none" stroke="currentColor" :stroke-width="strokeWidth" stroke-linecap="round">
      <path d="M3 3L9 9M9 3L3 9" />
    </svg>

    <!-- ===== 最小化（横线）===== -->
    <svg v-else-if="name === 'min'" :class="`w-${size} h-${size}`" viewBox="0 0 12 12" fill="none" stroke="currentColor" :stroke-width="strokeWidth" stroke-linecap="round">
      <rect x="2" y="5.5" width="8" height="1" />
    </svg>

    <!-- ===== 最大化（空心方框）===== -->
    <svg v-else-if="name === 'max'" :class="`w-${size} h-${size}`" viewBox="0 0 12 12" fill="none" stroke="currentColor" :stroke-width="strokeWidth" stroke-linecap="round">
      <rect x="2" y="2" width="8" height="8" rx="0.5" fill="none" />
    </svg>

    <!-- ===== 主题切换（日月）===== -->
    <svg v-else-if="name === 'theme'" :class="`w-${size} h-${size}`" viewBox="0 0 24 24" fill="none" stroke="currentColor" :stroke-width="strokeWidth" stroke-linecap="round" stroke-linejoin="round">
      <path d="M21 12.79A9 9 0 1111.21 3 7 7 0 0021 12.79z" />
    </svg>

    <!-- ===== 铃铛/消息（通知）===== -->
    <svg v-else-if="name === 'bell'" :class="`w-${size} h-${size}`" viewBox="0 0 24 24" fill="none" stroke="currentColor" :stroke-width="strokeWidth" stroke-linecap="round" stroke-linejoin="round">
      <path d="M18 8A6 6 0 006 8c0 7-3 9-3 9h18s-3-2-3-9" />
      <path d="M13.73 21a2 2 0 01-3.46 0" />
    </svg>

    <!-- ===== 信息（圆中 i）===== -->
    <svg v-else-if="name === 'info'" :class="`w-${size} h-${size}`" viewBox="0 0 24 24" fill="none" stroke="currentColor" :stroke-width="strokeWidth" stroke-linecap="round" stroke-linejoin="round">
      <circle cx="12" cy="12" r="10" />
      <line x1="12" y1="16" x2="12" y2="12" />
      <line x1="12" y1="8" x2="12.01" y2="8" />
    </svg>

    <!-- ===== 成功/勾选（圆中勾）===== -->
    <svg v-else-if="name === 'success'" :class="`w-${size} h-${size}`" viewBox="0 0 24 24" fill="none" stroke="currentColor" :stroke-width="strokeWidth" stroke-linecap="round" stroke-linejoin="round">
      <path d="M22 11.08V12a10 10 0 11-5.93-9.14" />
      <polyline points="22 4 12 14.01 9 11.01" />
    </svg>

    <!-- ===== 警告（三角感叹号）===== -->
    <svg v-else-if="name === 'warning'" :class="`w-${size} h-${size}`" viewBox="0 0 24 24" fill="none" stroke="currentColor" :stroke-width="strokeWidth" stroke-linecap="round" stroke-linejoin="round">
      <path d="M10.29 3.86L1.82 18a2 2 0 001.71 3h16.94a2 2 0 001.71-3L13.71 3.86a2 2 0 00-3.42 0z" />
      <line x1="12" y1="9" x2="12" y2="13" />
      <line x1="12" y1="17" x2="12.01" y2="17" />
    </svg>

    <!-- ===== 错误（圆中 X）===== -->
    <svg v-else-if="name === 'error'" :class="`w-${size} h-${size}`" viewBox="0 0 24 24" fill="none" stroke="currentColor" :stroke-width="strokeWidth" stroke-linecap="round" stroke-linejoin="round">
      <circle cx="12" cy="12" r="10" />
      <line x1="15" y1="9" x2="9" y2="15" />
      <line x1="9" y1="9" x2="15" y2="15" />
    </svg>

    <!-- ===== 刷新（旋转箭头）===== -->
    <svg v-else-if="name === 'refresh'" :class="`w-${size} h-${size}`" viewBox="0 0 24 24" fill="none" stroke="currentColor" :stroke-width="strokeWidth" stroke-linecap="round" stroke-linejoin="round">
      <polyline points="23 4 23 10 17 10" />
      <polyline points="1 20 1 14 7 14" />
      <path d="M3.51 9a9 9 0 0114.85-3.36L23 10M1 14l4.64 4.36A9 9 0 0020.49 15" />
    </svg>

    <!-- ===== 模组管理（四格方块）===== -->
    <svg v-else-if="name === 'mods'" :class="`w-${size} h-${size}`" viewBox="0 0 24 24" fill="none" stroke="currentColor" :stroke-width="strokeWidth" stroke-linecap="round" stroke-linejoin="round">
      <rect x="3" y="3" width="7" height="7" /><rect x="14" y="3" width="7" height="7" />
      <rect x="3" y="14" width="7" height="7" /><rect x="14" y="14" width="7" height="7" />
    </svg>

    <!-- ===== 模组市场（购物袋）===== -->
    <svg v-else-if="name === 'modMarket'" :class="`w-${size} h-${size}`" viewBox="0 0 24 24" fill="none" stroke="currentColor" :stroke-width="strokeWidth" stroke-linecap="round" stroke-linejoin="round">
      <path d="M6 2L3 6v14a2 2 0 002 2h14a2 2 0 002-2V6l-3-4z" /><line x1="3" y1="6" x2="21" y2="6" />
      <path d="M16 10a4 4 0 01-8 0" />
    </svg>

    <!-- ===== 搜索（放大镜）===== -->
    <svg v-else-if="name === 'search'" :class="`w-${size} h-${size}`" viewBox="0 0 24 24" fill="none" stroke="currentColor" :stroke-width="strokeWidth" stroke-linecap="round" stroke-linejoin="round">
      <circle cx="11" cy="11" r="8" /><line x1="21" y1="21" x2="16.65" y2="16.65" />
    </svg>

    <!-- ===== 外链===== -->
    <svg v-else-if="name === 'external-link'" :class="`w-${size} h-${size}`" viewBox="0 0 24 24" fill="none" stroke="currentColor" :stroke-width="strokeWidth" stroke-linecap="round" stroke-linejoin="round">
      <path d="M18 13v6a2 2 0 01-2 2H5a2 2 0 01-2-2V8a2 2 0 012-2h6" />
      <polyline points="15 3 21 3 21 9" /><line x1="10" y1="14" x2="21" y2="3" />
    </svg>

    <!-- ===== 游戏（手柄）===== -->
    <svg v-else-if="name === 'game'" :class="`w-${size} h-${size}`" viewBox="0 0 24 24" fill="none" stroke="currentColor" :stroke-width="strokeWidth" stroke-linecap="round" stroke-linejoin="round">
      <rect x="2" y="6" width="20" height="12" rx="2" /><path d="M6 12h4M8 10v4" />
      <circle cx="16" cy="12" r="1" fill="currentColor" /><circle cx="19" cy="10" r="1" fill="currentColor" />
    </svg>

    <!-- ===== 箭头右===== -->
    <svg v-else-if="name === 'chevron-right'" :class="`w-${size} h-${size}`" viewBox="0 0 24 24" fill="none" stroke="currentColor" :stroke-width="strokeWidth" stroke-linecap="round" stroke-linejoin="round">
      <polyline points="9 18 15 12 9 6" />
    </svg>

    <!-- ===== 箭头左===== -->
    <svg v-else-if="name === 'chevron-left'" :class="`w-${size} h-${size}`" viewBox="0 0 24 24" fill="none" stroke="currentColor" :stroke-width="strokeWidth" stroke-linecap="round" stroke-linejoin="round">
      <polyline points="15 18 9 12 15 6" />
    </svg>

  </span>
</template>
