<template>
  <div class="ingestion-view">
    <header class="ingestion-header">
      <div><h2>{{ t('ingestion.title') }}</h2><p class="text-secondary">{{ t(sourceFiles.length ? 'ingestion.externalDescription' : 'ingestion.description') }}</p></div>
      <button class="btn btn-primary" :disabled="sourceBusy || (!sourceFiles.length && !sourceDir.trim()) || !targetDir" @click="handlePreview">
        {{ scanning ? t('ingestion.scanning') : t('ingestion.preview') }}
      </button>
    </header>

    <Transition name="status">
      <section v-if="busy" ref="progressPanel" class="progress-panel glass-panel" role="status" aria-live="polite">
        <div class="progress-heading">
          <div class="progress-title"><span class="progress-spinner" aria-hidden="true" /><h3>{{ t(executing ? 'ingestion.executing' : 'ingestion.scanning') }}</h3></div>
          <span v-if="executing" class="progress-percentage">{{ progressPercent }}%</span>
        </div>
        <progress :value="executing ? processedGroups : undefined" :max="Math.max(progress?.total || 0, 1)" :aria-label="t(executing ? 'ingestion.executing' : 'ingestion.scanning')" />
        <p v-if="executing && progress" class="text-secondary">{{ t('ingestion.result', progress) }}</p>
      </section>
    </Transition>

    <section class="config-panel glass-panel">
      <template v-if="sourceFiles.length">
        <span class="field-label">{{ t('ingestion.externalFiles', { count: sourceFiles.length }) }}</span>
        <ul class="selected-files"><li v-for="path in sourceFiles" :key="path">{{ path }}</li></ul>
        <button class="btn btn-secondary" :disabled="sourceBusy" @click="chooseSource()">{{ t('ingestion.externalChooseFolder') }}</button>
      </template>
      <label v-else for="ingestion-source">{{ t('ingestion.source') }}</label>
      <div v-if="!sourceFiles.length" class="directory-row">
        <div ref="sourcePicker" class="source-picker" @focusout="handleSourceFocusOut">
          <input id="ingestion-source" ref="sourceInput" v-model="sourceDir" :disabled="sourceBusy" :placeholder="t('ingestion.sourcePlaceholder')" role="combobox" aria-autocomplete="none" aria-haspopup="listbox" :aria-expanded="sourceMenuOpen" aria-controls="ingestion-common-paths" :aria-activedescendant="sourceMenuOpen && activePathIndex >= 0 ? `ingestion-common-path-${activePathIndex}` : undefined" autocomplete="off" @focus="openSourceMenu" @click="openSourceMenu" @keydown="handleSourceKeydown" />
          <div v-if="sourceMenuOpen" class="source-dropdown">
            <p class="source-dropdown-title">{{ t('ingestion.commonPaths') }}</p>
            <div id="ingestion-common-paths" class="common-path-list" role="listbox" :aria-label="t('ingestion.commonPaths')">
              <button v-for="(path, index) in ingestionStore.commonPaths" :id="`ingestion-common-path-${index}`" :key="path" class="common-path-option" :class="{ active: activePathIndex === index }" role="option" :aria-selected="sourceDir === path" :title="path" @mouseenter="activePathIndex = index" @click="selectCommonPath(path)">
                <svg width="18" height="18" viewBox="0 0 24 24" fill="none" aria-hidden="true"><path d="M3 7a2 2 0 0 1 2-2h5l2 2h7a2 2 0 0 1 2 2v10H3Z" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" /></svg>
                <span>{{ path }}</span>
                <svg v-if="sourceDir === path" class="common-path-check" width="16" height="16" viewBox="0 0 24 24" fill="none" aria-hidden="true"><path d="m5 12 4 4L19 6" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" /></svg>
              </button>
            </div>
            <button class="add-common-path" @click="chooseSource(true)">
              <svg width="18" height="18" viewBox="0 0 24 24" fill="none" aria-hidden="true"><path d="M12 5v14M5 12h14" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" /></svg>
              {{ t('ingestion.addCommonPath') }}
            </button>
            <p v-if="sourceMenuError" class="source-dropdown-error error-message" role="alert">{{ sourceMenuError }}</p>
          </div>
        </div>
        <button class="btn btn-secondary" :disabled="sourceBusy" @click="chooseSource()">{{ t('settings.browse') }}</button>
      </div>
      <div class="target-directory">
        <span class="field-label">{{ t('ingestion.target') }}</span>
        <p class="target-path">{{ targetDir || t('ingestion.noGame') }}</p>
      </div>
      <fieldset :disabled="busy" class="rules">
        <legend>{{ t('ingestion.rule') }}</legend>
        <div class="rule-options">
          <label v-for="rule in rules" :key="rule.value" class="rule-option" :class="{ selected: mode === rule.value, 'custom-rule': rule.value === 'custom' }">
            <input v-model="mode" type="radio" name="ingestion-rule" :value="rule.value" :aria-labelledby="`ingestion-rule-${rule.value}`" :aria-describedby="`ingestion-rule-help-${rule.value}`" />
            <span class="rule-icon" aria-hidden="true"><svg width="24" height="24" viewBox="0 0 24 24" fill="none"><path :d="rule.icon" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" /></svg></span>
            <span class="rule-content">
              <span :id="`ingestion-rule-${rule.value}`" class="rule-title">{{ t(rule.label) }}</span>
              <span :id="`ingestion-rule-help-${rule.value}`" class="rule-description">{{ t(`ingestion.ruleHelp.${rule.value}`) }}</span>
            </span>
          </label>
        </div>
      </fieldset>
      <template v-if="mode === 'custom'">
        <label for="ingestion-custom">{{ t('ingestion.customFolder') }}</label>
        <input id="ingestion-custom" v-model="customFolder" :disabled="busy" :placeholder="t('ingestion.customPlaceholder')" />
      </template>
      <div class="policy-notes">
        <p class="policy-note">{{ t('ingestion.policy') }}</p>
        <p class="dependency-note">{{ t('ingestion.dependencyNote') }}</p>
      </div>
    </section>

    <p v-if="error" class="error-message" role="alert">{{ error }}</p>
    <dialog ref="previewDialog" class="preview-dialog" aria-labelledby="ingestion-preview-title" aria-describedby="ingestion-preview-summary" @cancel.prevent="cancelPreview" @close="handlePreviewClose">
      <template v-if="preview">
        <div class="preview-header">
          <div>
            <h3 id="ingestion-preview-title">{{ t('ingestion.previewTitle') }}</h3>
            <p id="ingestion-preview-summary" class="text-secondary">{{ t('ingestion.summary', { total: preview.total_files, moved: preview.move_count, retired: preview.retire_count, skipped: preview.skipped }) }}</p>
          </div>
        </div>
        <div class="preview-body">
          <p v-if="!preview.groups.length" class="empty-message">{{ t('ingestion.empty') }}</p>
          <div v-else class="table-scroll">
            <table>
              <thead><tr><th>{{ t('ingestion.package') }}</th><th>{{ t('ingestion.action') }}</th><th>{{ t('ingestion.paths') }}</th><th>{{ t('ingestion.retired') }}</th></tr></thead>
              <tbody>
                <tr v-for="group in visibleGroups" :key="group.source">
                  <td class="package-cell" :title="group.package_id">{{ group.package_id }}</td>
                  <td class="action-cell"><span class="action-tag" :class="{ incoming: group.incoming }">{{ t(group.incoming ? 'ingestion.move' : 'ingestion.keep') }}</span></td>
                  <td class="path-cell"><div :title="group.source">{{ group.source }}</div><div v-if="group.incoming" class="text-secondary" :title="group.destination">→ {{ group.destination }}</div></td>
                  <td><details v-if="group.retired.length"><summary>{{ t('ingestion.retiredCount', { count: group.retired.length }) }}</summary><div v-for="file in group.retired" :key="file.path" class="retired-path">{{ file.path }}</div></details><span v-else>—</span></td>
                </tr>
              </tbody>
            </table>
          </div>
          <div v-if="pageCount > 1" class="pagination">
            <button class="btn btn-secondary" :disabled="page === 1" @click="page--">{{ t('common.back') }}</button>
            <span>{{ page }} / {{ pageCount }}</span>
            <button class="btn btn-secondary" :disabled="page === pageCount" @click="page++">{{ t('common.next') }}</button>
          </div>
          <details v-if="preview.warnings.length" class="warnings"><summary>{{ t('ingestion.warnings', { count: preview.warnings.length }) }}</summary><p v-for="(warning, index) in preview.warnings" :key="index">{{ warning }}</p></details>
        </div>
        <div class="preview-actions">
          <button class="btn btn-secondary" autofocus @click="cancelPreview">{{ t('common.cancel') }}</button>
          <button class="btn btn-primary" :disabled="sourceBusy || consumed || !preview.groups.length" @click="handleExecute">{{ t('ingestion.execute') }}</button>
        </div>
      </template>
    </dialog>

    <dialog ref="resultDialog" class="result-dialog" aria-labelledby="ingestion-result-title" aria-describedby="ingestion-result-summary">
      <div class="result-heading">
        <span class="result-icon" :class="{ incomplete: runIncomplete }" aria-hidden="true">
          <svg width="24" height="24" viewBox="0 0 24 24" fill="none"><path :d="runIncomplete ? 'M12 8v5m0 3h.01M10.3 3.9 2.2 18a2 2 0 0 0 1.7 3h16.2a2 2 0 0 0 1.7-3L13.7 3.9a2 2 0 0 0-3.4 0Z' : 'M5 12l4 4L19 6'" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" /></svg>
        </span>
        <h3 id="ingestion-result-title">{{ t(runIncomplete ? 'ingestion.incomplete' : 'ingestion.finished') }}</h3>
      </div>
      <p id="ingestion-result-summary" class="text-secondary">{{ status ? t('ingestion.result', status) : error }}</p>
      <dl v-if="status" class="result-stats">
        <div><dt>{{ t('ingestion.imported') }}</dt><dd>{{ status.moved }}</dd></div>
        <div><dt>{{ t('ingestion.recycled') }}</dt><dd>{{ status.retired }}</dd></div>
        <div :class="{ 'failed-stat': status.failed > 0 }"><dt>{{ t('migration.failed') }}</dt><dd>{{ status.failed }}</dd></div>
      </dl>
      <div v-if="status?.errors.length || (status && error)" class="result-errors" role="alert">
        <p v-if="error" class="error-message">{{ error }}</p>
        <p v-for="(message, index) in status?.errors" :key="index" class="error-message">{{ message }}</p>
      </div>
      <p class="result-help text-secondary">{{ t(result ? 'ingestion.afterRun' : 'ingestion.retryAfterFailure') }}</p>
      <div class="result-actions">
        <RouterLink v-if="status?.retired" to="/trash" class="btn btn-secondary" @click="resultDialog?.close()">{{ t('ingestion.openTrash') }}</RouterLink>
        <button class="btn btn-primary" autofocus @click="resultDialog?.close()">{{ t('common.confirm') }}</button>
      </div>
    </dialog>
  </div>
