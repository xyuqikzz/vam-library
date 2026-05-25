<template>
  <div class="deduplication-view animate-fadeIn">
    <!-- Summary Cards -->
    <section class="summary-grid stagger-children">
      <StatCard
        :title="$t('deduplication.duplicateGroups')"
        :value="dedupSummary?.duplicate_groups ?? 0"
        :subtitle="$t('deduplication.duplicateGroupsSub')"
        :icon="icons.duplicates"
        :trend="(dedupSummary?.duplicate_groups ?? 0) > 0 ? 'up' : 'neutral'"
        :color="(dedupSummary?.duplicate_groups ?? 0) > 0 ? '#f59e0b' : '#3ecf8e'"
      />
      <StatCard
        :title="$t('deduplication.wastedSpace')"
        :value="((dedupSummary?.total_wasted_bytes ?? 0) > 0) ? formatSize(dedupSummary!.total_wasted_bytes) : '—'"
        :subtitle="$t('deduplication.wastedSpaceSub')"
        :icon="icons.space"
        trend="neutral"
        color="#ef4444"
      />
      <StatCard
        :title="$t('deduplication.safeToClean')"
        :value="dedupSummary?.safe_to_clean_count ?? 0"
        :subtitle="$t('deduplication.safeToCleanSub')"
        :icon="icons.clean"
        trend="neutral"
        color="#3ecf8e"
      />
    </section>

    <!-- Action Bar -->
    <div class="action-bar">
      <button
        class="scan-btn"
        :disabled="isDedupScanning || isScanning || !vamRootPath"
        @click="handleScan"
      >
        <span>{{ isDedupScanning ? $t('common.loading') : $t('deduplication.scanButton') }}</span>
      </button>
      <button
        v-if="cleanupList.length > 0"
        class="cleanup-btn"
        @click="showCleanupPreview = true"
      >
        <svg width="14" height="14" viewBox="0 0 24 24" fill="none">
          <path d="M22 11.08V12a10 10 0 1 1-5.93-9.14M22 4L12 14.01l-3-3" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" />
        </svg>
        <span>{{ $t('deduplication.reviewCleanup', { count: cleanupList.length }) }}</span>
      </button>
      <button
        v-if="trashList.length > 0"
        class="restore-btn"
        :disabled="isRestoring"
        @click="handleRestoreLatest"
      >
        <svg width="14" height="14" viewBox="0 0 24 24" fill="none">
          <path d="M3 7h18M8 7V5a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2m3 0-.8 12A2 2 0 0 1 16.2 21H7.8a2 2 0 0 1-2-2L5 7m5 5 2-2 2 2m-2-2v7" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" />
        </svg>
        <span>{{ $t('deduplication.restoreLatest', { count: trashList.length }) }}</span>
      </button>
    </div>

    <!-- Duplicate Groups List -->
    <div v-if="duplicateGroups.length > 0" class="dedup-content glass-panel">
      <div class="dedup-header">
        <h3 class="dedup-header-title">{{ $t('deduplication.duplicates') }}</h3>
        <span class="dedup-header-count text-xs text-tertiary">{{ $t('deduplication.groups_count', { count: duplicateGroups.length }) }}</span>
      </div>
      <div class="dedup-body">
        <div
          v-for="group in duplicateGroups"
          :key="group.id"
          :class="['dup-group', { expanded: expandedGroup === group.id }]"
        >
          <!-- Group header -->
          <div class="dup-group-header" @click="toggleGroup(group.id)">
            <div class="dup-group-info">
              <div class="dup-group-path text-sm">{{ group.resource_path }}</div>
              <div class="dup-group-meta text-xs text-tertiary">
                {{ $t('deduplication.copies', { count: group.file_count }) }} &middot; {{ $t('deduplication.wasted', { size: formatSize(group.total_wasted_bytes) }) }}
              </div>
            </div>
            <div class="dup-group-actions">
              <button
                v-if="!allSelectedFor(group)"
                class="dup-action-btn dup-action-keep text-xs"
                @click.stop="selectAllForKeep(group)"
              >
                {{ $t('deduplication.keepAll') }}
              </button>
              <button
                v-if="!allSelectedFor(group)"
                class="dup-action-btn dup-action-clean text-xs"
                @click.stop="selectRecommendedForKeep(group)"
              >
                {{ $t('deduplication.keepRecommended') }}
              </button>
              <span
                v-if="allSelectedFor(group)"
                class="dup-selected-badge text-xs"
              >{{ $t('deduplication.selected') }}</span>
              <span class="dup-expand-icon">{{ expandedGroup === group.id ? '▾' : '▸' }}</span>
            </div>
          </div>

          <!-- Group instances -->
          <div v-if="expandedGroup === group.id" class="dup-instances">
            <div
              v-for="instance in group.instances"
              :key="instance.package_id"
              :class="['dup-instance', {
                'dup-keep': selectedInstances[group.id]?.has(instance.package_id),
                'dup-remove': !selectedInstances[group.id]?.has(instance.package_id),
              }]"
              @click="toggleInstance(group.id, instance.package_id)"
            >
              <div class="dup-inst-indicator">
                <svg
                  v-if="selectedInstances[group.id]?.has(instance.package_id)"
                  width="16" height="16" viewBox="0 0 16 16" fill="none"
                >
                  <rect x="1" y="1" width="14" height="14" rx="3" fill="#3ecf8e" />
                  <path d="M5 8L7 10L11 6" stroke="white" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" />
                </svg>
                <svg v-else width="16" height="16" viewBox="0 0 16 16" fill="none">
                  <rect x="1" y="1" width="14" height="14" rx="3" fill="none" stroke="var(--border-subtle)" stroke-width="1.5" />
                </svg>
              </div>
              <div class="dup-inst-info">
                <span class="dup-inst-path text-xs">{{ instance.package_id }}.var</span>
                <span class="dup-inst-meta text-xs text-tertiary">
                  {{ instance.file_path }} &middot; {{ formatSize(instance.size_bytes) }}
                </span>
              </div>
              <span class="dup-source-badge text-xs">{{ sourceTypeLabel(instance.source_type) }}</span>
              <span
                v-if="instance.is_recommended_keep"
                class="dup-rec-badge text-xs"
              >{{ $t('deduplication.recommended') }}</span>
            </div>
          </div>
        </div>
      </div>
    </div>

    <!-- Empty State -->
    <div v-else-if="hasScanned && duplicateGroups.length === 0" class="dedup-content glass-panel">
      <div class="dedup-body">
          <EmptyState
            :icon="icons.empty"
            :title="$t('deduplication.noDuplicates')"
            :description="emptyStateDescription"
          />
      </div>
    </div>

    <div v-else class="dedup-content glass-panel">
      <div class="dedup-body">
        <EmptyState
          :icon="icons.empty"
          :title="$t('deduplication.noDuplicates')"
          :description="$t('deduplication.noDuplicatesDesc')"
        />
      </div>
    </div>

    <!-- Cleanup Preview Modal -->
    <Transition name="fade">
      <div v-if="showCleanupPreview" class="modal-overlay" @click.self="showCleanupPreview = false">
        <div class="modal-container glass-panel animate-scaleUp">
          <div class="modal-header">
            <h3 class="modal-title">
              <svg width="18" height="18" viewBox="0 0 24 24" fill="none" class="title-icon">
                <path d="M19 7l-.867 12.142A2 2 0 0116.138 21H7.862a2 2 0 01-1.995-1.858L5 7m5 4v6m4-6v6m1-10V4a1 1 0 00-1-1h-4a1 1 0 00-1 1v3M4 7h16" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"/>
              </svg>
              {{ $t('deduplication.confirmModalTitle') }}
            </h3>
            <button class="close-btn" :disabled="isCleaning" @click="showCleanupPreview = false">
              <svg width="16" height="16" viewBox="0 0 24 24" fill="none">
                <path d="M18 6L6 18M6 6l12 12" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" />
              </svg>
            </button>
          </div>
          <div class="modal-body">
            <p class="share-desc text-secondary text-sm">
              {{ $t('deduplication.confirmModalDesc') }}
            </p>
            <div class="cleanup-scroll-list list-scrollbar">
              <div v-for="item in cleanupList" :key="item.file_path" class="cleanup-preview-item">
                <span class="path text-xs text-primary truncate" :title="item.file_path">{{ item.file_path }}</span>
                <span class="meta text-xs text-tertiary">
                  {{ $t('packages.type') }}: {{ item.package_id }}.var &middot; {{ formatSize(item.size_bytes) }}
                </span>
              </div>
            </div>
            <div class="cleanup-summary border-t">
              <span class="text-xs text-primary">{{ $t('deduplication.filesToClean', { count: cleanupList.length }) }}</span>
              <span class="text-xs text-primary">
                {{ $t('deduplication.estimatedFreedSpace', { size: '' }) }}
                <strong class="highlight-red font-numeric">
                  {{ formatSize(cleanupList.reduce((sum, item) => sum + item.size_bytes, 0)) }}
                </strong>
              </span>
            </div>
            <div class="action-footer">
              <button class="action-btn secondary-btn" :disabled="isCleaning" @click="showCleanupPreview = false">
                {{ $t('common.cancel') }}
              </button>
              <button class="action-btn cleanup-confirm-btn" :disabled="isCleaning" @click="handleCleanup">
                <span v-if="isCleaning" class="spinner"></span>
                <span>{{ isCleaning ? $t('deduplication.cleaning') : $t('deduplication.confirmCleanup') }}</span>
              </button>
            </div>
          </div>
        </div>
      </div>
    </Transition>
  </div>
