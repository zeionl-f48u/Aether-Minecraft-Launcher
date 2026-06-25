<!-- ============================================================
  Download.vue — 下载管理页面（统一版本安装界面）
  ============================================================
  功能：
  - 主Tab：版本安装 / 模组市场 / 整合包(占位) / 服务端(占位)
  - 版本安装流程：选版本 → 选加载器+OptiFine → 安装
  ============================================================ -->

<script setup>
import { ref, onMounted, watch } from "vue";
import AppSection from "../components/common/AppSection.vue";
import AppCard from "../components/common/AppCard.vue";
import AppButton from "../components/common/AppButton.vue";
import AppIcon from "../components/common/AppIcon.vue";
import AppBadge from "../components/common/AppBadge.vue";
import { api } from "../composables/useTauriApi.js";

// ======================================================================
//  Tab 配置
// ======================================================================
const tabs = [
  { id: "version",  label: "版本", icon: "download" },
  { id: "modmarket", label: "模组市场", icon: "search" },
  { id: "modpack",   label: "整合包", icon: "mods" },
  { id: "serversoft",label: "服务端", icon: "game" },
];
const activeTab = ref("version");
const loading = ref(false);
const error = ref("");

// ======================================================================
//  版本安装流程
// ======================================================================

// --- 第1步：版本选择 ---
const vanillaVersions = ref([]);
const selectedVersion = ref(null);     // 用户选中的版本对象

function selectVersion(v) {
  selectedVersion.value = v;
  // 选中版本后自动获取该版本可用的各加载器信息
  if (v) {
    loadLoaderVersions(v.id);
  }
}

function backToVersionList() {
  selectedVersion.value = null;
}

// --- 第2步：Mod 加载器选择 ---
const loaderTabs = [
  { id: "none",     label: "无" },
  { id: "fabric",   label: "Fabric" },
  { id: "forge",    label: "Forge" },
  { id: "neoforge", label: "NeoForge" },
  { id: "quiltmc",  label: "QuiltMC" },
];
const activeLoader = ref("none");
const installing = ref(false);

// 各加载器版本数据
const fabricLoaders = ref([]);
const fabricSelectedLoader = ref("");
const forgeVersions = ref(null);
const forgeSelectedVersion = ref("");
const neoforgeVersions = ref(null);
const neoforgeSelectedVersion = ref("");
const quiltmcLoaders = ref([]);
const quiltmcSelectedLoader = ref("");

async function loadLoaderVersions(versionId) {
  loading.value = true;
  error.value = "";
  try {
    // 并行获取所有加载器信息
    const [fab, forg, neo, quilt] = await Promise.allSettled([
      api.getFabricLoaders(versionId),
      api.getForgeVersions(versionId),
      api.getNeoForgeVersions(versionId),
      api.getQuiltMCLoaders(versionId),
    ]);

    if (fab.status === "fulfilled") {
      fabricLoaders.value = fab.value;
      if (fab.value.length > 0) fabricSelectedLoader.value = fab.value[0].loader;
    }
    if (forg.status === "fulfilled") {
      forgeVersions.value = forg.value;
      forgeSelectedVersion.value = forg.value.recommended || forg.value.latest || (forg.value.all_versions?.[0]) || "";
    }
    if (neo.status === "fulfilled") {
      neoforgeVersions.value = neo.value;
      neoforgeSelectedVersion.value = neo.value.latest || (neo.value.all_versions?.[0]) || "";
    }
    if (quilt.status === "fulfilled") {
      quiltmcLoaders.value = quilt.value;
      if (quilt.value.length > 0) quiltmcSelectedLoader.value = quilt.value[0].loader;
    }
  } catch (e) {
    error.value = String(e);
  } finally {
    loading.value = false;
  }
}

// --- 第3步：OptiFine ---
const enableOptifine = ref(false);
const optifineVersions = ref([]);
const optifineSelected = ref(null);
const optifineAsMod = ref(true); // 默认作为模组安装

// 当启用 OptiFine 且选中版本时，加载 OptiFine 版本
async function loadOptifineForVersion(versionId) {
  if (!enableOptifine.value || !versionId) return;
  try {
    optifineVersions.value = await api.getOptiFineVersions(versionId);
    if (optifineVersions.value.length > 0) {
      optifineSelected.value = optifineVersions.value[0];
    }
  } catch (e) {
    // OptiFine 不阻塞主流程
    console.warn("获取 OptiFine 版本失败:", e);
  }
}