</template>

<script setup lang="ts">
import { computed, nextTick, onActivated, onDeactivated, onMounted, onUnmounted, ref, watch } from 'vue'
import { onBeforeRouteLeave, useRoute } from 'vue-router'
import { useI18n } from 'vue-i18n'
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { open } from '@tauri-apps/plugin-dialog'
import { useAppStore } from '@/stores/app'
import { useIngestionStore } from '@/stores/ingestion'

interface IngestionGroup {
  package_id: string
  source: string
  destination: string
  incoming: boolean
  retired: { path: string; size: number; modified: string }[]
}
interface IngestionPreview {
  plan_id: string
  source_dir: string
  target_dir: string
  total_files: number
  move_count: number
  retire_count: number
  skipped: number
  warnings: string[]
  groups: IngestionGroup[]
}
interface IngestionResult {
  plan_id: string
  completed: number
  moved: number
  retired: number
  failed: number
  total: number
  errors: string[]
}

const { t, locale } = useI18n()
const appStore = useAppStore()
const ingestionStore = useIngestionStore()
const route = useRoute()
const sourceDir = ref('')
const sourceFiles = ref<string[]>([])
const sourcePicker = ref<HTMLElement | null>(null)
const sourceInput = ref<HTMLInputElement | null>(null)
const sourceMenuOpen = ref(false)
const sourceMenuError = ref('')
const activePathIndex = ref(-1)
const choosingSource = ref(false)
const mode = ref('by_type')
const customFolder = ref('')
const scanning = ref(false)
const executing = ref(false)
const busy = computed(() => scanning.value || executing.value)
const sourceBusy = computed(() => busy.value || choosingSource.value)
const consumed = ref(false)
const error = ref('')
const preview = ref<IngestionPreview | null>(null)
const previewDialog = ref<HTMLDialogElement | null>(null)
const previewOpen = ref(false)
const progress = ref<IngestionResult | null>(null)
const result = ref<IngestionResult | null>(null)
const status = computed(() => result.value || progress.value)
const progressPanel = ref<HTMLElement | null>(null)
const resultDialog = ref<HTMLDialogElement | null>(null)
const processedGroups = computed(() => (progress.value?.completed || 0) + (progress.value?.failed || 0))
const progressPercent = computed(() => progress.value?.total ? Math.min(100, Math.round(processedGroups.value / progress.value.total * 100)) : 0)
const runIncomplete = computed(() => !!error.value || !!status.value && (status.value.failed > 0 || status.value.errors.length > 0 || status.value.completed < status.value.total))
const page = ref(1)
const pageCount = computed(() => Math.max(1, Math.ceil((preview.value?.groups.length || 0) / 50)))
const visibleGroups = computed(() => preview.value?.groups.slice((page.value - 1) * 50, page.value * 50) || [])
const targetDir = computed(() => appStore.installContext?.realAddonDir || (appStore.vamRootPath ? `${appStore.vamRootPath}/AddonPackages` : ''))
const rules = [
  { value: 'flatten', label: 'migration.modeFlatten', icon: 'M12 3v12m-5-5 5 5 5-5M4 20h16' },
  { value: 'by_type', label: 'migration.modeByType', icon: 'M3 3h6v6H3zM15 3h6v6h-6zM3 15h6v6H3zM15 15h6v6h-6z' },
  { value: 'by_creator', label: 'migration.modeByCreator', icon: 'M16 21v-2a4 4 0 0 0-4-4H6a4 4 0 0 0-4 4v2m20 0v-2a4 4 0 0 0-3-3.9M13 7a4 4 0 1 1-8 0 4 4 0 0 1 8 0Zm3-3.9a4 4 0 0 1 0 7.8' },
  { value: 'by_scene', label: 'migration.modeByScene', icon: 'M3 4h18v16H3zM3 8h18M6 6h.01M9 6h.01' },
  { value: 'custom', label: 'migration.modeByCustom', icon: 'M3 7a2 2 0 0 1 2-2h5l2 2h7a2 2 0 0 1 2 2v10H3ZM12 11v6m-3-3h6' },
]

