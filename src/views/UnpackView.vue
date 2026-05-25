<template>
  <div class="unpack-view">
    <!-- Header -->
    <div class="view-header">
      <h1 class="view-title">{{ $t('unpack.title') }}</h1>
      <p class="view-subtitle">{{ $t('unpack.subtitle') }}</p>
    </div>

    <!-- main content container -->
    <div class="glass-panel main-container">
      
      <!-- Stage 1: Drop Zone / File Picker (Idle State) -->
      <div 
        v-if="stage === 'idle'"
        :class="['drop-zone', { 'is-dragover': isDragOver }]"
        @dragover.prevent="onDragOver"
        @dragleave="onDragLeave"
        @drop.prevent="onDrop"
        @click="selectFile"
      >
        <div class="drop-icon-wrapper">
          <svg width="48" height="48" viewBox="0 0 24 24" fill="none" class="drop-icon">
            <path d="M12 2L3 7v10l9 5 9-5V7l-9-5z" stroke="currentColor" stroke-width="1.5" stroke-linejoin="round" />
            <path d="M12 22V12" stroke="currentColor" stroke-width="1.5" />
            <path d="M21 7l-9 5L3 7" stroke="currentColor" stroke-width="1.5" />
            <path d="M12 12V4" stroke="currentColor" stroke-width="2" stroke-linecap="round" />
            <path d="M8 8l4-4 4 4" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" />
          </svg>
        </div>
        <h3 class="drop-title">
          {{ $t('unpack.dropZoneTitle') }} <span class="click-link">{{ $t('unpack.dropZoneClick') }}</span>
        </h3>
        <p class="drop-hint">{{ $t('unpack.dropZoneHint') }}</p>
      </div>

      <!-- Stage 2: Deep Analyzing -->
      <div v-else-if="stage === 'analyzing'" class="analyzing-zone">
        <div class="spinner-wrapper">
          <div class="spinner"></div>
        </div>
        <h3 class="status-title">{{ $t('unpack.analyzing') }}</h3>
        <p class="status-filename truncate">{{ selectedFilePath }}</p>
      </div>

      <!-- Stage 3: Archive Analysis & Action Recommendation -->
      <div v-else-if="stage === 'reviewed' && analysis" class="review-zone">
        <div class="review-header-info">
          <div class="archive-main-info">
            <h3 class="archive-name truncate" :title="analysis.fileName">{{ analysis.fileName }}</h3>
            <div class="archive-badges">
              <span class="badge size-badge">{{ formatBytes(analysis.sizeBytes) }}</span>
              <span class="badge type-badge" :class="analysis.archiveType">
                {{ $t(`unpack.type_${analysis.archiveType}`) }}
              </span>
            </div>
          </div>
        </div>

        <div class="review-grid">
          <!-- Left Panel: Detected Components -->
          <div class="detected-panel">
            <h4 class="section-title">检测到的资源结构</h4>
            
            <div class="stat-list">
              <div class="stat-item">
                <span class="stat-label">{{ $t('unpack.fileCount') }}</span>
                <span class="stat-value">{{ analysis.fileCount }} 个文件</span>
              </div>

              <div class="stat-item">
                <span class="stat-label">{{ $t('unpack.containsMeta') }}</span>
                <span :class="['stat-indicator', { 'has': analysis.containsMetaJson }]">
                  {{ analysis.containsMetaJson ? '✅ 包含' : '❌ 无' }}
                </span>
              </div>

              <div class="stat-item" v-if="analysis.containsVarFiles">
                <span class="stat-label">{{ $t('unpack.containsVars') }}</span>
                <span class="stat-indicator has">✅ {{ analysis.varFiles.length }} 个包</span>
              </div>

              <div class="stat-item">
                <span class="stat-label">{{ $t('unpack.hasImages') }}</span>
                <span :class="['stat-indicator', { 'has': analysis.hasImageFiles }]">
                  {{ analysis.hasImageFiles ? '✅ 包含' : '❌ 无' }}
                </span>
              </div>

              <div class="stat-item" v-if="analysis.suspectedVarId">
                <span class="stat-label">{{ $t('unpack.suspectedId') }}</span>
                <span class="stat-value code-font truncate" :title="analysis.suspectedVarId">{{ analysis.suspectedVarId }}</span>
              </div>

              <div class="stat-item" v-if="analysis.basePathInArchive">
                <span class="stat-label">{{ $t('unpack.detectedWrapper') }}</span>
                <span class="stat-value code-font truncate" :title="analysis.basePathInArchive">
                  {{ analysis.basePathInArchive }}
                </span>
              </div>
            </div>

            <!-- Sample entries -->
            <div class="sample-panel">
              <h5 class="sample-title">文件结构预览：</h5>
              <div class="sample-list scrollbar-subtle">
                <div v-for="(file, idx) in analysis.sampleFiles" :key="idx" class="sample-file truncate" :title="file">
                  📄 {{ file }}
                </div>
                <div v-if="analysis.fileCount > analysis.sampleFiles.length" class="sample-more">
                  ...还有 {{ analysis.fileCount - analysis.sampleFiles.length }} 个文件
                </div>
              </div>
            </div>
          </div>

          <!-- Right Panel: Recommendation & Action -->
          <div class="action-panel">
            <div class="recommendation-card">
              <div class="rec-icon">
                <svg width="24" height="24" viewBox="0 0 24 24" fill="none">
                  <path d="M9 12l2 2 4-4m5.618-4.016A11.955 11.955 0 0112 2.944a11.955 11.955 0 01-8.618 3.04A12.02 12.02 0 003 9c0 5.591 3.824 10.29 9 11.622 5.176-1.332 9-6.03 9-11.622 0-1.042-.133-2.052-.382-3.016z" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"/>
                </svg>
              </div>
              <div class="rec-details">
                <h4 class="rec-title">{{ $t('unpack.recommendation') }}</h4>
                <p class="rec-action-desc">{{ $t(`unpack.action_${analysis.recommendedAction}`) }}</p>
              </div>
            </div>

            <div class="destination-preview">
              <span class="dest-label">{{ $t('unpack.destination') }}</span>
              <p class="dest-path code-font" :title="destinationDir">{{ destinationDir }}</p>
            </div>

            <div class="action-buttons">
              <button class="btn btn-secondary" @click="cancelUnpack">{{ $t('common.cancel') }}</button>
              <button class="btn btn-primary" @click="startUnpack">
                {{ $t('unpack.executeBtn') }}
              </button>
            </div>
          </div>
        </div>
      </div>

      <!-- Stage 4: Processing Unpack -->
      <div v-else-if="stage === 'processing'" class="processing-zone">
        <div class="processing-graphics">
          <div class="pulse-ring"></div>
          <svg width="40" height="40" viewBox="0 0 24 24" fill="none" class="extracting-icon">
            <path d="M4 16v1a3 3 0 003 3h10a3 3 0 003-3v-1M12 4v12m0 0l4-4m-4 4l-4-4" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" />
          </svg>
        </div>
        <h3 class="status-title">{{ $t('unpack.processing') }}</h3>
        
        <div class="progress-container">
          <div class="progress-meta">
            <span class="progress-percent">{{ unpackProgress ? Math.round(unpackProgress.percentage) : 0 }}%</span>
            <span class="progress-counts" v-if="unpackProgress && unpackProgress.total > 0">
              {{ unpackProgress.processed }} / {{ unpackProgress.total }}
            </span>
          </div>
          <div class="progress-track">
            <div class="progress-fill" :style="{ width: `${unpackProgress ? unpackProgress.percentage : 0}%` }" />
          </div>
          <p class="progress-file truncate" v-if="unpackProgress?.currentFile" :title="unpackProgress.currentFile">
            {{ unpackProgress.currentFile }}
          </p>
          <p class="status-filename truncate" v-else>{{ selectedFileName }}</p>
        </div>
      </div>

      <!-- Stage 5: Success Results -->
      <div v-else-if="stage === 'success' && unpackResult" class="success-zone">
        <div class="success-header">
          <div class="success-badge-anim">
            <svg width="48" height="48" viewBox="0 0 24 24" fill="none" class="check-icon">
              <path d="M5 13l4 4L19 7" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round"/>
            </svg>
          </div>
          <h3 class="success-title">{{ $t('unpack.successTitle') }}</h3>
          <p class="success-desc">{{ unpackResult.message }}</p>
        </div>

        <div class="success-details-card">
          <div class="dest-row">
            <div>
              <span class="dest-label">{{ $t('unpack.destination') }}</span>
              <p class="dest-path code-font" :title="unpackResult.destinationPath">{{ unpackResult.destinationPath }}</p>
            </div>
            <button class="btn btn-secondary open-folder-btn" @click="openDestFolder">
              <svg width="14" height="14" viewBox="0 0 24 24" fill="none">
                <path d="M3 7v10a2 2 0 002 2h14a2 2 0 002-2V9a2 2 0 00-2-2h-6l-2-2H5a2 2 0 00-2 2z" stroke="currentColor" stroke-width="1.5" stroke-linejoin="round"/>
              </svg>
              {{ $t('unpack.openFolder') }}
            </button>
          </div>

          <div class="file-list-toggle" @click="toggleFileList">
            <span>{{ $t('unpack.extractedFiles', { count: unpackResult.extractedFiles.length }) }}</span>
            <svg width="12" height="12" viewBox="0 0 24 24" fill="none" :class="['toggle-arrow', { 'expanded': showFileList }]">
              <path d="M19 9l-7 7-7-7" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round"/>
            </svg>
          </div>

          <div v-if="showFileList" class="extracted-files-list scrollbar-subtle">
            <div v-for="(file, idx) in unpackResult.extractedFiles" :key="idx" class="extracted-file truncate" :title="file">
              ✅ {{ file }}
            </div>
          </div>
        </div>

        <div class="success-actions">
          <button class="btn btn-secondary" @click="resetUnpack">{{ $t('unpack.startNew') }}</button>
          <button class="btn btn-primary" @click="goToPackages">{{ $t('sidebar.packages') }}</button>
        </div>
      </div>

    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted } from 'vue'
