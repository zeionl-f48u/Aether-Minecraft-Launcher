const fs = require('fs');
const content = `<script setup>
defineProps({
  active: {
    type: String,
    default: "home",
  },
});

const menuItems = [
  { id: "home", label: "首页" },
  { id: "versions", label: "版本管理" },
  { id: "download", label: "下载" },
  { id: "settings", label: "设置" },
];
</script>

<template>
  <aside
    class="fixed left-0 top-[38px] bottom-0 z-40 flex flex-col bg-gray-800 text-white transition-all duration-200 w-14 hover:w-48 group overflow-hidden"
  >
    <nav class="flex flex-col flex-1 py-2">
      <div
        v-for="item in menuItems"
        :key="item.id"
        class="flex items-center h-11 px-3.5 cursor-pointer transition-colors whitespace-nowrap"
        :class="
          active === item.id
            ? 'bg-gray-700 text-white border-l-2 border-blue-500'
            : 'text-gray-400 hover:bg-gray-700/60 hover:text-gray-200'
        "
      >
        <span v-if="item.id === 'home'" class="flex-shrink-0 w-5 h-5 flex items-center justify-center">
          <svg class="w-5 h-5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
            <path d="M3 9l9-7 9 7v11a2 2 0 01-2 2H5a2 2 0 01-2-2z" />
            <polyline points="9 22 9 12 15 12 15 22" />
          </svg>
        </span>
        <span v-else-if="item.id === 'versions'" class="flex-shrink-0 w-5 h-5 flex items-center justify-center">
          <svg class="w-5 h-5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
            <rect x="3" y="3" width="18" height="18" rx="2" ry="2" />
            <line x1="3" y1="9" x2="21" y2="9" />
            <line x1="9" y1="21" x2="9" y2="9" />
          </svg>
        </span>
        <span v-else-if="item.id === 'download'" class="flex-shrink-0 w-5 h-5 flex items-center justify-center">
          <svg class="w-5 h-5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
            <path d="M21 15v4a2 2 0 01-2 2H5a2 2 0 01-2-2v-4" />
            <polyline points="7 10 12 15 17 10" />
            <line x1="12" y1="15" x2="12" y2="3" />
          </svg>
        </span>
        <span v-else-if="item.id === 'settings'" class="flex-shrink-0 w-5 h-5 flex items-center justify-center">
          <svg class="w-5 h-5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
            <circle cx="12" cy="12" r="3" />
            <path d="M19.4 15a1.65 1.65 0 00.33 1.82l.06.06a2 2 0 010 2.83 2 2 0 01-2.83 0l-.06-.06a1.65 1.65 0 00-1.82-.33 1.65 1.65 0 00-1 1.51V21a2 2 0 01-2 2 2 2 0 01-2-2v-.09A1.65 1.65 0 009 19.4a1.65 1.65 0 00-1.82.33l-.06.06a2 2 0 01-2.83 0 2 2 0 010-2.83l.06-.06A1.65 1.65 0 004.68 15a1.65 1.65 0 00-1.51-1H3a2 2 0 01-2-2 2 2 0 012-2h.09A1.65 1.65 0 004.6 9a1.65 1.65 0 00-.33-1.82l-.06-.06a2 2 0 010-2.83 2 2 0 012.83 0l.06.06A1.65 1.65 0 009 4.68a1.65 1.65 0 001-1.51V3a2 2 0 012-2 2 2 0 012 2v.09a1.65 1.65 0 001 1.51 1.65 1.65 0 001.82-.33l.06-.06a2 2 0 012.83 0 2 2 0 010 2.83l-.06.06A1.65 1.65 0 0019.32 9a1.65 1.65 0 001.51 1H21a2 2 0 012 2 2 2 0 01-2 2h-.09a1.65 1.65 0 00-1.51 1z" />
          </svg>
        </span>
        <span class="ml-3 text-sm font-medium opacity-0 group-hover:opacity-100 transition-opacity duration-200">
          {{ item.label }}
        </span>
      </div>
    </nav>
    <div class="flex items-center h-11 px-3.5 text-gray-400 hover:bg-gray-700/60 hover:text-gray-200 cursor-pointer transition-colors whitespace-nowrap">
      <svg class="w-5 h-5 flex-shrink-0" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
        <path d="M20 21v-2a4 4 0 00-4-4H8a4 4 0 00-4 4v2" />
        <circle cx="12" cy="7" r="4" />
      </svg>
      <span class="ml-3 text-sm font-medium opacity-0 group-hover:opacity-100 transition-opacity duration-200">用户</span>
    </div>
  </aside>
</template>
`;
fs.writeFileSync('src/components/layout/Sidebar.vue', content, 'utf8');
console.log('Sidebar.vue written successfully');