watch([sourceDir, sourceFiles, mode, customFolder, targetDir, locale], () => {
  cancelPreview()
  page.value = 1
  error.value = ''
})
watch([sourceBusy, previewOpen], ([isBusy, isPreviewOpen]) => {
  ingestionStore.busy = isBusy || isPreviewOpen
  if (isBusy || isPreviewOpen) closeSourceMenu()
}, { flush: 'sync' })
watch(() => ingestionStore.selection, applyExternalSelection, { immediate: true })
onActivated(applyExternalSelection)
onDeactivated(() => { cancelPreview(); resultDialog.value?.close(); closeSourceMenu() })
onMounted(() => document.addEventListener('pointerdown', handleSourceOutsideClick))
onUnmounted(() => document.removeEventListener('pointerdown', handleSourceOutsideClick))
onBeforeRouteLeave(() => !sourceBusy.value)

function openSourceMenu() {
  if (sourceBusy.value) return
  sourceMenuOpen.value = true
}

function closeSourceMenu() {
  sourceMenuOpen.value = false
  activePathIndex.value = -1
}

function cancelPreview() {
  previewDialog.value?.close()
  previewOpen.value = false
  preview.value = null
  page.value = 1
}

function handlePreviewClose() {
  if (!previewDialog.value?.open) cancelPreview()
}

