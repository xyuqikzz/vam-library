<template>
<div class="download-center-view animate-fadeIn b2-page"><PageHeader :title="$t('sidebar.downloadCenter')" ><template #filters>
      <!-- Left Filters -->
      <div class="filter-tabs">
        <UiChip :class="['', { active: activeTab === 'active' }]" @click="activeTab = 'active'" :selected="activeTab === 'active'">
          <span>{{ $t('download.tabs.active', { count: downloadStore.activeAndPendingCount }) }}</span>
        </UiChip>
        <UiChip :class="['', { active: activeTab === 'completed' }]" @click="activeTab = 'completed'" :selected="activeTab === 'completed'">
          <span>{{ $t('download.tabs.completed', { count: downloadStore.completedCount }) }}</span>
        </UiChip>
        <UiChip :class="['', { active: activeTab === 'all' }]" @click="activeTab = 'all'" :selected="activeTab === 'all'">
          <span>{{ $t('download.tabs.all') }}</span>
          <span class="tab-badge">{{ downloadStore.queue.length }}</span>
        </UiChip>
      </div>

      <!-- Right Global Actions -->
      <div class="global-actions">
        <span class="target-summary" :title="appStore.installContext?.downloadTargetDir || ''">
          {{ downloadModeLabel }}
        </span>
        <UiButton :disabled="!hasActiveTasks" @click="downloadStore.pauseAll" variant="secondary">
          <Icon  :size="14" name="outline-21" />
          <span>{{ $t('download.actions.pauseAll') }}</span>
        </UiButton>
        <UiButton :disabled="!hasPausedTasks" @click="downloadStore.resumeAll" variant="secondary">
          <Icon  :size="14" name="outline-22" />
          <span>{{ $t('download.actions.resumeAll') }}</span>
        </UiButton>
        <UiButton :disabled="downloadStore.completedCount === 0" @click="downloadStore.clearCompleted" variant="danger">
          <Icon  :size="14" name="outline-18" />
          <span>{{ $t('download.actions.clearCompleted') }}</span>
        </UiButton>
      </div>
    </template></PageHeader><div class="b2-page-body">
    <!-- Top Stats Section -->
    <section class="summary-grid stagger-children">
      <StatCard
        :title="$t('download.stats.total')"
        :value="downloadStore.queue.length"
        :subtitle="`Pending: ${downloadStore.pendingDownloads.length}`"
        icon="M9 5H7a2 2 0 00-2 2v12a2 2 0 002 2h10a2 2 0 002-2V7a2 2 0 00-2-2h-2M9 5a2 2 0 002 2h2a2 2 0 002-2M9 5a2 2 0 012-2h2a2 2 0 012 2m-3 7h3m-3 4h3m-6-4h.01M9 16h.01"
        trend="neutral"
        color="var(--accent-primary)"
      />
      <StatCard
        :title="$t('download.stats.downloading')"
        :value="downloadStore.activeDownloads.length"
        :subtitle="`Completed: ${downloadStore.completedCount}`"
        icon="M7 16a4 4 0 01-.88-7.903A5 5 0 1115.9 6L16 6a5 5 0 011 9.9M9 19l3 3m0 0l3-3m-3 3V10"
        :trend="downloadStore.activeDownloads.length > 0 ? 'up' : 'neutral'"
        color="var(--color-info)"
      />
      <StatCard
        :title="$t('download.stats.speed')"
        :value="formatSpeed(downloadStore.totalSpeed)"
        :subtitle="downloadStore.activeDownloads.length > 0 ? 'Downloading packages' : 'Idle'"
        icon="M13 10V3L4 14h7v7l9-11h-7z"
        :trend="downloadStore.totalSpeed > 0 ? 'up' : 'neutral'"
        color="var(--color-success)"
      />
      <StatCard
        :title="$t('download.stats.savedSpace')"
        :value="formatSize(downloadStore.totalWastedBytesSaved)"
        :subtitle="'Avoided duplicate downloads'"
        icon="M5 3v4M3 5h4M6 17v4m-2-2h4m5-16l2.286 6.857L21 12l-5.714 2.286L13 21l-2.286-5.714L5 13l5.714-2.286L13 3z"
        trend="neutral"
        color="var(--accent-primary)"
      />
    </section>

    <!-- Toolbar / Action Bar -->


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
              <UiButton v-if="item.status === 'Downloading' || item.status === 'Pending'" :title="$t('download.actions.pause')" @click="downloadStore.pauseTask(item.id)" variant="ghost" icon-only :aria-label="$t('download.actions.pause')">
                <Icon  :size="16" name="outline-23" />
              </UiButton>

              <UiButton v-if="item.status === 'Paused'" :title="$t('download.actions.resume')" @click="downloadStore.resumeTask(item.id)" variant="ghost" icon-only :aria-label="$t('download.actions.resume')">
                <Icon  :size="16" name="outline-24" />
              </UiButton>

              <!-- Retry for Failed -->
              <UiButton v-if="item.status === 'Failed' || (item.status === 'Completed' && !item.indexed)" class="text-warning" :title="item.status === 'Completed' ? $t('download.actions.retryIndex') : $t('download.actions.retry')" @click="downloadStore.retryTask(item.id)" variant="ghost" icon-only :aria-label="item.status === 'Completed' ? $t('download.actions.retryIndex') : $t('download.actions.retry')">
                <Icon  :size="16" name="outline-25" />
              </UiButton>

              <!-- Cancel / Delete -->
              <UiButton class="text-danger" :title="$t('download.actions.delete')" @click="downloadStore.cancelTask(item.id)" variant="danger" icon-only :aria-label="$t('download.actions.delete')">
                <Icon  :size="16" name="outline-26" />
              </UiButton>
            </div>
          </div>

          <div v-if="item.status === 'Completed' && item.warning_msg" class="warning-msg-bar text-xs" role="status">
            <strong>{{ $t('download.card.savedWarning') }}:</strong>
            <span>{{ item.warning_msg }}</span>
          </div>
          <div v-if="item.status === 'Downloading' && item.error_msg" class="warning-msg-bar text-xs" role="status">{{ item.error_msg }}</div>
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
  </div></div>
