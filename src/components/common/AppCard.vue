<!-- ============================================================
  AppCard.vue — 通用卡片容器组件
  ============================================================
  功能：
  - 用于展示内容块的统一卡片样式
  - 支持可选的头部插槽（header）和默认内容插槽
  - 可控制 hover 效果开关
  - 内置 anim-scale-in 入场缩放动画

  使用方式：
  <AppCard>
    <template #header>
      <h3>标题</h3>
      <AppBadge>标签</AppBadge>
    </template>
    <p>卡片内容...</p>
  </AppCard>
  ============================================================ -->

<script setup>
/**
 * Props 定义
 *
 * @prop {boolean} hover  - 是否启用悬停效果（边框变色+阴影），默认 true
 * @prop {string}  padding - 内边距，默认 "p-5"，可传入任意 Tailwind padding 类
 */
defineProps({
  hover: {
    type: Boolean,
    default: true,
  },
  padding: {
    type: String,
    default: "p-5",
  },
});
</script>

<template>
  <!--
    卡片容器：
    - bg-gray-800 border-gray-700：深色卡片背景与边框
    - anim-scale-in：入场时从 0.92 倍缩放到 1
    - hover 开启时：悬停边框变亮 + 阴影加深
  -->
  <div
    :class="[
      'rounded-lg anim-scale-in transition-colors duration-200',
      'bg-[var(--card-bg)] border border-[var(--card-border)]',
      hover
        ? 'hover:border-[var(--border-hover)] hover:shadow-lg hover:shadow-[var(--glass-shadow)] transition-all duration-200 ease-out'
        : '',
      padding,
    ]"
  >
    <!--
      头部区域（可选）：
      通过 $slots.header 检测是否传入 header 插槽，
      如果没传入则不渲染此 div，避免多余空白。
      flex items-center justify-between：标题在左，操作/标签在右
    -->
    <div v-if="$slots.header" class="flex items-center justify-between mb-3">
      <slot name="header" />
    </div>

    <!-- 卡片主体内容 -->
    <div>
      <slot />
    </div>
  </div>
</template>