import { useRouter } from 'vue-router'
import { useI18n } from 'vue-i18n'
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { useNotification } from '@/composables/useNotification'
import { useAppStore } from '@/stores/app'

const { t } = useI18n()
const router = useRouter()
const notify = useNotification()
const appStore = useAppStore()

// State Machine
// Stage options: 'idle' | 'analyzing' | 'reviewed' | 'processing' | 'success' | 'error'
const stage = ref<'idle' | 'analyzing' | 'reviewed' | 'processing' | 'success' | 'error'>('idle')
const isDragOver = ref(false)

const selectedFilePath = ref('')
const selectedFileName = ref('')
const analysis = ref<any | null>(null)
const unpackResult = ref<any | null>(null)
const showFileList = ref(false)

interface UnpackProgress {
  percentage: number
  currentFile: string
  processed: number
  total: number
}

const unpackProgress = ref<UnpackProgress | null>(null)
let unlistenUnpackProgress: (() => void) | null = null

// Resolve the destination path to be the same directory as the ZIP file
const destinationDir = computed(() => {
  if (!analysis.value) return ''
  const path = analysis.value.filePath
  const lastIndex = Math.max(path.lastIndexOf('/'), path.lastIndexOf('\\'))
  if (lastIndex === -1) return path
  return path.substring(0, lastIndex)
})

