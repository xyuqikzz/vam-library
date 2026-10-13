<template>
  <UiModal
    v-if="visible"
    large
    :title="$t('share.title')"
    :description="$t('share.desc')"
    :close-disabled="isZipping"
    @close="handleClose"
  >
    <div v-if="loading" class="share-state" role="status">
      <span class="spinner-ring" aria-hidden="true" />
      <p>{{ $t('share.preparing') }}</p>
    </div>

    <div v-else-if="phase === 'error'" class="share-state">
      <span class="state-icon error"><Icon name="warning" :size="24" /></span>
      <h4>{{ $t('share.failedTitle') }}</h4>
      <p class="state-detail">{{ progressError || $t('share.unknownError') }}</p>
      <UiButton @click="fetchPreview">{{ $t('download.actions.retry') }}</UiButton>
    </div>

    <div v-else-if="phase === 'done'" class="share-state">
      <span class="state-icon success"><Icon name="check" :size="24" /></span>
      <h4>{{ $t('share.successTitle') }}</h4>
      <p>{{ $t('share.successDesc') }}</p>
      <div class="state-actions">
        <UiButton icon="folder" @click="revealZip">{{ $t('share.reveal') }}</UiButton>
        <UiButton variant="primary" @click="handleClose">{{ $t('share.close') }}</UiButton>
      </div>
    </div>

    <div v-else-if="phase === 'zipping'" class="share-progress" role="status">
      <div class="progress-head">
        <span>{{ $t('share.zipping') }}</span>
        <span class="progress-percent">{{ percentage.toFixed(0) }}%</span>
      </div>
      <div class="progress-track">
        <div class="progress-fill" :style="{ width: `${percentage}%` }" />
      </div>
      <p class="progress-file" :title="currentFile">{{ currentFile }}</p>
      <p class="progress-count">{{ processedFiles }} / {{ totalFiles }}</p>
    </div>

    <div v-else class="share-preview">
      <p class="share-total">
        {{ $t('share.total', { count: totalFiles, size: formatSize(totalSizeBytes) }) }}
      </p>

      <section class="share-section">
        <h4>{{ $t('share.packList') }}</h4>
        <ul class="share-list">
          <li>
            <span class="item-name" :title="packageId">{{ packageId }}</span>
            <span class="item-tag">{{ $t('share.selectedPackage') }}</span>
          </li>
          <li v-for="dep in dependencies" :key="dep.id">
            <span class="item-name" :title="dep.id">{{ dep.id }}</span>
            <span class="item-size">{{ formatSize(dep.size_bytes) }}</span>
          </li>
        </ul>
        <p v-if="dependencies.length === 0" class="share-hint">{{ $t('share.noDependencies') }}</p>
      </section>

      <section v-if="missingDependencies.length > 0" class="share-section">
        <h4 class="missing-title">
          <Icon name="warning" :size="14" />
          {{ $t('share.missingCount', { count: missingDependencies.length }) }}
        </h4>
        <ul class="share-list missing">
          <li v-for="missing in missingDependencies" :key="missing">
            <span class="item-name" :title="missing">{{ missing }}</span>
            <span class="item-size">{{ $t('share.missing') }}</span>
          </li>
        </ul>
      </section>

      <section class="share-exclude">
        <div class="exclude-text">
          <strong>{{ excludeListPath ? $t('share.importedExcludeList', { name: excludeListName }) : $t('share.excludeTitle') }}</strong>
          <span>
            {{ excludeListPath ? (excludedCount > 0 ? $t('share.excludedInfo', { count: excludedCount }) : $t('share.excludedNone')) : $t('share.excludeHint') }}
          </span>
        </div>
        <UiButton v-if="!excludeListPath" size="sm" icon="upload" @click="handleImportExcludeList">
          {{ $t('share.importExcludeBtn') }}
        </UiButton>
        <UiButton v-else size="sm" variant="ghost" @click="handleClearExcludeList">
          {{ $t('share.clearExcludeList') }}
        </UiButton>
      </section>
    </div>

    <template v-if="!loading && phase !== 'error' && phase !== 'done' && phase !== 'zipping'" #footer>
      <UiButton @click="handleClose">{{ $t('common.cancel') }}</UiButton>
      <UiButton variant="primary" @click="startSharing">{{ $t('share.confirmBtn') }}</UiButton>
    </template>
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
.share-state {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: var(--space-3);
  padding: var(--space-8) var(--space-4);
  text-align: center;
  color: var(--text-secondary);
  font-size: var(--text-base);
}

