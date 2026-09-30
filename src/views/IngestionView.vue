<template>
  <div class="ingestion-view">
    <header class="ingestion-header">
      <div><h2>{{ t('ingestion.title') }}</h2><p class="text-secondary">{{ t('ingestion.description') }}</p></div>
    </header>

    <section class="config-panel glass-panel">
      <label for="ingestion-source">{{ t('ingestion.source') }}</label>
      <div class="directory-row">
        <input id="ingestion-source" v-model="sourceDir" :disabled="busy" :placeholder="t('ingestion.sourcePlaceholder')" />
        <button class="btn btn-secondary" :disabled="busy" @click="chooseSource">{{ t('settings.browse') }}</button>
      </div>
      <div class="target-directory">
        <span class="field-label">{{ t('ingestion.target') }}</span>
        <p class="target-path">{{ targetDir || t('ingestion.noGame') }}</p>
      </div>
      <fieldset :disabled="busy" class="rules">
        <legend>{{ t('ingestion.rule') }}</legend>
        <div class="rule-options">
        <label v-for="rule in rules" :key="rule.value" class="rule-option" :class="{ selected: mode === rule.value }">
          <input v-model="mode" type="radio" name="ingestion-rule" :value="rule.value" />
          <span>{{ t(rule.label) }}</span>
        </label>
        </div>
      </fieldset>
      <template v-if="mode === 'custom'">
        <label for="ingestion-custom">{{ t('ingestion.customFolder') }}</label>
        <input id="ingestion-custom" v-model="customFolder" :disabled="busy" :placeholder="t('ingestion.customPlaceholder')" />
      </template>
      <p class="text-secondary rule-help">{{ t(`ingestion.ruleHelp.${mode}`) }}</p>
      <div class="policy-notes">
        <p class="policy-note">{{ t('ingestion.policy') }}</p>
        <p class="dependency-note">{{ t('ingestion.dependencyNote') }}</p>
      </div>
      <div class="config-actions">
        <button class="btn btn-primary" :disabled="busy || !sourceDir.trim() || !targetDir" @click="handlePreview">
          {{ scanning ? t('ingestion.scanning') : t('ingestion.preview') }}
        </button>
      </div>
    </section>

    <p v-if="error" class="error-message" role="alert">{{ error }}</p>
    <section v-if="preview" class="preview-panel glass-panel">
      <div class="preview-header">
        <div>
          <h3>{{ t('ingestion.previewTitle') }}</h3>
          <p class="text-secondary">{{ t('ingestion.summary', { total: preview.total_files, moved: preview.move_count, retired: preview.retire_count, skipped: preview.skipped }) }}</p>
        </div>
        <button class="btn btn-primary" :disabled="busy || consumed || !preview.groups.length" @click="handleExecute">
          {{ executing ? t('ingestion.executing') : t('ingestion.execute') }}
        </button>
      </div>
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
    </section>

    <section v-if="progress || result" class="result-panel glass-panel" aria-live="polite">
      <h3>{{ t(executing ? 'ingestion.executing' : error || result?.errors.length ? 'ingestion.incomplete' : 'ingestion.finished') }}</h3>
      <template v-if="status">
        <progress :value="status.completed + status.failed" :max="Math.max(status.total, 1)" />
        <p>{{ t('ingestion.result', { completed: status.completed, total: status.total, moved: status.moved, retired: status.retired, failed: status.failed }) }}</p>
        <p v-for="(message, index) in status.errors" :key="index" class="error-message">{{ message }}</p>
      </template>
      <p v-if="result" class="text-secondary">{{ t('ingestion.afterRun') }}</p>
      <RouterLink v-if="result?.retired" to="/trash" class="trash-link">{{ t('ingestion.openTrash') }}</RouterLink>
    </section>
  </div>
</template>

<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { onBeforeRouteLeave } from 'vue-router'
import { useI18n } from 'vue-i18n'
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { open } from '@tauri-apps/plugin-dialog'
import { useAppStore } from '@/stores/app'

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
const sourceDir = ref('')
const mode = ref('flatten')
const customFolder = ref('')
const scanning = ref(false)
const executing = ref(false)
const busy = computed(() => scanning.value || executing.value)
const consumed = ref(false)
const error = ref('')
const preview = ref<IngestionPreview | null>(null)
const progress = ref<IngestionResult | null>(null)
const result = ref<IngestionResult | null>(null)
const status = computed(() => result.value || progress.value)
const page = ref(1)
const pageCount = computed(() => Math.max(1, Math.ceil((preview.value?.groups.length || 0) / 50)))
const visibleGroups = computed(() => preview.value?.groups.slice((page.value - 1) * 50, page.value * 50) || [])
const targetDir = computed(() => appStore.installContext?.realAddonDir || (appStore.vamRootPath ? `${appStore.vamRootPath}/AddonPackages` : ''))
const rules = [
  { value: 'flatten', label: 'migration.modeFlatten' },
  { value: 'by_type', label: 'migration.modeByType' },
  { value: 'by_creator', label: 'migration.modeByCreator' },
  { value: 'by_scene', label: 'migration.modeByScene' },
  { value: 'custom', label: 'migration.modeByCustom' },
]

watch([sourceDir, mode, customFolder, targetDir, locale], () => {
  preview.value = null
  page.value = 1
  error.value = ''
})
onBeforeRouteLeave(() => !busy.value)

