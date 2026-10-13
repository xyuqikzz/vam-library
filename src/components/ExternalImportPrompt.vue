<script setup lang="ts">
import UiButton from '@/components/ui/UiButton.vue'
import UiModal from '@/components/ui/UiModal.vue'
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
const dialog = ref<InstanceType<typeof UiModal> | null>(null)
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
  <UiModal
    ref="dialog"
    aria-labelledby="external-import-title"
    aria-describedby="external-import-description"
    @cancel.prevent="cancel"
    imperative
    :show-close="false"
  >
    <h3 id="external-import-title">{{ t('ingestion.externalTitle') }}</h3>
    <p id="external-import-description">
      {{ t('ingestion.externalQuestion', { count: files.length }) }}
    </p>
    <ul class="file-list">
      <li v-for="path in files" :key="path">{{ path }}</li>
    </ul>
    <p class="target-path">{{ t('ingestion.target') }}: {{ targetDir || t('ingestion.noGame') }}</p>
    <fieldset :disabled="navigating">
      <legend>{{ t('ingestion.rule') }}</legend>
      <label class="choice-card" :class="{ selected: mode === 'flatten' }">
        <input v-model="mode" type="radio" name="external-import-mode" value="flatten" />
        <span>
          {{ t('migration.modeFlatten') }}
          <small>{{ t('ingestion.ruleHelp.flatten') }}</small>
        </span>
      </label>
      <label class="choice-card" :class="{ selected: mode === 'by_type' }">
        <input v-model="mode" type="radio" name="external-import-mode" value="by_type" />
        <span>
          {{ t('migration.modeByType') }}
          <small>{{ t('ingestion.ruleHelp.by_type') }}</small>
        </span>
      </label>
    </fieldset>
    <p class="help">{{ t('ingestion.externalHelp') }}</p>
    <p v-if="error" class="error" role="alert">{{ error }}</p>
    <div class="actions">
      <UiButton :disabled="navigating" autofocus @click="cancel" variant="secondary">
        {{ t('common.cancel') }}
      </UiButton>
      <UiButton v-if="!targetDir" @click="openSettings" variant="primary">
        {{ t('ingestion.externalSettings') }}
      </UiButton>
      <UiButton v-else :disabled="navigating" @click="previewImport" variant="primary">
        {{ t('ingestion.preview') }}
      </UiButton>
    </div>
  </UiModal>
</template>

<style scoped>
h3 {
  font-size: var(--text-dialog);
  font-weight: var(--font-heavy);
  margin-bottom: var(--space-2);
}
p {
  font-size: var(--text-sm);
  line-height: 1.7;
  overflow-wrap: anywhere;
}
#external-import-description {
  color: var(--text-secondary);
  font-size: var(--text-description);
}
.file-list {
  margin: var(--space-4) 0 var(--space-2);
  padding: var(--space-3) var(--space-4);
  max-height: 160px;
  overflow: auto;
  background: var(--bg-input);
  border-radius: var(--radius-md);
  font-family: var(--font-mono);
  font-size: var(--text-xs);
  line-height: 1.8;
  overflow-wrap: anywhere;
  user-select: text;
}
.target-path {
  color: var(--text-tertiary);
  font-size: var(--text-xs);
  user-select: text;
}
fieldset {
  border: none;
  padding: 0;
  margin: var(--space-5) 0 var(--space-4);
  display: grid;
  gap: var(--space-2);
}
legend {
  font-size: var(--text-sm);
  font-weight: var(--font-bold);
  margin-bottom: var(--space-2);
}
label {
  display: flex;
  gap: var(--space-3);
  align-items: center;
  padding: var(--space-3) var(--space-4);
  border-radius: var(--radius-md);
  background: var(--bg-input);
  cursor: pointer;
  font-size: var(--text-base);
  font-weight: var(--font-bold);
  transition: background var(--duration-fast) var(--ease);
}
label:hover {
  background: var(--bg-control);
}
label.selected {
  background: var(--accent-subtle);
  box-shadow: inset 0 0 0 1.5px var(--accent-primary);
}
small {
  display: block;
  color: var(--text-secondary);
  margin-top: 2px;
  font-size: var(--text-xs);
  font-weight: var(--font-medium);
  line-height: 1.6;
}
.help {
  color: var(--text-tertiary);
  font-size: var(--text-xs);
}
.error {
  color: var(--color-error);
}
.actions {
  display: flex;
  justify-content: flex-end;
  gap: var(--space-2);
  margin-top: var(--space-5);
}
</style>
