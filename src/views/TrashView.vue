<template>
  <div class="trash-view animate-fadeIn b2-page">
    <PageHeader :title="$t('sidebar.trash')">
      <template #actions>
        <UiButton :disabled="isLoading" @click="loadTrash" variant="secondary">
          <Icon :class="['refresh-icon', { 'spin': isLoading }]" :size="14" name="outline-52" />
          <span>{{ $t('common.refresh') }}</span>
        </UiButton>

        <UiButton
          v-if="trashList.length > 0"
          :disabled="isOperating"
          @click="confirmEmptyTrash"
          variant="danger"
        >
          <Icon :size="14" name="outline-18" />
          <span>{{ $t('trash.emptyBtn') }}</span>
        </UiButton>
      </template>
    </PageHeader>
    <div class="b2-page-body">
      <section class="trash-toolbar">
        <dl class="summary-grid">
          <div class="summary-item">
            <dt>{{ $t('trash.statsTotal') }}</dt>
            <dd>{{ trashList.length.toLocaleString() }}</dd>
          </div>
          <div class="summary-item">
            <dt>{{ $t('trash.statsSize') }}</dt>
            <dd>{{ formatSize(totalSize) }}</dd>
          </div>
        </dl>
      </section>

      <!-- Trash Content -->
      <div v-if="trashList.length > 0" class="trash-content glass-panel">
        <div class="trash-header">
          <h3 class="trash-header-title">{{ $t('trash.listTitle') }}</h3>
          <span class="trash-header-info text-xs text-tertiary">
            {{ $t('trash.autoPurgeTip') }}
          </span>
        </div>

        <div class="trash-body list-scrollbar">
          <table class="trash-table">
            <thead>
              <tr>
                <th class="col-package">{{ $t('packages.package') }}</th>
                <th class="col-size">{{ $t('packages.size') }}</th>
                <th class="col-time">{{ $t('trash.deletedTime') }}</th>
                <th class="col-remaining">{{ $t('trash.remainingTime') }}</th>
                <th class="col-actions">{{ $t('onDemand.action') }}</th>
              </tr>
            </thead>
            <tbody>
              <tr v-for="item in trashList" :key="item.id" class="trash-row">
                <td class="col-package font-medium text-primary">
                  <div class="package-cell">
                    <span
                      class="truncate package-name"
                      :title="item.original_path.split(/[\\/]/).pop()"
                    >
                      {{ item.original_path.split(/[\\/]/).pop() }}
                    </span>
                    <span
                      class="truncate-path"
                      :title="`${$t('trash.originalPath')}: ${item.original_path}`"
                    >
                      {{ item.original_path }}
                    </span>
                  </div>
                </td>
                <td class="col-size font-numeric text-secondary text-sm">
                  {{ formatSize(item.size_bytes) }}
                </td>
                <td class="col-time text-secondary text-xs">
                  <time :title="formatDate(item.created_at)">
                    <span>{{ formatDate(item.created_at).split(' ')[0] }}</span>
                    <span class="time-detail">{{ formatDate(item.created_at).split(' ')[1] }}</span>
                  </time>
                </td>
                <td class="col-remaining text-warning text-xs font-semibold">
                  {{ getRemainingTime(item.created_at) }}
                </td>
                <td class="col-actions">
                  <div class="action-buttons">
                    <UiButton
                      class="restore"
                      :disabled="isOperating"
                      :title="$t('common.restore')"
                      @click="handleRestore(item)"
                      variant="secondary"
                    >
                      {{ $t('common.restore') || '恢复' }}
                    </UiButton>
                    <UiButton
                      class="delete"
                      :disabled="isOperating"
                      :title="$t('common.delete')"
                      @click="confirmDelete(item)"
                      variant="danger"
                    >
                      {{ $t('common.delete') || '永久删除' }}
                    </UiButton>
                  </div>
                </td>
              </tr>
            </tbody>
          </table>
        </div>
      </div>

      <!-- Empty State -->
      <div v-else class="trash-content glass-panel">
        <div class="trash-body empty-body">
          <EmptyState
            :icon="icons.emptyTrash"
            :title="$t('trash.emptyTitle')"
            :description="$t('trash.emptyDesc')"
          />
        </div>
      </div>

      <!-- Confirm Delete Modal -->
      <UiModal
        v-if="confirmDialog.show"
        :show-close="false"
        :close-disabled="isOperating"
        @close="closeConfirmDialog"
      >
        <div class="modal-header">
          <h3 class="modal-title">
            <Icon class="title-icon text-error" :size="18" name="outline-14" />
            {{ confirmDialog.title }}
          </h3>
          <UiButton
            :disabled="isOperating"
            @click="closeConfirmDialog"
            variant="ghost"
            icon-only
            :aria-label="$t('common.close')"
          >
            <Icon :size="16" name="outline-9" />
          </UiButton>
        </div>
        <div class="modal-body text-sm text-secondary">
          <p>{{ confirmDialog.message }}</p>
          <p
            v-if="confirmDialog.detail"
            class="mt-2 text-xs text-tertiary break-all font-mono bg-black bg-opacity-20 p-2 rounded"
          >
            {{ confirmDialog.detail }}
          </p>
          <div class="action-footer mt-4">
            <UiButton :disabled="isOperating" @click="closeConfirmDialog" variant="secondary">
              {{ $t('common.cancel') }}
            </UiButton>
            <UiButton
              :class="{ 'danger': confirmDialog.isDanger }"
              :disabled="isOperating"
              @click="executeConfirmedAction"
              variant="danger"
            >
              <span v-if="isOperating" class="spinner-mini"></span>
              <span>{{ $t('common.confirm') }}</span>
            </UiButton>
          </div>
        </div>
      </UiModal>
    </div>
  </div>