// 监听启用 OptiFine 切换
watch(enableOptifine, (val) => {
  if (val && selectedVersion.value) {
    loadOptifineForVersion(selectedVersion.value.id);
  }
});

// --- 第4步：选项 ---
const versionIsolation = ref(true);
const isServer = ref(false); // 服务端下载（未来功能，默认关闭）

// --- 安装 ---
async function doInstall() {
  if (!selectedVersion.value) return;
  installing.value = true;
  error.value = "";
  const vid = selectedVersion.value.id;

  try {
    // 1. 安装原版
    await api.installVanilla(vid);

    // 2. 安装 Mod 加载器
    const loader = activeLoader.value;
    if (loader === "fabric" && fabricSelectedLoader.value) {
      await api.installFabric({
        versionName: `${vid}-fabric`,
        vanillaVersion: vid,
        loaderVersion: fabricSelectedLoader.value,
      });
    } else if (loader === "forge" && forgeSelectedVersion.value) {
      await api.installForge({
        versionName: `${vid}-forge`,
        vanillaVersion: vid,
        forgeVersion: forgeSelectedVersion.value,
      });
    } else if (loader === "neoforge" && neoforgeSelectedVersion.value) {
      await api.installNeoForge({
        versionName: `${vid}-neoforge`,
        vanillaVersion: vid,
        neoforgeVersion: neoforgeSelectedVersion.value,
      });
    } else if (loader === "quiltmc" && quiltmcSelectedLoader.value) {
      await api.installQuiltMC({
        versionName: `${vid}-quiltmc`,
        vanillaVersion: vid,
        loaderVersion: quiltmcSelectedLoader.value,
      });
    }

    // 3. 安装 OptiFine
    if (enableOptifine.value && optifineSelected.value) {
      const loaderSuffix = loader !== "none" ? `-${loader}` : "";
      await api.installOptiFine({
        versionName: `${vid}${loaderSuffix}-optifine`,
        vanillaVersion: vid,
        optifineType: optifineSelected.value.type,
        optifinePatch: optifineSelected.value.patch,
        asMod: optifineAsMod.value,
      });
    }

    // 4. 更新配置中的版本隔离设置
    const config = await api.getConfig();
    if (config.game_independent !== versionIsolation.value) {
      config.game_independent = versionIsolation.value;
      await api.updateConfig(config);
    }
  } catch (e) {
    error.value = String(e);
  } finally {
    installing.value = false;
  }
}

// ======================================================================
//  获取版本列表
// ======================================================================
async function loadVersions() {
  loading.value = true;
  error.value = "";
  try {
    vanillaVersions.value = await api.fetchVersionManifest();
  } catch (e) {
    error.value = String(e);
  } finally {
    loading.value = false;
  }
}

// ======================================================================
//  模组市场
// ======================================================================
const searchQuery = ref("");
const searchResults = ref([]);
const activeSource = ref("modrinth");
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

onMounted(loadVersions);
</script>

