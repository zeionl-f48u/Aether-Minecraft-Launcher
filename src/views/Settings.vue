<!-- ============================================================
  Settings.vue — 设置页面（完整版）
  ============================================================
  功能：
  - 展示和编辑应用全部配置项
  - Java 路径选择 / 下载源切换 / 代理设置 / 内存分配
  - JVM 参数 / 游戏参数 / 版本独立
  ============================================================ -->

<script setup>
import { ref, onMounted } from "vue";
import AppSection from "../components/common/AppSection.vue";
import AppCard from "../components/common/AppCard.vue";
import AppButton from "../components/common/AppButton.vue";
import AppIcon from "../components/common/AppIcon.vue";
import { api } from "../composables/useTauriApi.js";

const config = ref(null);
const javaRuntimes = ref([]);
const loading = ref(false);
const saving = ref(false);
const message = ref("");

async function loadConfig() {
  loading.value = true;
  message.value = "";
  try {
    config.value = await api.getConfig();
    javaRuntimes.value = await api.searchJava();
  } catch (e) {
    message.value = "加载配置失败: " + String(e);
  } finally {
    loading.value = false;
  }
}

async function saveConfig() {
  if (!config.value) return;
  saving.value = true;
  message.value = "";
  try {
    await api.updateConfig(config.value);
    message.value = "配置已保存";
  } catch (e) {
    message.value = "保存失败: " + String(e);
  } finally {
    saving.value = false;
  }
}

async function resetConfig() {
  message.value = "";
  try {
    await api.resetConfig();
    await loadConfig();
    message.value = "配置已重置为默认值";
  } catch (e) {
    message.value = "重置失败: " + String(e);
  }
}

function selectJava(path) {
  if (config.value) config.value.java_path = path;
}

onMounted(loadConfig);
</script>

