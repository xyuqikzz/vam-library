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

    <section class="scan-options glass-panel">
      <label for="dedup-root">{{ t('deduplication.sourceDirectory') }}</label>
      <div class="scan-directory-row">
        <input id="dedup-root" v-model="resourceRoot" :placeholder="defaultRoot" :disabled="busy" />
        <button class="restore-btn" :disabled="busy" @click="chooseRoot">{{ t('deduplication.browse') }}</button>
      </div>
      <p class="text-sm text-secondary">{{ t('deduplication.rules') }}</p>
      <p class="text-xs text-tertiary">{{ t('deduplication.ordinaryRule') }}</p>
      <p v-if="dedupSummary" class="text-xs">{{ t('deduplication.scanSummary', { count: dedupSummary.total_files, archive: dedupSummary.archive_count }) }}</p>
      <details v-if="dedupSummary?.warnings.length" class="text-xs">
        <summary>{{ t('deduplication.scanWarnings', { count: dedupSummary.warnings.length }) }}</summary>
        <p v-for="warning in dedupSummary.warnings" :key="warning">{{ warning }}</p>
      </details>
    </section>
    <!-- Action Bar -->
    <div class="action-bar">
      <button
        class="scan-btn"
        :disabled="busy || !effectiveRoot"
        @click="handleScan"
      >
        <span>{{ isDedupScanning ? $t('common.loading') : $t('deduplication.scanButton') }}</span>
      </button>
      <button
        v-if="cleanupList.length > 0 || archiveList.length > 0"
        class="cleanup-btn"
        :disabled="busy"
        @click="showCleanupPreview = true"
      >
        <svg width="14" height="14" viewBox="0 0 24 24" fill="none">
          <path d="M22 11.08V12a10 10 0 1 1-5.93-9.14M22 4L12 14.01l-3-3" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" />
        </svg>
        <span>{{ t('deduplication.reviewOrganize', { count: cleanupList.length, archive: archiveList.length }) }}</span>
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

    <!-- Keep groups at their natural height; paginate large scan results. -->
    <section v-if="duplicateGroups.length > 0" class="dedup-content glass-panel">
      <div class="dedup-header">
        <h3 class="dedup-header-title">{{ t('deduplication.duplicates') }}</h3>
        <span class="text-xs text-tertiary">{{ t('deduplication.filteredGroups', { count: filteredGroups.length, total: duplicateGroups.length }) }}</span>
      </div>
      <div class="dedup-search">
        <input v-model="groupSearch" type="search" :aria-label="t('deduplication.searchPaths')" :placeholder="t('deduplication.searchPaths')" />
        <span class="text-xs text-tertiary">{{ t('deduplication.pathsHint') }}</span>
      </div>
      <div ref="groupListEl" class="dedup-body">
        <article v-for="group in visibleGroups" :key="group.id" class="dup-group">
          <div class="dup-group-header">
            <button class="dup-group-toggle" :aria-expanded="!collapsedGroups.has(group.id)" @click="toggleGroup(group.id)">
              <span class="dup-expand-icon">{{ collapsedGroups.has(group.id) ? '▸' : '▾' }}</span>
              <span class="dup-group-info">
                <span class="dup-group-path text-sm">{{ group.resource_path }}</span>
                <span class="dup-group-meta text-xs text-tertiary">
                  {{ t('deduplication.copies', { count: group.file_count }) }} · {{ t('deduplication.wasted', { size: formatSize(group.total_wasted_bytes) }) }}
                </span>
              </span>
            </button>
            <div class="dup-group-actions">
              <button class="dup-action-btn text-xs" @click="copyPaths(group.instances.map(i => i.file_path))">{{ t('deduplication.copyGroupPaths') }}</button>
              <button v-if="!allSelectedFor(group)" class="dup-action-btn dup-action-keep text-xs" :disabled="busy" @click="selectAllForKeep(group)">{{ t('deduplication.keepAll') }}</button>
              <button v-else class="dup-action-btn dup-action-clean text-xs" :disabled="busy" @click="selectRecommendedForKeep(group)">{{ t('deduplication.keepRecommended') }}</button>
            </div>
          </div>
          <div v-if="!collapsedGroups.has(group.id)" class="dup-instances">
            <div v-for="instance in group.instances" :key="instance.file_path" class="dup-instance">
              <input
                type="checkbox"
                :checked="selectedInstances[group.id]?.has(instance.file_path)"
                :disabled="busy || instance.is_recommended_keep"
                :aria-label="t('deduplication.keepFile', { path: instance.file_path })"
                @change="toggleInstance(group.id, instance.file_path)"
              />
              <div class="dup-inst-info">
                <div class="dup-inst-heading">
                  <span class="dup-inst-name text-sm">{{ fileName(instance.file_path) }}</span>
                  <span :class="['dup-status', instance.archive_destination ? 'archive' : selectedInstances[group.id]?.has(instance.file_path) ? 'keep' : 'recycle']">
                    {{ instance.archive_destination ? t('deduplication.willArchive') : selectedInstances[group.id]?.has(instance.file_path) ? t('deduplication.keepStatus') : t('deduplication.recycleStatus') }}
                  </span>
                  <span v-if="instance.referenced_by.length" class="dup-source-badge text-xs" :title="instance.referenced_by.join('\n')">{{ t('deduplication.pinned') }}</span>
                </div>
                <span class="dup-file-path text-xs">{{ instance.file_path }}</span>
                <span class="dup-inst-meta text-xs text-tertiary">{{ formatSize(instance.size_bytes) }} · {{ t('deduplication.modified') }}: {{ new Date(instance.modified_time).toLocaleString() }}</span>
                <span v-if="instance.archive_destination" class="dup-file-path text-xs text-secondary">{{ t('deduplication.archiveTarget') }}: {{ instance.archive_destination }}</span>
              </div>
              <button class="dup-copy-btn text-xs" :aria-label="t('deduplication.copyFilePath', { path: instance.file_path })" @click="copyPaths([instance.file_path])">{{ t('deduplication.copyPath') }}</button>
            </div>
          </div>
        </article>
        <p v-if="!filteredGroups.length" class="dedup-no-match text-sm text-secondary">{{ t('deduplication.noMatchingGroups') }}</p>
      </div>
      <nav class="dedup-pagination" :aria-label="t('deduplication.groupPages')">
        <span class="text-xs text-tertiary">{{ t('deduplication.pageInfo', { page: currentPage, total: pageCount, size: pageSize }) }}</span>
        <div class="dup-group-actions">
          <button class="dup-page-btn" :disabled="currentPage <= 1" @click="currentPage--">{{ t('deduplication.previousPage') }}</button>
          <button class="dup-page-btn" :disabled="currentPage >= pageCount" @click="currentPage++">{{ t('deduplication.nextPage') }}</button>
        </div>
      </nav>
    </section>

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
                  {{ fileName(item.file_path) }} &middot; {{ formatSize(item.size_bytes) }}
                </span>
              </div>
            </div>
            <div v-if="archiveList.length" class="cleanup-scroll-list list-scrollbar">
              <strong>{{ t('deduplication.archivePreview') }}</strong>
              <div v-for="item in archiveList" :key="item.file_path" class="cleanup-preview-item">
                <span class="text-xs">{{ item.file_path }} → {{ item.archive_destination }}</span>
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
import { nextTick, onMounted, ref, reactive, computed, watch } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { open } from '@tauri-apps/plugin-dialog'
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
  modified_time: string
  version: number | null
  referenced_by: string[]
  archive_destination: string | null
}