function handleSourceOutsideClick(event: PointerEvent) {
  if (event.target instanceof Node && !sourcePicker.value?.contains(event.target)) closeSourceMenu()
}

function handleSourceFocusOut(event: FocusEvent) {
  if (!(event.relatedTarget instanceof Node) || !sourcePicker.value?.contains(event.relatedTarget)) closeSourceMenu()
}

function selectCommonPath(path: string) {
  if (sourceBusy.value) return
  sourceDir.value = path
  sourceMenuError.value = ''
  sourceInput.value?.focus()
  closeSourceMenu()
}

function handleSourceKeydown(event: KeyboardEvent) {
  if (event.key === 'Escape' && sourceMenuOpen.value) {
    event.preventDefault()
    closeSourceMenu()
  } else if (event.key === 'ArrowDown' || event.key === 'ArrowUp') {
    event.preventDefault()
    openSourceMenu()
    const count = ingestionStore.commonPaths.length
    if (!count) return
    activePathIndex.value = activePathIndex.value < 0
      ? (event.key === 'ArrowDown' ? 0 : count - 1)
      : (activePathIndex.value + (event.key === 'ArrowDown' ? 1 : -1) + count) % count
    void nextTick(() => document.getElementById(`ingestion-common-path-${activePathIndex.value}`)?.scrollIntoView({ block: 'nearest' }))
  } else if (event.key === 'Enter' && sourceMenuOpen.value && activePathIndex.value >= 0) {
    event.preventDefault()
    selectCommonPath(ingestionStore.commonPaths[activePathIndex.value])
  }
}

