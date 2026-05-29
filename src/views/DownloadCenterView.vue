<template>
  <div class="download-center-view animate-fadeIn">
    <!-- Top Stats Section -->
    <section class="summary-grid stagger-children">
      <StatCard
        :title="$t('download.stats.total')"
        :value="downloadStore.queue.length"
        :subtitle="`Pending: ${downloadStore.pendingDownloads.length}`"
        icon="M9 5H7a2 2 0 00-2 2v12a2 2 0 002 2h10a2 2 0 002-2V7a2 2 0 00-2-2h-2M9 5a2 2 0 002 2h2a2 2 0 002-2M9 5a2 2 0 012-2h2a2 2 0 012 2m-3 7h3m-3 4h3m-6-4h.01M9 16h.01"
        trend="neutral"
        color="#6e6bf0"
      />
      <StatCard
        :title="$t('download.stats.downloading')"
        :value="downloadStore.activeDownloads.length"
        :subtitle="`Completed: ${downloadStore.completedCount}`"
        icon="M7 16a4 4 0 01-.88-7.903A5 5 0 1115.9 6L16 6a5 5 0 011 9.9M9 19l3 3m0 0l3-3m-3 3V10"
        :trend="downloadStore.activeDownloads.length > 0 ? 'up' : 'neutral'"
        color="#5b8def"
      />
      <StatCard
        :title="$t('download.stats.speed')"
        :value="formatSpeed(downloadStore.totalSpeed)"
        :subtitle="downloadStore.activeDownloads.length > 0 ? 'Downloading packages' : 'Idle'"
        icon="M13 10V3L4 14h7v7l9-11h-7z"
        :trend="downloadStore.totalSpeed > 0 ? 'up' : 'neutral'"
        color="#3ecf8e"
      />
      <StatCard
        :title="$t('download.stats.savedSpace')"
        :value="formatSize(downloadStore.totalWastedBytesSaved)"
        :subtitle="'Avoided duplicate downloads'"
        icon="M5 3v4M3 5h4M6 17v4m-2-2h4m5-16l2.286 6.857L21 12l-5.714 2.286L13 21l-2.286-5.714L5 13l5.714-2.286L13 3z"
        trend="neutral"
        color="#a78bfa"
      />
    </section>

    <!-- Toolbar / Action Bar -->
    <div class="action-bar glass-panel">
      <!-- Left Filters -->
      <div class="filter-tabs">
        <button
          :class="['tab-btn', { active: activeTab === 'active' }]"
          @click="activeTab = 'active'"
        >
          <span>{{ $t('download.tabs.active', { count: downloadStore.activeAndPendingCount }) }}</span>
        </button>
        <button
          :class="['tab-btn', { active: activeTab === 'completed' }]"
          @click="activeTab = 'completed'"
        >
          <span>{{ $t('download.tabs.completed', { count: downloadStore.completedCount }) }}</span>
        </button>
        <button
          :class="['tab-btn', { active: activeTab === 'all' }]"
          @click="activeTab = 'all'"
        >
          <span>{{ $t('download.tabs.all') }}</span>
          <span class="tab-badge">{{ downloadStore.queue.length }}</span>
        </button>
      </div>

      <!-- Right Global Actions -->
      <div class="global-actions">
        <span class="target-summary" :title="appStore.installContext?.downloadTargetDir || ''">
          {{ downloadModeLabel }}
        </span>
        <button
          class="action-btn-secondary"
          :disabled="!hasActiveTasks"
          @click="downloadStore.pauseAll"
        >
          <svg width="14" height="14" viewBox="0 0 24 24" fill="none">
            <path d="M10 9v6m4-6v6m7-3a9 9 0 11-18 0 9 9 0 0118 0z" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" />
          </svg>
          <span>{{ $t('download.actions.pauseAll') }}</span>
        </button>
        <button
          class="action-btn-secondary"
          :disabled="!hasPausedTasks"
          @click="downloadStore.resumeAll"
        >
          <svg width="14" height="14" viewBox="0 0 24 24" fill="none">
            <path d="M14.752 11.168l-3.197-2.132A1 1 0 0010 9.87v4.263a1 1 0 001.555.832l3.197-2.132a1 1 0 000-1.664z" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" />
            <path d="M21 12a9 9 0 11-18 0 9 9 0 0118 0z" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" />
          </svg>
          <span>{{ $t('download.actions.resumeAll') }}</span>
        </button>
        <button
          class="action-btn-danger"
          :disabled="downloadStore.completedCount === 0"
          @click="downloadStore.clearCompleted"
        >
          <svg width="14" height="14" viewBox="0 0 24 24" fill="none">
            <path d="M19 7l-.867 12.142A2 2 0 0116.138 21H7.862a2 2 0 01-1.995-1.858L5 7m5 4v6m4-6v6m1-10V4a1 1 0 00-1-1h-4a1 1 0 00-1 1v3M4 7h16" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" />
          </svg>
          <span>{{ $t('download.actions.clearCompleted') }}</span>
        </button>
      </div>
    </div>

    <!-- Download Tasks List -->
    <div class="list-container glass-panel flex-1">
      <div v-if="filteredQueue.length > 0" class="task-list">
        <div
          v-for="item in filteredQueue"
          :key="item.id"
          class="task-card-wrapper"
        >
          <div class="task-card">
            <!-- Left Info Block -->
            <div class="task-info">
              <div class="task-meta">
                <span class="creator-badge text-xs">{{ item.creator || 'Hub' }}</span>
                <span class="version-label text-xs">v{{ item.version }}</span>
                <span :class="['status-badge', item.status.toLowerCase()]">
                  {{ $t(`download.card.${item.status.toLowerCase()}`) }}
                </span>
              </div>
              <h4 class="filename text-sm" :title="item.filename">
                {{ item.filename }}
              </h4>
              <div class="target-line text-xs" :title="item.final_path || ''">
                <span>{{ installModeLabel(item.install_mode) }}</span>
                <span v-if="item.final_path">{{ item.final_path }}</span>
                <span v-if="item.status === 'Completed'">{{ item.indexed ? $t('download.card.indexed') : $t('download.card.notIndexed') }}</span>
                <span v-if="item.status === 'Completed'">{{ downloadAfterActionLabel }}</span>
              </div>
            </div>

            <!-- Middle Progress Bar & Details -->
            <div class="task-progress-details">
              <div class="progress-info text-xs">
                <span class="downloaded-size">
                  {{ formatSize(item.downloaded_bytes) }} / {{ formatSize(item.total_bytes) }}
                </span>
                <span v-if="item.status === 'Downloading'" class="speed-indicator text-success">
                  ⚡ {{ formatSpeed(item.speed_bytes_per_sec) }}
                </span>
                <span class="progress-percent font-medium">{{ item.progress.toFixed(0) }}%</span>
              </div>
              
              <!-- Smooth Progress Track -->
              <div class="progress-track">
                <div 
                  class="progress-fill" 
                  :style="{ width: `${item.progress}%` }"
                  :class="{ active: item.status === 'Downloading', paused: item.status === 'Paused', failed: item.status === 'Failed' }"
                />
              </div>
            </div>

            <!-- Right Controls Block -->
            <div class="task-controls">
              <!-- Play / Pause -->
              <button
                v-if="item.status === 'Downloading' || item.status === 'Pending'"
                class="icon-btn"
                :title="$t('download.actions.pause')"
                @click="downloadStore.pauseTask(item.id)"
              >
                <svg width="16" height="16" viewBox="0 0 24 24" fill="none">
                  <path d="M15.75 5.25v13.5m-7.5-13.5v13.5" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"/>
                </svg>
              </button>
              
              <button
                v-if="item.status === 'Paused'"
                class="icon-btn"
                :title="$t('download.actions.resume')"
                @click="downloadStore.resumeTask(item.id)"
              >
                <svg width="16" height="16" viewBox="0 0 24 24" fill="none">
                  <path d="M5.25 5.653c0-.856.917-1.398 1.667-.986l11.54 6.348a1.125 1.125 0 010 1.972l-11.54 6.347a1.125 1.125 0 01-1.667-.985V5.653z" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"/>
                </svg>
              </button>
              
              <!-- Retry for Failed -->
              <button
                v-if="item.status === 'Failed'"
                class="icon-btn text-warning"
                :title="$t('download.actions.retry')"
                @click="downloadStore.retryTask(item.id)"
              >
                <svg width="16" height="16" viewBox="0 0 24 24" fill="none">
                  <path d="M16.023 9.348h4.992v-.001M2.985 19.644v-4.992m0 0h4.992m-4.993 0l3.181 3.183a8.25 8.25 0 0013.803-3.7M4.031 9.865a8.25 8.25 0 0113.803-3.7l3.181 3.182m0-4.991v4.99" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"/>
                </svg>
              </button>

              <!-- Cancel / Delete -->
              <button
                class="icon-btn text-danger"
                :title="$t('download.actions.delete')"
                @click="downloadStore.cancelTask(item.id)"
              >
                <svg width="16" height="16" viewBox="0 0 24 24" fill="none">
                  <path d="M6 18L18 6M6 6l12 12" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"/>
                </svg>
              </button>
            </div>
          </div>

          <!-- Error Message Sub-card -->
          <div v-if="item.status === 'Failed' && item.error_msg" class="error-msg-bar text-xs">
            <span class="error-label font-semibold">{{ $t('download.card.error') }}:</span>
            <span class="error-text">{{ item.error_msg }}</span>
          </div>
        </div>
      </div>

      <!-- Elegant Empty State -->
      <div v-else class="empty-state-wrapper">
        <EmptyState
          icon="M12 16.5V9.75m0 0l3 3m-3-3l-3 3M6.75 19.5h10.5a2.25 2.25 0 002.25-2.25V6.75A2.25 2.25 0 0017.25 4.5H6.75A2.25 2.25 0 004.5 6.75v10.5a2.25 2.25 0 002.25 2.25z"
          :title="$t('download.empty')"
          :description="$t('download.emptyDesc')"
        />
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, computed } from 'vue'
import { useDownloadStore } from '@/stores/download'
import { useAppStore } from '@/stores/app'
import StatCard from '@/components/common/StatCard.vue'
import EmptyState from '@/components/common/EmptyState.vue'

