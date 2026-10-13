<template>
  <section v-if="loading || batches.length || error" ref="section" class="batch-section">
    <header class="section-head">
      <h2>{{ t('cleanup.batches') }}</h2>
      <span v-if="batches.length" class="section-count">{{ batches.length }}</span>
    </header>
    <p class="section-desc">{{ t('cleanup.batchesDesc') }}</p>
    <p v-if="error" class="error" role="alert">{{ error }}</p>
    <p v-if="loading && !batches.length" class="muted" role="status">{{ t('common.loading') }}</p>
    <div class="batch-list">
      <details
        v-for="batch in batches"
        :key="batch.id"
        class="batch"
        :data-batch-id="batch.id"
        :open="expandedIds.has(batch.id)"
        @toggle="expanded($event, batch.id)"
      >
        <summary>
          <Icon class="batch-chevron" name="chevron-down" :size="16" />
          <span class="batch-title">{{ date(batch.created_at) }}</span>
          <span class="status-tag" :class="statusTone(batch.status)">
            {{ t(`cleanup.status.${batch.status}`) }}
          </span>
          <span class="batch-meta">
            {{ t('cleanup.batchSize', { count: remaining(batch).length, size: formatSize(remaining(batch).reduce((sum, f) => sum + f.original.size, 0)) }) }}
          </span>
          <span class="batch-expiry" :class="{ overdue: expired(batch) }">
            {{ expired(batch) ? t('cleanup.overdue') : t('cleanup.expiry', { date: day(batch.expires_at) }) }}
          </span>
        </summary>
        <div class="batch-body">
          <p v-for="message in batch.errors" :key="message" class="error">{{ message }}</p>
          <ul v-if="expandedIds.has(batch.id)" class="batch-files">
            <li
              v-for="(file, index) in batch.files.slice(
                ((pages[batch.id] || 1) - 1) * 100,
                (pages[batch.id] || 1) * 100,
              )"
              :key="index"
            >
              <span class="file-name" :title="file.original.path">{{ file.package_id }}</span>
              <span class="file-size">{{ formatSize(file.original.size) }}</span>
              <span class="file-status">{{ t(`cleanup.fileStatus.${file.status}`) }}</span>
            </li>
          </ul>
          <div v-if="batch.files.length > 100" class="batch-pager">
            <UiButton
              size="sm"
              variant="ghost"
              icon="chevron-left"
              icon-only
              :aria-label="t('cleanup.previousPage')"
              :disabled="(pages[batch.id] || 1) <= 1"
              @click="pages[batch.id] = (pages[batch.id] || 1) - 1"
            />
            <span>{{ pages[batch.id] || 1 }} / {{ Math.ceil(batch.files.length / 100) }}</span>
            <UiButton
              size="sm"
              variant="ghost"
              icon="chevron-right"
              icon-only
              :aria-label="t('cleanup.nextPage')"
              :disabled="(pages[batch.id] || 1) * 100 >= batch.files.length"
              @click="pages[batch.id] = (pages[batch.id] || 1) + 1"
            />
          </div>
          <div v-if="activeBatchId === batch.id" class="batch-progress" role="status">
            <span>{{ t('cleanup.purging') }} {{ progressText }}</span>
            <UiProgress :value="progress?.current || 0" :max="progress?.total || 1" :indeterminate="!progress?.total" :label="t('cleanup.purging')" />
          </div>
          <div class="batch-actions">
            <UiButton
              size="sm"
              :disabled="busy || batch.status === 'purged' || batch.status === 'restored'"
              @click="operate('restore_resource_cleanup_batch', batch)"
            >
              {{ t('cleanup.restore') }}
            </UiButton>
            <UiButton
              size="sm"
              variant="ghost"
              :disabled="busy || batch.status === 'purged'"
              @click="operate('extend_resource_cleanup_batch', batch)"
            >
              {{ t('cleanup.extend') }}
            </UiButton>
            <UiButton
              size="sm"
              variant="danger"
              :disabled="busy || !['completed', 'purging'].includes(batch.status)"
              @click="purge(batch)"
            >
              {{ t('cleanup.purge') }}
            </UiButton>
          </div>
        </div>
      </details>
    </div>
  </section>
</template>
<script setup lang="ts">
import UiButton from '@/components/ui/UiButton.vue'
import Icon from '@/components/ui/Icon.vue'
import UiProgress from '@/components/ui/UiProgress.vue'
import { computed, nextTick, onMounted, onUnmounted, ref, watch } from 'vue'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import { invoke } from '@tauri-apps/api/core'
import { useI18n } from 'vue-i18n'
import { useAppStore } from '@/stores/app'
import { useCleanupStore } from '@/stores/cleanup'
import { useLocalLibraryStore } from '@/stores/localLibrary'
import { useConfirm } from '@/composables/useConfirm'
import { formatSize } from '@/utils/bytes'
import type { CleanupBatch, CleanupProgress } from '@/types/cleanup'
// 只展示「资源清理」产生的批次记录；保留天数、自动删除等策略在设置页的「回收站」里。
const app = useAppStore(),
  cleanup = useCleanupStore(),
  library = useLocalLibraryStore()
