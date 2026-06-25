<!-- ============================================================
  Versions.vue — 版本管理视图（连接后端）
  ============================================================
  功能：
  - 从后端获取已安装版本列表
  - 显示版本类型、加载器信息
  - 版本详情查看
  - 启动游戏（集成认证）
  ============================================================ -->

<script setup>
import { ref, onMounted } from "vue";
import AppSection from "../components/common/AppSection.vue";
import AppBadge from "../components/common/AppBadge.vue";
import AppButton from "../components/common/AppButton.vue";
import AppIcon from "../components/common/AppIcon.vue";
import AppCard from "../components/common/AppCard.vue";
import { api } from "../composables/useTauriApi.js";

const versions = ref([]);
const loading = ref(false);
const error = ref("");
const details = ref({});
const expandedVersion = ref(null);

// 模组管理（按版本名称索引）
const modsMap = ref({});           // { versionId: [mod, ...] }
const modsLoading = ref({});       // { versionId: true/false }
const modsExpanded = ref({});      // { versionId: true/false }

async function loadVersions() {
  loading.value = true;
  error.value = "";
  try {
    versions.value = await api.getLocalVersions();
  } catch (e) {
    error.value = String(e);
  } finally {
    loading.value = false;
  }
}

async function showDetail(versionId) {
  if (expandedVersion.value === versionId) {
    expandedVersion.value = null;
    return;
  }
  expandedVersion.value = versionId;
  error.value = "";
  try {
    details.value[versionId] = await api.getVersionDetail(versionId);
  } catch (e) {
    error.value = String(e);
    details.value[versionId] = null;
  }
}

// ======================================================================
//  模组管理（嵌入版本详情）
// ======================================================================

async function loadMods(versionId) {
  modsLoading.value[versionId] = true;
  error.value = "";
  try {
    modsMap.value[versionId] = await api.listMods(versionId);
  } catch (e) {
    error.value = String(e);
  } finally {
    modsLoading.value[versionId] = false;
  }
}

async function doToggleMod(versionId, fileName, enabled) {
  error.value = "";
  try {
    await api.toggleMod(fileName, enabled);
    await loadMods(versionId);
  } catch (e) {
    error.value = String(e);
  }
}

async function doDeleteMod(versionId, fileName) {
  error.value = "";
  try {
    await api.deleteMod(fileName);
    modsMap.value[versionId] = (modsMap.value[versionId] || []).filter((m) => m.file_name !== fileName);
  } catch (e) {
    error.value = String(e);
  }
}

function toggleModsSection(versionId) {
  modsExpanded.value[versionId] = !modsExpanded.value[versionId];
  if (modsExpanded.value[versionId] && !modsMap.value[versionId]) {
    loadMods(versionId);
  }
}

function getTypeBadgeColor(type) {
  switch (type) {
    case "release": return "green";
    case "snapshot": return "yellow";
    case "old_beta": return "orange";
    case "old_alpha": return "red";
    default: return "gray";
  }
}

function getTypeLabel(type) {
  switch (type) {
    case "release": return "正式版";
    case "snapshot": return "快照";
    case "old_beta": return "Beta";
    case "old_alpha": return "Alpha";
    default: return type || "未知";
  }
}

onMounted(loadVersions);
</script>

