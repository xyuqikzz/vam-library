<template>
  <UiModal v-if="visible" :show-close="false"  :close-disabled="isZipping" @close="handleClose">

        <!-- Header -->
        <div class="modal-header">
          <h3 class="modal-title">
            <Icon class="title-icon" :size="18" name="outline-8" />
            {{ $t('share.title') }}
          </h3>
          <UiButton :disabled="isZipping" @click="handleClose" variant="ghost" icon-only :aria-label="$t('common.close')">
            <Icon  :size="16" name="outline-9" />
          </UiButton>
        </div>

        <!-- Body -->
        <div class="modal-body">
          <p class="share-desc text-secondary text-sm">
            {{ $t('share.desc') }}
          </p>

          <!-- Loading State -->
          <div v-if="loading" class="loading-state">
            <div class="spinner"></div>
            <p class="text-secondary text-sm">{{ $t('share.preparing') }}</p>
          </div>

          <!-- Error Phase State -->
          <div v-else-if="phase === 'error'" class="status-panel error-panel">
            <div class="status-icon-wrapper error">
              <Icon  :size="36" name="outline-10" />
            </div>
            <h4 class="text-sm font-semibold text-primary" style="margin-top: var(--space-2);">
              {{ $t('common.error') }}
            </h4>
            <p class="text-secondary text-xs" style="margin-top: var(--space-1); text-align: center; max-width: 90%;">
              {{ $t('share.failed', { error: progressError || 'Unknown Error' }) }}
            </p>
            <UiButton @click="fetchPreview" variant="secondary">
              {{ $t('download.actions.retry') }}
            </UiButton>
          </div>

          <!-- Done Phase State -->
          <div v-else-if="phase === 'done'" class="status-panel success-panel">
            <div class="status-icon-wrapper success">
              <Icon class="checkmark" :size="36" name="outline-11" />
            </div>
            <h4 class="text-sm font-semibold text-primary" style="margin-top: var(--space-2);">
              {{ $t('share.successTitle') }}
            </h4>
            <p class="text-secondary text-xs" style="margin-top: var(--space-1); text-align: center; max-width: 90%;">
              {{ $t('share.successDesc') }}
            </p>
            <div class="success-actions">
              <UiButton @click="revealZip" variant="secondary">
                {{ $t('share.reveal') }}
              </UiButton>
              <UiButton @click="handleClose" variant="primary" size="lg">
                {{ $t('share.close') }}
              </UiButton>
            </div>
          </div>

          <!-- Zipping Phase State -->
          <div v-else-if="phase === 'zipping'" class="status-panel zipping-panel">
            <div class="progress-info">
              <span class="text-primary text-sm font-semibold">{{ $t('share.zipping') }}</span>
              <span class="text-secondary text-sm font-semibold font-numeric">{{ percentage.toFixed(0) }}%</span>
            </div>
            <div class="progress-track" style="margin-top: var(--space-2);">
              <div class="progress-fill" :style="{ width: `${percentage}%` }"></div>
            </div>
            <div class="zipping-details" style="margin-top: var(--space-3);">
              <p class="current-file text-xs text-primary truncate" :title="currentFile">
                {{ currentFile }}
              </p>
              <p class="file-counts text-xs text-secondary font-numeric" style="margin-top: 2px;">
                {{ processedFiles }} / {{ totalFiles }}
              </p>
            </div>
          </div>

          <!-- Preview & Action State -->
          <div v-else class="preview-layout animate-fadeIn">

            <!-- Summary Stats Card -->
            <div class="summary-stats-card">
              <div class="stat-column">
                <span class="stat-val font-numeric">{{ totalFiles }}</span>
                <span class="stat-lbl text-xs text-tertiary">{{ $t('share.packageCount', { count: totalFiles }).split(':')[0] }}</span>
              </div>
              <div class="stat-divider"></div>
              <div class="stat-column">
                <span class="stat-val font-numeric">{{ formatSize(totalSizeBytes) }}</span>
                <span class="stat-lbl text-xs text-tertiary">{{ $t('share.totalSize', { size: '' }).split(':')[0] }}</span>
              </div>
            </div>

            <!-- Exclusion List Panel -->
            <div class="exclusion-list-card">
              <div v-if="!excludeListPath" class="exclusion-import-trigger" @click="handleImportExcludeList">
                <Icon class="import-icon" :size="14" name="outline-12" />
                <span class="text-xs">{{ $t('share.importExcludeBtn') }}</span>
              </div>
              <div v-else class="exclusion-active-state">
                <div class="exclusion-info-wrapper">
                  <Icon class="active-icon" :size="14" name="outline-13" />
                  <div class="exclusion-text">
                    <p class="exclusion-title text-xs truncate" :title="excludeListName">
                      {{ $t('share.importedExcludeList', { name: excludeListName }) }}
                    </p>
                    <p v-if="excludedCount > 0" class="exclusion-subtitle text-xs text-secondary">
                      {{ $t('share.excludedInfo', { count: excludedCount }) }}
                    </p>
                  </div>
                </div>
                <UiButton class="text-xs" @click="handleClearExcludeList" variant="secondary">
                  {{ $t('share.clearExcludeList') }}
                </UiButton>
              </div>
            </div>

            <!-- Content Lists -->
            <div class="list-sections-container">
              <!-- Selected Target -->
              <div class="selected-target-wrapper">
                <h5 class="section-title text-xs text-tertiary">{{ $t('share.selectedPackage') }}</h5>
                <div class="selected-package-row text-sm text-primary">
                  <span class="type-dot type-scene"></span>
                  <span class="package-name truncate" :title="packageId">{{ packageId }}</span>
                </div>
              </div>

              <!-- Dependencies To Pack -->
              <div class="dependency-list-wrapper">
                <h5 class="section-title text-xs text-tertiary">
                  {{ $t('share.dependencies') }}
                  <span class="count-badge font-numeric" v-if="dependencies.length > 0">{{ dependencies.length }}</span>
                </h5>
                <div v-if="dependencies.length === 0" class="empty-list-hint text-xs text-tertiary">
                  {{ $t('share.noDependencies') }}
                </div>
                <div v-else class="list-scrollbar list-content">
                  <div v-for="dep in dependencies" :key="dep.id" class="list-item-row text-sm">
                    <span class="dep-name truncate text-primary" :title="dep.id">{{ dep.id }}</span>
                    <span class="dep-size font-numeric text-secondary text-xs">{{ formatSize(dep.size_bytes) }}</span>
                  </div>
                </div>
              </div>

              <!-- Missing Dependencies (Warning list) -->
              <div v-if="missingDependencies.length > 0" class="dependency-list-wrapper missing-wrapper">
                <h5 class="section-title text-xs text-error">
                  {{ $t('share.missingDependencies') }}
                  <span class="count-badge missing font-numeric">{{ missingDependencies.length }}</span>
                </h5>
                <div class="list-scrollbar list-content missing-list">
                  <div v-for="missing in missingDependencies" :key="missing" class="list-item-row text-sm missing-row">
                    <span class="dep-name truncate text-error" :title="missing">{{ missing }}</span>
                    <span class="dep-status text-error text-xs">{{ $t('share.missing') }}</span>
                  </div>
                </div>
              </div>
            </div>

            <!-- Warning Banner -->
            <div v-if="missingDependencies.length > 0" class="warning-banner text-xs">
              <Icon class="warn-icon" :size="14" name="outline-14" />
              <span>{{ $t('share.missingCount', { count: missingDependencies.length }) }}</span>
            </div>

            <!-- Confirm Button -->
            <div class="action-footer">
              <UiButton @click="startSharing" variant="primary" size="lg">
                {{ $t('share.confirmBtn') }}
              </UiButton>
            </div>

          </div>
        </div>

      </UiModal>
