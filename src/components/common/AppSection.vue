<!-- ============================================================
  AppSection.vue — 通用区块容器组件
  ============================================================
  功能：
  - 统一页面区块的标题和描述样式
  - 接收 title（区块标题）和 description（区块描述）
  - 通过插槽承载区块内容
  - 内置 anim-fade-in 入场动画

  使用方式：
  <AppSection title="版本管理" description="管理已安装的版本">
    <AppCard>...</AppCard>
  </AppSection>
  ============================================================ -->

<script setup>
/**
 * Props 定义
 *
 * @prop {string} title       - 区块标题（可选）
 * @prop {string} description - 区块描述文字（可选）
 */
defineProps({
  title: {
    type: String,
    default: "",
  },
  description: {
    type: String,
    default: "",
  },
});
</script>

<template>
  <!-- mb-8：区块底部间距，与其他区块拉开距离 -->
  <div class="mb-8 anim-fade-in">
    <!--
      标题区：只有 title 或 description 非空时才渲染
      避免传入空字符串时产生多余的 DOM 节点
    -->
    <div v-if="title || description" class="mb-4">
      <h2 v-if="title" class="text-xl font-bold text-[var(--text-primary)]">{{ title }}</h2>
      <p v-if="description" class="mt-1 text-sm text-[var(--text-secondary)]">{{ description }}</p>
    </div>
    <!-- 默认插槽：区块内容 -->
    <slot />
  </div>
</template>