<template>
  <div class="max-w-5xl mx-auto">
    <AppSection title="版本管理" description="管理已安装的 Minecraft 版本">
      <div class="flex items-center justify-between mb-4">
        <span class="text-xs text-[var(--text-tertiary)]">{{ versions.length }} 个版本</span>
        <AppButton variant="ghost" size="sm" @click="loadVersions" :disabled="loading">
          <AppIcon name="refresh" size="3.5" /> {{ loading ? '加载中...' : '刷新' }}
        </AppButton>
      </div>

      <div v-if="error" class="p-3 mb-4 text-sm text-red-400 bg-red-500/10 rounded-lg">{{ error }}</div>

      <!-- 版本列表 -->
      <div v-if="versions.length === 0 && !loading" class="py-12 text-center text-sm text-[var(--text-tertiary)]">
        暂无已安装的版本，请前往下载页面安装
      </div>

      <div v-if="loading" class="flex items-center justify-center py-8 text-sm text-[var(--text-tertiary)]">
        <svg class="w-4 h-4 mr-2 animate-spin" viewBox="0 0 24 24" fill="none">
          <circle class="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" stroke-width="4" />
          <path class="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4z" />
        </svg>扫描中...
      </div>

      <div v-for="(v, i) in versions" :key="v.id" class="mb-2">
        <div
          class="flex items-center justify-between p-4 rounded-lg bg-[var(--card-bg)] border border-[var(--card-border)] hover:border-[var(--border-hover)] transition-all cursor-pointer"
          :class="`anim-stagger-${Math.min(i + 1, 5)}`"
          @click="showDetail(v.id)"
        >
          <div class="flex items-center gap-3 flex-1 min-w-0">
            <AppIcon name="versions" size="5" class="text-[var(--text-tertiary)] flex-shrink-0" />
            <div>
              <span class="text-sm font-medium text-[var(--text-primary)]">{{ v.id }}</span>
              <div class="flex items-center gap-2 mt-1">
                <AppBadge :color="getTypeBadgeColor(v.version_type)" size="sm">
                  {{ getTypeLabel(v.version_type) }}
                </AppBadge>
              </div>
            </div>
          </div>
          <AppIcon name="chevron-right" size="4" class="text-[var(--text-tertiary)] flex-shrink-0"
            :class="expandedVersion === v.id ? 'rotate-90' : ''" />
        </div>

        <!-- 展开详情（含模组管理） -->
        <div v-if="expandedVersion === v.id && details[v.id]" class="mt-1 ml-4">
          <AppCard padding="p-4">
            <!-- 版本元信息 -->
            <div class="grid grid-cols-2 gap-3 text-xs">
              <div><span class="text-[var(--text-tertiary)]">主类: </span><span class="text-[var(--text-primary)]">{{ details[v.id].main_class }}</span></div>
              <div><span class="text-[var(--text-tertiary)]">所需 Java: </span><span class="text-[var(--text-primary)]">{{ details[v.id].required_java }}</span></div>
              <div><span class="text-[var(--text-tertiary)]">库数量: </span><span class="text-[var(--text-primary)]">{{ details[v.id].libraries_count }}</span></div>
              <div><span class="text-[var(--text-tertiary)]">继承自: </span><span class="text-[var(--text-primary)]">{{ details[v.id].inherits_from || '无' }}</span></div>
              <div v-if="details[v.id].assets_index"><span class="text-[var(--text-tertiary)]">资源索引: </span><span class="text-[var(--text-primary)]">{{ details[v.id].assets_index }}</span></div>
            </div>

            <!-- 操作按钮 -->
            <div class="flex gap-2 mt-3 pt-3 border-t border-[var(--border-base)]">
              <AppButton variant="primary" size="sm">
                <AppIcon name="launch" size="3" /> 启动
              </AppButton>
              <AppButton variant="secondary" size="sm" @click="toggleModsSection(v.id)">
                <AppIcon name="mods" size="3" /> {{ modsExpanded[v.id] ? '收起模组' : '管理模组' }}
                <span v-if="modsMap[v.id]?.length" class="ml-1 text-xs opacity-70">({{ modsMap[v.id].length }})</span>
              </AppButton>
            </div>

            <!-- 模组列表（嵌入） -->
            <div v-if="modsExpanded[v.id]" class="mt-3 pt-3 border-t border-[var(--border-base)]">
              <div v-if="modsLoading[v.id]" class="py-4 text-center text-xs text-[var(--text-tertiary)]">加载模组中...</div>
              <div v-else-if="!modsMap[v.id] || modsMap[v.id].length === 0" class="py-4 text-center text-xs text-[var(--text-tertiary)]">该版本没有模组</div>
              <div v-else class="space-y-2 max-h-64 overflow-y-auto">
                <div v-for="mod in modsMap[v.id]" :key="mod.id"
                  class="flex items-center justify-between p-3 rounded-lg bg-[var(--bg-hover)]"
                  :class="mod.enabled ? '' : 'opacity-60'">
                  <div class="flex-1 min-w-0">
                    <div class="flex items-center gap-2">
                      <span class="text-xs font-medium text-[var(--text-primary)] truncate">{{ mod.name || mod.file_name }}</span>
                      <AppBadge :color="mod.enabled ? 'green' : 'gray'" size="sm">{{ mod.enabled ? '已启用' : '已禁用' }}</AppBadge>
                    </div>
                    <p v-if="mod.description" class="text-xs text-[var(--text-tertiary)] truncate mt-0.5">{{ mod.description }}</p>
                  </div>
                  <div class="flex items-center gap-1 ml-2 flex-shrink-0">
                    <AppButton variant="ghost" size="sm" @click="doToggleMod(v.id, mod.file_name, !mod.enabled)">
                      {{ mod.enabled ? '禁用' : '启用' }}
                    </AppButton>
                    <AppButton variant="ghost" size="sm" class="!text-red-400 hover:!text-red-300" @click="doDeleteMod(v.id, mod.file_name)">
                      删除
                    </AppButton>
                  </div>
                </div>
              </div>
            </div>
          </AppCard>
        </div>
      </div>
    </AppSection>
  </div>
</template>
