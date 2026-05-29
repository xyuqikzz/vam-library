<template>
  <div class="vam-prefs-view animate-fadeIn">
    <div class="view-header">
      <h1 class="view-title">{{ $t('vamPrefs.title') }}</h1>
      <p class="view-subtitle">{{ $t('vamPrefs.subtitle') }}</p>
    </div>

    <GlassPanel :title="$t('vamPrefs.currentFile')">
      <div class="prefs-toolbar">
        <div class="path-display">
          <svg width="14" height="14" viewBox="0 0 24 24" fill="none" aria-hidden="true">
            <path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z" stroke="currentColor" stroke-width="1.5" stroke-linejoin="round" />
            <path d="M14 2v6h6" stroke="currentColor" stroke-width="1.5" stroke-linejoin="round" />
          </svg>
          <span class="path-text truncate">{{ prefsPath || $t('vamPrefs.pathPending') }}</span>
        </div>
        <div class="toolbar-actions">
          <button class="btn btn-secondary" type="button" @click="selectVamRoot">
            {{ $t('vamPrefs.selectRoot') }}
          </button>
          <button class="btn btn-secondary" type="button" :disabled="!canRead || isLoading" @click="loadPrefs">
            {{ isLoading ? $t('common.loading') : $t('common.refresh') }}
          </button>
          <button class="btn btn-primary" type="button" :disabled="!canSave" @click="savePrefs">
            {{ isSaving ? $t('common.loading') : $t('vamPrefs.saveWithBackup') }}
          </button>
        </div>
      </div>

      <div class="status-grid">
        <div class="status-item">
          <span class="status-label">{{ $t('vamPrefs.vamRoot') }}</span>
          <span class="status-value truncate">{{ appStore.vamRootPath || $t('settings.notConfigured') }}</span>
        </div>
        <div class="status-item">
          <span class="status-label">{{ $t('vamPrefs.fileStatus') }}</span>
          <span :class="['status-pill', prefsExists ? 'ok' : 'warn']">
            {{ prefsExists ? $t('vamPrefs.fileExists') : $t('vamPrefs.fileMissing') }}
          </span>
        </div>
      </div>

      <p class="warning-text">{{ $t('vamPrefs.runningWarning') }}</p>
    </GlassPanel>

    <GlassPanel :title="$t('vamPrefs.editor')">
      <div class="editor-actions">
        <div class="mode-selector">
          <button
            :class="['mode-btn', { active: editorMode === 'visual' }]"
            type="button"
            @click="editorMode = 'visual'"
          >
            {{ $t('vamPrefs.visualMode') }}
          </button>
          <button
            :class="['mode-btn', { active: editorMode === 'json' }]"
            type="button"
            @click="editorMode = 'json'"
          >
            {{ $t('vamPrefs.jsonMode') }}
          </button>
        </div>
        <button v-if="editorMode === 'json'" class="btn btn-secondary" type="button" :disabled="!prefsContent" @click="formatContent">
          {{ $t('vamPrefs.formatJson') }}
        </button>
        <button class="btn btn-secondary" type="button" :disabled="!prefsContent" @click="copyContent">
          {{ $t('common.copy') }}
        </button>
      </div>

      <div v-if="editorMode === 'visual'" class="visual-editor-shell">
        <JsonVisualEditor
          v-if="prefsValue !== null"
          :model-value="prefsValue"
          @update:model-value="updateVisualValue"
          @invalid="showVisualError"
        />
        <div v-else class="empty-editor">
          {{ $t('vamPrefs.editorPlaceholder') }}
        </div>
      </div>

      <textarea
        v-else
        v-model="prefsContent"
        class="prefs-editor"
        spellcheck="false"
        :placeholder="$t('vamPrefs.editorPlaceholder')"
        @input="syncJsonContent"
      />

      <div class="editor-footer">
        <span :class="['validation-message', jsonError ? 'error' : 'ok']">
          {{ jsonError || $t('vamPrefs.jsonValid') }}
        </span>
        <span v-if="isDirty" class="dirty-dot">{{ $t('vamPrefs.unsaved') }}</span>
      </div>
    </GlassPanel>

    <GlassPanel v-if="lastBackupPath" :title="$t('vamPrefs.lastBackup')">
      <p class="backup-path">{{ lastBackupPath }}</p>
    </GlassPanel>
  </div>
</template>

<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { useI18n } from 'vue-i18n'
import GlassPanel from '@/components/common/GlassPanel.vue'
import JsonVisualEditor from '@/components/prefs/JsonVisualEditor.vue'
import { useAppStore } from '@/stores/app'
import { useNotification } from '@/composables/useNotification'

type JsonValue = any

interface VamPrefsFile {
  path: string
  content: string
  exists: boolean
}

