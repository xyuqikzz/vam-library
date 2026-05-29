<template>
  <div class="trash-view animate-fadeIn">
    <!-- Stat Cards -->
    <section class="summary-grid stagger-children">
      <StatCard
        :title="$t('trash.statsTotal')"
        :value="trashList.length"
        :subtitle="$t('trash.statsTotalSub')"
        :icon="icons.trash"
        trend="neutral"
        color="#8888a8"
      />
      <StatCard
        :title="$t('trash.statsSize')"
        :value="totalSize > 0 ? formatSize(totalSize) : '—'"
        :subtitle="$t('trash.statsSizeSub')"
        :icon="icons.space"
        trend="neutral"
        color="#6e6bf0"
      />
    </section>

    <!-- Action Bar -->
    <div class="action-bar">
      <button
        class="refresh-btn"
        :disabled="isLoading"
        @click="loadTrash"
      >
        <svg :class="['refresh-icon', { 'spin': isLoading }]" width="14" height="14" viewBox="0 0 24 24" fill="none">
          <path d="M21.5 2v6h-6M21.34 15.57a10 10 0 1 1-.57-8.38l.73-.73" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" />
        </svg>
        <span>{{ $t('common.refresh') }}</span>
      </button>

      <button
        v-if="trashList.length > 0"
        class="empty-btn"
        :disabled="isOperating"
        @click="confirmEmptyTrash"
      >
        <svg width="14" height="14" viewBox="0 0 24 24" fill="none">
          <path d="M19 7l-.867 12.142A2 2 0 0116.138 21H7.862a2 2 0 01-1.995-1.858L5 7m5 4v6m4-6v6m1-10V4a1 1 0 00-1-1h-4a1 1 0 00-1 1v3M4 7h16" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" />
        </svg>
        <span>{{ $t('trash.emptyBtn') }}</span>
      </button>
    </div>

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
              <th class="col-path">{{ $t('trash.originalPath') }}</th>
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
                  <span class="type-dot-sm type-scene"></span>
                  <span class="truncate" :title="item.package_id">{{ item.package_id }}.var</span>
                </div>
              </td>
              <td class="col-path text-secondary text-xs">
                <span class="truncate-path" :title="item.original_path">{{ item.original_path }}</span>
              </td>
              <td class="col-size font-numeric text-secondary text-sm">
                {{ formatSize(item.size_bytes) }}
              </td>
              <td class="col-time text-secondary text-xs">
                {{ formatDate(item.created_at) }}
              </td>
              <td class="col-remaining text-warning text-xs font-semibold">
                {{ getRemainingTime(item.created_at) }}
              </td>
              <td class="col-actions">
                <div class="action-buttons">
                  <button
                    class="action-btn-mini restore"
                    :disabled="isOperating"
                    :title="$t('deduplication.restoreLatest').split(' ')[0]"
                    @click="handleRestore(item)"
                  >
                    {{ $t('common.restore') || '恢复' }}
                  </button>
                  <button
                    class="action-btn-mini delete"
                    :disabled="isOperating"
                    :title="$t('common.delete')"
                    @click="confirmDelete(item)"
                  >
                    {{ $t('common.delete') || '永久删除' }}
                  </button>
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
    <Transition name="fade">
      <div v-if="confirmDialog.show" class="modal-overlay" @click.self="closeConfirmDialog">
        <div class="modal-container glass-panel animate-scaleUp max-w-md">
          <div class="modal-header">
            <h3 class="modal-title">
              <svg width="18" height="18" viewBox="0 0 24 24" fill="none" class="title-icon text-error">
                <path d="M12 9v4m0 4h.01m-6.938 4h13.856c1.54 0 2.502-1.667 1.732-3L13.732 4c-.77-1.333-2.694-1.333-3.464 0L3.34 16c-.77 1.333.192 3 1.732 3z" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"/>
              </svg>
              {{ confirmDialog.title }}
            </h3>
            <button class="close-btn" :disabled="isOperating" @click="closeConfirmDialog">
              <svg width="16" height="16" viewBox="0 0 24 24" fill="none">
                <path d="M18 6L6 18M6 6l12 12" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" />
              </svg>
            </button>
          </div>
          <div class="modal-body text-sm text-secondary">
            <p>{{ confirmDialog.message }}</p>
            <p v-if="confirmDialog.detail" class="mt-2 text-xs text-tertiary break-all font-mono bg-black bg-opacity-20 p-2 rounded">
              {{ confirmDialog.detail }}
            </p>
            <div class="action-footer mt-4">
              <button class="action-btn secondary-btn" :disabled="isOperating" @click="closeConfirmDialog">
                {{ $t('common.cancel') }}
              </button>
              <button
                class="action-btn confirm-btn"
                :class="{ 'danger': confirmDialog.isDanger }"
                :disabled="isOperating"
                @click="executeConfirmedAction"
              >
                <span v-if="isOperating" class="spinner-mini"></span>
                <span>{{ $t('common.confirm') }}</span>
              </button>
            </div>
          </div>
        </div>
      </div>
    </Transition>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { useI18n } from 'vue-i18n'