</template>

<script setup lang="ts">
import UiChip from '@/components/ui/UiChip.vue'
import PageHeader from '@/components/layout/PageHeader.vue'
import Icon from '@/components/ui/Icon.vue'
import UiButton from '@/components/ui/UiButton.vue'
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
.warning-msg-bar { display: flex; gap: 8px; padding: 10px 16px; color: var(--color-warning); background: var(--color-warning-bg); border-top: 1px solid var(--color-warning-bg); overflow-wrap: anywhere; user-select: text; }

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

.filter-tabs {
  display: flex;
  gap: var(--space-2);
}

.tab-badge {
  font-variant-numeric: tabular-nums;
  background: var(--border-strong);
  color: var(--text-secondary);
  padding: 1px 6px;
  border-radius: var(--radius-full);
  font-size: var(--font-size-xs);
  transition: background var(--transition-fast);
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

/* ── Tasks List ────────────────────────────────────────────── */
.list-container {
  display: flex;
  flex-direction: column;
  overflow: hidden;
  border: none;
  border-radius: var(--radius-lg);
  background: var(--glass-bg);
  backdrop-filter: var(--glass-blur);
  -webkit-backdrop-filter: var(--glass-blur);
  box-shadow: none;
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
  border: none;
  border-radius: var(--radius-md);
  background: var(--bg-subtle);
  transition: border-color var(--transition-fast), background var(--transition-fast);
}

.task-card-wrapper:hover {
  border-color: var(--accent-muted);
  background: var(--bg-subtle);
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
  background: var(--border-default);
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
  font-size: var(--text-xs);
  font-weight: var(--font-weight-semibold);
  text-transform: uppercase;
}

.status-badge.pending {
  background: var(--accent-muted);
  color: var(--type-appearance);
}

.status-badge.downloading {
  background: var(--color-info-bg);
  color: var(--color-info);
}

.status-badge.paused {
  background: var(--color-warning-bg);
  color: var(--color-warning);
}

.status-badge.completed {
  background: var(--color-success-bg);
  color: var(--color-success);
}

.status-badge.failed {
  background: var(--color-error-bg);
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
  background: var(--bg-subtle);
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
  background: var(--bg-elevated);
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

/* Error bar */
.error-msg-bar {
  padding: var(--space-2) var(--space-4);
  border-top: 1px solid var(--color-error-bg);
  background: var(--color-error-bg);
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

.b2-page { display: flex; flex-direction: column; height: 100%; min-height: 0; max-width: none; margin: 0; padding: 0; gap: 0; overflow: hidden; }
.b2-page-body { flex: 1; min-height: 0; min-width: 0; overflow: auto; padding: 20px 24px; display: flex; flex-direction: column; gap: 20px; }
.b2-page :deep(.page-header) { width: 100%; }
@media(max-width: 800px) { .b2-page-body { padding: 12px; } }
.task-card-wrapper { background: var(--bg-elevated); border: none; border-radius: var(--radius-card); box-shadow: none; }.task-card { padding: 14px 16px; }.task-icon { width: 40px; height: 40px; border-radius: var(--radius-sm); }.progress-track { height: 4px; border-radius: var(--radius-progress); background: var(--bg-active); }.progress-fill { background: var(--accent-primary); border-radius: var(--radius-progress); }
</style>