onMounted(async () => {
  await appStore.refreshInstallContext()
  unlistenUnpackProgress = await listen<UnpackProgress>('unpack-progress', (event) => {
    unpackProgress.value = event.payload
  })
})

onUnmounted(() => {
  if (unlistenUnpackProgress) {
    unlistenUnpackProgress()
    unlistenUnpackProgress = null
  }
})

// File Drag & Drop Event Handlers
function onDragOver() {
  isDragOver.value = true
}

function onDragLeave() {
  isDragOver.value = false
}

async function onDrop(e: DragEvent) {
  isDragOver.value = false
  const files = e.dataTransfer?.files
  if (!files || files.length === 0) return

  const file = files[0]
  // In tauri standard desktop, file.path is the absolute path to the dropped file
  const path = (file as any).path || ''
  
  if (path && path.toLowerCase().endsWith('.zip')) {
    await processFile(path)
  } else {
    notify.error('仅支持 .zip 格式的压缩文件')
  }
}

// Click Drop Zone File Picker Handler
async function selectFile() {
  try {
    const { open } = await import('@tauri-apps/plugin-dialog')
    const selected = await open({
      multiple: false,
      directory: false,
      title: t('unpack.title'),
      filters: [{
        name: 'Zip Archive',
        extensions: ['zip']
      }]
    })

    if (selected && typeof selected === 'string') {
      await processFile(selected)
    }
  } catch (err) {
    console.error('File open error:', err)
  }
}