</template>

<script setup lang="ts">
import { onMounted, ref, reactive, computed, watch } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { storeToRefs } from 'pinia'
import { useI18n } from 'vue-i18n'
import { useAppStore } from '@/stores/app'
import { useLocalLibraryStore } from '@/stores/localLibrary'
import { useNotification } from '@/composables/useNotification'
import StatCard from '@/components/common/StatCard.vue'
import EmptyState from '@/components/common/EmptyState.vue'

interface DuplicateInstance {
  package_id: string
  file_path: string
  size_bytes: number
  is_recommended_keep: boolean
  source_type: string
  link_type: string | null
}

interface DuplicateGroup {
  id: string
  strategy: string
  file_hash: string
  resource_path: string
  total_wasted_bytes: number
  file_count: number
  instances: DuplicateInstance[]
}

interface DedupSummary {
  duplicate_groups: number
  total_wasted_bytes: number
  safe_to_clean_count: number
  total_files: number
  scanned_files: number
}

interface CleanupTrashEntry {
  id: number
  package_id: string
  original_path: string
  trash_path: string
  size_bytes: number
  created_at: string
}

const appStore = useAppStore()
const localLibraryStore = useLocalLibraryStore()
const { t } = useI18n()
const { vamRootPath, isScanning } = storeToRefs(appStore)
const { revision } = storeToRefs(localLibraryStore)
const notify = useNotification()