interface SaveVamPrefsResult {
  path: string
  backupPath: string | null
}

const { t } = useI18n()
const appStore = useAppStore()
const notify = useNotification()

const prefsPath = ref('')
const prefsContent = ref('')
const originalContent = ref('')
const prefsValue = ref<JsonValue | null>(null)
const prefsExists = ref(false)
const lastBackupPath = ref('')
const jsonError = ref('')
const isLoading = ref(false)
const isSaving = ref(false)
const editorMode = ref<'visual' | 'json'>('visual')

const canRead = computed(() => Boolean(appStore.vamRootPath))
const isDirty = computed(() => prefsContent.value !== originalContent.value)
const canSave = computed(() => canRead.value && isDirty.value && !jsonError.value && !isSaving.value)

onMounted(() => {
  if (appStore.vamRootPath) {
    void loadPrefs()
  }
})

async function selectVamRoot() {
  try {
    const { open } = await import('@tauri-apps/plugin-dialog')
    const selected = await open({
      directory: true,
      title: t('settings.selectVamDirectory'),
    })
    if (selected && typeof selected === 'string') {
      await appStore.setVamRoot(selected)
      await loadPrefs()
    }
  } catch {
    // 用户取消选择时无需提示。
  }
}

async function loadPrefs() {
  if (!appStore.vamRootPath) return
  isLoading.value = true
  try {
    const result = await invoke<VamPrefsFile>('read_vam_prefs', {
      vamRoot: appStore.vamRootPath,
    })
    prefsPath.value = result.path
    prefsContent.value = result.content
    originalContent.value = result.content
    try {
      prefsValue.value = JSON.parse(result.content)
      editorMode.value = 'visual'
    } catch (e) {
      prefsValue.value = null
      editorMode.value = 'json'
      jsonError.value = t('vamPrefs.jsonInvalid') + ': ' + String(e)
    }
    prefsExists.value = result.exists
    lastBackupPath.value = ''
    if (prefsValue.value !== null) {
      validateContent()
    }
  } catch (e) {
    notify.error(t('vamPrefs.loadFailed') + ': ' + String(e))
  } finally {
    isLoading.value = false
  }
}

async function savePrefs() {
  if (!appStore.vamRootPath || !canSave.value) return
  isSaving.value = true
  try {
    const result = await invoke<SaveVamPrefsResult>('save_vam_prefs', {
      vamRoot: appStore.vamRootPath,
      content: stringifyPrefsValue(),
    })
    prefsPath.value = result.path
    prefsContent.value = stringifyPrefsValue()
    originalContent.value = prefsContent.value
    prefsContent.value = originalContent.value
    prefsExists.value = true
    lastBackupPath.value = result.backupPath || ''
    notify.success(t('vamPrefs.saveSuccess'))
  } catch (e) {
    notify.error(t('vamPrefs.saveFailed') + ': ' + String(e))
  } finally {
    isSaving.value = false
  }
}

function formatContent() {
  try {
    prefsContent.value = `${JSON.stringify(JSON.parse(prefsContent.value), null, 2)}\n`
    prefsValue.value = JSON.parse(prefsContent.value)
    jsonError.value = ''
  } catch (e) {
    jsonError.value = t('vamPrefs.jsonInvalid') + ': ' + String(e)
  }
}

async function copyContent() {
  try {
    await navigator.clipboard.writeText(prefsContent.value)
    notify.success(t('common.copySuccess'))
  } catch (e) {
    notify.error(String(e))
  }
}

function validateContent() {
  try {
    prefsValue.value = JSON.parse(prefsContent.value)
    jsonError.value = ''
  } catch (e) {
    jsonError.value = t('vamPrefs.jsonInvalid') + ': ' + String(e)
  }
}

function syncJsonContent() {
  validateContent()
}

function updateVisualValue(value: JsonValue) {
  prefsValue.value = value
  prefsContent.value = `${JSON.stringify(value, null, 2)}\n`
  jsonError.value = ''
}

function showVisualError(message: string) {
  jsonError.value = t('vamPrefs.jsonInvalid') + ': ' + message
}

function stringifyPrefsValue() {
  if (prefsValue.value === null) {
    return prefsContent.value.endsWith('\n') ? prefsContent.value : `${prefsContent.value}\n`
  }
  return `${JSON.stringify(prefsValue.value, null, 2)}\n`
}
</script>

<style scoped>
.vam-prefs-view {
  display: flex;
  flex-direction: column;
  gap: var(--space-5);
  max-width: 980px;
}

.view-header {
  margin-bottom: var(--space-1);
}

.view-title {
  margin-bottom: var(--space-1);
  color: var(--text-primary);
  font-size: var(--text-2xl);
  font-weight: var(--font-bold);
}