const downloadStore = useDownloadStore()
const appStore = useAppStore()
const activeTab = ref<'all' | 'active' | 'completed'>('active')

const downloadModeLabel = computed(() => {
  const context = appStore.installContext
  if (!context) return '下载路径未配置'
  return context.isManaged ? '托管库下载' : 'AddonPackages 下载'
})

const downloadAfterActionLabel = computed(() => {
  const action = appStore.installContext?.downloadAfterAction
  if (action === 'add_to_active_plan') return '下载后加入当前方案'
  if (action === 'add_and_apply') return '下载后加入并应用'
  return '仅入库'
})

const hasActiveTasks = computed(() => {
  return downloadStore.queue.some(i => i.status === 'Downloading' || i.status === 'Pending')
})

const hasPausedTasks = computed(() => {
  return downloadStore.queue.some(i => i.status === 'Paused' || i.status === 'Failed')
})

const filteredQueue = computed(() => {
  switch (activeTab.value) {
    case 'active':
      return downloadStore.queue.filter(i => i.status === 'Downloading' || i.status === 'Pending' || i.status === 'Paused' || i.status === 'Failed')
    case 'completed':
      return downloadStore.queue.filter(i => i.status === 'Completed')
    case 'all':
    default:
      return downloadStore.queue
  }
})

