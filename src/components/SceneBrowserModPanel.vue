<template>
  <GlassPanel :title="t('sceneBrowserMod.title')">
    <div class="mod-panel">
      <p>{{ t('sceneBrowserMod.description') }}</p>
      <p v-if="!app.vamRootPath">{{ t('sceneBrowserMod.noGame') }}</p>
      <p v-if="error" role="alert">{{ error }}</p>
      <template v-if="status">
        <p>{{ status.installed ? t('sceneBrowserMod.installed') : t('sceneBrowserMod.notInstalled') }} · v{{ status.version }}</p>
        <p v-if="status.updateAvailable">{{ t('sceneBrowserMod.updateAvailable') }}</p>
        <p v-if="status.conflict" role="alert">{{ t('sceneBrowserMod.conflict') }}</p>
        <p v-if="status.reason" role="alert">{{ status.reason }}</p>
      </template>
      <div class="actions">
        <button :disabled="busy || !app.vamRootPath" @click="refresh">{{ t('sceneBrowserMod.refresh') }}</button>
        <button :disabled="busy || !status?.compatible || status?.conflict || status?.installed" @click="change('install')">{{ t(status?.updateAvailable ? 'sceneBrowserMod.update' : 'sceneBrowserMod.install') }}</button>
        <button :disabled="busy || !(status?.installed || status?.updateAvailable)" @click="change('uninstall')">{{ t('sceneBrowserMod.uninstall') }}</button>
        <span v-if="busy" role="status">{{ t('common.loading') }}</span>
      </div>
    </div>
  </GlassPanel>
</template>

<script setup lang="ts">
import { ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { invoke } from '@tauri-apps/api/core'
import GlassPanel from '@/components/common/GlassPanel.vue'
import { useAppStore } from '@/stores/app'
import { useNotification } from '@/composables/useNotification'

interface ModStatus {
  version: string
  installed: boolean
  updateAvailable: boolean
  compatible: boolean
  conflict: boolean
  reason: string | null
  targetPath: string
}
const app = useAppStore()
const { t } = useI18n()
const notify = useNotification()
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
    const result = await invoke<ModStatus>(`${action}_scene_browser_mod`, { vamRoot: root })
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
.mod-panel { display: grid; gap: 12px; }
p { margin: 0; line-height: 1.6; }
.actions { display: flex; flex-wrap: wrap; align-items: center; gap: 10px; }
button { padding: 8px 16px; border: 1px solid var(--border-subtle); border-radius: 6px; background: var(--bg-surface); color: var(--text-primary); cursor: pointer; }
button:disabled { opacity: .45; cursor: default; }
</style>