</template>

<script setup lang="ts">
import UiModal from '@/components/ui/UiModal.vue'
import UiButton from '@/components/ui/UiButton.vue'
import Icon from '@/components/ui/Icon.vue'
import { formatSize } from '@/utils/bytes'
import { ref, watch, onUnmounted } from 'vue'
import { useI18n } from 'vue-i18n'
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'

const props = defineProps<{
  visible: boolean
  packageId: string
}>()

const emit = defineEmits<{
  close: []
}>()

const { t } = useI18n()

// ── State ──────────────────────────────────────────────────
const loading = ref(false)
const dependencies = ref<any[]>([])
const missingDependencies = ref<string[]>([])
const totalSizeBytes = ref(0)
const totalFiles = ref(0)

// Exclusion list support
const excludeListPath = ref<string | null>(null)
const excludeListName = ref('')
const excludedCount = ref(0)

// Progress Phase States
// phase: 'preview' | 'zipping' | 'done' | 'error'
const phase = ref<'preview' | 'zipping' | 'done' | 'error'>('preview')
const isZipping = ref(false)
const currentFile = ref('')
const processedFiles = ref(0)
const percentage = ref(0)
const progressError = ref('')
const exportedZipPath = ref('')

let unlistenProgress: (() => void) | null = null

// Watch visibility and packageId change to fetch data
watch(
  () => [props.visible, props.packageId],
  async ([vis, pkgId]) => {
    if (vis && pkgId) {
      phase.value = 'preview'
      isZipping.value = false
      currentFile.value = ''
      processedFiles.value = 0
      percentage.value = 0
      progressError.value = ''
      exportedZipPath.value = ''
      excludeListPath.value = null
      excludeListName.value = ''
      excludedCount.value = 0
      await fetchPreview()
    } else {
      cleanupListener()
    }
  },
  { immediate: true }
)

