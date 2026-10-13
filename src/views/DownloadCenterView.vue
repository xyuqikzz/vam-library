<template>
  <div class="download-center-view animate-fadeIn b2-page">
    <PageHeader :title="$t('sidebar.downloadCenter')" :subtitle="targetLabel">
      <template #actions>
        <UiButton icon="pause" :disabled="!hasActiveTasks" @click="downloadStore.pauseAll">
          {{ $t('download.actions.pauseAll') }}
        </UiButton>
        <UiButton icon="play" :disabled="!hasPausedTasks" @click="downloadStore.resumeAll">
          {{ $t('download.actions.resumeAll') }}
        </UiButton>
        <UiButton
          :disabled="downloadStore.completedCount === 0"
          :tooltip="$t('download.actions.clearCompletedHint')"
          @click="downloadStore.clearCompleted"
        >
          {{ $t('download.actions.clearCompleted') }}
        </UiButton>
      </template>
      <template #filters>
        <div class="filter-tabs chip-row">
          <UiChip :count="activeQueue.length" :selected="activeTab === 'active'" @click="activeTab = 'active'">
            {{ $t('download.tabs.active') }}
          </UiChip>
          <UiChip :count="downloadStore.completedCount" :selected="activeTab === 'completed'" @click="activeTab = 'completed'">
            {{ $t('download.tabs.completed') }}
          </UiChip>
          <UiChip :count="downloadStore.queue.length" :selected="activeTab === 'all'" @click="activeTab = 'all'">
            {{ $t('download.tabs.all') }}
          </UiChip>
        </div>
      </template>
    </PageHeader>
    <div class="b2-page-body">
      <section class="summary-grid">
        <StatCard
          :title="$t('download.stats.total')"
          :value="downloadStore.queue.length"
          :subtitle="$t('download.stats.pendingNote', { count: downloadStore.pendingDownloads.length })"
          icon="M9 5H7a2 2 0 00-2 2v12a2 2 0 002 2h10a2 2 0 002-2V7a2 2 0 00-2-2h-2M9 5a2 2 0 002 2h2a2 2 0 002-2M9 5a2 2 0 012-2h2a2 2 0 012 2m-3 7h3m-3 4h3m-6-4h.01M9 16h.01"
          color="var(--accent-primary)"
        />
        <StatCard
          :title="$t('download.stats.downloading')"
          :value="downloadStore.activeDownloads.length"
          :subtitle="$t('download.stats.completedNote', { count: downloadStore.completedCount })"
          icon="M7 16a4 4 0 01-.88-7.903A5 5 0 1115.9 6L16 6a5 5 0 011 9.9M9 19l3 3m0 0l3-3m-3 3V10"
          color="var(--color-info)"
        />
        <StatCard
          :title="$t('download.stats.speed')"
          :value="formatSpeed(downloadStore.totalSpeed)"
          :subtitle="$t(downloadStore.activeDownloads.length > 0 ? 'download.stats.speedActive' : 'download.stats.speedIdle')"
          icon="M13 10V3L4 14h7v7l9-11h-7z"
          color="var(--color-success)"
        />
        <StatCard
          :title="$t('download.stats.savedSpace')"
          :value="formatSize(downloadStore.totalWastedBytesSaved)"
          :subtitle="$t(downloadStore.totalWastedBytesSaved > 0 ? 'download.stats.savedNote' : 'download.stats.savedNone')"
          icon="M5 3v4M3 5h4M6 17v4m-2-2h4m5-16l2.286 6.857L21 12l-5.714 2.286L13 21l-2.286-5.714L5 13l5.714-2.286L13 3z"
          color="var(--accent-primary)"
        />
      </section>

      <section class="list-container">
        <div v-if="filteredQueue.length > 0" class="task-list">
          <!-- 任务行只展示状态，不可点击，所以没有悬停底色 -->
          <article
            v-for="item in filteredQueue"
            :key="item.id"
            class="task"
            :class="`is-${item.status.toLowerCase()}`"
          >
            <div class="task-main">
              <div class="task-info">
                <div class="task-title-row">
                  <h4 class="filename" :title="item.filename">{{ item.filename }}</h4>
                  <span class="status-tag" :class="item.status.toLowerCase()">
                    {{ $t(`download.card.${item.status.toLowerCase()}`) }}
                  </span>
                </div>
                <p class="task-sub" :title="item.final_path || ''">
                  <span>{{ item.creator || 'Hub' }} · v{{ item.version }}</span>
                  <span class="task-location">{{ locationLabel(item) }}</span>
                </p>
              </div>

              <div v-if="item.status !== 'Completed'" class="task-progress">
                <div class="progress-info">
                  <span>{{ formatSize(item.downloaded_bytes) }} / {{ formatSize(item.total_bytes) }}</span>
                  <span v-if="item.status === 'Downloading' && item.speed_bytes_per_sec > 0" class="speed">
                    {{ formatSpeed(item.speed_bytes_per_sec) }}
                  </span>
                  <span class="progress-percent">{{ item.progress.toFixed(0) }}%</span>
                </div>
                <div class="progress-track">
                  <div class="progress-fill" :style="{ width: `${item.progress}%` }" />
                </div>
              </div>
              <div v-else class="task-done">
                <span>{{ formatSize(item.total_bytes) }}</span>
                <span :class="{ 'not-indexed': !item.indexed }">
                  {{ item.indexed ? $t('download.card.indexed') : $t('download.card.notIndexed') }}
                </span>
              </div>

              <div class="task-controls">
                <UiButton
                  v-if="item.status === 'Downloading' || item.status === 'Pending'"
                  size="sm" variant="ghost" icon="pause" icon-only
                  :aria-label="$t('download.actions.pause')"
                  @click="downloadStore.pauseTask(item.id)"
                />
                <UiButton
                  v-if="item.status === 'Paused'"
                  size="sm" variant="ghost" icon="play" icon-only
                  :aria-label="$t('download.actions.resume')"
                  @click="downloadStore.resumeTask(item.id)"
                />
                <UiButton
                  v-if="item.status === 'Failed' || (item.status === 'Completed' && !item.indexed)"
                  size="sm" variant="ghost" icon="refresh" icon-only
                  :aria-label="item.status === 'Completed' ? $t('download.actions.retryIndex') : $t('download.actions.retry')"
                  @click="downloadStore.retryTask(item.id)"
                />
                <UiButton
                  v-if="item.status === 'Completed' && item.final_path"
                  size="sm" variant="ghost" icon="folder" icon-only
                  :aria-label="$t('download.actions.reveal')"
                  @click="reveal(item.final_path)"
                />
                <UiButton
                  size="sm" variant="ghost" icon="close" icon-only
                  :aria-label="item.status === 'Completed' || item.status === 'Failed' ? $t('download.actions.remove') : $t('download.actions.cancel')"
                  @click="downloadStore.cancelTask(item.id)"
                />
              </div>
            </div>

            <div v-if="noticeOf(item)" class="task-notice" :class="noticeOf(item)!.tone" role="status">
              <Icon name="warning" :size="14" />
              <span class="notice-text">{{ noticeOf(item)!.text }}</span>
              <details v-if="noticeOf(item)!.detail" class="notice-detail">
                <summary>{{ $t('download.card.details') }}</summary>
                <p>{{ noticeOf(item)!.detail }}</p>
              </details>
            </div>
          </article>
        </div>

        <EmptyState
          v-else
          icon="M12 16.5V9.75m0 0l3 3m-3-3l-3 3M6.75 19.5h10.5a2.25 2.25 0 002.25-2.25V6.75A2.25 2.25 0 0017.25 4.5H6.75A2.25 2.25 0 004.5 6.75v10.5a2.25 2.25 0 002.25 2.25z"
          :title="$t('download.empty')"
          :description="$t('download.emptyDesc')"
        >
          <template #action>
            <UiButton variant="primary" @click="router.push('/online')">{{ $t('download.browseOnline') }}</UiButton>
          </template>
        </EmptyState>
      </section>
    </div>
  </div>
