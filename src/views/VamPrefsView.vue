<template>
  <div class="vam-prefs-view animate-fadeIn b2-page">
    <PageHeader :title="$t('vamPrefs.title')" :subtitle="$t('vamPrefs.subtitle')">
      <template #actions>
        <UiButton type="button" @click="selectVamRoot" variant="secondary">
          {{ $t('vamPrefs.selectRoot') }}
        </UiButton>
        <UiButton
          type="button"
          :disabled="!canRead || isLoading"
          @click="loadPrefs"
          variant="secondary"
        >
          {{ isLoading ? $t('common.loading') : $t('common.refresh') }}
        </UiButton>
        <UiButton type="button" :disabled="!canSave" @click="savePrefs" variant="primary" size="lg">
          {{ isSaving ? $t('common.loading') : $t('vamPrefs.saveWithBackup') }}
        </UiButton>
      </template>
    </PageHeader>
    <div class="b2-page-body">
      <GlassPanel :title="$t('vamPrefs.currentFile')">
        <div class="prefs-toolbar">
          <div class="path-display">
            <Icon :size="14" name="outline-53" />
            <span class="path-text truncate">{{ prefsPath || $t('vamPrefs.pathPending') }}</span>
          </div>
        </div>

        <div class="status-grid">
          <div class="status-item">
            <span class="status-label">{{ $t('vamPrefs.vamRoot') }}</span>
            <span class="status-value truncate">
              {{ appStore.vamRootPath || $t('settings.notConfigured') }}
            </span>
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
            <UiChip
              :class="['', { active: editorMode === 'visual' }]"
              type="button"
              @click="editorMode = 'visual'"
              :selected="editorMode === 'visual'"
            >
              {{ $t('vamPrefs.visualMode') }}
            </UiChip>
            <UiChip
              :class="['', { active: editorMode === 'json' }]"
              type="button"
              @click="editorMode = 'json'"
              :selected="editorMode === 'json'"
            >
              {{ $t('vamPrefs.jsonMode') }}
            </UiChip>
          </div>
          <UiButton
            v-if="editorMode === 'json'"
            type="button"
            :disabled="!prefsContent"
            @click="formatContent"
            variant="secondary"
          >
            {{ $t('vamPrefs.formatJson') }}
          </UiButton>
          <UiButton
            type="button"
            :disabled="!prefsContent"
            @click="copyContent"
            variant="secondary"
          >
            {{ $t('common.copy') }}
          </UiButton>
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

        <UiInput
          v-else
          v-model="prefsContent"
          class="prefs-editor"
          spellcheck="false"
          :placeholder="$t('vamPrefs.editorPlaceholder')"
          @input="syncJsonContent"
          multiline
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
  </div>
</template>

<script setup lang="ts">
import UiChip from '@/components/ui/UiChip.vue'
import PageHeader from '@/components/layout/PageHeader.vue'
import UiInput from '@/components/ui/UiInput.vue'
import UiButton from '@/components/ui/UiButton.vue'
import Icon from '@/components/ui/Icon.vue'
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
  } catch (error) {
    notify.error(String(error))
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
  border: none;
  border-radius: var(--radius-sm);
  background: var(--bg-input);
  color: var(--text-secondary);
  font-size: var(--text-sm);
}

.path-text {
  min-width: 0;
}

.editor-actions {
  display: flex;
  align-items: center;
  gap: var(--space-2);
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
  border: none;
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
  border: none;
  border-radius: var(--radius-md);
  color: var(--text-tertiary);
  font-size: var(--text-sm);
}

.prefs-editor {
  width: 100%;
  min-height: 520px;
  resize: vertical;
  font-family: var(--font-mono);
  line-height: 1.65;
  outline: none;
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
  .prefs-toolbar {
    align-items: stretch;
    flex-direction: column;
  }

  .status-grid {
    grid-template-columns: 1fr;
  }
}

.b2-page {
  display: flex;
  flex-direction: column;
  height: 100%;
  min-height: 0;
  max-width: none;
  margin: 0;
  padding: 0;
  gap: 0;
  overflow: hidden;
}
.b2-page-body {
  flex: 1;
  min-height: 0;
  min-width: 0;
  overflow: auto;
  padding: 20px 24px;
  display: flex;
  flex-direction: column;
  gap: 20px;
}
.b2-page :deep(.page-header) {
  width: 100%;
}
@media (max-width: 800px) {
  .b2-page-body {
    padding: 12px;
  }
}
</style>