onUnmounted(() => {
  cleanupListener()
})

function cleanupListener() {
  if (unlistenProgress) {
    unlistenProgress()
    unlistenProgress = null
  }
}

// ── Fetch Dependency Share Preview ───────────────────────────
async function fetchPreview() {
  if (!props.packageId) return
  loading.value = true
  phase.value = 'preview'
  try {
    const preview = await invoke<any>('get_share_preview', {
      packageId: props.packageId,
      excludeListPath: excludeListPath.value
    })
    dependencies.value = preview.dependencies || []
    missingDependencies.value = preview.missing_dependencies || []
    totalSizeBytes.value = preview.total_size_bytes || 0
    totalFiles.value = preview.total_files || 0
    excludedCount.value = preview.excluded_count || 0
  } catch (err: any) {
    console.error('Failed to get share preview:', err)
    phase.value = 'error'
    progressError.value = err.toString()
  } finally {
    loading.value = false
  }
}

// ── Native Dialog & Start Sharing ────────────────────────────
async function startSharing() {
  try {
    // 1. Choose Save Location using Native Save Dialog
    const { save } = await import('@tauri-apps/plugin-dialog')

    // Suggested filename from package id
    const cleanPackageId = props.packageId.replace(/[\/\\:\*\?"<>\|]/g, '_')
    const defaultFilename = `${cleanPackageId}_shared.zip`

    const selectedPath = await save({
      title: t('share.chooseDest'),
      filters: [{
        name: 'ZIP Archive',
        extensions: ['zip']
      }],
      defaultPath: defaultFilename
    })

    if (!selectedPath) {
      return // User cancelled
    }

    exportedZipPath.value = selectedPath

    // 2. Set up Tauri Event Listener for share-progress
    cleanupListener()
    unlistenProgress = await listen<any>('share-progress', (event) => {
      const payload = event.payload
      if (!payload) return

      if (payload.phase === 'zipping') {
        phase.value = 'zipping'
        isZipping.value = true
        currentFile.value = payload.current_file || ''
        processedFiles.value = payload.processed_files || 0
        totalFiles.value = payload.total_files || 0
        percentage.value = payload.percentage || 0
      } else if (payload.phase === 'done') {
        phase.value = 'done'
        isZipping.value = false
        percentage.value = 100
        cleanupListener()
      } else if (payload.phase === 'error') {
        phase.value = 'error'
        isZipping.value = false
        progressError.value = payload.current_file || 'Packaging failed'
        cleanupListener()
      }
    })

    // 3. Trigger Tauri background packaging command
    phase.value = 'zipping'
    isZipping.value = true
    await invoke('export_share_zip', {
      packageId: props.packageId,
      targetZipPath: selectedPath,
      excludeListPath: excludeListPath.value
    })

  } catch (err: any) {
    console.error('Error starting sharing:', err)
    phase.value = 'error'
    progressError.value = err.toString()
    isZipping.value = false
    cleanupListener()
  }
}

async function handleImportExcludeList() {
  try {
    const { open } = await import('@tauri-apps/plugin-dialog')
    const selected = await open({
      title: t('share.importExcludeBtn'),
      filters: [{
        name: 'Text File',
        extensions: ['txt']
      }]
    })
    if (selected && typeof selected === 'string') {
      excludeListPath.value = selected
      const parts = selected.split(/[\\/]/)
      excludeListName.value = parts.pop() || selected
      await fetchPreview()
    }
  } catch (err) {
    console.error('Failed to import exclusion list:', err)
  }
}

function handleClearExcludeList() {
  excludeListPath.value = null
  excludeListName.value = ''
  excludedCount.value = 0
  fetchPreview()
}

// ── Reveal Zip in File Explorer ──────────────────────────────
async function revealZip() {
  if (!exportedZipPath.value) return
  try {
    const { revealItemInDir } = await import('@tauri-apps/plugin-opener')
    await revealItemInDir(exportedZipPath.value)
  } catch (e) {
    console.error('Failed to open file explorer:', e)
  }
}

function handleClose() {
  if (isZipping.value) return
  emit('close')
}

// ── Helpers ──────────────────────────────────────────────────
</script>

<style scoped>
/* Header */
.modal-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: var(--space-4) var(--space-5);
  border-bottom: 1px solid var(--border-subtle);
  flex-shrink: 0;
}

.modal-title {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  font-size: var(--text-md);
  font-weight: var(--font-semibold);
  color: var(--text-primary);
}

.title-icon {
  color: var(--accent-primary);
}

/* Body */
.modal-body {
  padding: var(--space-5);
  overflow-y: auto;
  flex: 1;
  display: flex;
  flex-direction: column;
}

.share-desc {
  margin-bottom: var(--space-4);
  line-height: 1.5;
}

/* Loading & Status Panels */
.loading-state {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  padding: var(--space-8) 0;
  gap: var(--space-4);
}

.spinner {
  width: 32px;
  height: 32px;
  border: none;
  border-top-color: var(--accent-primary);
  border-radius: var(--radius-xl)%;
  animation: spin 1s linear infinite;
}

.status-panel {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  padding: var(--space-6) var(--space-4);
  background: var(--bg-subtle);
  border: none;
  border-radius: var(--radius-md);
}

.status-icon-wrapper {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 60px;
  height: 60px;
  border-radius: var(--radius-xl)%;
}

.status-icon-wrapper.success {
  background: var(--color-success-bg);
  color: var(--color-success);
}

.status-icon-wrapper.error {
  background: var(--color-error-bg);
  color: var(--color-error);
}

.success-actions {
  display: flex;
  gap: var(--space-3);
  margin-top: var(--space-5);
  width: 100%;
  justify-content: center;
}

/* Zipping styling */
.zipping-panel {
  align-items: stretch;
}

.progress-info {
  display: flex;
  justify-content: space-between;
}

.progress-track {
  height: 8px;
  background: var(--bg-subtle);
  border-radius: var(--radius-full);
  overflow: hidden;
}

.progress-fill {
  height: 100%;
  background: var(--accent-gradient);
  border-radius: var(--radius-full);
  transition: width 0.2s cubic-bezier(0.4, 0, 0.2, 1);
}

.zipping-details {
  display: flex;
  justify-content: space-between;
  align-items: center;
}

.current-file {
  max-width: 70%;
}

/* Preview Layout */
.preview-layout {
  display: flex;
  flex-direction: column;
  gap: var(--space-4);
  flex: 1;
}

.summary-stats-card {
  display: flex;
  background: var(--bg-subtle);
  border: none;
  border-radius: var(--radius-md);
  padding: var(--space-3) 0;
}

.stat-column {
  flex: 1;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 2px;
}

.stat-divider {
  width: 1px;
  background: var(--border-subtle);
}

.stat-val {
  font-size: var(--text-lg);
  font-weight: var(--font-bold);
  color: var(--text-primary);
}

.stat-lbl {
  text-transform: uppercase;
  letter-spacing: 0.05em;
}

/* Scrollable Lists */
.list-sections-container {
  display: flex;
  flex-direction: column;
  gap: var(--space-3);
  max-height: 260px;
  overflow: hidden;
}

.selected-target-wrapper {
  background: var(--bg-subtle);
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-sm);
  padding: var(--space-2) var(--space-3);
}