</template>

<script setup lang="ts">
import UiChip from '@/components/ui/UiChip.vue'
import PageHeader from '@/components/layout/PageHeader.vue'
import Icon from '@/components/ui/Icon.vue'
import UiButton from '@/components/ui/UiButton.vue'
import { ref, computed } from 'vue'
import { useRouter } from 'vue-router'
import { useI18n } from 'vue-i18n'
import { useDownloadStore, type DownloadItem } from '@/stores/download'
import { useAppStore } from '@/stores/app'
import { useNotification } from '@/composables/useNotification'
import StatCard from '@/components/common/StatCard.vue'
import EmptyState from '@/components/common/EmptyState.vue'

const downloadStore = useDownloadStore()
const appStore = useAppStore()
const router = useRouter()
const notify = useNotification()
const { t } = useI18n()
const activeTab = ref<'all' | 'active' | 'completed'>('active')

const targetLabel = computed(() => {
  const context = appStore.installContext
  if (!context) return t('download.targetUnset')
  return t('download.targetTo', { name: context.isManaged ? t('download.managedLibrary') : 'AddonPackages' })
})

const hasActiveTasks = computed(() =>
  downloadStore.queue.some(i => i.status === 'Downloading' || i.status === 'Pending'),
)
const hasPausedTasks = computed(() =>
  downloadStore.queue.some(i => i.status === 'Paused' || i.status === 'Failed'),
)
// 「下载中」标签包含排队、暂停和失败的任务，计数与列表保持一致。
const activeQueue = computed(() => downloadStore.queue.filter(i => i.status !== 'Completed'))
const filteredQueue = computed(() => {
  switch (activeTab.value) {
    case 'active':
      return activeQueue.value
    case 'completed':
      return downloadStore.queue.filter(i => i.status === 'Completed')
    default:
      return downloadStore.queue
  }
})