const dedupSummary = ref<DedupSummary | null>(null)
const duplicateGroups = ref<DuplicateGroup[]>([])
const hasScanned = ref(false)
const expandedGroup = ref<string | null>(null)
const showCleanupPreview = ref(false)
const isCleaning = ref(false)
const isRestoring = ref(false)
const isDedupScanning = ref(false)
const trashList = ref<CleanupTrashEntry[]>([])

// Track selected (keep) instances per group
const selectedInstances = reactive<Record<string, Set<string>>>({})

// Computed cleanup list: contains all duplicate instances that are NOT selected for keeping
const cleanupList = computed(() => {
  const list: DuplicateInstance[] = []
  for (const group of duplicateGroups.value) {
    const selected = selectedInstances[group.id]
    if (!selected) continue
    for (const instance of group.instances) {
      if (!selected.has(instance.package_id)) {
        list.push(instance)
      }
    }
  }
  return list
})

const emptyStateDescription = computed(() => {
  if (!hasScanned.value || !dedupSummary.value) {
    return t('deduplication.noDuplicatesDesc')
  }

  return t('deduplication.noDuplicatesScannedDesc', {
    count: dedupSummary.value.total_files,
  })
})

onMounted(async () => {
  await localLibraryStore.ensureLoaded()
  await loadData()
  await loadTrash()
})

