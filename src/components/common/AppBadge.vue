<!-- ============================================================
  AppBadge.vue — 通用徽章/标签组件
  ============================================================
  功能：
  - 用于展示状态标签、版本号、分类标记等
  - 8 种预设颜色：blue / green / red / yellow / purple / orange / indigo / gray
  - 3 种样式变体：soft（半透明背景） / solid（实心） / outline（描边）
  - 2 种尺寸：sm / md
  - 内置 anim-fade-in 入场动画

  使用方式：
  <AppBadge color="green" variant="soft">正式版</AppBadge>
  ============================================================ -->

<script setup>
/**
 * Props 定义
 *
 * @prop {string} color   - 颜色主题（默认 "blue"）
 * @prop {string} variant - 样式变体（默认 "soft"）
 *   "soft":    半透明背景 + 同色文字，适合深色背景
 *   "solid":   实心填充 + 白色文字，适合浅色背景
 *   "outline": 仅描边 + 同色文字，适合轻量标记
 * @prop {string} size    - 尺寸（默认 "sm"）
 */
defineProps({
  color: {
    type: String,
    default: "blue",
  },
  variant: {
    type: String,
    default: "soft",
  },
  size: {
    type: String,
    default: "sm",
  },
});

/* ---- 各变体的颜色映射表 ---- */

// soft：半透明背景（适合深色主题）
const colorSoft = {
  blue: "bg-blue-600/20 text-blue-400",
  green: "bg-green-600/20 text-green-400",
  red: "bg-red-600/20 text-red-400",
  yellow: "bg-yellow-600/20 text-yellow-400",
  purple: "bg-purple-600/20 text-purple-400",
  orange: "bg-orange-600/20 text-orange-400",
  indigo: "bg-indigo-600/20 text-indigo-400",
  gray: "bg-gray-600/20 text-gray-400",
};

// solid：实心填充
const colorSolid = {
  blue: "bg-blue-600 text-white",
  green: "bg-green-600 text-white",
  red: "bg-red-600 text-white",
  yellow: "bg-yellow-600 text-white",
  purple: "bg-purple-600 text-white",
  orange: "bg-orange-600 text-white",
  indigo: "bg-indigo-600 text-white",
  gray: "bg-gray-600 text-white",
};

// outline：仅描边
const colorOutline = {
  blue: "border border-blue-500/50 text-blue-400",
  green: "border border-green-500/50 text-green-400",
  red: "border border-red-500/50 text-red-400",
  yellow: "border border-yellow-500/50 text-yellow-400",
  purple: "border border-purple-500/50 text-purple-400",
  orange: "border border-orange-500/50 text-orange-400",
  indigo: "border border-indigo-500/50 text-indigo-400",
  gray: "border border-gray-500/50 text-gray-400",
};

// 变体名到颜色映射表的查找
const variantMap = { soft: colorSoft, solid: colorSolid, outline: colorOutline };

// 尺寸映射
const sizeClasses = { sm: "px-2 py-0.5 text-xs", md: "px-2.5 py-1 text-sm" };
</script>

<template>
  <!--
    注意：导出 variantMap 和 sizeClasses 是为了方便
    父组件在需要时直接引用（虽然本组件内并未直接使用 export）。
    :class 优先级：显式传入的 color → 默认 blue
  -->
  <span
    :class="[
      'inline-flex items-center font-semibold rounded anim-fade-in',
      (variantMap[variant] || colorSoft)[color] || colorSoft.blue,
      sizeClasses[size] || sizeClasses.sm,
    ]"
  >
    <!-- 默认插槽：徽章文字 -->
    <slot />
  </span>
</template>