function locationLabel(item: DownloadItem): string {
  if (item.status === 'Completed' && item.final_path) return item.final_path
  if (item.install_mode === 'managed_library') return t('download.card.toManaged')
  if (item.install_mode === 'real_addon') return t('download.card.toAddon')
  return t('download.card.waiting')
}

// 后端给出的警告是技术原文；常见情况换成一句用户能看懂的话，原文放在「详情」里。
function noticeOf(item: DownloadItem): { tone: 'warning' | 'error'; text: string; detail?: string } | null {
  if (item.status === 'Completed' && item.warning_msg) {
    const known = /meta\.json/i.test(item.warning_msg)
    return { tone: 'warning', text: known ? t('download.card.metaWarning') : item.warning_msg, detail: known ? item.warning_msg : undefined }
  }
  if (item.status === 'Downloading' && item.error_msg) return { tone: 'warning', text: item.error_msg }
  if (item.status === 'Failed' && item.error_msg) return { tone: 'error', text: t('download.card.failedHint'), detail: item.error_msg }
  return null
}

async function reveal(path: string) {
  try {
    const { revealItemInDir } = await import('@tauri-apps/plugin-opener')
    await revealItemInDir(path)
  } catch (error) {
    notify.error(String(error))
  }
}

function formatSize(bytes: number): string {
  if (!bytes || bytes === 0) return '0 B'
  const units = ['B', 'KB', 'MB', 'GB', 'TB']
  const i = Math.floor(Math.log(bytes) / Math.log(1024))
  return `${(bytes / Math.pow(1024, i)).toFixed(1)} ${units[i]}`
}

function formatSpeed(bytesPerSec: number): string {
  if (!bytesPerSec || bytesPerSec === 0) return '0 KB/s'
  const kbps = bytesPerSec / 1024
  if (kbps < 1024) return `${kbps.toFixed(1)} KB/s`
  return `${(kbps / 1024).toFixed(1)} MB/s`
}
</script>

<style scoped>
.download-center-view {
  display: flex;
  flex-direction: column;
}

.summary-grid {
  display: grid;
  grid-template-columns: repeat(4, 1fr);
  gap: var(--space-grid);
}

@media (max-width: 1200px) {
  .summary-grid {
    grid-template-columns: repeat(2, 1fr);
  }
}

.list-container {
  display: flex;
  flex-direction: column;
  min-height: 0;
}

.task-list {
  display: flex;
  flex-direction: column;
  gap: var(--space-item);
}

/* 每个任务是一张卡片：底色比页面亮一级，提示条在卡片内部、带圆角，不再通栏 */
.task {
  display: flex;
  flex-direction: column;
  border-radius: var(--radius-card);
  background: var(--bg-elevated);
}

