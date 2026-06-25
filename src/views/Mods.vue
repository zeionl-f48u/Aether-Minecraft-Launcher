<!-- ============================================================
  Mods.vue — 模组管理页面
  ============================================================
  功能：
  - 查看已安装的模组列表
  - 启用/禁用模组
  - 删除模组
  ============================================================ -->

<script setup>
import { ref } from "vue";
import AppSection from "../components/common/AppSection.vue";
import AppCard from "../components/common/AppCard.vue";
import AppButton from "../components/common/AppButton.vue";
import AppIcon from "../components/common/AppIcon.vue";
import AppBadge from "../components/common/AppBadge.vue";
import { api } from "../composables/useTauriApi.js";

const mods = ref([]);
const loading = ref(false);
const error = ref("");
const versionName = ref("");

async function loadMods() {
  if (!versionName.value.trim()) {
    error.value = "请输入版本名称";
    return;
  }
  loading.value = true;
  error.value = "";
  try {
    mods.value = await api.listMods(versionName.value.trim());
  } catch (e) {
    error.value = String(e);
  } finally {
    loading.value = false;
  }
}

async function doToggleMod(fileName, enabled) {
  error.value = "";
  try {
    await api.toggleMod(fileName, enabled);
    await loadMods();
  } catch (e) {
    error.value = String(e);
  }
}

async function doDeleteMod(fileName) {
  error.value = "";
  try {
    await api.deleteMod(fileName);
    mods.value = mods.value.filter((m) => m.file_name !== fileName);
  } catch (e) {
    error.value = String(e);
  }
}
</script>

<template>
  <div class="max-w-5xl mx-auto">
    <AppSection title="模组管理" description="管理已安装的 Minecraft 模组">

      <div v-if="error" class="p-3 mb-4 text-sm text-red-400 bg-red-500/10 rounded-lg">{{ error }}</div>

      <AppCard padding="p-5">
        <div class="flex items-end gap-3 mb-4">
          <div class="flex-1">
            <label class="block text-xs text-[var(--text-tertiary)] mb-1">版本名称</label>
            <input v-model="versionName" placeholder="例如: 1.20.4-fabric"
              class="w-full px-3 py-2 text-sm rounded-lg bg-[var(--bg-elevated)] border border-[var(--border-base)] text-[var(--text-primary)] focus:outline-none focus:border-[var(--border-accent)]"
              @keyup.enter="loadMods" />
          </div>
          <AppButton variant="primary" size="md" @click="loadMods" :disabled="loading">
            <AppIcon name="search" size="3.5" />
            {{ loading ? '加载中...' : '扫描模组' }}
          </AppButton>
        </div>
      </AppCard>

      <!-- 模组列表 -->
      <div v-if="mods.length > 0" class="space-y-2">
        <div
          v-for="mod in mods"
          :key="mod.id"
          class="flex items-center justify-between p-4 rounded-lg bg-[var(--card-bg)] border border-[var(--card-border)] transition-all duration-150"
          :class="mod.enabled ? '' : 'opacity-60'"
        >
          <div class="flex-1 min-w-0">
            <div class="flex items-center gap-2">
              <span class="text-sm font-medium text-[var(--text-primary)] truncate">
                {{ mod.name || mod.file_name }}
              </span>
              <AppBadge :color="mod.enabled ? 'green' : 'gray'" size="sm">
                {{ mod.enabled ? '已启用' : '已禁用' }}
              </AppBadge>
            </div>
            <p v-if="mod.description" class="text-xs text-[var(--text-tertiary)] truncate mt-1">{{ mod.description }}</p>
            <div class="flex items-center gap-3 mt-1">
              <span v-if="mod.version" class="text-xs text-[var(--text-tertiary)]">v{{ mod.version }}</span>
              <span v-if="mod.authors?.length" class="text-xs text-[var(--text-tertiary)]">{{ mod.authors.join(', ') }}</span>
            </div>
          </div>
          <div class="flex items-center gap-2 ml-4">
            <AppButton
              variant="secondary"
              size="sm"
              @click="doToggleMod(mod.file_name, !mod.enabled)"
            >
              {{ mod.enabled ? '禁用' : '启用' }}
            </AppButton>
            <AppButton variant="danger" size="sm" @click="doDeleteMod(mod.file_name)">
              删除
            </AppButton>
          </div>
        </div>
      </div>

      <div v-else-if="!loading && mods.length === 0 && versionName.trim()" class="py-8 text-center text-sm text-[var(--text-tertiary)]">
        该版本没有安装模组
      </div>
    </AppSection>
  </div>
</template>