// File Analysis Handler
async function processFile(filePath: string) {
  selectedFilePath.value = filePath
  selectedFileName.value = filePath.split(/[\\/]/).pop() || 'unknown.zip'
  stage.value = 'analyzing'
  
  try {
    analysis.value = await invoke('analyze_archive', { archivePath: filePath })
    stage.value = 'reviewed'
  } catch (err) {
    stage.value = 'idle'
    notify.error(`压缩包分析失败: ${err}`)
  }
}

// Unpacking execution trigger
async function startUnpack() {
  if (!analysis.value) return
  stage.value = 'processing'
  unpackProgress.value = null

  try {
    unpackResult.value = await invoke('execute_unpack', {
      analysis: analysis.value,
      customTargetDir: null
    })
    
    stage.value = 'success'
    showFileList.value = false
    notify.success('资源解压提取成功！')
    
    // Automatically trigger packages refresh or watcher handles it
    appStore.refreshInstallContext()

    // Automatically locate and select the extracted folder/file in explorer!
    setTimeout(() => {
      openDestFolder()
    }, 500)
  } catch (err) {
    stage.value = 'reviewed'
    notify.error(`资源整理失败: ${err}`)
  }
}

function cancelUnpack() {
  stage.value = 'idle'
  analysis.value = null
}

function resetUnpack() {
  stage.value = 'idle'
  analysis.value = null
  unpackResult.value = null
  selectedFilePath.value = ''
  selectedFileName.value = ''
}

function toggleFileList() {
  showFileList.value = !showFileList.value
}

// Open output folder directly in system explorer
async function openDestFolder() {
  if (!unpackResult.value) return
  try {
    // If the success action extracted a direct single .var file, we select it, otherwise open the folder
    let target = unpackResult.value.destinationPath
    if (analysis.value?.recommendedAction === 'rename_to_var' && unpackResult.value.extractedFiles.length > 0) {
      // Append the filename to make explorer select it
      const sep = target.includes('/') ? '/' : '\\'
      target = `${target}${sep}${unpackResult.value.extractedFiles[0]}`
    }
    await invoke('open_package_in_explorer', { filePath: target })
  } catch (err) {
    notify.error(`打不开目录: ${err}`)
  }
}

function goToPackages() {
  router.push('/packages')
}