const confirm = useConfirm()
const emit = defineEmits<{ totals: [value: { count: number; bytes: number; batches: number }] }>()
const { t, locale } = useI18n()
const batches = ref<CleanupBatch[]>([])
const expandedIds = ref(new Set<string>())
const pages = ref<Record<string, number>>({})
function expanded(event: Event, id: string) {
  const next = new Set(expandedIds.value)
  if ((event.target as HTMLDetailsElement).open) next.add(id)
  else next.delete(id)
  expandedIds.value = next
}
const totals = computed(() => {
  const files = batches.value.flatMap((b) => b.files.filter((f) => f.status === 'trashed'))
  return {
    count: files.length,
    bytes: files.reduce((sum, f) => sum + f.original.size, 0),
    batches: batches.value.length,
  }
})
watch(totals, (value) => emit('totals', value), { immediate: true })
const loading = ref(false),
  busy = ref(false),
  error = ref('')
let epoch = 0
const section = ref<HTMLElement | null>(null)
const activeBatchId = ref('')
const progress = ref<CleanupProgress | null>(null)
const progressText = computed(() => {
  if (!progress.value?.total) return ''
  const { stage, current, total } = progress.value
  return stage.endsWith('_bytes')
    ? t('cleanup.verifiedBytes', { current: formatSize(current), total: formatSize(total) })
    : `${current} / ${total}`
})
let unlisten: UnlistenFn | undefined
let disposed = false
async function focusBatch() {
  const id = cleanup.focusedBatchId
  if (!id || !batches.value.some(batch => batch.id === id)) return
  expandedIds.value = new Set([...expandedIds.value, id])
  await nextTick()
  const target = [...(section.value?.querySelectorAll<HTMLElement>('.batch') || [])]
    .find(element => element.dataset.batchId === id)
  target?.scrollIntoView?.({ block: 'nearest' })
}
watch(() => cleanup.latestBatch, batch => {
  if (!batch) return
  // A list request started before cleanup completed must not hide the new record.
  epoch++
  loading.value = false
  batches.value = [batch, ...batches.value.filter(item => item.id !== batch.id)]
  void focusBatch()
}, { immediate: true })
watch(() => cleanup.focusedBatchId, focusBatch)
const date = (value: string) =>
  new Date(value).toLocaleString(locale.value, { dateStyle: 'medium', timeStyle: 'short' })
const day = (value: string) => new Date(value).toLocaleDateString(locale.value, { dateStyle: 'medium' })
const remaining = (batch: CleanupBatch) =>
  batch.files.filter((f) => !['restored', 'purged'].includes(f.status))