async function applyExternalSelection() {
  const selection = ingestionStore.selection
  if (!selection || sourceBusy.value || previewOpen.value || route.name !== 'ingestion') return
  ingestionStore.selection = null
  sourceFiles.value = selection.files
  sourceDir.value = ''
  mode.value = selection.mode
  await handlePreview()
}

async function chooseSource(saveCommon = false) {
  if (sourceBusy.value) return
  choosingSource.value = true
  sourceMenuError.value = ''
  try {
    const selected = await open({ directory: true, multiple: false })
    if (typeof selected === 'string') {
      sourceFiles.value = []
      sourceDir.value = selected
      if (saveCommon) {
        try { ingestionStore.addCommonPath(selected) }
        catch (err) { sourceMenuError.value = t('ingestion.commonPathSaveFailed', { error: String(err) }) }
      }
    }
  } catch (err) { error.value = String(err) }
  finally {
    choosingSource.value = false
    if (saveCommon) {
      await nextTick()
      sourceInput.value?.focus()
      openSourceMenu()
    }
  }
}

async function handlePreview() {
  if (sourceBusy.value) return
  scanning.value = true
  cancelPreview()
  result.value = null
  progress.value = null
  resultDialog.value?.close()
  error.value = ''
  consumed.value = false
  page.value = 1
  const config = { source_dir: sourceDir.value.trim(), source_files: sourceFiles.value.length ? [...sourceFiles.value] : null, mode: mode.value, custom_folder: customFolder.value, locale: locale.value }
  const target = targetDir.value
  try {
    const response = await invoke<IngestionPreview>('preview_ingestion', { config })
    if (target === targetDir.value && config.locale === locale.value) {
      preview.value = response
      await nextTick()
      previewDialog.value?.showModal()
      previewOpen.value = !!previewDialog.value?.open
    }
  } catch (err) { cancelPreview(); error.value = String(err) }
  finally { scanning.value = false }
}

async function handleExecute() {
  if (!preview.value || sourceBusy.value || consumed.value) return
  const planId = preview.value.plan_id
  progress.value = { plan_id: planId, completed: 0, moved: 0, retired: 0, failed: 0, total: preview.value.groups.length, errors: [] }
  result.value = null
  executing.value = true
  consumed.value = true
  error.value = ''
  cancelPreview()
  let unlisten: (() => void) | undefined
  try {
    await nextTick()
    progressPanel.value?.scrollIntoView({ block: 'start', behavior: 'auto' })
    unlisten = await listen<IngestionResult>('ingestion-progress', event => {
      if (event.payload.plan_id === planId) progress.value = event.payload
    })
    result.value = await invoke<IngestionResult>('execute_ingestion', { planId })
  } catch (err) { error.value = String(err) }
  finally {
    unlisten?.()
    executing.value = false
    await nextTick()
    resultDialog.value?.showModal()
  }
}
</script>