</template>

<script setup lang="ts">
import UiModal from '@/components/ui/UiModal.vue'
import PageHeader from '@/components/layout/PageHeader.vue'
import Icon from '@/components/ui/Icon.vue'
import UiButton from '@/components/ui/UiButton.vue'
import { formatSize } from '@/utils/bytes'
import { ref, computed, onMounted } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { useI18n } from 'vue-i18n'
import { useNotification } from '@/composables/useNotification'
import EmptyState from '@/components/common/EmptyState.vue'

interface CleanupTrashEntry {
  id: number
  package_id: string
  original_path: string
  trash_path: string
  size_bytes: number
  created_at: string
}

const { t } = useI18n()
const notify = useNotification()

const trashList = ref<CleanupTrashEntry[]>([])
const isLoading = ref(false)
const isOperating = ref(false)

const totalSize = computed(() => {
  return trashList.value.reduce((sum, item) => sum + item.size_bytes, 0)
})

const confirmDialog = ref({
  show: false,
  title: '',
  message: '',
  detail: '',
  isDanger: false,
  actionType: '' as 'delete' | 'empty',
  targetItem: null as CleanupTrashEntry | null
})

onMounted(() => {
  loadTrash()
})

async function loadTrash() {
  if (isLoading.value) return
  isLoading.value = true
  try {
    trashList.value = await invoke<CleanupTrashEntry[]>('list_cleanup_trash')
  } catch (err) {
    console.error('Failed to load trash list:', err)
    notify.error(String(err), t('common.error'))
  } finally {
    isLoading.value = false
  }
}

async function handleRestore(item: CleanupTrashEntry) {
  if (isOperating.value) return
  isOperating.value = true
  try {
    await invoke('restore_cleanup_trash_item', { trashId: item.id })
    notify.success(t('trash.restoreSuccess', { name: item.package_id }), t('common.success'))
    await loadTrash()
  } catch (err) {
    notify.error(String(err), t('common.error'))
  } finally {
    isOperating.value = false
  }
}

function confirmDelete(item: CleanupTrashEntry) {
  confirmDialog.value = {
    show: true,
    title: t('trash.deleteConfirmTitle'),
    message: t('trash.deleteConfirmMsg', { name: item.package_id }),
    detail: item.original_path,
    isDanger: true,
    actionType: 'delete',
    targetItem: item
  }
}