const expired = (batch: CleanupBatch) => Date.parse(batch.expires_at) < Date.now()
function statusTone(status: string) {
  if (status === 'completed') return 'neutral'
  if (status === 'restored' || status === 'rolled_back') return 'success'
  if (status === 'purged') return 'muted'
  return 'warning'
}
async function load() {
  const token = ++epoch
  error.value = ''
  if (!app.vamRootPath) {
    batches.value = []
    return
  }
  loading.value = true
  try {
    const items = await invoke<CleanupBatch[]>('list_resource_cleanup_batches', {
      vamRoot: app.vamRootPath,
    })
    if (token === epoch) {
      batches.value = items
      await focusBatch()
    }
  } catch (e) {
    if (token === epoch) error.value = String(e)
  } finally {
    if (token === epoch) loading.value = false
  }
}
async function operate(command: string, batch: CleanupBatch) {
  if (busy.value) return
  busy.value = true
  error.value = ''
  const root = app.vamRootPath
  if (command === 'delete_resource_cleanup_batch') activeBatchId.value = batch.id
  progress.value = null
  try {
    const result = await invoke<CleanupBatch>(command, {
      vamRoot: app.vamRootPath,
      batchId: batch.id,
      days: 30,
    })
    if (root === app.vamRootPath) {
      epoch++
      loading.value = false
      if (result.errors.length)
        error.value = `${t('cleanup.restoreConflict')} ${result.errors.join('; ')}`
      batches.value = batches.value.map((b) => (b.id === result.id ? result : b))
      if (cleanup.latestBatch?.id === result.id) cleanup.recordBatch(result)
      if (command === 'restore_resource_cleanup_batch')
        await Promise.all([cleanup.load(), library.refreshAll('refreshing')])
    }
  } catch (e) {
    if (root === app.vamRootPath) {
      await load()
      error.value = String(e)
    }
  } finally {
    activeBatchId.value = ''
    progress.value = null
    busy.value = false
  }
}
async function purge(batch: CleanupBatch) {
  const ok = await confirm({
    title: t('cleanup.purge'),
    message: t('cleanup.purgeConfirm'),
    confirmText: t('cleanup.purge'),
    danger: true,
  })
  if (ok && batches.value.some(item => item.id === batch.id)) await operate('delete_resource_cleanup_batch', batch)
}
watch(() => app.vamRootPath, () => {
  batches.value = []
  expandedIds.value = new Set()
  pages.value = {}
  void load()
})
onMounted(() => {
  void load()
  void listen<CleanupProgress>('cleanup-progress', event => {
    if (event.payload.request_id === activeBatchId.value) progress.value = event.payload
  }).then(stop => { if (disposed) stop(); else unlisten = stop }).catch(() => { /* Deletion remains available without progress events. */ })
})
onUnmounted(() => { disposed = true; epoch++; unlisten?.() })
defineExpose({ load })
</script>
<style scoped>
.batch-section {
  display: flex;
  flex-direction: column;
  gap: var(--space-stack);
  padding: var(--space-panel);
  border-radius: var(--radius-lg);
  background: var(--bg-elevated);
}
.section-head {
  display: flex;
  align-items: center;
  gap: var(--space-2);
}
.section-head h2 {
  margin: 0;
  font-size: var(--text-lg);
  font-weight: var(--font-heavy);
}
.section-count {
  display: inline-flex;
  align-items: center;
  min-height: 22px;
  padding: 0 var(--space-2);
  border-radius: var(--radius-full);
  background: var(--bg-control);
  color: var(--text-secondary);
  font-size: var(--text-xs);
  font-weight: var(--font-bold);
}
.section-desc,
.muted {
  margin-top: calc(-1 * var(--space-1));
  color: var(--text-tertiary);
  font-size: var(--text-sm);
}
.error {
  color: var(--color-error);
  font-size: var(--text-sm);
  overflow-wrap: anywhere;
}
.batch-list {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
}
.batch {
  border-radius: var(--radius-md);
  background: var(--bg-input);
}
.batch summary {
  width: auto;
  display: flex;
  align-items: center;
  gap: var(--space-3);
  min-height: 52px;
  padding: var(--space-2) var(--space-4);
  border-radius: var(--radius-md);
  cursor: pointer;
  list-style: none;
  transition: background var(--duration-fast) var(--ease);
}
.batch summary::-webkit-details-marker,
.batch summary::before {
  display: none;
}
.batch summary:hover {
  background: var(--bg-control);
}
.batch-chevron {
  flex-shrink: 0;
  color: var(--text-tertiary);
  transform: rotate(-90deg);
  transition: transform var(--duration-fast) var(--ease);
}
.batch[open] .batch-chevron {
  transform: none;
}
.batch-title {
  font-size: var(--text-base);
  font-weight: var(--font-bold);
  white-space: nowrap;
}
.status-tag {
  display: inline-flex;
  align-items: center;
  min-height: 22px;
  padding: 0 var(--space-2);
  border-radius: var(--radius-full);
  font-size: var(--text-xs);
  font-weight: var(--font-bold);
  white-space: nowrap;
}
.status-tag.neutral {
  background: var(--bg-control);
  color: var(--text-secondary);
}
.status-tag.success {
  background: var(--color-success-bg);
  color: var(--color-success);
}
.status-tag.warning {
  background: var(--color-warning-bg);
  color: var(--color-warning);
}
.status-tag.muted {
  background: var(--bg-control);
  color: var(--text-tertiary);
}
.batch-meta {
  color: var(--text-secondary);
  font-size: var(--text-sm);
  font-variant-numeric: tabular-nums;
  white-space: nowrap;
}
.batch-expiry {
  margin-left: auto;
  color: var(--text-tertiary);
  font-size: var(--text-sm);
  white-space: nowrap;
}
.batch-expiry.overdue {
  color: var(--color-warning);
}
.batch-body {
  display: flex;
  flex-direction: column;
  gap: var(--space-3);
  padding: 0 var(--space-4) var(--space-4) calc(var(--space-4) + 16px + var(--space-3));
}
.batch-files {
  max-height: 280px;
  overflow: auto;
  display: flex;
  flex-direction: column;
}
.batch-files li {
  display: grid;
  grid-template-columns: minmax(0, 1fr) 96px 96px;
  align-items: center;
  gap: var(--space-3);
  min-height: 32px;
  font-size: var(--text-sm);
}
.batch-files li + li {
  border-top: 1px solid var(--border-subtle);
}
.file-name {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  user-select: text;
}
.file-size,
.file-status {
  color: var(--text-tertiary);
  font-variant-numeric: tabular-nums;
  text-align: right;
}
.batch-pager {
  display: flex;
  align-items: center;
  gap: var(--space-1);
  color: var(--text-secondary);
  font-size: var(--text-sm);
}
.batch-progress {
  display: grid;
  gap: var(--space-item);
  color: var(--text-secondary);
  font-size: var(--text-sm);
}
.batch-actions {
  display: flex;
  flex-wrap: wrap;
  gap: var(--space-2);
}
</style>
