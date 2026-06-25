<!-- ============================================================
  Versions.vue — 版本管理视图
  ============================================================
  功能：
  - 展示已安装的 Minecraft 版本列表
  - 表格形式显示版本号、类型、发布日期、加载器
  - 每行提供启动按钮
  - 使用错落动画（anim-stagger）实现行逐条滑入

  使用的公共组件：
  - AppSection: 页面标题
  - AppBadge:  版本类型标签（正式版/快照）
  - AppButton: 行内启动按钮
  - AppIcon:   启动图标
  ============================================================ -->

<script setup>
import { ref } from "vue";

// ---------- 公共组件导入 ----------
import AppSection from "../components/common/AppSection.vue";
import AppBadge from "../components/common/AppBadge.vue";
import AppButton from "../components/common/AppButton.vue";
import AppIcon from "../components/common/AppIcon.vue";

// ---------- 版本列表数据 ----------
/**
 * versions — 已安装的 Minecraft 版本列表
 * 每项包含：
 *   id:     版本号（如 "1.20.4"）
 *   type:   类型（"release" 正式版 / "snapshot" 快照）
 *   date:   发布日期
 *   loader: 对应 Mod 加载器
 */
const versions = ref([
  { id: "1.20.4", type: "release", date: "2023-12-07", loader: "Fabric" },
  { id: "1.20.1", type: "release", date: "2023-06-12", loader: "Forge" },
  { id: "1.19.4", type: "release", date: "2023-03-14", loader: "Quilt" },
  { id: "24w14a", type: "snapshot", date: "2024-04-03", loader: "Fabric" },
]);
</script>

<template>
  <!-- 居中限制最大宽度 -->
  <div class="max-w-5xl mx-auto">

    <!-- === 页面标题 === -->
    <AppSection title="版本管理" description="管理已安装的 Minecraft 版本" />

    <!-- === 版本列表表格 ===
         深色卡片容器包裹，overflow-x-auto 支持横向滚动 -->
    <div class="bg-[var(--card-bg)] border border-[var(--card-border)] rounded-lg overflow-hidden anim-slide-up">
      <div class="overflow-x-auto">
        <table class="w-full text-sm text-left">
          <!-- 表头 -->
          <thead class="text-xs text-[var(--text-tertiary)] uppercase bg-[var(--bg-hover)]">
            <tr>
              <th class="px-4 py-3">版本</th>
              <th class="px-4 py-3">类型</th>
              <th class="px-4 py-3">发布日期</th>
              <th class="px-4 py-3">加载器</th>
              <th class="px-4 py-3">操作</th>
            </tr>
          </thead>
          <!-- 表体：每行使用错落动画从左滑入 -->
          <tbody>
            <tr
              v-for="(v, index) in versions"
              :key="index"
              class="border-b border-[var(--border-base)] hover:bg-[var(--bg-hover)] transition-colors duration-150 anim-slide-left"
              :class="`anim-stagger-${index + 1}`"
            >
              <!-- 版本号 -->
              <td class="px-4 py-3 font-medium text-[var(--text-primary)]">{{ v.id }}</td>
              <!-- 类型徽章：正式版绿色，快照黄色 -->
              <td class="px-4 py-3">
                <AppBadge :color="v.type === 'release' ? 'green' : 'yellow'" size="sm">
                  {{ v.type === "release" ? "正式版" : "快照" }}
                </AppBadge>
              </td>
              <!-- 发布日期 -->
              <td class="px-4 py-3 text-[var(--text-secondary)]">{{ v.date }}</td>
              <!-- Mod 加载器 -->
              <td class="px-4 py-3 text-[var(--text-secondary)]">{{ v.loader }}</td>
              <!-- 启动按钮 -->
              <td class="px-4 py-3">
                <AppButton variant="primary" size="sm">
                  <AppIcon name="launch" size="3" />
                  启动
                </AppButton>
              </td>
            </tr>
          </tbody>
        </table>
      </div>
    </div>
  </div>
</template>