interface DuplicateGroup {
  id: string
  strategy: string
  resource_path: string
  total_wasted_bytes: number
  file_count: number
  instances: DuplicateInstance[]
}

interface DedupSummary {
  scan_id: string
  root: string
  archive_count: number
  warnings: string[]
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

const resourceRoot = ref('')
const defaultRoot = computed(() => vamRootPath.value ? `${vamRootPath.value}/AddonPackages` : '')
const effectiveRoot = computed(() => resourceRoot.value.trim() || defaultRoot.value)
const busy = computed(() => isDedupScanning.value || isScanning.value || isCleaning.value || isRestoring.value)
const dedupSummary = ref<DedupSummary | null>(null)
const duplicateGroups = ref<DuplicateGroup[]>([])
const hasScanned = ref(false)
const collapsedGroups = ref(new Set<string>())
const groupSearch = ref('')
const currentPage = ref(1)
const pageSize = 20
const groupListEl = ref<HTMLElement | null>(null)
const filteredGroups = computed(() => {
  const query = groupSearch.value.trim().toLocaleLowerCase().replace(/\\/g, '/')
  if (!query) return duplicateGroups.value
  const matches = (value: string) => value.toLocaleLowerCase().replace(/\\/g, '/').includes(query)
  return duplicateGroups.value.filter(group => matches(group.resource_path) || group.instances.some(i => matches(i.file_path)))
})
const pageCount = computed(() => Math.max(1, Math.ceil(filteredGroups.value.length / pageSize)))
const visibleGroups = computed(() => filteredGroups.value.slice((currentPage.value - 1) * pageSize, currentPage.value * pageSize))
watch(groupSearch, () => { currentPage.value = 1 })
watch(pageCount, count => { currentPage.value = Math.min(currentPage.value, count) })
watch([currentPage, groupSearch], async () => {
  await nextTick()
  if (groupListEl.value) groupListEl.value.scrollTop = 0
})
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
      if (!selected.has(instance.file_path)) {
        list.push(instance)
      }
    }
  }
  return list
})