.section-title {
  font-weight: var(--font-semibold);
  text-transform: uppercase;
  letter-spacing: 0.05em;
  margin-bottom: var(--space-2);
  display: flex;
  align-items: center;
  justify-content: space-between;
}

.count-badge {
  background: var(--border-default);
  padding: 1px 6px;
  border-radius: var(--radius-full);
  font-size: var(--text-xs);
  color: var(--text-secondary);
}

.count-badge.missing {
  background: var(--color-error-bg);
  color: var(--color-error);
}

.selected-package-row {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  font-weight: var(--font-medium);
}

.type-dot {
  width: 8px;
  height: 8px;
  border-radius: var(--radius-xl)%;
}

.type-dot.type-scene {
  background: var(--color-scene);
}

.dependency-list-wrapper {
  flex: 1;
  display: flex;
  flex-direction: column;
  min-height: 90px;
}

.empty-list-hint {
  padding: var(--space-3);
  text-align: center;
  border: none;
  border-radius: var(--radius-sm);
  background: var(--bg-subtle);
}

.list-content {
  flex: 1;
  overflow-y: auto;
  border: none;
  border-radius: var(--radius-sm);
  background: var(--border-subtle);
  padding: var(--space-1) 0;
}

.list-item-row {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: var(--space-2) var(--space-3);
  border-bottom: 1px solid var(--bg-subtle);
}