// Formatting helpers
function formatSize(bytes: number): string {
  if (!bytes || bytes === 0) return '0 B'
  const units = ['B', 'KB', 'MB', 'GB', 'TB']
  const i = Math.floor(Math.log(bytes) / Math.log(1024))
  return `${(bytes / Math.pow(1024, i)).toFixed(1)} ${units[i]}`
}

function formatSpeed(bytesPerSec: number): string {
  if (!bytesPerSec || bytesPerSec === 0) return '0 KB/s'
  const kbps = bytesPerSec / 1024
  if (kbps < 1024) {
    return `${kbps.toFixed(1)} KB/s`
  }
  const mbps = kbps / 1024
  return `${mbps.toFixed(1)} MB/s`
}

function installModeLabel(mode: string | null): string {
  if (mode === 'managed_library') return '托管库'
  if (mode === 'real_addon') return 'AddonPackages'
  return '等待解析路径'
}
</script>

<style scoped>
.download-center-view {
  display: flex;
  flex-direction: column;
  gap: var(--space-5);
  height: 100%;
}

.summary-grid {
  display: grid;
  grid-template-columns: repeat(4, 1fr);
  gap: var(--space-5);
}

@media (max-width: 1200px) {
  .summary-grid {
    grid-template-columns: repeat(2, 1fr);
  }
}