function confirmEmptyTrash() {
  confirmDialog.value = {
    show: true,
    title: t('trash.emptyConfirmTitle'),
    message: t('trash.emptyConfirmMsg'),
    detail: '',
    isDanger: true,
    actionType: 'empty',
    targetItem: null
  }
}

function closeConfirmDialog() {
  confirmDialog.value.show = false
}

async function executeConfirmedAction() {
  if (isOperating.value) return
  isOperating.value = true
  const { actionType, targetItem } = confirmDialog.value

  try {
    if (actionType === 'delete' && targetItem) {
      await invoke('delete_cleanup_trash_item', { trashId: targetItem.id })
      notify.success(t('trash.deleteSuccess'), t('common.success'))
    } else if (actionType === 'empty') {
      const count = await invoke<number>('empty_cleanup_trash')
      notify.success(t('trash.emptySuccess', { count }), t('common.success'))
    }
    closeConfirmDialog()
    await loadTrash()
  } catch (err) {
    notify.error(String(err), t('common.error'))
  } finally {
    isOperating.value = false
  }
}

function getRemainingTime(createdAtStr: string): string {
  if (!createdAtStr) return ''
  const utcStr = createdAtStr.replace(' ', 'T') + 'Z'
  const createdTime = new Date(utcStr).getTime()
  if (isNaN(createdTime)) return ''

  const expiryTime = createdTime + 24 * 60 * 60 * 1000 // 24 hours
  const now = Date.now()
  const diffMs = expiryTime - now

  if (diffMs <= 0) {
    return t('trash.purgingSoon')
  }

  const diffHours = Math.floor(diffMs / (60 * 60 * 1000))
  if (diffHours >= 1) {
    return t('trash.hoursLeft', { count: diffHours })
  }

  const diffMins = Math.floor(diffMs / (60 * 1000))
  if (diffMins >= 1) {
    return t('trash.minutesLeft', { count: diffMins })
  }

  return t('trash.purgingSoon')
}


function formatDate(dateStr: string): string {
  if (!dateStr) return '-'
  try {
    // Standardize ISO format
    const formatted = dateStr.replace(' ', 'T')
    const date = new Date(formatted + (dateStr.includes('Z') ? '' : 'Z'))
    if (isNaN(date.getTime())) return dateStr.slice(0, 16)

    // Add local timezone offset manually to print native local time
    const localDate = new Date(date.getTime() - date.getTimezoneOffset() * 60000)
    return localDate.toISOString().slice(0, 16).replace('T', ' ')
  } catch {
    return dateStr.slice(0, 16)
  }
}

const icons = {
  trash: 'M19 7l-.867 12.142A2 2 0 0116.138 21H7.862a2 2 0 01-1.995-1.858L5 7m5 4v6m4-6v6m1-10V4a1 1 0 00-1-1h-4a1 1 0 00-1 1v3M4 7h16',
  space: 'M4 7V4a2 2 0 0 1 2-2h8.5L20 7.5V20a2 2 0 0 1-2 2H6a2 2 0 0 1-2-2v-3M2 12h16M6 9v6',
  emptyTrash: 'M19 7l-.867 12.142A2 2 0 0116.138 21H7.862a2 2 0 01-1.995-1.858L5 7M4 7h16m-10 4v6m4-6v6',
}
</script>

<style scoped>
.trash-view {
  display: flex;
  flex-direction: column;
  gap: var(--space-4);
  height: 100%;
  min-height: 0;
  min-width: 0;
}

.trash-toolbar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--space-5);
  flex-wrap: wrap;
  flex-shrink: 0;
  padding: var(--space-4) var(--space-5);
  border: none;
  border-radius: var(--radius-md);
  background: var(--bg-surface);
}

.summary-grid {
  display: flex;
  gap: var(--space-6);
  margin: 0;
}

.summary-item + .summary-item {
  border-left: 1px solid var(--border-subtle);
  padding-left: var(--space-6);
}