watch(revision, async () => {
  if (isCleaning.value || isRestoring.value || isDedupScanning.value) return
  await loadData()
  await loadTrash()
})

async function loadData() {
  try {
    duplicateGroups.value = await invoke<DuplicateGroup[]>('get_duplicate_groups')
    for (const key of Object.keys(selectedInstances)) {
      delete selectedInstances[key]
    }
    if (duplicateGroups.value.length > 0) {
      hasScanned.value = true
      // Initialize selection: all instances selected (keep all) by default
      for (const group of duplicateGroups.value) {
        selectedInstances[group.id] = new Set(group.instances.map(i => i.package_id))
      }
    }
  } catch {
    // No data yet
  }
}

async function loadTrash() {
  try {
    trashList.value = await invoke<CleanupTrashEntry[]>('list_cleanup_trash')
  } catch {
    trashList.value = []
  }
}

async function handleScan() {
  if (!vamRootPath.value || isScanning.value || isDedupScanning.value) return
  isDedupScanning.value = true
  hasScanned.value = false
  duplicateGroups.value = []
  try {
    dedupSummary.value = await invoke<DedupSummary>('scan_for_duplicates', {
      vamRoot: vamRootPath.value,
    })
    hasScanned.value = true
    await loadData()
  } catch (err) {
    notify.error(String(err), t('dashboard.scanFailed'))
  } finally {
    isDedupScanning.value = false
  }
}

async function handleCleanup() {
  if (cleanupList.value.length === 0 || isCleaning.value) return
  isCleaning.value = true
  try {
    const deletedCount = await invoke<number>('execute_cleanup', {
      instances: cleanupList.value,
    })
    notify.success(`成功清理了 ${deletedCount} 个物理重复包！`, '清理成功')
    showCleanupPreview.value = false
    // Refresh list
    if (vamRootPath.value) {
      dedupSummary.value = await invoke('scan_for_duplicates', { vamRoot: vamRootPath.value })
    }
    await loadData()
    await loadTrash()
  } catch (err: any) {
    notify.error(`清理失败: ${err}`, '错误')
  } finally {
    isCleaning.value = false
  }
}

async function handleRestoreLatest() {
  if (trashList.value.length === 0 || isRestoring.value) return
  isRestoring.value = true
  try {
    await invoke('restore_cleanup_trash_item', { trashId: trashList.value[0].id })
    notify.success(t('deduplication.restoreSuccess'), t('common.success'))
    await loadTrash()
    await loadData()
  } catch (err: any) {
    notify.error(`恢复失败: ${err}`, t('common.error'))
  } finally {
    isRestoring.value = false
  }
}

function toggleGroup(groupId: string) {
  expandedGroup.value = expandedGroup.value === groupId ? null : groupId
}

function toggleInstance(groupId: string, pkgId: string) {
  if (!selectedInstances[groupId]) return
  if (selectedInstances[groupId].has(pkgId)) {
    // Don't allow deselecting the last instance
    if (selectedInstances[groupId].size <= 1) return
    selectedInstances[groupId].delete(pkgId)
  } else {
    selectedInstances[groupId].add(pkgId)
  }
}

