<template>
  <div class="trash-view animate-fadeIn b2-page">
    <PageHeader :title="$t('sidebar.trash')" :subtitle="summaryText">
      <template #actions>
        <UiButton :disabled="isLoading" icon="refresh" @click="refreshAll">
          {{ $t('common.refresh') }}
        </UiButton>
      </template>
    </PageHeader>
    <div class="b2-page-body">
      <EmptyState
        v-if="isEmpty"
        :icon="icons.emptyTrash"
        :title="$t('trash.emptyTitle')"
        :description="$t('trash.emptyDesc')"
        hide-action
      />

      <CleanupBatchPanel ref="batchPanel" @totals="batchTotals = $event" />

      <section v-if="trashList.length > 0" class="trash-section">
        <header class="section-head">
          <h2>{{ $t('trash.listTitle') }}</h2>
          <span class="section-count">{{ trashList.length }}</span>
          <UiButton
            class="section-action"
            size="sm"
            variant="danger"
            :disabled="isOperating"
            @click="confirmEmptyTrash"
          >
            {{ $t('trash.emptyBtn') }}
          </UiButton>
        </header>
        <p class="section-desc">{{ $t('trash.listDesc') }}</p>

        <div class="trash-table-wrap">
          <table class="trash-table">
            <thead>
              <tr>
                <th class="col-package">{{ $t('trash.colPackage') }}</th>
                <th class="col-size">{{ $t('packages.size') }}</th>
                <th class="col-time">{{ $t('trash.deletedTime') }}</th>
                <th class="col-actions"><span class="sr-only">{{ $t('onDemand.action') }}</span></th>
              </tr>
            </thead>
            <tbody>
              <tr v-for="item in trashList" :key="item.id" class="trash-row">
                <td class="col-package">
                  <span class="package-name" :title="fileName(item.original_path)">
                    {{ fileName(item.original_path) }}
                  </span>
                  <span class="package-path" :title="item.original_path">
                    {{ item.original_path }}
                  </span>
                </td>
                <td class="col-size">{{ formatSize(item.size_bytes) }}</td>
                <td class="col-time">
                  <time :datetime="item.created_at">{{ formatDate(item.created_at) }}</time>
                </td>
                <td class="col-actions">
                  <div class="action-buttons">
                    <UiButton size="sm" :disabled="isOperating" @click="handleRestore(item)">
                      {{ $t('trash.restore') }}
                    </UiButton>
                    <UiButton
                      size="sm"
                      variant="ghost"
                      class="delete-forever"
                      :disabled="isOperating"
                      @click="confirmDelete(item)"
                    >
                      {{ $t('trash.deleteForever') }}
                    </UiButton>
                  </div>
                </td>
              </tr>
            </tbody>
          </table>
        </div>
      </section>
    </div>
  </div>
</template>

<script setup lang="ts">
import PageHeader from '@/components/layout/PageHeader.vue'
import UiButton from '@/components/ui/UiButton.vue'
import EmptyState from '@/components/common/EmptyState.vue'
import CleanupBatchPanel from '@/components/CleanupBatchPanel.vue'
import { formatSize } from '@/utils/bytes'
import { ref, computed, onMounted, onActivated } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { useI18n } from 'vue-i18n'
import { useNotification } from '@/composables/useNotification'
import { useConfirm } from '@/composables/useConfirm'

interface CleanupTrashEntry {
  id: number
  package_id: string
  original_path: string
  trash_path: string
  size_bytes: number
  created_at: string
}

const { t, locale } = useI18n()
const notify = useNotification()
const confirm = useConfirm()

const trashList = ref<CleanupTrashEntry[]>([])
const batchTotals = ref({ count: 0, bytes: 0, batches: 0 })
const batchPanel = ref<InstanceType<typeof CleanupBatchPanel> | null>(null)
const isLoading = ref(false)
const isOperating = ref(false)
const loaded = ref(false)

const totalCount = computed(() => trashList.value.length + batchTotals.value.count)
const totalSize = computed(() =>
  trashList.value.reduce((sum, item) => sum + item.size_bytes, batchTotals.value.bytes),
)
const isEmpty = computed(
  () => loaded.value && trashList.value.length === 0 && batchTotals.value.batches === 0,
)
const summaryText = computed(() =>
  totalCount.value
    ? t('trash.summary', { count: totalCount.value.toLocaleString(), size: formatSize(totalSize.value) })
    : t('trash.subtitle'),
)

onMounted(loadTrash)
let firstActivation = true
onActivated(() => {
  if (firstActivation) firstActivation = false
  else void refreshAll()
})