// Helper to format bytes
function formatBytes(bytes: number, decimals = 2) {
  if (!+bytes) return '0 Bytes'
  const k = 1024
  const dm = decimals < 0 ? 0 : decimals
  const sizes = ['Bytes', 'KB', 'MB', 'GB', 'TB']
  const i = Math.floor(Math.log(bytes) / Math.log(k))
  return `${parseFloat((bytes / Math.pow(k, i)).toFixed(dm))} ${sizes[i]}`
}
</script>

<style scoped>
.unpack-view {
  display: flex;
  flex-direction: column;
  gap: var(--space-5);
  max-width: 900px;
  margin: 0 auto;
  padding: 0 var(--space-2);
}

.view-header {
  margin-bottom: var(--space-2);
}

.view-title {
  font-size: var(--text-2xl);
  font-weight: var(--font-bold);
  color: var(--text-primary);
  margin-bottom: var(--space-1);
}

.view-subtitle {
  font-size: var(--text-sm);
  color: var(--text-secondary);
}

.main-container {
  min-height: 420px;
  display: flex;
  flex-direction: column;
  padding: var(--space-6);
  background: var(--glass-bg);
  backdrop-filter: var(--glass-blur);
  -webkit-backdrop-filter: var(--glass-blur);
  border: var(--glass-border);
  border-radius: var(--radius-lg);
  box-shadow: var(--glass-shadow);
  transition: all 300ms var(--ease);
}

/* ── Stage 1: Drop Zone ── */
.drop-zone {
  flex: 1;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  border: 2px dashed rgba(255, 255, 255, 0.1);
  border-radius: var(--radius-md);
  cursor: pointer;
  padding: var(--space-8);
  background: rgba(255, 255, 255, 0.01);
  transition: all 350ms cubic-bezier(0.2, 0.8, 0.2, 1);
}

.drop-zone:hover, .drop-zone.is-dragover {
  border-color: var(--accent-primary);
  background: rgba(124, 92, 252, 0.05);
  box-shadow: 0 0 30px rgba(124, 92, 252, 0.08);
}

.drop-icon-wrapper {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 80px;
  height: 80px;
  border-radius: var(--radius-xl);
  background: rgba(255, 255, 255, 0.02);
  border: 1px solid var(--border-subtle);
  color: var(--text-secondary);
  margin-bottom: var(--space-5);
  transition: all 350ms cubic-bezier(0.2, 0.8, 0.2, 1);
}

.drop-zone:hover .drop-icon-wrapper, .drop-zone.is-dragover .drop-icon-wrapper {
  color: var(--text-primary);
  background: var(--accent-gradient);
  border-color: transparent;
  box-shadow: 0 8px 24px rgba(124, 92, 252, 0.3);
  transform: translateY(-4px) scale(1.05);
}

.drop-icon {
  transition: transform 350ms cubic-bezier(0.2, 0.8, 0.2, 1);
}

.drop-zone:hover .drop-icon, .drop-zone.is-dragover .drop-icon {
  transform: translateY(-1px);
}

.drop-title {
  font-size: var(--text-md);
  font-weight: var(--font-medium);
  color: var(--text-secondary);
  margin-bottom: var(--space-2);
  text-align: center;
}

.click-link {
  color: var(--accent-primary);
  font-weight: var(--font-bold);
}

.drop-hint {
  font-size: var(--text-xs);
  color: var(--text-tertiary);
  text-align: center;
}

/* ── Stage 2 & 4: Statuses ── */
.analyzing-zone, .processing-zone {
  flex: 1;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  padding: var(--space-8);
}

.spinner-wrapper {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 70px;
  height: 70px;
  margin-bottom: var(--space-5);
}

.spinner {
  width: 44px;
  height: 44px;
  border: 3.5px solid rgba(124, 92, 252, 0.1);
  border-top-color: var(--accent-primary);
  border-radius: 50%;
  animation: spin-anim 1s linear infinite;
}

@keyframes spin-anim {
  0% { transform: rotate(0deg); }
  100% { transform: rotate(360deg); }
}