function selectAllForKeep(group: DuplicateGroup) {
  selectedInstances[group.id] = new Set(group.instances.map(i => i.package_id))
}

function selectRecommendedForKeep(group: DuplicateGroup) {
  selectedInstances[group.id] = new Set(
    group.instances
      .filter(i => i.is_recommended_keep)
      .map(i => i.package_id)
  )
  // If no recommended, keep at least one
  if (selectedInstances[group.id].size === 0) {
    selectedInstances[group.id].add(group.instances[0].package_id)
  }
}

function allSelectedFor(group: DuplicateGroup): boolean {
  return selectedInstances[group.id]?.size === group.instances.length
}

function formatSize(bytes: number): string {
  if (bytes === 0) return '0 B'
  const units = ['B', 'KB', 'MB', 'GB', 'TB']
  const i = Math.floor(Math.log(bytes) / Math.log(1024))
  return `${(bytes / Math.pow(1024, i)).toFixed(i > 0 ? 1 : 0)} ${units[i]}`
}

function sourceTypeLabel(sourceType: string): string {
  const labels: Record<string, string> = {
    managed_library: '托管库',
    real_file: '真实文件',
    hard_link: '硬链接',
    symlink: '符号链接',
    external: '外部路径',
  }
  return labels[sourceType] || sourceType || '未知来源'
}

const icons = {
  duplicates: 'M3 3h12v12H3zM9 9h12v12H9',
  space: 'M4 7V4a2 2 0 0 1 2-2h8.5L20 7.5V20a2 2 0 0 1-2 2H6a2 2 0 0 1-2-2v-3M2 12h16M6 9v6',
  clean: 'M22 11.08V12a10 10 0 1 1-5.93-9.14M22 4L12 14.01l-3-3',
  empty: 'M3 3h12v12H3zM9 9h12v12H9',
}
</script>

<style scoped>
.deduplication-view {
  display: flex;
  flex-direction: column;
  gap: var(--space-5);
  height: 100%;
}

.summary-grid {
  display: grid;
  grid-template-columns: repeat(3, 1fr);
  gap: var(--space-5);
}

@media (max-width: 1000px) {
  .summary-grid { grid-template-columns: 1fr; }
}

.action-bar {
  display: flex;
  gap: var(--space-3);
}

.scan-btn,
.cleanup-btn,
.restore-btn {
  display: inline-flex;
  align-items: center;
  gap: var(--space-2);
  padding: var(--space-2) var(--space-4);
  height: 34px;
  font-size: var(--text-sm);
  font-weight: var(--font-semibold);
  border-radius: var(--radius-md);
  transition:
    opacity var(--duration-fast) var(--ease),
    transform var(--duration-fast) var(--ease);
}

.scan-btn {
  background: var(--accent-gradient);
  color: white;
}

.scan-btn:disabled { opacity: 0.5; cursor: not-allowed; }

.scan-btn:not(:disabled):hover { opacity: 0.9; transform: translateY(-1px); }

.spin-icon {
  animation: spin 1s linear infinite;
  transform-origin: center;
}

.cleanup-btn {
  background: var(--color-success);
  color: white;
}

.cleanup-btn:hover { opacity: 0.9; transform: translateY(-1px); }

.restore-btn {
  background: rgba(96, 165, 250, 0.14);
  color: var(--color-info);
  border: 1px solid rgba(96, 165, 250, 0.28);
}

.restore-btn:hover:not(:disabled) { background: rgba(96, 165, 250, 0.22); transform: translateY(-1px); }
.restore-btn:disabled { opacity: 0.5; cursor: not-allowed; }

.dedup-content {
  flex: 1;
  display: flex;
  flex-direction: column;
  overflow: hidden;
}

.dedup-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: var(--space-3) var(--space-5);
  border-bottom: 1px solid var(--border-subtle);
  flex-shrink: 0;
}