<style scoped>
.ingestion-view { display: flex; flex-direction: column; gap: var(--space-4); min-width: 0; }
.ingestion-header, .preview-header { display: flex; align-items: center; justify-content: space-between; gap: var(--space-4); }
h2 { font-size: var(--text-xl); margin-bottom: var(--space-2); }
h3 { font-size: var(--text-base); margin-bottom: var(--space-2); }
p { font-size: var(--text-sm); line-height: 1.7; overflow-wrap: anywhere; }
.config-panel, .progress-panel { padding: var(--space-5); min-width: 0; border-radius: var(--radius-md); }
.config-panel { display: flex; flex-direction: column; gap: var(--space-3); }
.selected-files { max-height: 180px; overflow: auto; padding: var(--space-3) var(--space-5); background: var(--bg-subtle); border-radius: var(--radius-sm); font-size: var(--text-xs); line-height: 1.8; overflow-wrap: anywhere; user-select: text; }
label, legend, .field-label { font-size: var(--text-sm); font-weight: var(--font-medium); }
.directory-row { display: flex; align-items: center; gap: var(--space-2); }
.source-picker { position: relative; flex: 1; min-width: 0; }
.source-dropdown { position: absolute; top: calc(100% + var(--space-2)); left: 0; right: 0; z-index: 10; padding: var(--space-2); background: var(--bg-surface); border: 1px solid var(--border-strong); border-radius: var(--radius-md); box-shadow: var(--shadow-lg); }
.source-dropdown-title { padding: var(--space-1) var(--space-2) var(--space-2); color: var(--text-secondary); font-size: var(--text-xs); }
.common-path-list { max-height: 220px; overflow: auto; }
.common-path-option, .add-common-path { display: flex; align-items: center; gap: var(--space-2); width: 100%; min-height: 38px; padding: var(--space-2); border-radius: var(--radius-sm); text-align: left; font-size: var(--text-sm); }
.common-path-option > svg, .add-common-path > svg { flex-shrink: 0; color: var(--accent-primary-hover); }
.common-path-option > span { flex: 1; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.common-path-option:hover, .common-path-option.active, .add-common-path:hover { background: var(--bg-hover); }
.common-path-option[aria-selected='true'] { background: var(--accent-subtle); }
.common-path-option:focus-visible, .add-common-path:focus-visible { outline: 2px solid var(--accent-primary); outline-offset: -2px; }
.common-path-check { margin-left: auto; }
.add-common-path { color: var(--accent-primary-hover); }
.common-path-list:not(:empty) + .add-common-path { margin-top: var(--space-2); border-top: 1px solid var(--border-subtle); border-radius: 0 0 var(--radius-sm) var(--radius-sm); }
.source-dropdown-error { padding: var(--space-2); font-size: var(--text-xs); }
input[type='text'], input:not([type]) { min-width: 0; width: 100%; height: 38px; padding: 9px 12px; border: 1px solid var(--border-default); border-radius: var(--radius-sm); color: var(--text-primary); background: var(--bg-input, var(--bg-base)); font-size: var(--text-sm); }
input:focus-visible { outline: 2px solid var(--accent-primary); outline-offset: 2px; }
.target-directory { display: grid; grid-template-columns: auto minmax(0, 1fr); align-items: baseline; gap: var(--space-3); padding: var(--space-3); background: var(--bg-subtle); border-radius: var(--radius-sm); }
.target-directory .field-label { color: var(--text-secondary); }
.target-path { color: var(--text-primary); margin: 0; user-select: text; }
.rules { border: 0; padding: 0; margin: var(--space-3) 0 0; min-width: 0; }
.rule-options { display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); gap: var(--space-4); }
legend { margin-bottom: var(--space-3); }
.rule-option { position: relative; display: flex; flex-direction: column; align-items: flex-start; gap: var(--space-3); min-height: 156px; padding: var(--space-6); border: 1px solid var(--border-default); background: var(--bg-subtle); border-radius: var(--radius-lg); cursor: pointer; line-height: 1.5; transition: background var(--duration-fast) var(--ease), border-color var(--duration-fast) var(--ease); }
.rules:not(:disabled) .rule-option:hover { background: var(--bg-hover); border-color: var(--border-strong); }
.rules:disabled .rule-option { opacity: 0.5; cursor: not-allowed; }
.rule-option:focus-within { outline: 2px solid var(--accent-primary); outline-offset: 2px; }
.rule-option.selected { background: var(--accent-subtle); border-color: var(--accent-primary); }
.rules:not(:disabled) .rule-option.selected:hover { border-color: var(--accent-primary); }
input[type='radio'] { position: absolute; top: var(--space-6); right: var(--space-6); width: 16px; height: 16px; accent-color: var(--accent-primary); cursor: inherit; }
.rule-icon { display: grid; place-items: center; flex-shrink: 0; width: 48px; height: 48px; border-radius: var(--radius-md); color: var(--accent-primary-hover); background: var(--accent-muted); }
.rule-content { display: flex; flex-direction: column; gap: var(--space-2); padding-right: var(--space-4); }
.rule-title { color: var(--text-primary); font-size: var(--text-md); font-weight: var(--font-semibold); }
.rule-description { color: var(--text-secondary); font-size: var(--text-sm); font-weight: var(--font-normal); line-height: 1.7; }
.custom-rule { grid-column: 1 / -1; flex-direction: row; align-items: center; min-height: 100px; }
.policy-notes { display: grid; gap: var(--space-2); border-top: 1px solid var(--border-subtle); padding-top: var(--space-4); margin-top: var(--space-1); }
.policy-notes p { margin: 0; color: var(--text-secondary); font-size: var(--text-xs); font-weight: var(--font-normal); line-height: 1.8; }
.table-scroll { overflow: auto; max-height: 480px; margin-top: var(--space-4); border: 1px solid var(--border-subtle); border-radius: var(--radius-sm); }
table { width: 100%; min-width: 680px; table-layout: fixed; border-collapse: collapse; font-size: var(--text-sm); text-align: left; }
th, td { padding: 12px; border-bottom: 1px solid var(--border-subtle); vertical-align: top; }
th { font-weight: var(--font-medium); color: var(--text-secondary); white-space: nowrap; position: sticky; top: 0; background: var(--bg-surface); z-index: 1; }
th:first-child { width: 24%; }
th:nth-child(2) { width: 100px; }
th:last-child { width: 140px; }
tbody tr:hover { background: var(--bg-subtle); }
.package-cell { font-weight: var(--font-medium); }
.action-cell { white-space: nowrap; }
.action-tag { display: inline-block; padding: 3px 8px; border-radius: var(--radius-sm); color: var(--text-secondary); background: var(--bg-subtle); font-size: var(--text-xs); }
.action-tag.incoming { color: var(--accent-primary-hover); background: var(--accent-muted); }
td { overflow-wrap: anywhere; }
.path-cell { font-size: var(--text-xs); line-height: 1.7; user-select: text; }
.path-cell > div { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.retired-path { margin-top: var(--space-2); max-width: 300px; color: var(--text-secondary); }
summary { cursor: pointer; }
.warnings, .pagination, .empty-message { margin-top: var(--space-4); }
.pagination { display: flex; align-items: center; justify-content: flex-end; gap: var(--space-3); }
.error-message { color: var(--color-error); white-space: pre-wrap; }
.progress-panel { position: sticky; top: 0; z-index: 5; border-color: var(--border-accent); background: var(--bg-surface); box-shadow: var(--shadow-md); }
.progress-heading, .progress-title { display: flex; align-items: center; gap: var(--space-3); }
.progress-heading { justify-content: space-between; }
.progress-title h3 { margin: 0; }
.progress-percentage { color: var(--accent-primary-hover); font-size: var(--text-lg); font-weight: var(--font-semibold); font-variant-numeric: tabular-nums; }
.progress-spinner { width: 16px; height: 16px; flex-shrink: 0; border: 2px solid var(--accent-muted); border-top-color: var(--accent-primary); border-radius: 50%; animation: progress-spin 0.8s linear infinite; }
progress { display: block; width: 100%; height: 6px; margin: var(--space-3) 0; overflow: hidden; appearance: none; border: 0; border-radius: var(--radius-full); background: var(--bg-elevated); accent-color: var(--accent-primary); }
progress::-webkit-progress-bar { background: var(--bg-elevated); border-radius: var(--radius-full); }
progress::-webkit-progress-value { background: var(--accent-primary); border-radius: var(--radius-full); transition: width var(--duration-base) var(--ease); }
progress::-moz-progress-bar { background: var(--accent-primary); border-radius: var(--radius-full); }
progress:indeterminate { background: linear-gradient(90deg, var(--bg-elevated) 0%, var(--bg-elevated) 35%, var(--accent-primary) 50%, var(--bg-elevated) 65%, var(--bg-elevated) 100%); background-size: 200% 100%; animation: progress-scan 1.4s linear infinite; }
progress:indeterminate::-webkit-progress-bar { background: transparent; }
.preview-dialog, .result-dialog { margin: auto; width: min(520px, calc(100vw - 48px)); max-height: calc(100vh - 64px); overflow: auto; padding: var(--space-6); color: var(--text-primary); background: var(--bg-surface); border: 1px solid var(--border-default); border-radius: var(--radius-lg); box-shadow: var(--shadow-lg); }
.preview-dialog[open], .result-dialog[open] { animation: result-enter var(--duration-base) var(--ease-out); }
.preview-dialog::backdrop, .result-dialog::backdrop { background: rgba(0, 0, 0, 0.6); }
.preview-dialog { width: min(1040px, calc(100vw - 48px)); overflow: hidden; }
.preview-dialog[open] { display: flex; flex-direction: column; }
.preview-header { flex-shrink: 0; }
.preview-header h3 { font-size: var(--text-lg); }
.preview-body { min-height: 0; overflow: auto; margin-top: var(--space-4); }
.preview-body .table-scroll { margin-top: 0; max-height: none; }
.preview-body .empty-message { margin-top: 0; }
.preview-actions { display: flex; flex-shrink: 0; justify-content: flex-end; gap: var(--space-2); margin-top: var(--space-5); padding-top: var(--space-4); border-top: 1px solid var(--border-subtle); }
.result-heading { display: flex; align-items: center; gap: var(--space-3); margin-bottom: var(--space-3); }
.result-heading h3 { margin: 0; font-size: var(--text-lg); }
.result-icon { display: grid; place-items: center; width: 44px; height: 44px; border-radius: var(--radius-md); color: var(--color-success); background: var(--color-success-bg); }
.result-icon.incomplete { color: var(--color-warning); background: var(--color-warning-bg); }
.result-stats { display: grid; grid-template-columns: repeat(3, minmax(0, 1fr)); gap: var(--space-3); margin: var(--space-5) 0; padding: var(--space-4); background: var(--bg-subtle); border-radius: var(--radius-md); }
.result-stats dt { font-size: var(--text-xs); color: var(--text-secondary); }
.result-stats dd { margin: var(--space-2) 0 0; font-size: var(--text-2xl); font-weight: var(--font-semibold); font-variant-numeric: tabular-nums; }
.failed-stat dd { color: var(--color-error); }
.result-errors { max-height: 180px; overflow: auto; padding: var(--space-3); background: var(--color-error-bg); border-radius: var(--radius-sm); user-select: text; }
.result-errors p + p { margin-top: var(--space-2); }
.result-help { margin-top: var(--space-4); font-size: var(--text-xs); }
.result-actions { display: flex; justify-content: flex-end; gap: var(--space-2); margin-top: var(--space-5); }
.status-enter-active, .status-leave-active { transition: opacity var(--duration-fast) var(--ease), transform var(--duration-fast) var(--ease); }
.status-enter-from, .status-leave-to { opacity: 0; transform: translateY(-4px); }
@keyframes progress-spin { to { transform: rotate(360deg); } }
@keyframes progress-scan { to { background-position: -200% 0; } }
@keyframes result-enter { from { opacity: 0; transform: translateY(8px); } to { opacity: 1; transform: translateY(0); } }
.btn:disabled { opacity: 0.45; cursor: not-allowed; }
.btn { display: inline-flex; align-items: center; justify-content: center; flex-shrink: 0; white-space: nowrap; min-height: 36px; padding: 0 var(--space-4); border-radius: var(--radius-sm); font-size: var(--text-sm); }
.directory-row .btn { min-width: 76px; height: 38px; }
.btn:focus-visible { outline: 2px solid var(--accent-primary); outline-offset: 2px; }
@media (max-width: 800px) { .ingestion-header, .preview-header { align-items: flex-start; flex-direction: column; } .target-directory { grid-template-columns: 1fr; gap: var(--space-1); } }
@media (max-width: 600px) { .rule-options { grid-template-columns: 1fr; gap: var(--space-3); } .rule-option { flex-direction: row; align-items: center; min-height: 112px; padding: var(--space-4); } .rule-content { padding-right: var(--space-5); } input[type='radio'] { top: var(--space-4); right: var(--space-4); } .result-actions { flex-wrap: wrap; } }
@media (prefers-reduced-motion: reduce) { .rule-option, progress::-webkit-progress-value, .status-enter-active, .status-leave-active { transition: none; } .progress-spinner, progress:indeterminate, .preview-dialog[open], .result-dialog[open] { animation: none; } }
</style>