const archiveList = computed(() => duplicateGroups.value.flatMap(group => group.instances.filter(i => i.archive_destination && selectedInstances[group.id]?.has(i.file_path))))
function invalidateScan() {
  duplicateGroups.value = []
  collapsedGroups.value = new Set()
  groupSearch.value = ''
  currentPage.value = 1
  dedupSummary.value = null
  hasScanned.value = false
  showCleanupPreview.value = false
  for (const key of Object.keys(selectedInstances)) delete selectedInstances[key]
}
watch([resourceRoot, vamRootPath], invalidateScan)
async function chooseRoot() {
  const path = await open({ directory: true, multiple: false })
  if (typeof path === 'string') resourceRoot.value = path
}
function fileName(path: string) { return path.split(/[\\/]/).pop() || path }

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
  await loadTrash()
})

watch(revision, async () => {
  if (isCleaning.value || isRestoring.value || isDedupScanning.value) return
  invalidateScan()
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
      // Keep recommended copies; protected versions cannot be deselected.
      for (const group of duplicateGroups.value) {
        selectedInstances[group.id] = new Set(group.instances.filter(i => i.is_recommended_keep).map(i => i.file_path))
      }
    }
  } catch (err) {
    invalidateScan()
    notify.error(String(err))
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
  if (!effectiveRoot.value || isScanning.value || isDedupScanning.value) return
  isDedupScanning.value = true
  invalidateScan()
  try {
    dedupSummary.value = await invoke<DedupSummary>('scan_for_duplicates', {
      vamRoot: effectiveRoot.value,
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
  if ((!cleanupList.value.length && !archiveList.value.length) || isCleaning.value || !dedupSummary.value) return
  isCleaning.value = true
  try {
    const result = await invoke<{ cleaned: number; archived: number; removed_directories: number; errors: string[] }>('execute_cleanup', {
      scanId: dedupSummary.value.scan_id,
      filePaths: cleanupList.value.map(i => i.file_path),
    })
    notify.success(t('deduplication.organized', { count: result.cleaned, archive: result.archived, folders: result.removed_directories }))
    if (result.errors.length) notify.error(result.errors.join('\n'))
    showCleanupPreview.value = false
    await handleScan()
    await loadTrash()
  } catch (err) {
    notify.error(String(err), t('common.error'))
    invalidateScan()
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
    await handleScan()
  } catch (err: any) {
    notify.error(`恢复失败: ${err}`, t('common.error'))
  } finally {
    isRestoring.value = false
  }
}

function toggleGroup(groupId: string) {
  if (collapsedGroups.value.has(groupId)) collapsedGroups.value.delete(groupId)
  else collapsedGroups.value.add(groupId)
}

async function copyPaths(paths: string[]) {
  try {
    await navigator.clipboard.writeText(paths.join('\n'))
    notify.success(t('deduplication.pathsCopied', { count: paths.length }))
  } catch (err) {
    notify.error(t('deduplication.copyFailed', { error: String(err) }))
  }
}

function toggleInstance(groupId: string, pkgId: string) {
  if (!selectedInstances[groupId]) return
  if (duplicateGroups.value.find(g => g.id === groupId)?.instances.find(i => i.file_path === pkgId)?.is_recommended_keep) return
  if (selectedInstances[groupId].has(pkgId)) {
    // Don't allow deselecting the last instance
    if (selectedInstances[groupId].size <= 1) return
    selectedInstances[groupId].delete(pkgId)
  } else {
    selectedInstances[groupId].add(pkgId)
  }
}

function selectAllForKeep(group: DuplicateGroup) {
  selectedInstances[group.id] = new Set(group.instances.map(i => i.file_path))
}

function selectRecommendedForKeep(group: DuplicateGroup) {
  selectedInstances[group.id] = new Set(
    group.instances
      .filter(i => i.is_recommended_keep)
      .map(i => i.file_path)
  )
  // If no recommended, keep at least one
  if (selectedInstances[group.id].size === 0) {
    selectedInstances[group.id].add(group.instances[0].file_path)
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

const icons = {
  duplicates: 'M3 3h12v12H3zM9 9h12v12H9',
  space: 'M4 7V4a2 2 0 0 1 2-2h8.5L20 7.5V20a2 2 0 0 1-2 2H6a2 2 0 0 1-2-2v-3M2 12h16M6 9v6',
  clean: 'M22 11.08V12a10 10 0 1 1-5.93-9.14M22 4L12 14.01l-3-3',
  empty: 'M3 3h12v12H3zM9 9h12v12H9',
}
</script>

<style scoped>
.scan-options { padding: 16px; display: flex; flex-direction: column; gap: 10px; }
.scan-directory-row { display: flex; gap: 8px; }
.scan-directory-row input { flex: 1; min-width: 0; padding: 9px 12px; background: var(--bg-base); border: 1px solid var(--border-subtle); border-radius: 6px; color: var(--text-primary); }
.scan-options details { max-height: 160px; overflow: auto; overflow-wrap: anywhere; }

.deduplication-view {
  display: flex;
  flex-direction: column;
  gap: var(--space-5);
  min-height: 100%;
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
  flex: 1 0 auto;
  min-width: 0;
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

.dedup-search { display: flex; align-items: center; gap: 12px; padding: 12px 16px; flex-wrap: wrap; }
.dedup-search input { flex: 1 1 280px; min-width: 0; padding: 8px 10px; background: var(--bg-input); border: 1px solid var(--border-subtle); border-radius: var(--radius-sm); }
.dedup-body { min-height: 260px; max-height: 65vh; overflow-y: auto; padding: 12px; display: block; }
.dup-group { border: 1px solid var(--border-subtle); border-radius: var(--radius-md); overflow: hidden; }
.dup-group + .dup-group { margin-top: 12px; }
.dup-group-header { display: flex; align-items: center; justify-content: space-between; gap: 12px; padding: 12px; background: var(--bg-hover); flex-wrap: wrap; }
.dup-group-toggle { display: flex; align-items: center; gap: 8px; text-align: left; flex: 1 1 260px; min-width: 0; }
.dup-group-info { display: flex; flex-direction: column; gap: 4px; min-width: 0; }
.dup-group-path { color: var(--text-primary); font-weight: var(--font-semibold); overflow-wrap: anywhere; }
.dup-group-meta, .dup-inst-meta { font-variant-numeric: tabular-nums; }
.dup-group-actions { display: flex; align-items: center; gap: 8px; flex-wrap: wrap; }
.dup-action-btn, .dup-page-btn, .dup-copy-btn { padding: 5px 10px; border: 1px solid var(--border-subtle); border-radius: var(--radius-sm); white-space: nowrap; }
.dup-action-btn:hover, .dup-page-btn:hover:not(:disabled), .dup-copy-btn:hover { background: var(--bg-hover); }
.dup-action-keep { color: var(--color-success); }
.dup-action-clean { color: var(--accent-primary); }
.dup-page-btn:disabled, .dup-action-btn:disabled { opacity: .4; cursor: not-allowed; }
.dup-expand-icon { color: var(--text-tertiary); flex: 0 0 14px; }
.dup-instances { border-top: 1px solid var(--border-subtle); }
.dup-instance { display: grid; grid-template-columns: 16px minmax(0, 1fr) auto; align-items: start; gap: 12px; padding: 14px; }
.dup-instance + .dup-instance { border-top: 1px solid var(--border-subtle); }
.dup-instance > input { margin-top: 4px; }
.dup-inst-info { display: flex; flex-direction: column; gap: 6px; min-width: 0; }
.dup-inst-heading { display: flex; align-items: center; gap: 8px; flex-wrap: wrap; }
.dup-inst-name { font-weight: var(--font-medium); overflow-wrap: anywhere; }
.dup-file-path, .dup-inst-name { -webkit-user-select: text; user-select: text; }
.dup-file-path { display: block; overflow-wrap: anywhere; white-space: normal; line-height: 1.65; color: var(--text-secondary); }
.dup-status, .dup-source-badge { padding: 2px 7px; border-radius: 4px; font-size: var(--text-xs); white-space: nowrap; }
.dup-status.keep { color: var(--color-success); background: rgba(62, 207, 142, .1); }
.dup-status.recycle { color: var(--color-error); background: rgba(239, 68, 68, .1); }
.dup-status.archive, .dup-source-badge { color: var(--color-info); background: rgba(96, 165, 250, .1); }
.dedup-pagination { display: flex; align-items: center; justify-content: space-between; gap: 12px; flex-wrap: wrap; padding: 12px 16px; border-top: 1px solid var(--border-subtle); }
.dedup-no-match { padding: 24px; text-align: center; }
@media (max-width: 700px) {
  .dup-instance { grid-template-columns: 16px minmax(0, 1fr); }
  .dup-copy-btn { grid-column: 2; justify-self: start; }
  .action-bar { flex-wrap: wrap; }
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
  background: rgba(28, 28, 30, 0.88);
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
  box-shadow: 0 4px 12px rgba(110, 107, 240, 0.3);
  gap: var(--space-2);
}

.cleanup-confirm-btn:hover:not(:disabled) {
  transform: translateY(-1px);
  box-shadow: 0 6px 16px rgba(110, 107, 240, 0.4);
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