<template>
  <div class="max-w-5xl mx-auto">
    <AppSection title="下载管理" description="浏览并安装 Minecraft 版本和模组加载器">

      <!-- Tab 导航 -->
      <div class="flex gap-1 mb-6 p-1 rounded-xl bg-[var(--bg-elevated)] border border-[var(--border-base)]">
        <button v-for="tab in tabs" :key="tab.id"
          class="flex items-center gap-1.5 px-4 py-2 text-sm font-medium rounded-lg transition-all duration-150"
          :class="activeTab === tab.id ? 'bg-[var(--button-primary)] text-white shadow-sm' : 'text-[var(--text-tertiary)] hover:text-[var(--text-primary)] hover:bg-[var(--bg-hover)]'"
          @click="activeTab = tab.id">
          <AppIcon :name="tab.icon" size="3.5" />{{ tab.label }}
        </button>
      </div>

      <!-- 错误提示 -->
      <div v-if="error" class="p-3 mb-4 text-sm text-red-400 bg-red-500/10 rounded-lg">{{ error }}</div>

      <!-- ================================================================
          版本 Tab — 统一安装流程
          ================================================================ -->
      <div v-if="activeTab === 'version'">

        <!-- 第1步：版本选择（尚未选择版本时） -->
        <div v-if="!selectedVersion">
          <AppCard padding="p-5">
            <div class="flex items-center justify-between mb-4">
              <h3 class="text-sm font-medium text-[var(--text-secondary)]">选择要安装的 Minecraft 版本</h3>
              <AppButton variant="ghost" size="sm" @click="loadVersions" :disabled="loading">
                <AppIcon name="refresh" size="3.5" />{{ loading ? '加载中...' : '刷新' }}
              </AppButton>
            </div>
            <div v-if="loading" class="flex items-center justify-center py-8 text-sm text-[var(--text-tertiary)]">
              <svg class="w-4 h-4 mr-2 animate-spin" viewBox="0 0 24 24" fill="none">
                <circle class="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" stroke-width="4" />
                <path class="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4z" />
              </svg>正在获取版本列表...
            </div>
            <div v-else-if="vanillaVersions.length === 0" class="py-8 text-center text-sm text-[var(--text-tertiary)]">暂无可用版本</div>
            <div v-else class="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 gap-3 max-h-[500px] overflow-y-auto">
              <div v-for="(v, i) in vanillaVersions" :key="v.id"
                class="flex items-center justify-between p-3 rounded-lg bg-[var(--bg-hover)] hover:bg-[var(--bg-active)] transition-colors duration-150 anim-fade-in cursor-pointer"
                :class="`anim-stagger-${Math.min(i + 1, 5)}`"
                @click="selectVersion(v)">
                <div class="flex-1 min-w-0">
                  <span class="text-sm font-medium text-[var(--text-primary)] truncate block">{{ v.id }}</span>
                  <span class="text-xs text-[var(--text-tertiary)]">{{ v.version_type }}</span>
                </div>
                <AppIcon name="chevron-right" size="4" class="text-[var(--text-tertiary)] flex-shrink-0" />
              </div>
            </div>
          </AppCard>
        </div>

        <!-- 第2-4步：已选择版本 → 配置面板 -->
        <div v-else>
          <!-- 版本信息头 -->
          <div class="flex items-center gap-3 mb-4">
            <AppButton variant="ghost" size="sm" @click="backToVersionList">
              <AppIcon name="chevron-left" size="3.5" /> 返回
            </AppButton>
            <AppBadge color="blue" size="md">{{ selectedVersion.id }}</AppBadge>
            <AppBadge :color="selectedVersion.version_type === 'release' ? 'green' : 'yellow'" size="sm">
              {{ selectedVersion.version_type === 'release' ? '正式版' : '快照' }}
            </AppBadge>
          </div>

          <div class="grid grid-cols-1 lg:grid-cols-3 gap-4">
            <!-- 左列：Mod 加载器选择 -->
            <div class="lg:col-span-2 space-y-4">
              <!-- Loader 选择选项卡 -->
              <AppCard padding="p-4">
                <template #header>
                  <h3 class="text-sm font-medium text-[var(--text-secondary)]">Mod 加载器</h3>
                </template>
                <div class="flex gap-1 mb-4 p-1 rounded-lg bg-[var(--bg-elevated)] border border-[var(--border-base)]">
                  <button v-for="lt in loaderTabs" :key="lt.id"
                    class="flex-1 px-3 py-1.5 text-xs font-medium rounded-md transition-all"
                    :class="activeLoader === lt.id ? 'bg-[var(--button-primary)] text-white' : 'text-[var(--text-tertiary)] hover:text-[var(--text-primary)]'"
                    @click="activeLoader = lt.id">{{ lt.label }}</button>
                </div>

                <!-- Fabric -->
                <div v-if="activeLoader === 'fabric'">
                  <label class="block text-xs text-[var(--text-tertiary)] mb-1">选择 Fabric 加载器版本</label>
                  <select v-model="fabricSelectedLoader"
                    class="w-full px-3 py-2 text-sm rounded-lg bg-[var(--bg-elevated)] border border-[var(--border-base)] text-[var(--text-primary)]">
                    <option v-for="l in fabricLoaders" :key="l.loader" :value="l.loader">
                      {{ l.loader }} {{ l.stable ? '(稳定)' : '(预览)' }}
                    </option>
                  </select>
                  <p v-if="fabricLoaders.length === 0" class="text-xs text-[var(--text-tertiary)] mt-2">正在加载...</p>
                </div>

                <!-- Forge -->
                <div v-if="activeLoader === 'forge'">
                  <div class="flex gap-2 mb-2">
                    <AppBadge v-if="forgeVersions?.recommended" color="green" size="sm">推荐: {{ forgeVersions.recommended }}</AppBadge>
                    <AppBadge v-if="forgeVersions?.latest" color="blue" size="sm">最新: {{ forgeVersions.latest }}</AppBadge>
                  </div>
                  <label class="block text-xs text-[var(--text-tertiary)] mb-1">选择 Forge 版本</label>
                  <select v-model="forgeSelectedVersion"
                    class="w-full px-3 py-2 text-sm rounded-lg bg-[var(--bg-elevated)] border border-[var(--border-base)] text-[var(--text-primary)]">
                    <option v-for="v in forgeVersions?.all_versions || []" :key="v" :value="v">{{ v }}</option>
                  </select>
                  <p v-if="!forgeVersions" class="text-xs text-[var(--text-tertiary)] mt-2">正在加载...</p>
                </div>

                <!-- NeoForge -->
                <div v-if="activeLoader === 'neoforge'">
                  <label class="block text-xs text-[var(--text-tertiary)] mb-1">选择 NeoForge 版本</label>
                  <select v-model="neoforgeSelectedVersion"
                    class="w-full px-3 py-2 text-sm rounded-lg bg-[var(--bg-elevated)] border border-[var(--border-base)] text-[var(--text-primary)]">
                    <option v-for="v in neoforgeVersions?.all_versions || []" :key="v" :value="v">{{ v }}</option>
                  </select>
                  <p v-if="!neoforgeVersions" class="text-xs text-[var(--text-tertiary)] mt-2">正在加载...</p>
                </div>

                <!-- QuiltMC -->
                <div v-if="activeLoader === 'quiltmc'">
                  <label class="block text-xs text-[var(--text-tertiary)] mb-1">选择 QuiltMC 加载器版本</label>
                  <select v-model="quiltmcSelectedLoader"
                    class="w-full px-3 py-2 text-sm rounded-lg bg-[var(--bg-elevated)] border border-[var(--border-base)] text-[var(--text-primary)]">
                    <option v-for="l in quiltmcLoaders" :key="l.loader" :value="l.loader">{{ l.loader }}</option>
                  </select>
                  <p v-if="quiltmcLoaders.length === 0" class="text-xs text-[var(--text-tertiary)] mt-2">正在加载...</p>
                </div>

                <!-- None -->
                <div v-if="activeLoader === 'none'" class="py-4 text-center text-xs text-[var(--text-tertiary)]">
                  仅安装纯净原版，不安装任何 Mod 加载器
                </div>
              </AppCard>

              <!-- OptiFine 选项 -->
              <AppCard padding="p-4">
                <template #header>
                  <h3 class="text-sm font-medium text-[var(--text-secondary)]">OptiFine</h3>
                </template>
                <div class="flex items-center gap-2 mb-3">
                  <input id="enableOptifine" v-model="enableOptifine" type="checkbox"
                    class="rounded bg-[var(--bg-elevated)] border-[var(--border-base)] text-[var(--button-primary)]" />
                  <label for="enableOptifine" class="text-xs text-[var(--text-tertiary)]">同时安装 OptiFine</label>
                </div>
                <div v-if="enableOptifine">
                  <div class="grid gap-2 max-h-40 overflow-y-auto mb-3">
                    <div v-for="of in optifineVersions" :key="of.patch"
                      class="flex items-center justify-between p-2 rounded-lg cursor-pointer transition-colors"
                      :class="optifineSelected?.patch === of.patch ? 'bg-blue-500/10 border border-blue-500/30' : 'hover:bg-[var(--bg-hover)]'"
                      @click="optifineSelected = of">
                      <span class="text-xs text-[var(--text-primary)]">{{ of.type }} {{ of.patch }}</span>
                      <span class="text-xs text-[var(--text-tertiary)]">{{ of.filename }}</span>
                    </div>
                  </div>
                  <div v-if="optifineVersions.length === 0" class="text-xs text-[var(--text-tertiary)] mb-3">
                    {{ loading ? '正在加载...' : '该版本暂无可用 OptiFine' }}
                  </div>
                  <div class="flex items-center gap-2">
                    <input id="optifineAsMod" v-model="optifineAsMod" type="checkbox"
                      class="rounded bg-[var(--bg-elevated)] border-[var(--border-base)] text-[var(--button-primary)]" />
                    <label for="optifineAsMod" class="text-xs text-[var(--text-tertiary)]">作为模组安装（而非独立版本）</label>
                  </div>
                </div>
              </AppCard>
            </div>

            <!-- 右列：选项 + 安装按钮 -->
            <div class="space-y-4">
              <AppCard padding="p-4">
                <template #header>
                  <h3 class="text-sm font-medium text-[var(--text-secondary)]">安装选项</h3>
                </template>
                <div class="space-y-3">
                  <div class="flex items-center gap-2">
                    <input id="versionIsolation" v-model="versionIsolation" type="checkbox"
                      class="rounded bg-[var(--bg-elevated)] border-[var(--border-base)] text-[var(--button-primary)]" />
                    <label for="versionIsolation" class="text-xs text-[var(--text-tertiary)]">版本隔离（默认开启）</label>
                  </div>
                  <div class="flex items-center gap-2 opacity-50 pointer-events-none">
                    <input id="isServer" v-model="isServer" type="checkbox" disabled
                      class="rounded bg-[var(--bg-elevated)] border-[var(--border-base)]" />
                    <label for="isServer" class="text-xs text-[var(--text-tertiary)]">服务端下载（即将支持）</label>
                  </div>
                </div>
              </AppCard>

              <!-- 安装按钮 -->
              <AppButton variant="primary" size="lg" fullWidth @click="doInstall" :disabled="installing">
                <AppIcon name="download" size="4" />
                {{ installing ? '安装中...' : '开始安装' }}
              </AppButton>

              <!-- 安装概要 -->
              <AppCard padding="p-3" :hover="false">
                <h4 class="text-xs font-medium text-[var(--text-secondary)] mb-2">安装概要</h4>
                <div class="space-y-1 text-xs text-[var(--text-tertiary)]">
                  <div>• 版本: <span class="text-[var(--text-primary)]">{{ selectedVersion.id }}</span></div>
                  <div>• 加载器: <span class="text-[var(--text-primary)]">{{ activeLoader === 'none' ? '无' : activeLoader }}</span></div>
                  <div v-if="enableOptifine">• OptiFine: <span class="text-[var(--text-primary)]">{{ optifineSelected?.patch || '待选' }}</span></div>
                  <div>• 版本隔离: <span class="text-[var(--text-primary)]">{{ versionIsolation ? '开启' : '关闭' }}</span></div>
                </div>
              </AppCard>
            </div>
          </div>
        </div>
      </div>

      <!-- ================================================================
          模组市场 Tab
          ================================================================ -->
      <div v-if="activeTab === 'modmarket'">
        <AppCard padding="p-5">
          <div class="flex items-center gap-3 mb-4">
            <div class="flex-1">
              <label class="block text-xs text-[var(--text-tertiary)] mb-1">搜索模组</label>
              <input v-model="searchQuery" placeholder="搜索关键词..."
                class="w-full px-3 py-2 text-sm rounded-lg bg-[var(--bg-elevated)] border border-[var(--border-base)] text-[var(--text-primary)] focus:outline-none focus:border-[var(--border-accent)]"
                @keyup.enter="doSearch" />
            </div>
            <div class="flex gap-1 p-1 rounded-lg bg-[var(--bg-elevated)] border border-[var(--border-base)] self-end">
              <button class="px-3 py-1.5 text-xs font-medium rounded-md transition-all"
                :class="activeSource === 'modrinth' ? 'bg-[var(--button-primary)] text-white' : 'text-[var(--text-tertiary)] hover:text-[var(--text-primary)]'"
                @click="activeSource = 'modrinth'">Modrinth</button>
              <button class="px-3 py-1.5 text-xs font-medium rounded-md transition-all"
                :class="activeSource === 'curseforge' ? 'bg-[var(--button-primary)] text-white' : 'text-[var(--text-tertiary)] hover:text-[var(--text-primary)]'"
                @click="activeSource = 'curseforge'">CurseForge</button>
            </div>
            <AppButton variant="primary" class="self-end" @click="doSearch" :disabled="loading || !searchQuery.trim()">
              <AppIcon name="search" size="3.5" /> {{ loading ? '搜索中...' : '搜索' }}
            </AppButton>
          </div>
          <div v-if="!selectedMod && searchResults.length > 0" class="grid grid-cols-1 md:grid-cols-2 gap-3 max-h-[500px] overflow-y-auto">
            <div v-for="mod in searchResults" :key="mod.project_id || mod.id"
              class="p-4 rounded-lg bg-[var(--bg-hover)] border border-[var(--card-border)] hover:border-[var(--border-hover)] transition-all cursor-pointer"
              @click="viewModVersions(mod)">
              <div class="flex items-start gap-3">
                <img v-if="mod.icon_url" :src="mod.icon_url" class="w-10 h-10 rounded-lg bg-[var(--bg-elevated)] object-cover" alt="" />
                <div v-else class="w-10 h-10 rounded-lg bg-[var(--bg-elevated)] flex items-center justify-center"><AppIcon name="mods" size="5" /></div>
                <div class="flex-1 min-w-0">
                  <h4 class="text-sm font-medium text-[var(--text-primary)] truncate">{{ mod.title || mod.name }}</h4>
                  <p class="text-xs text-[var(--text-tertiary)] line-clamp-2 mt-1">{{ mod.description || mod.summary }}</p>
                </div>
              </div>
            </div>
          </div>
          <div v-if="!selectedMod && !loading && searchResults.length === 0 && searchQuery.trim()" class="py-8 text-center text-sm text-[var(--text-tertiary)]">未找到匹配的模组</div>
          <div v-if="selectedMod">
            <div class="flex items-center gap-3 mb-4">
              <AppButton variant="ghost" size="sm" @click="goBack"><AppIcon name="chevron-left" size="3.5" /> 返回</AppButton>
              <h3 class="text-sm font-medium text-[var(--text-primary)]">{{ selectedMod.title || selectedMod.name }}</h3>
            </div>
            <h4 class="text-xs font-medium text-[var(--text-secondary)] mb-2">可用版本</h4>
            <div v-if="modVersions.length > 0" class="space-y-2 max-h-80 overflow-y-auto">
              <div v-for="(ver, i) in modVersions" :key="i" class="p-3 rounded-lg bg-[var(--bg-hover)]">
                <div class="flex flex-wrap gap-1 mb-2">
                  <AppBadge v-for="gv in ver.game_versions?.slice(0,5)" :key="gv" color="blue" size="sm">{{ gv }}</AppBadge>
                  <AppBadge v-for="ld in ver.loaders" :key="ld" color="purple" size="sm">{{ ld }}</AppBadge>
                </div>
                <div v-for="f in ver.files" :key="f.filename" class="flex items-center justify-between mt-1">
                  <span class="text-xs text-[var(--text-tertiary)] truncate">{{ f.filename }}</span>
                  <a v-if="f.url" :href="f.url" target="_blank" class="text-xs text-[var(--text-accent)] hover:underline flex items-center gap-1"><AppIcon name="external-link" size="3" /> 下载</a>
                </div>
              </div>
            </div>
            <div v-else class="text-xs text-[var(--text-tertiary)] py-4 text-center">暂无版本信息</div>
          </div>
        </AppCard>
      </div>

      <!-- ======== 整合包 Tab（占位）======== -->
      <div v-if="activeTab === 'modpack'">
        <AppCard padding="p-5">
          <div class="flex flex-col items-center justify-center py-12 text-center">
            <AppIcon name="mods" size="12" class="text-[var(--text-tertiary)] mb-4 opacity-40" />
            <h3 class="text-base font-medium text-[var(--text-secondary)] mb-2">整合包下载</h3>
            <p class="text-sm text-[var(--text-tertiary)] max-w-md">即将支持从 CurseForge、Modrinth 等平台搜索和安装整合包，敬请期待！</p>
          </div>
        </AppCard>
      </div>

      <!-- ======== 服务端 Tab（占位）======== -->
      <div v-if="activeTab === 'serversoft'">
        <AppCard padding="p-5">
          <div class="flex flex-col items-center justify-center py-12 text-center">
            <AppIcon name="game" size="12" class="text-[var(--text-tertiary)] mb-4 opacity-40" />
            <h3 class="text-base font-medium text-[var(--text-secondary)] mb-2">服务端下载</h3>
            <p class="text-sm text-[var(--text-tertiary)] max-w-md">即将支持下载 Vanilla、Fabric、Forge、Paper、Purpur 等服务端核心，敬请期待！</p>
          </div>
        </AppCard>
      </div>

    </AppSection>
  </div>
</template>