<!-- ============================================================
  Home.vue — 首页视图
  ============================================================
  功能：
  - 展示欢迎信息和应用简介
  - 显示三个版本卡片（最新版、推荐版、快照），每张卡片含启动按钮
  - 接收并展示系统消息/通知中心
  - 使用错落动画（anim-stagger）实现卡片逐张入场

  使用的公共组件：
  - AppSection:   区块标题
  - AppCard:      版本信息卡片
  - AppBadge:     版本号标签
  - AppButton:    启动按钮
  - AppIcon:      图标
  - MessageCenter:消息通知组件
  ============================================================ -->

<script setup>
import { ref } from "vue";

// ---------- 公共组件导入 ----------
import AppSection from "../components/common/AppSection.vue";
import AppCard from "../components/common/AppCard.vue";
import AppBadge from "../components/common/AppBadge.vue";
import AppButton from "../components/common/AppButton.vue";
import AppIcon from "../components/common/AppIcon.vue";

// ---------- 消息通知组件导入 ----------
import MessageCenter from "../components/common/MessageCenter.vue";

// ---------- 版本卡片数据 ----------
const cards = ref([
  {
    title: "最新版本",
    version: "1.20.4",
    desc: "最新正式版，包含最新特性与修复",
    color: "blue",
  },
  {
    title: "推荐版本",
    version: "1.20.1",
    desc: "稳定可靠，兼容性最佳的版本",
    color: "green",
  },
  {
    title: "最新快照",
    version: "24w14a",
    desc: "实验性功能，抢先体验新内容",
    color: "purple",
  },
]);
</script>

<template>
  <!-- 居中限制最大宽度 -->
  <div class="max-w-5xl mx-auto">

    <!-- === 欢迎区块 === -->
    <AppSection title="欢迎使用 Aether Minecraft Launcher" description="快速启动和管理你的 Minecraft 版本" />

    <!-- === 版本卡片网格 ===
         3列响应式网格，大屏3列，小屏1列
         每张卡片使用 anim-slide-up + anim-stagger-N 实现逐张上滑入场 -->
    <AppSection>
      <div class="grid grid-cols-1 md:grid-cols-3 gap-4">
        <AppCard
          v-for="(card, index) in cards"
          :key="index"
          padding="p-5"
          :class="`anim-slide-up anim-stagger-${index + 1}`"
        >
          <!-- 卡片头部：标题 + 版本号徽章 -->
          <template #header>
            <h3 class="text-sm font-medium text-[var(--text-secondary)]">{{ card.title }}</h3>
            <AppBadge :color="card.color">{{ card.version }}</AppBadge>
          </template>
          <!-- 卡片描述 -->
          <p class="text-sm text-[var(--text-tertiary)]">{{ card.desc }}</p>
          <!-- 启动按钮（全宽） -->
          <AppButton variant="primary" size="sm" fullWidth class="mt-4">
            <AppIcon name="launch" size="3.5" />
            启动
          </AppButton>
        </AppCard>
      </div>
    </AppSection>

    <!-- === 消息中心区块 ===
         使用 MessageCenter 组件，接收后端通知和系统消息 -->
    <AppSection title="消息中心" description="启动器消息与服务器通知">
      <MessageCenter />
    </AppSection>
  </div>
</template>