.dedup-header-title {
  font-size: var(--text-sm);
  font-weight: var(--font-semibold);
  color: var(--text-secondary);
  text-transform: uppercase;
  letter-spacing: 0.06em;
}

.dedup-header-count {
  font-variant-numeric: tabular-nums;
}

.dedup-body {
  flex: 1;
  overflow-y: auto;
  padding: var(--space-2);
  display: flex;
  flex-direction: column;
  gap: 2px;
}

/* ── Duplicate Group ──────────────────────────────────────── */
.dup-group {
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-md);
  overflow: hidden;
  transition: border-color var(--duration-fast) var(--ease);
}

.dup-group.expanded {
  border-color: var(--border-default);
}

.dup-group-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: var(--space-3) var(--space-4);
  cursor: pointer;
  transition: background var(--duration-fast) var(--ease);
}

.dup-group-header:hover {
  background: var(--bg-hover);
}

.dup-group-info {
  display: flex;
  flex-direction: column;
  gap: 2px;
  min-width: 0;
  flex: 1;
}

.dup-group-path {
  color: var(--text-primary);
  font-weight: var(--font-medium);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  max-width: 400px;
}

.dup-group-meta { font-variant-numeric: tabular-nums; }

.dup-group-actions {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  flex-shrink: 0;
}

.dup-action-btn {
  padding: 2px 10px;
  border-radius: var(--radius-full);
  font-weight: var(--font-semibold);
  transition: background var(--duration-fast) var(--ease);
}

.dup-action-keep { background: rgba(62, 207, 142, 0.15); color: var(--color-success); }
.dup-action-keep:hover { background: rgba(62, 207, 142, 0.25); }

.dup-action-clean { background: rgba(124, 92, 252, 0.12); color: var(--accent-primary); }
.dup-action-clean:hover { background: rgba(124, 92, 252, 0.2); }

.dup-selected-badge {
  padding: 2px 10px;
  border-radius: var(--radius-full);
  background: rgba(62, 207, 142, 0.12);
  color: var(--color-success);
  font-weight: var(--font-semibold);
}

.dup-expand-icon {
  color: var(--text-tertiary);
  font-size: 12px;
  width: 16px;
  text-align: center;
}

/* ── Instances ────────────────────────────────────────────── */
.dup-instances {
  border-top: 1px solid var(--border-subtle);
}

.dup-instance {
  display: flex;
  align-items: center;
  gap: var(--space-3);
  padding: var(--space-2) var(--space-4);
  cursor: pointer;
  transition: background var(--duration-fast) var(--ease);
}

.dup-instance:hover { background: var(--bg-hover); }

.dup-instance + .dup-instance {
  border-top: 1px solid var(--border-subtle);
}

.dup-instance.dup-keep { opacity: 1; }
.dup-instance.dup-remove { opacity: 0.5; }

.dup-inst-indicator {
  flex-shrink: 0;
}

.dup-inst-info {
  display: flex;
  flex-direction: column;
  gap: 1px;
  min-width: 0;
  flex: 1;
}

.dup-inst-path {
  color: var(--text-primary);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.dup-inst-meta { font-variant-numeric: tabular-nums; }

.dup-rec-badge {
  padding: 2px 8px;
  border-radius: var(--radius-full);
  background: rgba(124, 92, 252, 0.1);
  color: var(--accent-primary);
  font-weight: var(--font-semibold);
  white-space: nowrap;
  flex-shrink: 0;
}

.dup-source-badge {
  padding: 2px 8px;
  border-radius: var(--radius-full);
  background: rgba(96, 165, 250, 0.1);
  color: var(--color-info);
  font-weight: var(--font-semibold);
  white-space: nowrap;
  flex-shrink: 0;
}

/* ── Cleanup Modal ────────────────────────────────────────── */
.modal-overlay {
  position: fixed;
  top: 0;
  left: 0;
  width: 100vw;
  height: 100vh;
  background: rgba(0, 0, 0, 0.45);
  backdrop-filter: var(--glass-blur);
  -webkit-backdrop-filter: var(--glass-blur);
  display: flex;
  align-items: center;
  justify-content: center;
  z-index: 1000;
}

.modal-container {
  width: 520px;
  max-width: 90vw;
  max-height: 85vh;
  display: flex;
  flex-direction: column;
  background: rgba(22, 22, 42, 0.88);
  border: 1px solid rgba(255, 255, 255, 0.08);
  border-radius: var(--radius-lg);
  box-shadow: 0 24px 64px rgba(0, 0, 0, 0.4);
  overflow: hidden;
}

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
  overflow-y: auto;
  flex: 1;
  display: flex;
  flex-direction: column;
  gap: var(--space-4);
}