import { useNotification } from '@/composables/useNotification'
import StatCard from '@/components/common/StatCard.vue'
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

function formatSize(bytes: number): string {
  if (bytes === 0) return '0 B'
  const units = ['B', 'KB', 'MB', 'GB', 'TB']
  const i = Math.floor(Math.log(bytes) / Math.log(1024))
  return `${(bytes / Math.pow(1024, i)).toFixed(i > 0 ? 1 : 0)} ${units[i]}`
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
  gap: var(--space-5);
  height: 100%;
}

.summary-grid {
  display: grid;
  grid-template-columns: repeat(2, 1fr);
  gap: var(--space-5);
}

@media (max-width: 768px) {
  .summary-grid { grid-template-columns: 1fr; }
}

.action-bar {
  display: flex;
  gap: var(--space-3);
  align-items: center;
}

.refresh-btn,
.empty-btn {
  display: inline-flex;
  align-items: center;
  gap: var(--space-2);
  padding: var(--space-2) var(--space-4);
  height: 34px;
  font-size: var(--text-sm);
  font-weight: var(--font-semibold);
  border-radius: var(--radius-md);
  cursor: pointer;
  transition: all var(--transition-fast) var(--ease);
}

.refresh-btn {
  background: rgba(255, 255, 255, 0.05);
  color: var(--text-primary);
  border: 1px solid var(--border-subtle);
}

.refresh-btn:hover:not(:disabled) {
  background: rgba(255, 255, 255, 0.08);
  border-color: var(--border-default);
  transform: translateY(-1px);
}

.empty-btn {
  background: var(--color-error);
  color: white;
  border: none;
}

.empty-btn:hover:not(:disabled) {
  opacity: 0.9;
  transform: translateY(-1px);
  box-shadow: 0 4px 12px rgba(248, 113, 113, 0.3);
}

.empty-btn:disabled,
.refresh-btn:disabled {
  opacity: 0.5;
  cursor: not-allowed;
  transform: none;
  box-shadow: none;
}

.refresh-icon.spin {
  animation: spin 1s linear infinite;
}

.trash-content {
  flex: 1;
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
}

.trash-header-title {
  font-size: var(--text-sm);
  font-weight: var(--font-semibold);
  color: var(--text-secondary);
  text-transform: uppercase;
  letter-spacing: 0.06em;
}

.trash-header-info {
  opacity: 0.8;
}

.trash-body {
  flex: 1;
  overflow-y: auto;
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
  border-collapse: collapse;
  text-align: left;
}

.trash-table th,
.trash-table td {
  padding: var(--space-3) var(--space-5);
  border-bottom: 1px solid var(--border-subtle);
}

.trash-table th {
  font-size: var(--text-xs);
  font-weight: var(--font-semibold);
  color: var(--text-tertiary);
  text-transform: uppercase;
  letter-spacing: 0.05em;
  background: rgba(0, 0, 0, 0.15);
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
  align-items: center;
  gap: var(--space-2);
  max-width: 250px;
}