.list-item-row:last-child {
  border-bottom: none;
}

.dep-name {
  max-width: 75%;
}

.missing-list {
  border-color: var(--color-error-bg);
  background: var(--color-error-bg);
}

.missing-row {
  border-bottom-color: var(--color-error-bg);
}

/* Warning Banner */
.warning-banner {
  display: flex;
  align-items: flex-start;
  gap: var(--space-2);
  padding: var(--space-3);
  background: var(--color-warning-bg);
  border: none;
  border-radius: var(--radius-sm);
  color: var(--color-warning);
  line-height: 1.4;
}

.warn-icon {
  flex-shrink: 0;
  margin-top: 1px;
}

/* Actions footer */
.action-footer {
  display: flex;
  margin-top: var(--space-2);
}

/* Scrollbar styling */
.list-scrollbar::-webkit-scrollbar {
  width: 6px;
}

.list-scrollbar::-webkit-scrollbar-track {
  background: transparent;
}

.list-scrollbar::-webkit-scrollbar-thumb {
  background: var(--border-default);
  border-radius: var(--radius-full);
}

.list-scrollbar::-webkit-scrollbar-thumb:hover {
  background: var(--border-strong);
}

/* Animations */
@keyframes spin {
  to { transform: rotate(360deg); }
}

.checkmark {
  animation: scaleIn 0.3s cubic-bezier(0.34, 1.56, 0.64, 1) forwards;
}

@keyframes scaleIn {
  from { transform: scale(0.8); opacity: 0; }
  to { transform: scale(1); opacity: 1; }
}

.fade-enter-active, .fade-leave-active {
  transition: opacity 0.25s var(--ease);
}
.fade-enter-from, .fade-leave-to {
  opacity: 0;
}

@keyframes scaleUp {
  from { transform: scale(0.95); opacity: 0; }
  to { transform: scale(1); opacity: 1; }
}

.font-numeric {
  font-variant-numeric: tabular-nums;
}

/* Exclusion List Panel styling */
.exclusion-list-card {
  background: var(--bg-subtle);
  border: none;
  border-radius: var(--radius-md);
  padding: var(--space-3);
  transition: all var(--transition-normal);
}

.exclusion-list-card:hover {
  background: var(--bg-subtle);
  border-color: var(--border-strong);
}

.exclusion-import-trigger {
  display: flex;
  align-items: center;
  justify-content: center;
  gap: var(--space-2);
  color: var(--text-secondary);
  cursor: pointer;
  height: 38px;
  user-select: none;
  transition: color var(--transition-fast);
}

.exclusion-import-trigger:hover {
  color: var(--text-primary);
}

.import-icon {
  color: var(--accent-primary);
}

.exclusion-active-state {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--space-3);
}

.exclusion-info-wrapper {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  min-width: 0;
}

.active-icon {
  color: var(--color-success);
  flex-shrink: 0;
}

.exclusion-text {
  min-width: 0;
  display: flex;
  flex-direction: column;
  gap: 1px;
}

.exclusion-title {
  color: var(--text-primary);
  font-weight: var(--font-medium);
}

.exclusion-subtitle {
  color: var(--text-secondary);
  font-size: var(--text-xs);
}
</style>
