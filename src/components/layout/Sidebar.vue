<!-- ============================================================
  Sidebar.vue — 左侧导航侧边栏（Win11 经典风格）
  ============================================================
  职责：
  - 固定在窗口左侧，位于标题栏下方
  - 默认收起状态（w-14），鼠标悬停展开（w-48）
  - 主导航项在上方，设置按钮固定在底部
  - 所有图标居中对齐
  - 当前激活项以 Win11 蓝色指示条标记
  ============================================================ -->

<script setup>
// ---------- 组件导入 ----------
import AppIcon from "../common/AppIcon.vue";

// ---------- Props & Emits 定义 ----------
const props = defineProps({
  active: {
    type: String,
    default: "home",
  },
});

const emit = defineEmits(["navigate"]);

// ---------- 菜单配置 ----------
const menuItems = [
  { id: "home", label: "首页", icon: "home" },
  { id: "versions", label: "版本管理", icon: "versions" },
  { id: "download", label: "下载", icon: "download" },
];

/** 导航点击处理 */
function go(page) {
  emit("navigate", page);
}
</script>

<template>
  <aside
    class="fixed left-1 top-[48px] bottom-1 z-40 flex flex-col"
    :class="[
      'transition-all duration-200 ease-out w-14 hover:w-48 group overflow-hidden rounded-2xl shadow-lg',
      'bg-[var(--glass-bg)] backdrop-blur-md border border-[var(--glass-border)] shadow-[var(--glass-shadow)]'
    ]"
  >
    <!-- ---- 导航菜单（顶部分组）---- -->
    <nav class="flex flex-col flex-1 py-2 min-h-0">
      <div
        v-for="item in menuItems"
        :key="item.id"
        class="relative flex items-center h-11 px-4 cursor-pointer whitespace-nowrap transition-all duration-150 ease-out active:scale-95 mx-1 rounded-xl"
        :class="
          active === item.id
            ? 'text-[var(--text-accent)] bg-[var(--bg-hover)]'
            : 'text-[var(--text-tertiary)] hover:bg-[var(--bg-hover)] hover:text-[var(--text-secondary)]'
        "
        @click="go(item.id)"
      >
        <span
          v-if="active === item.id"
          class="absolute left-0 top-1/2 -translate-y-1/2 w-[3px] h-[23px] bg-[var(--text-accent)] rounded-r-full"
        />
        <span class="relative z-10 flex items-center justify-center w-5 h-5 flex-shrink-0">
          <AppIcon :name="item.icon" size="4" />
        </span>
        <span
          class="relative z-10 ml-3 text-sm font-medium leading-none opacity-0 group-hover:opacity-100 transition-all duration-200 delay-0 group-hover:delay-75 truncate"
        >
          {{ item.label }}
        </span>
      </div>
    </nav>

    <!-- ---- 底部分组：用户 + 设置 ---- -->
    <div class="flex flex-col py-2 border-t border-[var(--border-base)]">
      <!-- 用户入口 -->
      <div
        class="relative flex items-center h-11 mx-1 px-4 rounded-xl cursor-pointer whitespace-nowrap transition-all duration-150 ease-out active:scale-95"
        :class="
          active === 'user'
            ? 'text-[var(--text-accent)] bg-[var(--bg-hover)]'
            : 'text-[var(--text-tertiary)] hover:bg-[var(--bg-hover)] hover:text-[var(--text-secondary)]'
        "
        @click="go('user')"
      >
        <span
          v-if="active === 'user'"
          class="absolute left-0 top-1/2 -translate-y-1/2 w-[3px] h-[23px] bg-[var(--text-accent)] rounded-r-full"
        />
        <span class="flex items-center justify-center w-5 h-5 flex-shrink-0">
          <AppIcon name="user" size="4" />
        </span>
        <span
          class="ml-3 text-sm font-medium leading-none opacity-0 group-hover:opacity-100 transition-all duration-200 delay-0 group-hover:delay-75 truncate"
        >
          用户
        </span>
      </div>

      <!-- 设置按钮（固定在最下端） -->
      <div
        class="relative flex items-center h-11 mx-1 mb-1.5 px-4 rounded-xl cursor-pointer whitespace-nowrap transition-all duration-150 ease-out active:scale-95"
        :class="
          active === 'settings'
            ? 'text-[var(--text-accent)] bg-[var(--bg-hover)]'
            : 'text-[var(--text-tertiary)] hover:bg-[var(--bg-hover)] hover:text-[var(--text-secondary)]'
        "
        @click="go('settings')"
      >
        <span
          v-if="active === 'settings'"
          class="absolute left-0 top-1/2 -translate-y-1/2 w-[3px] h-[23px] bg-[var(--text-accent)] rounded-r-full"
        />
        <span class="flex items-center justify-center w-5 h-5 flex-shrink-0">
          <AppIcon name="settings" size="4" />
        </span>
        <span
          class="ml-3 text-sm font-medium leading-none opacity-0 group-hover:opacity-100 transition-all duration-200 delay-0 group-hover:delay-75 truncate"
        >
          设置
        </span>
      </div>
    </div>
  </aside>
</template>