.share-state h4 {
  color: var(--text-primary);
  font-size: var(--text-md);
  font-weight: var(--font-heavy);
}

.state-detail {
  max-width: 100%;
  color: var(--text-tertiary);
  font-size: var(--text-sm);
  overflow-wrap: anywhere;
  user-select: text;
}

.state-icon {
  display: grid;
  place-items: center;
  width: 52px;
  height: 52px;
  border-radius: var(--radius-full);
}

.state-icon.error {
  background: var(--color-error-bg);
  color: var(--color-error);
}

.state-icon.success {
  background: var(--color-success-bg);
  color: var(--color-success);
}

.state-actions {
  display: flex;
  gap: var(--space-2);
  margin-top: var(--space-2);
}

.share-progress {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
  padding: var(--space-6) 0;
}

.progress-head {
  display: flex;
  justify-content: space-between;
  font-size: var(--text-base);
  font-weight: var(--font-semibold);
}

.progress-percent,
.progress-count {
  color: var(--text-secondary);
  font-variant-numeric: tabular-nums;
}

.progress-track {
  height: 6px;
  border-radius: var(--radius-full);
  background: var(--bg-active);
  overflow: hidden;
}

.progress-fill {
  height: 100%;
  border-radius: var(--radius-full);
  background: var(--accent-primary);
  transition: width 0.2s ease;
}

.progress-file {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  color: var(--text-secondary);
  font-size: var(--text-xs);
}

.progress-count {
  font-size: var(--text-xs);
}

.share-preview {
  display: flex;
  flex-direction: column;
  gap: var(--space-4);
}

.share-total {
  color: var(--text-primary);
  font-size: var(--text-base);
  font-weight: var(--font-semibold);
}

.share-section {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
}

.share-section h4 {
  color: var(--text-secondary);
  font-size: var(--text-sm);
  font-weight: var(--font-bold);
}

.missing-title {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  color: var(--color-warning) !important;
}

.share-list {
  max-height: 240px;
  overflow: auto;
  border-radius: var(--radius-md);
  background: var(--bg-input);
}

.share-list li {
  display: flex;
  align-items: center;
  gap: var(--space-3);
  min-height: 40px;
  padding: var(--space-1) var(--space-4);
  font-size: var(--text-sm);
}

.share-list li + li {
  border-top: 1px solid var(--border-subtle);
}

.item-name {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.item-size {
  flex-shrink: 0;
  color: var(--text-tertiary);
  font-size: var(--text-xs);
  font-variant-numeric: tabular-nums;
}

.item-tag {
  flex-shrink: 0;
  display: inline-flex;
  align-items: center;
  min-height: 20px;
  padding: 0 var(--space-2);
  border-radius: var(--radius-full);
  background: var(--accent-subtle);
  color: var(--accent-primary);
  font-size: var(--text-xs);
  font-weight: var(--font-bold);
}

.share-list.missing {
  background: var(--color-warning-bg);
}

.share-list.missing .item-size {
  color: var(--color-warning);
}

.share-hint {
  color: var(--text-tertiary);
  font-size: var(--text-sm);
}

.share-exclude {
  display: flex;
  align-items: center;
  gap: var(--space-4);
  padding: var(--space-3) var(--space-4);
  border-radius: var(--radius-md);
  background: var(--bg-input);
}

.exclude-text {
  flex: 1;
  min-width: 0;
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.exclude-text strong {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  font-size: var(--text-sm);
}

.exclude-text span {
  color: var(--text-tertiary);
  font-size: var(--text-xs);
}
</style>
