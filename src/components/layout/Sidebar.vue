<!-- ============================================================
  Sidebar.vue — 左侧导航侧边栏
  ============================================================
  职责：
  - 固定在窗口左侧，位于标题栏下方
  - 默认收起状态（w-14），鼠标悬停展开（w-48）
  - 提供应用主导航菜单项：首页、版本管理、下载、设置
  - 当前激活项以 WinUI 风格蓝色指示条标记
  - 底部预留用户入口

  接收 Props：
  - active: String — 当前激活的菜单项 ID（默认 "home"）

  使用方式：
  <Sidebar active="home" />

  依赖：
  - AppIcon.vue — 通用图标组件
  ============================================================ -->

<script setup>
// ---------- 组件导入 ----------
import AppIcon from "../common/AppIcon.vue";

// ---------- Props 定义 ----------
/**
 * active — 当前选中的菜单项 ID
 * 由父组件（AppLayout）传入，用于控制高亮显示
 */
defineProps({
  active: {
    type: String,
    default: "home",
  },
});

// ---------- 菜单配置 ----------
/**
 * menuItems — 导航菜单项数据
 * 每个菜单项包含：
 *   - id:     唯一标识符，与 active 值对应
 *   - label:  显示文字
 *   - icon:   对应 AppIcon 的 name 属性
 * 如需新增菜单项，直接在此数组中添加即可。
 */
const menuItems = [
  { id: "home", label: "首页", icon: "home" },
  { id: "versions", label: "版本管理", icon: "versions" },
  { id: "download", label: "下载", icon: "download" },
  { id: "settings", label: "设置", icon: "settings" },
];
</script>

<template>
  <!--
    侧边栏容器：
    关键样式说明：
      - fixed left-1 top-[44px] bottom-1：在标题栏下方并留出边距
      - bg-gray-600/30 + backdrop-blur-md：半透明毛玻璃
      - w-14 hover:w-48：默认窄条，悬停展开（group 控制子元素显隐）
      - rounded-2xl + shadow-lg：圆角悬浮卡片风格
      - overflow-hidden：展开时内容不溢出
  -->
  <aside
    class="fixed left-1 top-[44px] bottom-1 z-40 flex flex-col bg-gray-600/30 backdrop-blur-md text-white transition-all duration-200 ease-out w-14 hover:w-48 group overflow-hidden rounded-2xl shadow-lg shadow-black/10 border border-white/10"
  >
    <!-- ---- 导航菜单 ---- -->
    <nav class="flex flex-col flex-1 py-2">
      <!--
        v-for 遍历 menuItems 渲染菜单项
        关键样式说明：
          - active:scale-95：点击缩放反馈
          - mx-1 rounded-xl：圆角矩形背景
          - text-white bg-white/10：激活状态样式
          - hover:bg-white/10：非激活项悬停样式
      -->
      <div
        v-for="(item, index) in menuItems"
        :key="item.id"
        class="relative flex items-center h-11 px-5 cursor-pointer whitespace-nowrap transition-all duration-150 ease-out active:scale-95 mx-1 rounded-xl"
        :class="
          active === item.id
            ? 'text-white bg-white/10'
            : 'text-gray-500 hover:bg-white/10 hover:text-gray-200'
        "
      >
        <!--
          WinUI 风格激活指示条：
          仅当前激活项显示，位于按钮左侧边缘
          w-[3px] × h-[23px] 蓝色矩形条，
          使用 top-1/2 -translate-y-1/2 垂直居中
        -->
        <span
          v-if="active === item.id"
          class="absolute left-0 top-1/2 -translate-y-1/2 w-[3px] h-[23px] bg-blue-400"
        />

        <!-- 菜单图标：固定 20×20 容器居中展示 -->
        <span class="relative z-10 flex items-center justify-center w-5 h-5">
          <AppIcon :name="item.icon" size="4" />
        </span>

        <!-- 菜单文字：悬停时才显示（opacity-0 → group-hover:opacity-100）
             leading-none 消除行高导致的垂直偏移 -->
        <span
          class="relative z-10 ml-3 text-sm font-medium leading-none opacity-0 group-hover:opacity-100 transition-all duration-200 delay-0 group-hover:delay-75"
        >
          {{ item.label }}
        </span>
      </div>
    </nav>

    <!-- ---- 底部用户入口 ----
         与菜单项样式保持一致，当前无点击逻辑 -->
    <div class="flex items-center h-11 mx-1 mb-1.5 px-5 rounded-xl text-gray-500 hover:bg-white/10 hover:text-gray-200 cursor-pointer transition-all duration-150 ease-out active:scale-95 whitespace-nowrap">
      <span class="flex items-center justify-center w-5 h-5">
        <AppIcon name="user" size="4" />
      </span>
      <span class="ml-3 text-sm font-medium leading-none opacity-0 group-hover:opacity-100 transition-all duration-200 delay-0 group-hover:delay-75">
        用户
      </span>
    </div>
  </aside>
</template>