.share-desc {
  line-height: 1.5;
  color: var(--text-secondary);
}

.cleanup-scroll-list {
  max-height: 200px;
  overflow-y: auto;
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-md);
  background: rgba(0, 0, 0, 0.15);
  padding: var(--space-1) 0;
}

.cleanup-preview-item {
  display: flex;
  flex-direction: column;
  gap: 2px;
  padding: var(--space-2) var(--space-3);
  border-bottom: 1px solid rgba(255, 255, 255, 0.02);
}

.cleanup-preview-item:last-child {
  border-bottom: none;
}

.cleanup-preview-item .path {
  font-weight: var(--font-medium);
}

.cleanup-summary {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: var(--space-3) 0 var(--space-1) 0;
  border-top: 1px solid var(--border-subtle);
}

.highlight-red {
  color: var(--color-error);
  font-weight: var(--font-bold);
  font-size: var(--text-md);
}

.action-footer {
  display: flex;
  justify-content: flex-end;
  gap: var(--space-3);
  margin-top: var(--space-2);
}

.action-btn {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  padding: 0 var(--space-5);
  height: 38px;
  font-size: var(--text-sm);
  font-weight: var(--font-semibold);
  border-radius: var(--radius-md);
  cursor: pointer;
  border: none;
  transition: all var(--transition-fast);
}

.secondary-btn {
  background: transparent;
  color: var(--text-primary);
  border: 1px solid var(--border-default);
}

.secondary-btn:hover:not(:disabled) {
  background: var(--bg-hover);
}

.secondary-btn:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}

.cleanup-confirm-btn {
  background: var(--accent-gradient);
  color: white;
  box-shadow: 0 4px 12px rgba(124, 92, 252, 0.3);
  gap: var(--space-2);
}

.cleanup-confirm-btn:hover:not(:disabled) {
  transform: translateY(-1px);
  box-shadow: 0 6px 16px rgba(124, 92, 252, 0.4);
}

.cleanup-confirm-btn:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}

.spinner {
  width: 16px;
  height: 16px;
  border: 2px solid rgba(255, 255, 255, 0.2);
  border-top-color: white;
  border-radius: 50%;
  animation: spin 1s linear infinite;
}

.list-scrollbar::-webkit-scrollbar {
  width: 6px;
}

.list-scrollbar::-webkit-scrollbar-track {
  background: transparent;
}

.list-scrollbar::-webkit-scrollbar-thumb {
  background: rgba(255, 255, 255, 0.08);
  border-radius: var(--radius-full);
}

.list-scrollbar::-webkit-scrollbar-thumb:hover {
  background: rgba(255, 255, 255, 0.15);
}

@keyframes spin {
  to { transform: rotate(360deg); }
}

/* Modal Animations */
.animate-scaleUp {
  animation: scaleUp 0.30s cubic-bezier(0.34, 1.56, 0.64, 1) forwards;
}

@keyframes scaleUp {
  from { transform: scale(0.92); opacity: 0; }
  to { transform: scale(1); opacity: 1; }
}

.fade-enter-active, .fade-leave-active {
  transition: opacity 0.2s var(--ease);
}

.fade-enter-from, .fade-leave-to {
  opacity: 0;
}
</style>