.status-title {
  font-size: var(--text-md);
  font-weight: var(--font-semibold);
  color: var(--text-primary);
  margin-bottom: var(--space-2);
}

.status-filename {
  font-size: var(--text-sm);
  color: var(--text-tertiary);
  max-width: 400px;
}

.progress-container {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
  width: 100%;
  max-width: 480px;
  margin-top: var(--space-4);
}

.progress-meta {
  display: flex;
  justify-content: space-between;
  font-size: var(--text-xs);
  font-weight: var(--font-medium);
}

.progress-percent {
  color: var(--accent-primary);
}

.progress-counts {
  color: var(--text-secondary);
}

.progress-track {
  height: 6px;
  background: rgba(255, 255, 255, 0.05);
  border-radius: var(--radius-full);
  overflow: hidden;
  border: 1px solid var(--border-subtle);
}

.progress-fill {
  height: 100%;
  background: var(--accent-gradient);
  border-radius: var(--radius-full);
  transition: width 0.15s ease-out;
  box-shadow: 0 0 10px rgba(124, 92, 252, 0.3);
}

.progress-file {
  font-size: var(--text-xs);
  color: var(--text-tertiary);
  text-align: center;
  word-break: break-all;
  margin-top: var(--space-1);
}

.processing-graphics {
  position: relative;
  width: 80px;
  height: 80px;
  display: flex;
  align-items: center;
  justify-content: center;
  border-radius: 50%;
  color: var(--accent-primary);
  background: rgba(124, 92, 252, 0.1);
  margin-bottom: var(--space-5);
}

.pulse-ring {
  position: absolute;
  width: 100%;
  height: 100%;
  border-radius: 50%;
  border: 2px solid var(--accent-primary);
  animation: pulse-ring-anim 1.8s cubic-bezier(0.215, 0.610, 0.355, 1) infinite;
}

@keyframes pulse-ring-anim {
  0% { transform: scale(0.95); opacity: 0.8; }
  100% { transform: scale(1.4); opacity: 0; }
}

.extracting-icon {
  animation: bouncing-extract 1.2s ease-in-out infinite;
}

@keyframes bouncing-extract {
  0%, 100% { transform: translateY(0); }
  50% { transform: translateY(4px); }
}

/* ── Stage 3: Review Zone ── */
.review-zone {
  display: flex;
  flex-direction: column;
  gap: var(--space-5);
}

.review-header-info {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  border-bottom: 1px solid var(--border-subtle);
  padding-bottom: var(--space-4);
}

.archive-name {
  font-size: var(--text-lg);
  font-weight: var(--font-bold);
  color: var(--text-primary);
  max-width: 650px;
  margin-bottom: var(--space-2);
}

.archive-badges {
  display: flex;
  align-items: center;
  gap: var(--space-2);
}

.badge {
  display: inline-flex;
  align-items: center;
  padding: var(--space-1) var(--space-2);
  border-radius: var(--radius-sm);
  font-size: 11px;
  font-weight: var(--font-semibold);
}

.size-badge {
  background: var(--bg-hover);
  border: 1px solid var(--border-subtle);
  color: var(--text-secondary);
}

.type-badge {
  border: 1px solid transparent;
}

.type-badge.misnamed_var {
  background: rgba(167, 139, 250, 0.08);
  border-color: rgba(167, 139, 250, 0.2);
  color: #a78bfa;
}

.type-badge.var_container {
  background: rgba(96, 165, 250, 0.08);
  border-color: rgba(96, 165, 250, 0.2);
  color: #60a5fa;
}

.type-badge.vam_content {
  background: rgba(244, 114, 182, 0.08);
  border-color: rgba(244, 114, 182, 0.2);
  color: #f472b6;
}

.type-badge.flat_content {
  background: rgba(52, 211, 153, 0.08);
  border-color: rgba(52, 211, 153, 0.2);
  color: #34d399;
}