@media (max-width: 768px) {
  .summary-grid {
    grid-template-columns: 1fr;
  }
}

/* ── Toolbar Action Bar ────────────────────────────────────── */
.action-bar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: var(--space-3) var(--space-4);
  flex-shrink: 0;
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-lg);
  background: var(--glass-bg);
  backdrop-filter: var(--glass-blur);
  -webkit-backdrop-filter: var(--glass-blur);
  box-shadow: var(--glass-shadow);
}

.filter-tabs {
  display: flex;
  gap: var(--space-2);
}

.tab-btn {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  padding: var(--space-2) var(--space-4);
  height: 32px;
  border-radius: var(--radius-md);
  color: var(--text-secondary);
  font-size: var(--font-size-sm);
  font-weight: var(--font-weight-medium);
  transition: all var(--transition-fast);
}

.tab-btn:hover {
  color: var(--text-primary);
  background: var(--bg-hover);
}

.tab-btn.active {
  color: var(--accent-primary);
  background: rgba(110, 107, 240, 0.12);
}

.tab-badge {
  font-variant-numeric: tabular-nums;
  background: rgba(255, 255, 255, 0.1);
  color: var(--text-secondary);
  padding: 1px 6px;
  border-radius: var(--radius-full);
  font-size: var(--font-size-xs);
  transition: background var(--transition-fast);
}

.tab-btn.active .tab-badge {
  background: rgba(110, 107, 240, 0.2);
  color: var(--accent-primary);
}

.global-actions {
  display: flex;
  align-items: center;
  gap: var(--space-2);
}

.target-summary {
  max-width: 220px;
  overflow: hidden;
  padding: 0 var(--space-3);
  color: var(--text-secondary);
  font-size: var(--font-size-xs);
  text-overflow: ellipsis;
  white-space: nowrap;
}

.action-btn-secondary,
.action-btn-danger {
  display: inline-flex;
  align-items: center;
  gap: var(--space-2);
  padding: 0 var(--space-3);
  height: 32px;
  font-size: var(--font-size-sm);
  font-weight: var(--font-weight-medium);
  border-radius: var(--radius-md);
  transition: all var(--transition-fast);
}

.action-btn-secondary {
  color: var(--text-secondary);
  background: transparent;
  border: 1px solid var(--border-subtle);
}

.action-btn-secondary:hover:not(:disabled) {
  color: var(--text-primary);
  background: var(--bg-hover);
  border-color: var(--border-default);
}

.action-btn-secondary:disabled,
.action-btn-danger:disabled {
  opacity: 0.4;
  cursor: not-allowed;
}

.action-btn-danger {
  color: var(--color-error);
  background: rgba(248, 113, 113, 0.08);
  border: 1px solid rgba(248, 113, 113, 0.15);
}

.action-btn-danger:hover:not(:disabled) {
  background: rgba(248, 113, 113, 0.15);
  border-color: rgba(248, 113, 113, 0.3);
}

/* ── Tasks List ────────────────────────────────────────────── */
.list-container {
  display: flex;
  flex-direction: column;
  overflow: hidden;
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-lg);
  background: var(--glass-bg);
  backdrop-filter: var(--glass-blur);
  -webkit-backdrop-filter: var(--glass-blur);
  box-shadow: var(--glass-shadow);
  min-height: 300px;
}

.task-list {
  flex: 1;
  overflow-y: auto;
  padding: var(--space-4);
  display: flex;
  flex-direction: column;
  gap: var(--space-3);
}

