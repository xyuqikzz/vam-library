<template>
  <GlassPanel :title="t('sceneBrowserMod.title')">
    <div class="mod-panel">
      <p>{{ t('sceneBrowserMod.description') }}</p>
      <p v-if="!app.vamRootPath">{{ t('sceneBrowserMod.noGame') }}</p>
      <p v-if="error" role="alert">{{ error }}</p>
      <template v-if="status">
        <div class="status-row">
          <span v-if="status.conflict" class="status-pill is-error">{{ t('sceneBrowserMod.unrecognized') }}</span>
          <span v-else-if="status.updateAvailable" class="status-pill is-warning">{{ t('sceneBrowserMod.olderInstalled') }}</span>
          <span v-else-if="status.installed" class="status-pill is-success">
            {{ t('sceneBrowserMod.installed') }} · v{{ status.version }}
          </span>
          <span v-else class="status-pill">{{ t('sceneBrowserMod.notInstalled') }}</span>
          <span v-if="!status.installed && status.version" class="status-meta">
            {{ t('sceneBrowserMod.bundledVersion', { version: status.version }) }}
          </span>
        </div>
        <p v-if="status.compatible && !status.verified">{{ t('sceneBrowserMod.unverified') }}</p>
        <p v-if="status.runtime">{{ t('sceneBrowserMod.runtimeStatus', { browser: t(status.runtime.browser === 'ok' ? 'sceneBrowserMod.available' : 'sceneBrowserMod.unavailable'), launch: t(status.runtime.launch === 'ok' ? 'sceneBrowserMod.available' : 'sceneBrowserMod.unavailable') }) }}</p>
        <p v-if="status.updateAvailable">{{ t('sceneBrowserMod.updateAvailable') }}</p>
        <div v-if="status.conflict" class="conflict-help" role="alert">
          <p>{{ t('sceneBrowserMod.conflict') }}</p>
          <p>{{ t('sceneBrowserMod.conflictHelp') }}</p>
          <p class="plugin-path">
            {{ t('sceneBrowserMod.pluginFile') }}
            <code>{{ status.targetPath }}</code>
          </p>
        </div>
        <p v-if="status.reason" role="alert">{{ status.reason }}</p>
      </template>
      <div class="actions">
        <UiButton :disabled="busy || !app.vamRootPath" @click="refresh" variant="secondary">
          {{ t('sceneBrowserMod.refresh') }}
        </UiButton>
        <UiButton
          :disabled="busy || !status?.compatible || status?.conflict || status?.installed"
          @click="change('install')"
          variant="secondary"
        >
          {{ t(status?.updateAvailable ? 'sceneBrowserMod.update' : 'sceneBrowserMod.install') }}
        </UiButton>
        <UiButton
          :disabled="busy || !(status?.installed || status?.updateAvailable)"
          @click="change('uninstall')"
          variant="secondary"
        >
          {{ t('sceneBrowserMod.uninstall') }}
        </UiButton>
        <span v-if="busy" role="status">{{ t('common.loading') }}</span>
      </div>
    </div>
  </GlassPanel>
</template>

<script setup lang="ts">
import UiButton from '@/components/ui/UiButton.vue'
import { ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { invoke } from '@tauri-apps/api/core'
import GlassPanel from '@/components/common/GlassPanel.vue'
import { useAppStore } from '@/stores/app'
import { useConfirm } from '@/composables/useConfirm'
import { useNotification } from '@/composables/useNotification'

interface ModStatus {
  version: string
  installed: boolean
  updateAvailable: boolean
  compatible: boolean
  verified: boolean
  runtime: { browser: string; launch: string } | null
  conflict: boolean
  reason: string | null
  targetPath: string
}
const app = useAppStore()
const { t } = useI18n()
const notify = useNotification()
const confirm = useConfirm()
const status = ref<ModStatus | null>(null)
const error = ref('')
const busy = ref(false)
let request = 0

async function refresh() {
  const id = ++request
  const root = app.vamRootPath
  status.value = null
  error.value = ''
  if (!root) { busy.value = false; return }
  busy.value = true
  try {
    const result = await invoke<ModStatus>('get_scene_browser_mod_status', { vamRoot: root })
    if (id === request) status.value = result
  } catch (e) {
    if (id === request) error.value = String(e)
  } finally {
    if (id === request) busy.value = false
  }
}

async function change(action: 'install' | 'uninstall') {
  if (busy.value || !app.vamRootPath) return
  const id = ++request
  const root = app.vamRootPath
  busy.value = true
  error.value = ''
  try {
    let allowUnverified = false
    if (action === 'install' && status.value?.compatible && !status.value.verified) {
      allowUnverified = await confirm({ title: t('sceneBrowserMod.unverifiedTitle'), message: t('sceneBrowserMod.unverifiedHelp'), confirmText: t('sceneBrowserMod.installAnyway') })
      if (!allowUnverified || id !== request || root !== app.vamRootPath) return
    }
    const result = await invoke<ModStatus>(`${action}_scene_browser_mod`, { vamRoot: root, ...(action === 'install' ? { allowUnverified } : {}) })
    if (id === request) {
      status.value = result
      notify.success(t(action === 'install' ? 'sceneBrowserMod.installSuccess' : 'sceneBrowserMod.uninstallSuccess'))
    }
  } catch (e) {
    if (id === request) error.value = String(e)
  } finally {
    if (id === request) busy.value = false
  }
}

watch(() => app.vamRootPath, refresh, { immediate: true })
</script>

<style scoped>
.mod-panel {
  display: grid;
  gap: var(--space-3);
}
.conflict-help {
  display: grid;
  gap: var(--space-2);
}
.status-row {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: var(--space-3);
}
.status-pill {
  display: inline-flex;
  align-items: center;
  height: 24px;
  padding: 0 var(--space-3);
  border-radius: var(--radius-full);
  background: var(--bg-control);
  color: var(--text-secondary);
  font-size: var(--text-xs);
  font-weight: var(--font-bold);
}
.status-pill.is-success { background: var(--color-success-bg); color: var(--color-success); }
.status-pill.is-warning { background: var(--color-warning-bg); color: var(--color-warning); }
.status-pill.is-error { background: var(--color-error-bg); color: var(--color-error); }
.status-meta {
  font-size: var(--text-sm);
  color: var(--text-tertiary);
}
.plugin-path {
  overflow-wrap: anywhere;
}
p {
  margin: 0;
  font-size: var(--text-sm);
  color: var(--text-secondary);
  line-height: 1.6;
}
.actions {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: var(--space-item);
}
</style>