.task-main {
  display: grid;
  grid-template-columns: minmax(0, 2fr) minmax(0, 3fr) 112px;
  align-items: center;
  gap: var(--space-6);
  padding: var(--space-4) var(--space-4) var(--space-4) var(--space-5);
}

.task-main:has(+ .task-notice) {
  padding-bottom: var(--space-3);
}

.task-info {
  display: flex;
  flex-direction: column;
  gap: 2px;
  min-width: 0;
}

.task-title-row {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  min-width: 0;
}

.filename {
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  color: var(--text-primary);
  font-size: var(--text-base);
  font-weight: var(--font-bold);
}

.status-tag {
  flex-shrink: 0;
  display: inline-flex;
  align-items: center;
  min-height: 20px;
  padding: 0 var(--space-2);
  border-radius: var(--radius-full);
  font-size: var(--text-xs);
  font-weight: var(--font-bold);
}

.status-tag.pending {
  background: var(--bg-control);
  color: var(--text-secondary);
}

.status-tag.downloading {
  background: var(--color-info-bg);
  color: var(--color-info);
}

.status-tag.paused {
  background: var(--color-warning-bg);
  color: var(--color-warning);
}

.status-tag.completed {
  background: var(--color-success-bg);
  color: var(--color-success);
}

.status-tag.failed {
  background: var(--color-error-bg);
  color: var(--color-error);
}

.task-sub {
  display: flex;
  gap: var(--space-2);
  min-width: 0;
  color: var(--text-tertiary);
  font-size: var(--text-xs);
}

.task-sub span {
  flex-shrink: 0;
}

.task-sub .task-location {
  flex-shrink: 1;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.task-progress {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
  min-width: 0;
}

.progress-info {
  display: flex;
  align-items: center;
  gap: var(--space-3);
  color: var(--text-secondary);
  font-size: var(--text-xs);
  font-variant-numeric: tabular-nums;
}

.progress-info .speed {
  color: var(--text-secondary);
}

.progress-percent {
  margin-left: auto;
}

.progress-track {
  height: 4px;
  border-radius: var(--radius-full);
  background: var(--bg-active);
  overflow: hidden;
}

.progress-fill {
  height: 100%;
  border-radius: var(--radius-full);
  background: var(--accent-primary);
  transition: width 0.3s ease;
}

.is-pending .progress-fill {
  background: var(--text-disabled);
}

.is-paused .progress-fill {
  background: var(--color-warning);
}

.is-failed .progress-fill {
  background: var(--color-error);
}

.task-done {
  display: flex;
  gap: var(--space-3);
  color: var(--text-secondary);
  font-size: var(--text-sm);
  font-variant-numeric: tabular-nums;
}

.task-done .not-indexed {
  color: var(--color-warning);
}

.task-controls {
  display: flex;
  justify-content: flex-end;
  gap: var(--space-1);
}

.task-notice {
  display: flex;
  align-items: flex-start;
  flex-wrap: wrap;
  gap: var(--space-2);
  margin: 0 var(--space-4) var(--space-4) var(--space-5);
  padding: var(--space-2) var(--space-3);
  border-radius: var(--radius-sm);
  color: var(--text-secondary);
  font-size: var(--text-sm);
  line-height: 1.5;
  overflow-wrap: anywhere;
}

.task-notice > svg {
  flex-shrink: 0;
  margin-top: 3px;
}

.notice-text {
  flex: 1;
  min-width: 0;
}

.task-notice.warning {
  background: var(--color-warning-bg);
}

.task-notice.warning > svg {
  color: var(--color-warning);
}

.task-notice.error {
  background: var(--color-error-bg);
}

.task-notice.error > svg {
  color: var(--color-error);
}

.notice-detail {
  flex-basis: 100%;
  padding-left: 22px;
  color: var(--text-secondary);
  font-size: var(--text-xs);
  user-select: text;
}

.notice-detail summary {
  width: fit-content;
  cursor: pointer;
}

.notice-detail summary:hover {
  color: var(--text-primary);
}

.notice-detail p {
  margin-top: var(--space-1);
}

@media (max-width: 900px) {
  .task-main {
    grid-template-columns: minmax(0, 1fr) 112px;
  }
  .task-progress,
  .task-done {
    grid-column: 1 / -1;
    grid-row: 2;
  }
}
</style>