<template>
  <div class="max-w-3xl mx-auto">
    <AppSection title="设置" description="配置启动器参数">

      <!-- 消息提示 -->
      <div v-if="message" class="p-3 mb-4 text-sm rounded-lg"
        :class="message.includes('失败') ? 'text-red-400 bg-red-500/10' : 'text-green-400 bg-green-500/10'">
        {{ message }}
      </div>

      <div v-if="loading" class="flex items-center justify-center py-8 text-sm text-[var(--text-tertiary)]">加载中...</div>

      <template v-if="config">
        <!-- 游戏目录 -->
        <AppCard padding="p-5" class="mb-4">
          <template #header>
            <h3 class="text-sm font-medium text-[var(--text-secondary)]">游戏目录</h3>
          </template>
          <div class="space-y-3">
            <input v-model="config.minecraft_dir" placeholder=".minecraft 目录路径"
              class="w-full px-3 py-2 text-sm rounded-lg bg-[var(--bg-elevated)] border border-[var(--border-base)] text-[var(--text-primary)] focus:outline-none focus:border-[var(--border-accent)]" />
            <div class="flex items-center gap-2">
              <input id="gameIndependent" v-model="config.game_independent" type="checkbox"
                class="rounded bg-[var(--bg-elevated)] border-[var(--border-base)] text-[var(--button-primary)]" />
              <label for="gameIndependent" class="text-xs text-[var(--text-tertiary)]">版本独立（每个版本使用独立的游戏目录）</label>
            </div>
          </div>
        </AppCard>

        <!-- Java 运行时 -->
        <AppCard padding="p-5" class="mb-4">
          <template #header>
            <h3 class="text-sm font-medium text-[var(--text-secondary)]">Java 运行时</h3>
          </template>
          <input v-model="config.java_path" placeholder="java 可执行文件路径"
            class="w-full px-3 py-2 text-sm rounded-lg bg-[var(--bg-elevated)] border border-[var(--border-base)] text-[var(--text-primary)] focus:outline-none focus:border-[var(--border-accent)] mb-3" />
          <div v-if="javaRuntimes.length > 0" class="space-y-1 max-h-40 overflow-y-auto">
            <div v-for="j in javaRuntimes" :key="j.path"
              class="flex items-center gap-2 p-2 rounded-lg cursor-pointer transition-colors"
              :class="config.java_path === j.path ? 'bg-blue-500/10 border border-blue-500/30' : 'hover:bg-[var(--bg-hover)]'"
              @click="selectJava(j.path)">
              <span class="text-xs text-[var(--text-primary)] truncate flex-1">{{ j.path }}</span>
              <span class="text-xs text-[var(--text-tertiary)]">v{{ j.main_version }} {{ j.is_64bit ? '64' : '32' }}位</span>
            </div>
          </div>
          <p v-else class="text-xs text-[var(--text-tertiary)]">未检测到 Java 运行时</p>
        </AppCard>

        <!-- 内存与性能 -->
        <AppCard padding="p-5" class="mb-4">
          <template #header>
            <h3 class="text-sm font-medium text-[var(--text-secondary)]">性能</h3>
          </template>
          <div class="space-y-3">
            <div>
              <label class="block text-xs text-[var(--text-tertiary)] mb-1">最大内存 (MB)</label>
              <input v-model.number="config.max_memory_mb" type="number" min="512" max="65536" step="256"
                class="w-full px-3 py-2 text-sm rounded-lg bg-[var(--bg-elevated)] border border-[var(--border-base)] text-[var(--text-primary)] focus:outline-none focus:border-[var(--border-accent)]" />
            </div>
            <div>
              <label class="block text-xs text-[var(--text-tertiary)] mb-1">下载并发数</label>
              <input v-model.number="config.download_parallel" type="number" min="1" max="256"
                class="w-full px-3 py-2 text-sm rounded-lg bg-[var(--bg-elevated)] border border-[var(--border-base)] text-[var(--text-primary)] focus:outline-none focus:border-[var(--border-accent)]" />
            </div>
            <div class="flex items-center gap-2">
              <input id="verifyFiles" v-model="config.verify_files" type="checkbox"
                class="rounded bg-[var(--bg-elevated)] border-[var(--border-base)] text-[var(--button-primary)]" />
              <label for="verifyFiles" class="text-xs text-[var(--text-tertiary)]">下载后校验文件完整性</label>
            </div>
          </div>
        </AppCard>

        <!-- 下载源 -->
        <AppCard padding="p-5" class="mb-4">
          <template #header>
            <h3 class="text-sm font-medium text-[var(--text-secondary)]">下载源</h3>
          </template>
          <select v-model="config.download_source"
            class="w-full px-3 py-2 text-sm rounded-lg bg-[var(--bg-elevated)] border border-[var(--border-base)] text-[var(--text-primary)] focus:outline-none focus:border-[var(--border-accent)]">
            <option value="Default">官方源（默认）</option>
            <option value="BMCLAPI">BMCLAPI 镜像源</option>
            <option value="MCBBS">MCBBS 镜像源</option>
          </select>
          <p class="text-xs text-[var(--text-tertiary)] mt-2">国内用户建议选择 BMCLAPI 或 MCBBS 镜像源以获得更快的下载速度</p>
        </AppCard>

        <!-- JVM / 游戏参数 -->
        <AppCard padding="p-5" class="mb-4">
          <template #header>
            <h3 class="text-sm font-medium text-[var(--text-secondary)]">高级参数</h3>
          </template>
          <div class="space-y-3">
            <div>
              <label class="block text-xs text-[var(--text-tertiary)] mb-1">JVM 参数</label>
              <textarea v-model="config.jvm_args" rows="2" placeholder="例如: -XX:+UseG1GC -XX:-OmitStackTraceInFastThrow"
                class="w-full px-3 py-2 text-sm rounded-lg bg-[var(--bg-elevated)] border border-[var(--border-base)] text-[var(--text-primary)] focus:outline-none focus:border-[var(--border-accent)] resize-none"></textarea>
            </div>
            <div>
              <label class="block text-xs text-[var(--text-tertiary)] mb-1">游戏参数</label>
              <textarea v-model="config.game_args" rows="2" placeholder="例如: --server=example.com --port=25565"
                class="w-full px-3 py-2 text-sm rounded-lg bg-[var(--bg-elevated)] border border-[var(--border-base)] text-[var(--text-primary)] focus:outline-none focus:border-[var(--border-accent)] resize-none"></textarea>
            </div>
          </div>
        </AppCard>

        <!-- 代理设置 -->
        <AppCard padding="p-5" class="mb-4">
          <template #header>
            <h3 class="text-sm font-medium text-[var(--text-secondary)]">代理设置</h3>
          </template>
          <div class="space-y-3">
            <div class="flex items-center gap-2">
              <input id="proxyEnabled" v-model="config.proxy_enabled" type="checkbox"
                class="rounded bg-[var(--bg-elevated)] border-[var(--border-base)] text-[var(--button-primary)]" />
              <label for="proxyEnabled" class="text-xs text-[var(--text-tertiary)]">启用代理</label>
            </div>
            <div v-if="config.proxy_enabled" class="grid grid-cols-2 gap-3">
              <div>
                <label class="block text-xs text-[var(--text-tertiary)] mb-1">主机地址</label>
                <input v-model="config.proxy_host" placeholder="127.0.0.1"
                  class="w-full px-3 py-2 text-sm rounded-lg bg-[var(--bg-elevated)] border border-[var(--border-base)] text-[var(--text-primary)] focus:outline-none focus:border-[var(--border-accent)]" />
              </div>
              <div>
                <label class="block text-xs text-[var(--text-tertiary)] mb-1">端口</label>
                <input v-model.number="config.proxy_port" type="number" placeholder="1080"
                  class="w-full px-3 py-2 text-sm rounded-lg bg-[var(--bg-elevated)] border border-[var(--border-base)] text-[var(--text-primary)] focus:outline-none focus:border-[var(--border-accent)]" />
              </div>
              <div>
                <label class="block text-xs text-[var(--text-tertiary)] mb-1">用户名（可选）</label>
                <input v-model="config.proxy_username"
                  class="w-full px-3 py-2 text-sm rounded-lg bg-[var(--bg-elevated)] border border-[var(--border-base)] text-[var(--text-primary)] focus:outline-none focus:border-[var(--border-accent)]" />
              </div>
              <div>
                <label class="block text-xs text-[var(--text-tertiary)] mb-1">密码（可选）</label>
                <input v-model="config.proxy_password" type="password"
                  class="w-full px-3 py-2 text-sm rounded-lg bg-[var(--bg-elevated)] border border-[var(--border-base)] text-[var(--text-primary)] focus:outline-none focus:border-[var(--border-accent)]" />
              </div>
            </div>
          </div>
        </AppCard>

        <!-- 外观 -->
        <AppCard padding="p-5" class="mb-4">
          <template #header>
            <h3 class="text-sm font-medium text-[var(--text-secondary)]">外观</h3>
          </template>
          <div class="space-y-3">
            <div>
              <label class="block text-xs text-[var(--text-tertiary)] mb-1">主题</label>
              <select v-model="config.theme"
                class="w-full px-3 py-2 text-sm rounded-lg bg-[var(--bg-elevated)] border border-[var(--border-base)] text-[var(--text-primary)] focus:outline-none focus:border-[var(--border-accent)]">
                <option value="dark">深色模式</option>
                <option value="light">浅色模式</option>
                <option value="system">跟随系统</option>
              </select>
            </div>
            <div>
              <label class="block text-xs text-[var(--text-tertiary)] mb-1">语言</label>
              <select v-model="config.language"
                class="w-full px-3 py-2 text-sm rounded-lg bg-[var(--bg-elevated)] border border-[var(--border-base)] text-[var(--text-primary)] focus:outline-none focus:border-[var(--border-accent)]">
                <option value="zh-CN">简体中文</option>
                <option value="en-US">English</option>
              </select>
            </div>
          </div>
        </AppCard>

        <!-- 操作按钮 -->
        <div class="flex justify-between items-center">
          <AppButton variant="ghost" size="sm" @click="resetConfig">
            <AppIcon name="refresh" size="3.5" /> 重置默认
          </AppButton>
          <AppButton variant="primary" @click="saveConfig" :disabled="saving">
            {{ saving ? '保存中...' : '保存配置' }}
          </AppButton>
        </div>
      </template>
    </AppSection>
  </div>
</template>