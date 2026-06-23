<!-- ============================================================
  AppButton.vue — 通用按钮组件
  ============================================================
  功能：
  - 提供四种样式变体：primary / secondary / ghost / danger
  - 三种尺寸：sm / md / lg
  - 支持禁用状态、全宽模式
  - 内置点击缩放反馈（active:scale-95）和焦点环
  - 通过具名插槽 "icon" 支持图标+文字组合

  使用方式：
  <AppButton variant="primary" size="md" @click="handler">
    <template #icon><AppIcon name="launch" /></template>
    启动
  </AppButton>
  ============================================================ -->

<script setup>
/**
 * Props 定义
 *
 * @prop {string} variant  - 样式变体
 *   "primary"(默认): 蓝色实心，用于主要操作
 *   "secondary": 灰色边框，用于次要操作
 *   "ghost": 透明背景，用于轻量操作
 *   "danger": 红色实心，用于删除/危险操作
 *
 * @prop {string} size     - 尺寸
 *   "sm": 小号（text-xs）
 *   "md"(默认): 中号（text-sm）
 *   "lg": 大号（text-base）
 *
 * @prop {boolean} disabled  - 禁用状态
 * @prop {boolean} fullWidth - 是否撑满父容器宽度
 */
defineProps({
  variant: {
    type: String,
    default: "primary",
  },
  size: {
    type: String,
    default: "md",
  },
  disabled: {
    type: Boolean,
    default: false,
  },
  fullWidth: {
    type: Boolean,
    default: false,
  },
});

/**
 * variant 样式映射表
 * 每种变体包含：背景、悬停、点击、文字颜色
 * hover: 鼠标悬停 / active: 鼠标按下
 */
const variantClasses = {
  primary:
    "bg-blue-600 hover:bg-blue-700 active:bg-blue-800 text-white shadow-sm shadow-blue-600/20",
  secondary:
    "bg-gray-700 hover:bg-gray-600 active:bg-gray-500 text-gray-200 border border-gray-600",
  ghost:
    "bg-transparent hover:bg-gray-700/60 active:bg-gray-700 text-gray-400 hover:text-gray-200 active:text-white",
  danger:
    "bg-red-600 hover:bg-red-700 active:bg-red-800 text-white shadow-sm shadow-red-600/20",
};

/**
 * size 尺寸映射表
 */
const sizeClasses = {
  sm: "px-2.5 py-1 text-xs",
  md: "px-3 py-1.5 text-sm",
  lg: "px-4 py-2 text-base",
};
</script>

<template>
  <!--
    button 根元素
    :class 使用数组语法动态组合多个样式源
    :disabled 控制原生禁用行为
  -->
  <button
    :class="[
      // 基础样式：flex 居中布局、圆角、字体
      'inline-flex items-center justify-center gap-1.5 font-medium rounded-lg',
      // 过渡动画：150ms ease-out 用于 hover/active 状态切换
      'transition-all duration-150 ease-out',
      // 点击缩放反馈效果
      'active:scale-95',
      // 键盘焦点指示环（可访问性）
      'focus:outline-none focus:ring-2 focus:ring-blue-500/50',
      // 变体和尺寸样式
      variantClasses[variant] || variantClasses.primary,
      sizeClasses[size] || sizeClasses.md,
      // 全宽模式
      fullWidth ? 'w-full' : '',
      // 禁用状态：半透明 + 禁止光标 + 取消点击缩放
      disabled ? 'opacity-50 cursor-not-allowed active:scale-100' : 'cursor-pointer',
    ]"
    :disabled="disabled"
  >
    <!-- 图标插槽（可选），位于文字左侧 -->
    <slot name="icon" />
    <!-- 默认插槽：按钮文字 -->
    <slot />
  </button>
</template>
