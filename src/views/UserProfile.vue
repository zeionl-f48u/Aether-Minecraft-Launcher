<!-- ============================================================
  UserProfile.vue — 用户中心页面
  ============================================================
  功能：
  - 离线登录
  - Microsoft 设备码登录（完整三步流程）
  - Authlib-Injector 外置登录
  - 令牌验证与刷新
  - 登录状态展示
  ============================================================ -->

<script setup>
import { ref, reactive } from "vue";
import AppSection from "../components/common/AppSection.vue";
import AppCard from "../components/common/AppCard.vue";
import AppButton from "../components/common/AppButton.vue";
import AppIcon from "../components/common/AppIcon.vue";
import AppBadge from "../components/common/AppBadge.vue";
import { api } from "../composables/useTauriApi.js";

// ---------- 登录状态 ----------
const authResult = ref(null);
const loading = ref(false);
const error = ref("");
const activeTab = ref("offline");

// ---------- 离线登录 ----------
const playerName = ref("");

async function doOfflineLogin() {
  if (!playerName.value.trim()) return;
  loading.value = true;
  error.value = "";
  try {
    authResult.value = await api.offlineLogin(playerName.value.trim());
  } catch (e) {
    error.value = String(e);
  } finally {
    loading.value = false;
  }
}

// ---------- Microsoft 设备码登录 ----------
const msStep = ref(0); // 0=未开始, 1=已获取设备码, 2=已完成
const deviceCode = ref(null);
const polling = ref(false);

async function startMsLogin() {
  loading.value = true;
  error.value = "";
  try {
    deviceCode.value = await api.microsoftDeviceLoginStart();
    msStep.value = 1;
    // 自动开始轮询
    pollMsLogin();
  } catch (e) {
    error.value = String(e);
  } finally {
    loading.value = false;
  }
}

async function pollMsLogin() {
  if (!deviceCode.value) return;
  polling.value = true;
  error.value = "";
  try {
    const tokenResp = await api.microsoftDeviceLoginPoll(deviceCode.value.device_code, 3);
    const complete = await api.microsoftDeviceLoginComplete(tokenResp.access_token, tokenResp.refresh_token);
    authResult.value = complete;
    msStep.value = 2;
  } catch (e) {
    if (String(e).includes("authorization_pending")) {
      // 继续轮询
      setTimeout(pollMsLogin, 3000);
    } else {
      error.value = String(e);
      msStep.value = 0;
    }
  } finally {
    polling.value = false;
  }
}

function copyUserCode() {
  if (deviceCode.value?.user_code) {
    navigator.clipboard.writeText(deviceCode.value.user_code);
  }
}

// ---------- Authlib 外置登录 ----------
const authlibServer = ref("");
const authlibUsername = ref("");
const authlibPassword = ref("");

async function doAuthlibLogin() {
  if (!authlibServer.value.trim() || !authlibUsername.value.trim()) return;
  loading.value = true;
  error.value = "";
  try {
    authResult.value = await api.authlibLogin(
      authlibServer.value.trim(),
      authlibUsername.value.trim(),
      authlibPassword.value,
    );
  } catch (e) {
    error.value = String(e);
  } finally {
    loading.value = false;
  }
}

// ---------- 令牌验证 ----------
async function doValidate() {
  if (!authResult.value) return;
  loading.value = true;
  error.value = "";
  try {
    const valid = await api.validateToken(JSON.stringify(authResult.value));
    if (!valid) {
      error.value = "令牌已过期，请重新登录";
    }
  } catch (e) {
    error.value = String(e);
  } finally {
    loading.value = false;
  }
}

// ---------- 登出 ----------
function doLogout() {
  authResult.value = null;
  msStep.value = 0;
  deviceCode.value = null;
}
</script>