async function loadTrash() {
  if (isLoading.value) return
  isLoading.value = true
  try {
    trashList.value = await invoke<CleanupTrashEntry[]>('list_cleanup_trash')
  } catch (err) {
    notify.error(String(err), t('common.error'))
  } finally {
    isLoading.value = false
    loaded.value = true
  }
}

async function refreshAll() {
  await Promise.all([loadTrash(), batchPanel.value?.load()])
}

async function handleRestore(item: CleanupTrashEntry) {
  if (isOperating.value) return
  isOperating.value = true
  try {
    await invoke('restore_cleanup_trash_item', { trashId: item.id })
    notify.success(t('trash.restoreSuccess', { name: item.package_id }))
    await loadTrash()
  } catch (err) {
    notify.error(String(err), t('common.error'))
  } finally {
    isOperating.value = false
  }
}

async function confirmDelete(item: CleanupTrashEntry) {
  const ok = await confirm({
    title: t('trash.deleteConfirmTitle'),
    message: t('trash.deleteConfirmMsg', { name: fileName(item.original_path) }),
    confirmText: t('trash.deleteForever'),
    danger: true,
  })
  if (ok) await run(async () => {
    await invoke('delete_cleanup_trash_item', { trashId: item.id })
    notify.success(t('trash.deleteSuccess'))
  })
}

async function confirmEmptyTrash() {
  const ok = await confirm({
    title: t('trash.emptyConfirmTitle'),
    message: t('trash.emptyConfirmMsg', { count: trashList.value.length }),
    confirmText: t('trash.emptyBtn'),
    danger: true,
  })
  if (ok) await run(async () => {
    const count = await invoke<number>('empty_cleanup_trash')
    notify.success(t('trash.emptySuccess', { count }))
  })
}

async function run(action: () => Promise<void>) {
  if (isOperating.value) return
  isOperating.value = true
  try {
    await action()
  } catch (err) {
    notify.error(String(err), t('common.error'))
  } finally {
    // A failed batch may have removed other files successfully.
    await loadTrash()
    isOperating.value = false
  }
}

function fileName(path: string) {
  return path.split(/[\\/]/).pop() || path
}

// 后端返回 "YYYY-MM-DD HH:mm:ss"（UTC）或 ISO 字符串，统一按本地时间显示。
function formatDate(value: string): string {
  if (!value) return '—'
  const iso = value.includes('T') ? value : value.replace(' ', 'T')
  const date = new Date(/[zZ]|[+-]\d\d:?\d\d$/.test(iso) ? iso : `${iso}Z`)
  if (Number.isNaN(date.getTime())) return value.slice(0, 16)
  return date.toLocaleString(locale.value, { dateStyle: 'medium', timeStyle: 'short' })
}

const icons = {
  emptyTrash:
    'M19 7l-.867 12.142A2 2 0 0116.138 21H7.862a2 2 0 01-1.995-1.858L5 7M4 7h16m-10 4v6m4-6v6',
}
</script>

<style scoped>
.trash-view {
  display: flex;
  flex-direction: column;
  min-height: 0;
  min-width: 0;
}

.trash-section {
  display: flex;
  flex-direction: column;
  gap: var(--space-stack);
  padding: var(--space-panel);
  border-radius: var(--radius-lg);
  background: var(--bg-elevated);
  min-width: 0;
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

.section-action {
  margin-left: auto;
}

.section-desc {
  margin-top: calc(-1 * var(--space-1));
  color: var(--text-tertiary);
  font-size: var(--text-sm);
}

.trash-table-wrap {
  overflow-x: auto;
}

.trash-table {
  width: 100%;
  min-width: 640px;
  table-layout: fixed;
  border-collapse: collapse;
  text-align: left;
}

.trash-table th,
.trash-table td {
  padding: var(--space-3);
  vertical-align: middle;
}

.trash-table th {
  color: var(--text-tertiary);
  font-size: var(--text-xs);
  font-weight: var(--font-semibold);
  white-space: nowrap;
}

.trash-row td {
  border-top: 1px solid var(--border-subtle);
}

.col-package {
  min-width: 0;
}

.col-size {
  width: 96px;
  text-align: right;
}

.col-time {
  width: 168px;
}

.col-actions {
  width: 184px;
}

td.col-size,
td.col-time {
  color: var(--text-secondary);
  font-size: var(--text-sm);
  font-variant-numeric: tabular-nums;
  white-space: nowrap;
}

.package-name,
.package-path {
  display: block;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.package-name {
  font-size: var(--text-base);
  font-weight: var(--font-semibold);
}

.package-path {
  margin-top: 2px;
  color: var(--text-tertiary);
  font-size: var(--text-xs);
  user-select: text;
}

.action-buttons {
  display: flex;
  justify-content: flex-end;
  gap: var(--space-2);
}

.action-buttons .ui-button.delete-forever {
  color: var(--color-error);
}
</style>