.review-grid {
  display: grid;
  grid-template-columns: 1.2fr 1fr;
  gap: var(--space-6);
}

.section-title {
  font-size: var(--text-sm);
  font-weight: var(--font-semibold);
  text-transform: uppercase;
  letter-spacing: 0.05em;
  color: var(--text-tertiary);
  margin-bottom: var(--space-3);
}

.detected-panel {
  display: flex;
  flex-direction: column;
  gap: var(--space-4);
  border-right: 1px solid var(--border-subtle);
  padding-right: var(--space-6);
}

.stat-list {
  display: flex;
  flex-direction: column;
  gap: 10px;
}

.stat-item {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 8px 12px;
  background: rgba(255, 255, 255, 0.01);
  border: 1px solid rgba(255, 255, 255, 0.03);
  border-radius: var(--radius-sm);
  font-size: var(--text-sm);
}

.stat-label {
  color: var(--text-secondary);
}

.stat-value {
  color: var(--text-primary);
  font-weight: var(--font-medium);
}

.stat-indicator {
  font-weight: var(--font-semibold);
}

.stat-indicator.has {
  color: var(--color-success);
}

.code-font {
  font-family: monospace;
  background: rgba(0, 0, 0, 0.15);
  padding: 2px 6px;
  border-radius: 4px;
  max-width: 200px;
}

.sample-panel {
  margin-top: var(--space-2);
}

.sample-title {
  font-size: var(--text-xs);
  font-weight: var(--font-semibold);
  color: var(--text-secondary);
  margin-bottom: var(--space-2);
}

.sample-list {
  max-height: 120px;
  overflow-y: auto;
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-sm);
  padding: var(--space-2);
  background: rgba(0, 0, 0, 0.1);
}

.sample-file {
  font-size: 11px;
  color: var(--text-tertiary);
  padding: 3px 0;
  border-bottom: 1px solid rgba(255, 255, 255, 0.02);
}

.sample-file:last-child {
  border-bottom: none;
}

.sample-more {
  font-size: 10px;
  color: var(--text-tertiary);
  text-align: center;
  padding-top: 4px;
  font-style: italic;
}

/* Right Action Panel */
.action-panel {
  display: flex;
  flex-direction: column;
  justify-content: space-between;
  gap: var(--space-5);
}

.recommendation-card {
  display: flex;
  gap: var(--space-3);
  padding: var(--space-4);
  background: rgba(124, 92, 252, 0.08);
  border: 1px solid rgba(124, 92, 252, 0.15);
  border-radius: var(--radius-md);
  color: var(--text-primary);
}

.rec-icon {
  color: var(--accent-primary);
  flex-shrink: 0;
}

.rec-title {
  font-size: var(--text-sm);
  font-weight: var(--font-bold);
  text-transform: uppercase;
  letter-spacing: 0.03em;
  color: var(--accent-primary);
  margin-bottom: var(--space-1);
}

.rec-action-desc {
  font-size: var(--text-sm);
  font-weight: var(--font-medium);
  line-height: 1.4;
}

.destination-preview {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
  padding: var(--space-3) var(--space-4);
  background: rgba(0, 0, 0, 0.15);
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-sm);
}

.dest-label {
  font-size: 11px;
  font-weight: var(--font-bold);
  text-transform: uppercase;
  color: var(--text-tertiary);
  letter-spacing: 0.05em;
}

.dest-path {
  font-size: var(--text-xs);
  color: var(--text-secondary);
  word-break: break-all;
}

.action-buttons {
  display: flex;
  gap: var(--space-3);
}

.btn {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  gap: var(--space-2);
  height: 38px;
  padding: 0 var(--space-4);
  font-size: var(--text-sm);
  font-weight: var(--font-medium);
  border-radius: var(--radius-sm);
  cursor: pointer;
  transition: all 200ms var(--ease);
}

