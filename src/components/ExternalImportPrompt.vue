<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref, watch } from 'vue'
import { useRouter } from 'vue-router'
import { useI18n } from 'vue-i18n'
import { invoke } from '@tauri-apps/api/core'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import { useAppStore } from '@/stores/app'
import { useIngestionStore, type ExternalImportMode } from '@/stores/ingestion'
import { useNotification } from '@/composables/useNotification'

const props = defineProps<{ ready: boolean }>()
const { t } = useI18n()
const router = useRouter()
const appStore = useAppStore()
const ingestionStore = useIngestionStore()
const notify = useNotification()
const batches = ref<string[][]>([])
const mode = ref<ExternalImportMode>('flatten')
const deferred = ref(false)
const navigating = ref(false)
const error = ref('')
const dialog = ref<HTMLDialogElement | null>(null)
const files = computed(() => batches.value[0] || [])
const visible = computed(() => props.ready && files.value.length > 0 && !deferred.value && !ingestionStore.busy)
const targetDir = computed(() => appStore.installContext?.realAddonDir || '')
let unlisten: UnlistenFn | undefined
let disposed = false
let draining = false
let drainAgain = false

watch(visible, value => {
  if (value) dialog.value?.showModal()
  else dialog.value?.close()
}, { flush: 'post' })
watch(targetDir, value => { if (value) deferred.value = false })

async function pullRequests() {
  if (draining) { drainAgain = true; return }
  draining = true
  try {
    do {
      drainAgain = false
      const requests = await invoke<string[][]>('take_external_import_requests')
      if (!disposed) batches.value.push(...requests.filter(paths => paths.length > 0))
    } while (drainAgain && !disposed)
  } catch (err) {
    notify.error(t('ingestion.externalReceiveFailed', { error: String(err) }))
  } finally { draining = false }
}

onMounted(async () => {
  try {
    unlisten = await listen('external-import-pending', () => { void pullRequests() })
    if (disposed) { unlisten(); return }
    await pullRequests()
  } catch (err) {
    notify.error(t('ingestion.externalReceiveFailed', { error: String(err) }))
  }
})
onUnmounted(() => { disposed = true; unlisten?.() })

function cancel() {
  if (navigating.value) return
  batches.value.shift()
  error.value = ''
}

async function previewImport() {
  if (navigating.value || ingestionStore.busy || !targetDir.value) return
  navigating.value = true
  error.value = ''
  try {
    const failure = await router.push('/ingestion')
    if (failure && router.currentRoute.value.name !== 'ingestion') {
      error.value = t('ingestion.externalBusy')
      return
    }
    ingestionStore.selection = { files: [...files.value], mode: mode.value }
    batches.value.shift()
  } catch (err) { error.value = String(err) }
  finally { navigating.value = false }
}

async function openSettings() {
  error.value = ''
  try {
    await router.push('/settings')
    if (router.currentRoute.value.name === 'settings') deferred.value = true
    else error.value = t('ingestion.externalBusy')
  } catch (err) { error.value = String(err) }
}
</script>

<template>
  <dialog ref="dialog" class="external-import-dialog glass-panel" aria-labelledby="external-import-title" aria-describedby="external-import-description" @cancel.prevent="cancel">
    <h3 id="external-import-title">{{ t('ingestion.externalTitle') }}</h3>
    <p id="external-import-description">{{ t('ingestion.externalQuestion', { count: files.length }) }}</p>
    <ul class="file-list">
      <li v-for="path in files" :key="path">{{ path }}</li>
    </ul>
    <p class="target-path">{{ t('ingestion.target') }}: {{ targetDir || t('ingestion.noGame') }}</p>
    <fieldset :disabled="navigating">
      <legend>{{ t('ingestion.rule') }}</legend>
      <label :class="{ selected: mode === 'flatten' }">
        <input v-model="mode" type="radio" name="external-import-mode" value="flatten" />
        <span>{{ t('migration.modeFlatten') }}<small>{{ t('ingestion.ruleHelp.flatten') }}</small></span>
      </label>
      <label :class="{ selected: mode === 'by_type' }">
        <input v-model="mode" type="radio" name="external-import-mode" value="by_type" />
        <span>{{ t('migration.modeByType') }}<small>{{ t('ingestion.ruleHelp.by_type') }}</small></span>
      </label>
    </fieldset>
    <p class="help">{{ t('ingestion.externalHelp') }}</p>
    <p v-if="error" class="error" role="alert">{{ error }}</p>
    <div class="actions">
      <button class="btn btn-secondary" :disabled="navigating" autofocus @click="cancel">{{ t('common.cancel') }}</button>
      <button v-if="!targetDir" class="btn btn-primary" @click="openSettings">{{ t('ingestion.externalSettings') }}</button>
      <button v-else class="btn btn-primary" :disabled="navigating" @click="previewImport">{{ t('ingestion.preview') }}</button>
    </div>
  </dialog>
</template>

<style scoped>
.external-import-dialog { margin: auto; width: min(560px, calc(100vw - 48px)); max-height: calc(100vh - 64px); overflow: auto; padding: var(--space-6); color: var(--text-primary); background: var(--bg-surface); border: 1px solid var(--border-default); border-radius: var(--radius-lg); box-shadow: var(--shadow-lg); }
.external-import-dialog::backdrop { background: rgba(0, 0, 0, 0.6); }
h3 { font-size: var(--text-lg); margin-bottom: var(--space-3); }
p { font-size: var(--text-sm); line-height: 1.7; overflow-wrap: anywhere; }
.file-list { margin: var(--space-3) 0; padding: var(--space-3) var(--space-5); max-height: 160px; overflow: auto; background: var(--bg-subtle); border-radius: var(--radius-sm); font-size: var(--text-xs); overflow-wrap: anywhere; user-select: text; }
.target-path { color: var(--text-secondary); user-select: text; }
fieldset { border: 0; padding: 0; margin: var(--space-4) 0; display: grid; gap: var(--space-2); }
legend { font-size: var(--text-sm); margin-bottom: var(--space-2); }
label { display: flex; gap: var(--space-3); align-items: center; padding: var(--space-3); border: 1px solid var(--border-default); border-radius: var(--radius-sm); cursor: pointer; font-size: var(--text-sm); }
label.selected { border-color: var(--accent-primary); background: var(--accent-subtle); }
label:focus-within, button:focus-visible { outline: 2px solid var(--accent-primary); outline-offset: 2px; }
input { accent-color: var(--accent-primary); }
small { display: block; color: var(--text-secondary); margin-top: var(--space-1); line-height: 1.6; }
.help { color: var(--text-secondary); font-size: var(--text-xs); }
.error { color: var(--color-error); }
.actions { display: flex; justify-content: flex-end; gap: var(--space-2); margin-top: var(--space-4); }
.btn { padding: var(--space-2) var(--space-4); border-radius: var(--radius-sm); font-size: var(--text-sm); }
.btn:disabled { opacity: 0.5; cursor: not-allowed; }
</style>