.summary-item dt {
  color: var(--text-secondary);
  font-size: var(--text-xs);
  margin-bottom: var(--space-1);
}

.summary-item dd {
  margin: 0;
  font-size: var(--text-xl);
  font-weight: var(--font-semibold);
  font-variant-numeric: tabular-nums;
  color: var(--text-primary);
}

.refresh-icon.spin {
  animation: spin 1s linear infinite;
}

.trash-content {
  flex: 1;
  min-height: 0;
  min-width: 0;
  display: flex;
  flex-direction: column;
  overflow: hidden;
}

.trash-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: var(--space-4) var(--space-5);
  border-bottom: 1px solid var(--border-subtle);
  flex-shrink: 0;
  gap: var(--space-2);
  flex-wrap: wrap;
}

.trash-header-title {
  font-size: var(--text-sm);
  font-weight: var(--font-semibold);
  color: var(--text-primary);
}

.trash-header-info {
  color: var(--text-secondary);
  line-height: 1.6;
}

.trash-body {
  flex: 1;
  min-height: 0;
  overflow: auto;
}

.empty-body {
  display: flex;
  align-items: center;
  justify-content: center;
  min-height: 350px;
}

/* ── Trash Table Styles ────────────────────────────────────── */
.trash-table {
  width: 100%;
  min-width: 720px;
  table-layout: fixed;
  border-collapse: collapse;
  text-align: left;
}

.trash-table th,
.trash-table td {
  padding: var(--space-3) var(--space-3);
  border-bottom: 1px solid var(--border-subtle);
  vertical-align: middle;
}

.trash-table .col-package {
  padding-left: var(--space-5);
}
.col-size {
  width: 88px;
  text-align: right;
  white-space: nowrap;
}
.col-time {
  width: 112px;
  white-space: nowrap;
}
.col-remaining {
  width: 150px;
  white-space: nowrap;
}
.col-actions {
  width: 148px;
  white-space: nowrap;
}
.trash-table .col-actions {
  padding-right: var(--space-5);
}
.col-time time {
  display: flex;
  flex-direction: column;
  gap: 2px;
  font-variant-numeric: tabular-nums;
}
.time-detail {
  color: var(--text-tertiary);
}
.trash-table td.col-remaining {
  color: var(--text-secondary);
  font-weight: var(--font-normal);
}

.trash-table th {
  font-size: var(--text-xs);
  font-weight: var(--font-semibold);
  color: var(--text-secondary);
  white-space: nowrap;
  background: var(--bg-surface);
  position: sticky;
  top: 0;
  z-index: 1;
}

.trash-row {
  transition: background var(--transition-fast) var(--ease);
}

.trash-row:hover {
  background: var(--bg-hover);
}

.package-cell {
  display: flex;
  flex-direction: column;
  gap: 4px;
  min-width: 0;
}

.package-name {
  font-size: var(--text-sm);
  line-height: 1.5;
}

.truncate {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.truncate-path {
  display: block;
  color: var(--text-tertiary);
  font-size: var(--text-xs);
  line-height: 1.5;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.action-buttons {
  display: flex;
  gap: var(--space-2);
  justify-content: flex-end;
}

/* ── Modal Overlay & Container ────────────────────────────── */

.modal-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: var(--space-4) var(--space-5);
  border-bottom: 1px solid var(--border-subtle);
}

.modal-title {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  font-size: var(--text-md);
  font-weight: var(--font-semibold);
  color: var(--text-primary);
}

.modal-body {
  padding: var(--space-5);
}

.action-footer {
  display: flex;
  justify-content: flex-end;
  gap: var(--space-3);
}

.spinner-mini {
  width: 14px;
  height: 14px;
  border: none;
  border-top-color: var(--text-primary);
  border-radius: var(--radius-xl) %;
  animation: spin 0.8s linear infinite;
  margin-right: var(--space-2);
}

@keyframes spin {
  from {
    transform: rotate(0deg);
  }
  to {
    transform: rotate(360deg);
  }
}

.mt-2 {
  margin-top: var(--space-2);
}
.mt-4 {
  margin-top: var(--space-4);
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