async function chooseSource() {
  try {
    const selected = await open({ directory: true, multiple: false })
    if (typeof selected === 'string') sourceDir.value = selected
  } catch (err) { error.value = String(err) }
}

async function handlePreview() {
  if (busy.value) return
  scanning.value = true
  preview.value = null
  result.value = null
  progress.value = null
  error.value = ''
  consumed.value = false
  page.value = 1
  const config = { source_dir: sourceDir.value.trim(), mode: mode.value, custom_folder: customFolder.value, locale: locale.value }
  const target = targetDir.value
  try {
    const response = await invoke<IngestionPreview>('preview_ingestion', { config })
    if (target === targetDir.value && config.locale === locale.value) preview.value = response
  } catch (err) { error.value = String(err) }
  finally { scanning.value = false }
}

async function handleExecute() {
  if (!preview.value || busy.value || consumed.value) return
  const planId = preview.value.plan_id
  executing.value = true
  consumed.value = true
  error.value = ''
  let unlisten: (() => void) | undefined
  try {
    unlisten = await listen<IngestionResult>('ingestion-progress', event => {
      if (event.payload.plan_id === planId) progress.value = event.payload
    })
    result.value = await invoke<IngestionResult>('execute_ingestion', { planId })
  } catch (err) { error.value = String(err) }
  finally { unlisten?.(); executing.value = false }
}
</script>

<style scoped>
.ingestion-view { display: flex; flex-direction: column; gap: var(--space-4); min-width: 0; }
.ingestion-header, .preview-header { display: flex; align-items: center; justify-content: space-between; gap: var(--space-4); }
h2 { font-size: var(--text-xl); margin-bottom: var(--space-2); }
h3 { font-size: var(--text-base); margin-bottom: var(--space-2); }
p { font-size: var(--text-sm); line-height: 1.7; overflow-wrap: anywhere; }
.config-panel, .preview-panel, .result-panel { padding: var(--space-5); min-width: 0; border-radius: var(--radius-md); }
.config-panel { display: flex; flex-direction: column; gap: var(--space-3); }
label, legend, .field-label { font-size: var(--text-sm); font-weight: var(--font-medium); }
.directory-row { display: flex; align-items: center; gap: var(--space-2); }
.directory-row input { flex: 1; }
input[type='text'], input:not([type]) { min-width: 0; width: 100%; height: 38px; padding: 9px 12px; border: 1px solid var(--border-default); border-radius: var(--radius-sm); color: var(--text-primary); background: var(--bg-input, var(--bg-base)); font-size: var(--text-sm); }
input:focus-visible { outline: 2px solid var(--accent-primary); outline-offset: 2px; }
.target-directory { display: grid; grid-template-columns: auto minmax(0, 1fr); align-items: baseline; gap: var(--space-3); padding: var(--space-3); background: var(--bg-subtle); border-radius: var(--radius-sm); }
.target-directory .field-label { color: var(--text-secondary); }
.target-path { color: var(--text-primary); margin: 0; user-select: text; }
.rules { border: 0; padding: 0; margin: var(--space-3) 0 0; min-width: 0; }
.rule-options { display: grid; grid-template-columns: repeat(auto-fit, minmax(150px, 1fr)); gap: var(--space-2); }
legend { margin-bottom: var(--space-3); }
.rule-option { display: flex; align-items: center; gap: var(--space-2); min-height: 42px; padding: 10px 12px; border: 1px solid var(--border-default); border-radius: var(--radius-sm); cursor: pointer; line-height: 1.5; transition: background var(--duration-fast) var(--ease), border-color var(--duration-fast) var(--ease); }
.rule-option:hover { background: var(--bg-hover); }
.rules:disabled .rule-option { opacity: 0.5; cursor: not-allowed; }
.rule-option:focus-within { outline: 2px solid var(--accent-primary); outline-offset: 2px; }
.rule-option.selected { background: var(--accent-subtle); border-color: var(--accent-primary); }
input[type='radio'] { accent-color: var(--accent-primary); flex-shrink: 0; }
.rule-help { margin: 0; }
.policy-notes { display: grid; gap: var(--space-2); border-top: 1px solid var(--border-subtle); padding-top: var(--space-4); margin-top: var(--space-1); }
.policy-notes p { margin: 0; color: var(--text-secondary); font-size: var(--text-xs); font-weight: var(--font-normal); line-height: 1.8; }
.config-actions { display: flex; justify-content: flex-end; padding-top: var(--space-2); }
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
.error-message { color: var(--error, #e57373); white-space: pre-wrap; }
progress { width: 100%; margin: var(--space-3) 0; accent-color: var(--accent-primary); }
.trash-link { color: var(--accent-primary); font-size: var(--text-sm); }
.btn:disabled { opacity: 0.45; cursor: not-allowed; }
.btn { display: inline-flex; align-items: center; justify-content: center; flex-shrink: 0; white-space: nowrap; min-height: 36px; padding: 0 var(--space-4); border-radius: var(--radius-sm); font-size: var(--text-sm); }
.directory-row .btn { min-width: 76px; height: 38px; }
.btn:focus-visible { outline: 2px solid var(--accent-primary); outline-offset: 2px; }
@media (max-width: 800px) { .ingestion-header, .preview-header { align-items: flex-start; flex-direction: column; } .target-directory { grid-template-columns: 1fr; gap: var(--space-1); } }
</style>