.btn-primary {
  flex: 1.5;
  background: var(--accent-gradient);
  color: white;
  border: none;
  box-shadow: 0 4px 12px rgba(124, 92, 252, 0.2);
}

.btn-primary:hover {
  box-shadow: 0 6px 16px rgba(124, 92, 252, 0.35);
  transform: translateY(-1px);
}

.btn-secondary {
  flex: 1;
  background: var(--bg-elevated);
  border: 1px solid var(--border-subtle);
  color: var(--text-primary);
}

.btn-secondary:hover {
  background: var(--bg-hover);
  border-color: var(--border-default);
}

/* ── Stage 5: Success ── */
.success-zone {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: var(--space-6);
  padding: var(--space-2) 0;
}

.success-header {
  text-align: center;
  display: flex;
  flex-direction: column;
  align-items: center;
}

.success-badge-anim {
  width: 72px;
  height: 72px;
  border-radius: 50%;
  background: rgba(52, 211, 153, 0.1);
  border: 1px solid rgba(52, 211, 153, 0.25);
  color: var(--color-success);
  display: flex;
  align-items: center;
  justify-content: center;
  margin-bottom: var(--space-4);
  box-shadow: 0 0 30px rgba(52, 211, 153, 0.1);
  animation: checkmark-pop 500ms cubic-bezier(0.34, 1.56, 0.64, 1);
}

@keyframes checkmark-pop {
  0% { transform: scale(0.5); opacity: 0; }
  100% { transform: scale(1); opacity: 1; }
}

.success-title {
  font-size: var(--text-xl);
  font-weight: var(--font-bold);
  color: var(--text-primary);
  margin-bottom: var(--space-1);
}

.success-desc {
  font-size: var(--text-sm);
  color: var(--text-secondary);
  max-width: 500px;
}

.success-details-card {
  width: 100%;
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-md);
  background: rgba(0, 0, 0, 0.12);
  padding: var(--space-4);
}

.dest-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--space-4);
  border-bottom: 1px solid rgba(255, 255, 255, 0.04);
  padding-bottom: var(--space-4);
  margin-bottom: var(--space-3);
}

.open-folder-btn {
  height: 32px;
  font-size: 12px;
  padding: 0 var(--space-3);
  flex-shrink: 0;
}

.file-list-toggle {
  display: flex;
  align-items: center;
  justify-content: space-between;
  font-size: var(--text-sm);
  font-weight: var(--font-medium);
  color: var(--text-secondary);
  cursor: pointer;
  padding: 2px var(--space-1);
  user-select: none;
}

.file-list-toggle:hover {
  color: var(--text-primary);
}

.toggle-arrow {
  transition: transform 250ms var(--ease);
}

.toggle-arrow.expanded {
  transform: rotate(180deg);
}

.extracted-files-list {
  max-height: 180px;
  overflow-y: auto;
  border-top: 1px solid rgba(255, 255, 255, 0.03);
  margin-top: var(--space-3);
  padding-top: var(--space-2);
}

.extracted-file {
  font-size: 12px;
  color: var(--text-tertiary);
  padding: 5px var(--space-1);
  border-bottom: 1px solid rgba(255, 255, 255, 0.01);
}

.extracted-file:last-child {
  border-bottom: none;
}

.success-actions {
  display: flex;
  gap: var(--space-4);
  width: 320px;
}

.success-actions .btn {
  flex: 1;
}

/* Scrollbar Styles */
.scrollbar-subtle::-webkit-scrollbar {
  width: 6px;
}

.scrollbar-subtle::-webkit-scrollbar-track {
  background: transparent;
}

.scrollbar-subtle::-webkit-scrollbar-thumb {
  background: rgba(255, 255, 255, 0.08);
  border-radius: 3px;
}

.scrollbar-subtle::-webkit-scrollbar-thumb:hover {
  background: rgba(255, 255, 255, 0.15);
}
</style>