.truncate {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.truncate-path {
  display: block;
  max-width: 320px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.action-buttons {
  display: flex;
  gap: var(--space-2);
}

.action-btn-mini {
  padding: 4px 10px;
  border-radius: var(--radius-sm);
  font-size: var(--text-xs);
  font-weight: var(--font-semibold);
  cursor: pointer;
  border: 1px solid transparent;
  transition: all var(--transition-fast) var(--ease);
}

.action-btn-mini.restore {
  background: rgba(62, 207, 142, 0.12);
  color: var(--color-success);
  border-color: rgba(62, 207, 142, 0.25);
}

.action-btn-mini.restore:hover:not(:disabled) {
  background: rgba(62, 207, 142, 0.22);
  transform: translateY(-1px);
}

.action-btn-mini.delete {
  background: rgba(248, 113, 113, 0.1);
  color: var(--color-error);
  border-color: rgba(248, 113, 113, 0.2);
}

.action-btn-mini.delete:hover:not(:disabled) {
  background: rgba(248, 113, 113, 0.2);
  transform: translateY(-1px);
}

.action-btn-mini:disabled {
  opacity: 0.5;
  cursor: not-allowed;
  transform: none !important;
}

/* ── Modal Overlay & Container ────────────────────────────── */
.modal-overlay {
  position: fixed;
  top: 0;
  left: 0;
  width: 100vw;
  height: 100vh;
  background: rgba(0, 0, 0, 0.5);
  backdrop-filter: var(--glass-blur);
  -webkit-backdrop-filter: var(--glass-blur);
  display: flex;
  align-items: center;
  justify-content: center;
  z-index: 1000;
}

.modal-container {
  width: 100%;
  display: flex;
  flex-direction: column;
  background: rgba(28, 28, 30, 0.9);
  border: 1px solid rgba(255, 255, 255, 0.08);
  border-radius: var(--radius-lg);
  box-shadow: var(--glass-shadow);
  overflow: hidden;
}

.max-w-md {
  max-width: 440px;
}

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

.close-btn {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 28px;
  height: 28px;
  border-radius: var(--radius-sm);
  color: var(--text-tertiary);
  background: transparent;
  border: none;
  cursor: pointer;
  transition: all var(--transition-fast);
}

.close-btn:hover:not(:disabled) {
  color: var(--text-primary);
  background: var(--bg-hover);
}

.close-btn:disabled {
  opacity: 0.3;
  cursor: not-allowed;
}

.modal-body {
  padding: var(--space-5);
}

.action-footer {
  display: flex;
  justify-content: flex-end;
  gap: var(--space-3);
}

.action-btn {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  height: 36px;
  padding: 0 var(--space-4);
  font-size: var(--text-sm);
  font-weight: var(--font-semibold);
  border-radius: var(--radius-md);
  cursor: pointer;
  transition: all var(--transition-fast) var(--ease);
}

.secondary-btn {
  background: rgba(255, 255, 255, 0.05);
  color: var(--text-primary);
  border: 1px solid var(--border-subtle);
}

.secondary-btn:hover:not(:disabled) {
  background: rgba(255, 255, 255, 0.08);
  border-color: var(--border-default);
}

.confirm-btn {
  background: var(--accent-gradient);
  color: white;
  border: none;
}

.confirm-btn:hover:not(:disabled) {
  opacity: 0.95;
  box-shadow: 0 4px 12px rgba(110, 107, 240, 0.3);
}

.confirm-btn.danger {
  background: var(--color-error);
}

.confirm-btn.danger:hover:not(:disabled) {
  box-shadow: 0 4px 12px rgba(248, 113, 113, 0.3);
}

.confirm-btn:disabled,
.secondary-btn:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}

.spinner-mini {
  width: 14px;
  height: 14px;
  border: 2px solid rgba(255, 255, 255, 0.3);
  border-top-color: white;
  border-radius: 50%;
  animation: spin 0.8s linear infinite;
  margin-right: var(--space-2);
}

@keyframes spin {
  from { transform: rotate(0deg); }
  to { transform: rotate(360deg); }
}

.mt-2 { margin-top: var(--space-2); }
.mt-4 { margin-top: var(--space-4); }
</style>