<template>
  <div class="max-w-3xl mx-auto">
    <AppSection title="用户中心" description="账户管理与登录">

      <!-- 错误提示 -->
      <div v-if="error" class="p-3 mb-4 text-sm text-red-400 bg-red-500/10 rounded-lg">{{ error }}</div>

      <!-- 登录方式 Tab -->
      <div v-if="!authResult" class="flex gap-1 mb-4 p-1 rounded-xl bg-[var(--bg-elevated)] border border-[var(--border-base)]">
        <button v-for="tab in [{id:'offline',label:'离线'},{id:'microsoft',label:'Microsoft'},{id:'authlib',label:'外置登录'}]" :key="tab.id"
          class="flex-1 px-3 py-2 text-sm font-medium rounded-lg transition-all duration-150"
          :class="activeTab === tab.id ? 'bg-[var(--button-primary)] text-white' : 'text-[var(--text-tertiary)] hover:text-[var(--text-primary)]'"
          @click="activeTab = tab.id">{{ tab.label }}</button>
      </div>

      <!-- ======== 离线登录 ======== -->
      <AppCard v-if="!authResult && activeTab === 'offline'" padding="p-5">
        <template #header>
          <h3 class="text-sm font-medium text-[var(--text-secondary)]">离线登录</h3>
          <AppIcon name="user" size="4" />
        </template>
        <p class="text-xs text-[var(--text-tertiary)] mb-3">无需网络验证，输入玩家名称即可</p>
        <input v-model="playerName" placeholder="输入玩家名称"
          class="w-full px-3 py-2 text-sm rounded-lg bg-[var(--bg-elevated)] border border-[var(--border-base)] text-[var(--text-primary)] focus:outline-none focus:border-[var(--border-accent)] mb-3"
          @keyup.enter="doOfflineLogin" />
        <AppButton variant="primary" size="sm" fullWidth @click="doOfflineLogin" :disabled="loading">登录</AppButton>
      </AppCard>

      <!-- ======== Microsoft 设备码登录 ======== -->
      <AppCard v-if="!authResult && activeTab === 'microsoft'" padding="p-5">
        <template #header>
          <h3 class="text-sm font-medium text-[var(--text-secondary)]">Microsoft 登录</h3>
          <AppIcon name="external-link" size="4" />
        </template>

        <div v-if="msStep === 0">
          <p class="text-xs text-[var(--text-tertiary)] mb-3">通过 Microsoft 账户登录，支持正版 Minecraft</p>
          <AppButton variant="primary" size="sm" fullWidth @click="startMsLogin" :disabled="loading">
            {{ loading ? '获取设备码...' : '开始登录' }}
          </AppButton>
        </div>

        <div v-if="msStep === 1 && deviceCode">
          <div class="p-4 rounded-lg bg-[var(--bg-hover)] mb-3 text-center">
            <p class="text-xs text-[var(--text-tertiary)] mb-2">请访问以下链接并输入设备码</p>
            <a :href="deviceCode.verification_uri" target="_blank"
              class="text-sm text-[var(--text-accent)] hover:underline block mb-2">
              {{ deviceCode.verification_uri }}
            </a>
            <div class="flex items-center justify-center gap-2">
              <code class="px-4 py-2 text-lg font-mono font-bold bg-[var(--bg-elevated)] rounded-lg">{{ deviceCode.user_code }}</code>
              <AppButton variant="ghost" size="sm" @click="copyUserCode">
                <AppIcon name="copy" size="3.5" />
              </AppButton>
            </div>
          </div>
          <div class="flex items-center justify-center gap-2">
            <svg v-if="polling" class="w-4 h-4 animate-spin" viewBox="0 0 24 24" fill="none">
              <circle class="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" stroke-width="4" />
              <path class="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4z" />
            </svg>
            <span class="text-xs text-[var(--text-tertiary)]">等待用户授权...</span>
          </div>
        </div>
      </AppCard>

      <!-- ======== Authlib 外置登录 ======== -->
      <AppCard v-if="!authResult && activeTab === 'authlib'" padding="p-5">
        <template #header>
          <h3 class="text-sm font-medium text-[var(--text-secondary)]">外置登录</h3>
          <AppIcon name="external-link" size="4" />
        </template>
        <p class="text-xs text-[var(--text-tertiary)] mb-3">使用 Authlib-Injector 兼容的认证服务器登录</p>
        <div class="space-y-3">
          <input v-model="authlibServer" placeholder="认证服务器地址 (https://...)"
            class="w-full px-3 py-2 text-sm rounded-lg bg-[var(--bg-elevated)] border border-[var(--border-base)] text-[var(--text-primary)] focus:outline-none focus:border-[var(--border-accent)]" />
          <input v-model="authlibUsername" placeholder="用户名/邮箱"
            class="w-full px-3 py-2 text-sm rounded-lg bg-[var(--bg-elevated)] border border-[var(--border-base)] text-[var(--text-primary)] focus:outline-none focus:border-[var(--border-accent)]" />
          <input v-model="authlibPassword" type="password" placeholder="密码"
            class="w-full px-3 py-2 text-sm rounded-lg bg-[var(--bg-elevated)] border border-[var(--border-base)] text-[var(--text-primary)] focus:outline-none focus:border-[var(--border-accent)]"
            @keyup.enter="doAuthlibLogin" />
          <AppButton variant="primary" size="sm" fullWidth @click="doAuthlibLogin" :disabled="loading">登录</AppButton>
        </div>
      </AppCard>

      <!-- ======== 当前登录状态 ======== -->
      <div v-if="authResult" class="space-y-4">
        <AppCard padding="p-5">
          <template #header>
            <h3 class="text-sm font-medium text-[var(--text-secondary)]">已登录</h3>
            <AppBadge color="green">在线</AppBadge>
          </template>
          <div class="flex items-center gap-4">
            <div class="w-14 h-14 rounded-full bg-[var(--button-primary)] flex items-center justify-center text-white text-xl font-bold">
              {{ (authResult.player_name || authResult.username || '?')[0].toUpperCase() }}
            </div>
            <div class="flex-1">
              <p class="text-base font-medium text-[var(--text-primary)]">{{ authResult.player_name || authResult.username }}</p>
              <p class="text-xs text-[var(--text-tertiary)]">
                类型: {{ authResult.type || (authResult.uuid ? '离线' : '在线') }}
              </p>
              <p v-if="authResult.uuid" class="text-xs text-[var(--text-tertiary)]">UUID: {{ authResult.uuid }}</p>
            </div>
          </div>
        </AppCard>

        <div class="flex gap-3">
          <AppButton variant="secondary" size="sm" @click="doValidate" :disabled="loading">
            <AppIcon name="refresh" size="3.5" /> 验证令牌
          </AppButton>
          <AppButton variant="danger" size="sm" @click="doLogout">
            登出
          </AppButton>
        </div>
      </div>
    </AppSection>
  </div>
</template>