.view-subtitle {
  color: var(--text-secondary);
  font-size: var(--text-sm);
}

.prefs-toolbar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--space-4);
}

.path-display {
  display: flex;
  align-items: center;
  min-width: 0;
  flex: 1;
  gap: var(--space-2);
  padding: var(--space-2) var(--space-3);
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-sm);
  background: var(--bg-input);
  color: var(--text-secondary);
  font-size: var(--text-sm);
}

.path-text {
  min-width: 0;
}

.toolbar-actions,
.editor-actions {
  display: flex;
  align-items: center;
  gap: var(--space-2);
}

.btn {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  height: 34px;
  padding: 0 var(--space-4);
  border-radius: var(--radius-sm);
  font-size: var(--text-sm);
  font-weight: var(--font-medium);
  transition:
    background var(--duration-fast) var(--ease),
    border-color var(--duration-fast) var(--ease),
    opacity var(--duration-fast) var(--ease);
}

.btn:disabled {
  cursor: not-allowed;
  opacity: 0.45;
}

.btn-secondary {
  border: 1px solid var(--border-subtle);
  background: var(--bg-elevated);
  color: var(--text-primary);
}

.btn-secondary:hover:not(:disabled) {
  border-color: var(--border-default);
  background: var(--bg-hover);
}

.btn-primary {
  border: 1px solid transparent;
  background: var(--accent-primary);
  color: white;
}

.btn-primary:hover:not(:disabled) {
  background: var(--accent-primary-hover);
}

.status-grid {
  display: grid;
  grid-template-columns: minmax(0, 1fr) 180px;
  gap: var(--space-3);
  margin-top: var(--space-4);
}

.status-item {
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
  min-width: 0;
  padding: var(--space-3);
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-sm);
  background: var(--bg-subtle);
}

.status-label {
  color: var(--text-tertiary);
  font-size: var(--text-xs);
}

.status-value {
  color: var(--text-primary);
  font-size: var(--text-sm);
}

.status-pill {
  display: inline-flex;
  align-items: center;
  width: fit-content;
  height: 24px;
  padding: 0 var(--space-2);
  border-radius: var(--radius-full);
  font-size: var(--text-xs);
  font-weight: var(--font-semibold);
}

.status-pill.ok {
  background: var(--color-success-bg);
  color: var(--color-success);
}

.status-pill.warn {
  background: var(--color-warning-bg);
  color: var(--color-warning);
}

.warning-text {
  margin-top: var(--space-4);
  color: var(--text-secondary);
  font-size: var(--text-sm);
  line-height: var(--leading-relaxed);
}

.editor-actions {
  justify-content: flex-end;
  margin-bottom: var(--space-3);
}

.mode-selector {
  display: flex;
  margin-right: auto;
  padding: 2px;
  border-radius: var(--radius-sm);
  background: var(--bg-base);
}

.mode-btn {
  height: 30px;
  padding: 0 var(--space-3);
  border-radius: var(--radius-xs);
  color: var(--text-tertiary);
  font-size: var(--text-sm);
  font-weight: var(--font-medium);
}

.mode-btn:hover {
  color: var(--text-secondary);
}

.mode-btn.active {
  background: var(--bg-elevated);
  color: var(--text-primary);
}

.visual-editor-shell {
  max-height: 620px;
  overflow: auto;
  padding-right: var(--space-1);
}

.empty-editor {
  display: flex;
  align-items: center;
  justify-content: center;
  min-height: 260px;
  border: 1px dashed var(--border-subtle);
  border-radius: var(--radius-md);
  color: var(--text-tertiary);
  font-size: var(--text-sm);
}

.prefs-editor {
  width: 100%;
  min-height: 520px;
  resize: vertical;
  padding: var(--space-4);
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-md);
  background: rgba(0, 0, 0, 0.16);
  color: var(--text-primary);
  font-family: var(--font-mono);
  font-size: 12px;
  line-height: 1.65;
  outline: none;
}

.prefs-editor:focus {
  border-color: var(--accent-primary);
}

.editor-footer {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--space-3);
  margin-top: var(--space-3);
}

.validation-message {
  font-size: var(--text-xs);
}

.validation-message.ok {
  color: var(--color-success);
}

.validation-message.error {
  color: var(--color-error);
}

.dirty-dot {
  color: var(--color-warning);
  font-size: var(--text-xs);
  font-weight: var(--font-semibold);
}

.backup-path {
  word-break: break-all;
  color: var(--text-secondary);
  font-family: var(--font-mono);
  font-size: var(--text-xs);
}

@media (max-width: 760px) {
  .prefs-toolbar,
  .toolbar-actions {
    align-items: stretch;
    flex-direction: column;
  }

  .status-grid {
    grid-template-columns: 1fr;
  }
}
</style>
