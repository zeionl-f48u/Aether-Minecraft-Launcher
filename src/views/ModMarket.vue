<!-- ============================================================
  ModMarket.vue — 模组市场页面
  ============================================================
  功能：
  - 搜索 Modrinth / CurseForge 模组
  - 查看模组详情和版本
  ============================================================ -->

<script setup>
import { ref } from "vue";
import AppSection from "../components/common/AppSection.vue";
import AppCard from "../components/common/AppCard.vue";
import AppButton from "../components/common/AppButton.vue";
import AppIcon from "../components/common/AppIcon.vue";
import AppBadge from "../components/common/AppBadge.vue";
import { api } from "../composables/useTauriApi.js";

const searchQuery = ref("");
const searchResults = ref([]);
const loading = ref(false);
const error = ref("");
const activeSource = ref("modrinth"); // modrinth | curseforge
const selectedMod = ref(null);
const modVersions = ref([]);

async function doSearch() {
  if (!searchQuery.value.trim()) return;
  loading.value = true;
  error.value = "";
  searchResults.value = [];
  selectedMod.value = null;
  try {
    if (activeSource.value === "modrinth") {
      searchResults.value = await api.searchModrinth({ query: searchQuery.value.trim() });
    } else {
      searchResults.value = await api.searchCurseForge({ query: searchQuery.value.trim() });
    }
  } catch (e) {
    error.value = String(e);
  } finally {
    loading.value = false;
  }
}

async function viewModVersions(mod) {
  selectedMod.value = mod;
  modVersions.value = [];
  error.value = "";
  try {
    modVersions.value = await api.getModrinthVersions(mod.project_id);
  } catch (e) {
    error.value = String(e);
  }
}

function goBack() {
  selectedMod.value = null;
  modVersions.value = [];
}
</script>

<template>
  <div class="max-w-5xl mx-auto">
    <AppSection title="模组市场" description="从 Modrinth 和 CurseForge 搜索并浏览模组">

      <div v-if="error" class="p-3 mb-4 text-sm text-red-400 bg-red-500/10 rounded-lg">{{ error }}</div>

      <!-- 搜索栏 -->
      <AppCard padding="p-5">
        <div class="flex items-end gap-3">
          <div class="flex-1">
            <label class="block text-xs text-[var(--text-tertiary)] mb-1">搜索关键词</label>
            <input v-model="searchQuery" placeholder="搜索模组..."
              class="w-full px-3 py-2 text-sm rounded-lg bg-[var(--bg-elevated)] border border-[var(--border-base)] text-[var(--text-primary)] focus:outline-none focus:border-[var(--border-accent)]"
              @keyup.enter="doSearch" />
          </div>
          <div class="flex gap-1 p-1 rounded-lg bg-[var(--bg-elevated)] border border-[var(--border-base)]">
            <button
              class="px-3 py-1.5 text-xs font-medium rounded-md transition-all"
              :class="activeSource === 'modrinth' ? 'bg-[var(--button-primary)] text-white' : 'text-[var(--text-tertiary)] hover:text-[var(--text-primary)]'"
              @click="activeSource = 'modrinth'"
            >Modrinth</button>
            <button
              class="px-3 py-1.5 text-xs font-medium rounded-md transition-all"
              :class="activeSource === 'curseforge' ? 'bg-[var(--button-primary)] text-white' : 'text-[var(--text-tertiary)] hover:text-[var(--text-primary)]'"
              @click="activeSource = 'curseforge'"
            >CurseForge</button>
          </div>
          <AppButton variant="primary" @click="doSearch" :disabled="loading || !searchQuery.trim()">
            <AppIcon name="search" size="3.5" />
            {{ loading ? '搜索中...' : '搜索' }}
          </AppButton>
        </div>
      </AppCard>

      <!-- 搜索结果 -->
      <div v-if="!selectedMod && searchResults.length > 0" class="mt-4 grid grid-cols-1 md:grid-cols-2 gap-3">
        <div
          v-for="mod in searchResults"
          :key="mod.project_id || mod.id"
          class="p-4 rounded-lg bg-[var(--card-bg)] border border-[var(--card-border)] hover:border-[var(--border-hover)] transition-all cursor-pointer"
          @click="viewModVersions(mod)"
        >
          <div class="flex items-start gap-3">
            <img v-if="mod.icon_url" :src="mod.icon_url" class="w-10 h-10 rounded-lg bg-[var(--bg-elevated)] object-cover" alt="" />
            <div v-else class="w-10 h-10 rounded-lg bg-[var(--bg-elevated)] flex items-center justify-center">
              <AppIcon name="mods" size="5" />
            </div>
            <div class="flex-1 min-w-0">
              <h4 class="text-sm font-medium text-[var(--text-primary)] truncate">{{ mod.title || mod.name }}</h4>
              <p class="text-xs text-[var(--text-tertiary)] line-clamp-2 mt-1">{{ mod.description || mod.summary }}</p>
              <span class="text-xs text-[var(--text-accent)] mt-1 inline-block">{{ mod.slug || `#${mod.id}` }}</span>
            </div>
          </div>
        </div>
      </div>

      <!-- 空结果 -->
      <div v-if="!selectedMod && !loading && searchResults.length === 0 && searchQuery.trim()" class="mt-8 py-8 text-center text-sm text-[var(--text-tertiary)]">
        未找到匹配的模组
      </div>

      <!-- 模组版本详情 -->
      <div v-if="selectedMod" class="mt-4">
        <AppCard padding="p-5">
          <div class="flex items-center gap-3 mb-4">
            <AppButton variant="ghost" size="sm" @click="goBack">
              <AppIcon name="chevron-left" size="3.5" /> 返回
            </AppButton>
            <h3 class="text-sm font-medium text-[var(--text-primary)]">{{ selectedMod.title || selectedMod.name }}</h3>
          </div>
          <p class="text-xs text-[var(--text-tertiary)] mb-4">{{ selectedMod.description || selectedMod.summary }}</p>

          <h4 class="text-xs font-medium text-[var(--text-secondary)] mb-2">可用版本</h4>
          <div v-if="modVersions.length > 0" class="space-y-2 max-h-80 overflow-y-auto">
            <div v-for="(ver, i) in modVersions" :key="i" class="p-3 rounded-lg bg-[var(--bg-hover)]">
              <div class="flex flex-wrap gap-1 mb-2">
                <AppBadge v-for="gv in ver.game_versions?.slice(0, 5)" :key="gv" color="blue" size="sm">{{ gv }}</AppBadge>
                <AppBadge v-for="ld in ver.loaders" :key="ld" color="purple" size="sm">{{ ld }}</AppBadge>
              </div>
              <div v-for="f in ver.files" :key="f.filename" class="flex items-center justify-between mt-1">
                <span class="text-xs text-[var(--text-tertiary)] truncate">{{ f.filename }}</span>
                <a v-if="f.url" :href="f.url" target="_blank" class="text-xs text-[var(--text-accent)] hover:underline flex items-center gap-1">
                  <AppIcon name="external-link" size="3" /> 下载
                </a>
              </div>
            </div>
          </div>
          <div v-else class="text-xs text-[var(--text-tertiary)] py-4 text-center">暂无版本信息</div>
        </AppCard>
      </div>
    </AppSection>
  </div>
</template>