.task-card-wrapper {
  display: flex;
  flex-direction: column;
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-md);
  background: rgba(255, 255, 255, 0.02);
  transition: border-color var(--transition-fast), background var(--transition-fast);
}

.task-card-wrapper:hover {
  border-color: rgba(110, 107, 240, 0.2);
  background: rgba(255, 255, 255, 0.04);
}

.task-card {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: var(--space-4);
  gap: var(--space-6);
}

@media (max-width: 900px) {
  .task-card {
    flex-direction: column;
    align-items: stretch;
    gap: var(--space-3);
  }
}

/* Info Column */
.task-info {
  flex: 2;
  min-width: 0;
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
}

.task-meta {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  flex-wrap: wrap;
}

.creator-badge {
  background: rgba(255, 255, 255, 0.06);
  color: var(--text-secondary);
  padding: 1px 8px;
  border-radius: var(--radius-full);
  font-weight: var(--font-weight-semibold);
}

.version-label {
  color: var(--text-tertiary);
  font-weight: var(--font-weight-medium);
}

.status-badge {
  padding: 1px 8px;
  border-radius: var(--radius-full);
  font-size: 10px;
  font-weight: var(--font-weight-semibold);
  text-transform: uppercase;
}

.status-badge.pending {
  background: rgba(167, 139, 250, 0.15);
  color: var(--type-appearance);
}

.status-badge.downloading {
  background: rgba(96, 165, 250, 0.15);
  color: var(--color-info);
}

.status-badge.paused {
  background: rgba(251, 191, 36, 0.15);
  color: var(--color-warning);
}

.status-badge.completed {
  background: rgba(52, 211, 153, 0.15);
  color: var(--color-success);
}

.status-badge.failed {
  background: rgba(248, 113, 113, 0.15);
  color: var(--color-error);
}

.filename {
  color: var(--text-primary);
  font-weight: var(--font-weight-semibold);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.target-line {
  display: flex;
  gap: var(--space-2);
  overflow: hidden;
  color: var(--text-tertiary);
}

.target-line span {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

/* Progress Column */
.task-progress-details {
  flex: 3;
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
}

.progress-info {
  display: flex;
  justify-content: space-between;
  align-items: center;
  color: var(--text-secondary);
  font-variant-numeric: tabular-nums;
}

.speed-indicator {
  font-weight: var(--font-weight-semibold);
}

.progress-track {
  height: 6px;
  background: rgba(255, 255, 255, 0.05);
  border-radius: var(--radius-full);
  overflow: hidden;
}

.progress-fill {
  height: 100%;
  border-radius: var(--radius-full);
  transition: width 0.3s ease;
  background: var(--accent-gradient);
}

.progress-fill.active {
  background: linear-gradient(90deg, var(--accent-primary) 0%, var(--accent-secondary) 100%);
}

.progress-fill.paused {
  background: var(--color-warning);
}

.progress-fill.failed {
  background: var(--color-error);
}

/* Controls Column */
.task-controls {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  flex-shrink: 0;
}

.icon-btn {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 32px;
  height: 32px;
  border-radius: var(--radius-md);
  color: var(--text-secondary);
  background: transparent;
  transition: all var(--transition-fast);
}

.icon-btn:hover {
  color: var(--text-primary);
  background: var(--bg-hover);
}

.icon-btn.text-danger:hover {
  color: var(--color-error);
  background: rgba(248, 113, 113, 0.1);
}

.icon-btn.text-warning:hover {
  color: var(--color-warning);
  background: rgba(251, 191, 36, 0.1);
}

/* Error bar */
.error-msg-bar {
  padding: var(--space-2) var(--space-4);
  border-top: 1px solid rgba(248, 113, 113, 0.15);
  background: rgba(248, 113, 113, 0.03);
  border-bottom-left-radius: var(--radius-md);
  border-bottom-right-radius: var(--radius-md);
  display: flex;
  gap: var(--space-2);
}

.error-label {
  color: var(--color-error);
  flex-shrink: 0;
}

.error-text {
  color: var(--text-secondary);
}

/* Empty State Wrapper */
.empty-state-wrapper {
  flex: 1;
  display: flex;
  align-items: center;
  justify-content: center;
  padding: var(--space-8);
}
</style>
