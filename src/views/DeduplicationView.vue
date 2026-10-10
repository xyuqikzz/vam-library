<template>
<div class="deduplication-view animate-fadeIn b2-page"><PageHeader :title="$t('sidebar.deduplication')" ><template #actions>
      <UiButton :disabled="busy || !effectiveRoot" @click="handleScan" variant="primary" size="lg">
        <span>{{ isDedupScanning ? $t('common.loading') : $t('deduplication.scanButton') }}</span>
      </UiButton>
      <UiButton v-if="cleanupList.length > 0 || archiveList.length > 0" :disabled="busy" @click="showCleanupPreview = true" variant="secondary">
        <Icon  :size="14" name="outline-16" />
        <span>{{ t('deduplication.reviewOrganize', { count: cleanupList.length, archive: archiveList.length }) }}</span>
      </UiButton>
      <UiButton v-if="trashList.length > 0" :disabled="isRestoring" @click="handleRestoreLatest" variant="secondary">
        <Icon  :size="14" name="outline-17" />
        <span>{{ $t('deduplication.restoreLatest', { count: trashList.length }) }}</span>
      </UiButton>
    </template></PageHeader><div class="b2-page-body">
    <!-- Summary Cards -->
    <section class="summary-grid stagger-children">
      <StatCard
        :title="$t('deduplication.duplicateGroups')"
        :value="dedupSummary?.duplicate_groups ?? 0"
        :subtitle="$t('deduplication.duplicateGroupsSub')"
        :icon="icons.duplicates"
        :trend="(dedupSummary?.duplicate_groups ?? 0) > 0 ? 'up' : 'neutral'"
        :color="(dedupSummary?.duplicate_groups ?? 0) > 0 ? 'var(--color-warning)' : 'var(--color-success)'"
      />
      <StatCard
        :title="$t('deduplication.wastedSpace')"
        :value="((dedupSummary?.total_wasted_bytes ?? 0) > 0) ? formatSize(dedupSummary!.total_wasted_bytes) : '—'"
        :subtitle="$t('deduplication.wastedSpaceSub')"
        :icon="icons.space"
        trend="neutral"
        color="var(--color-error)"
      />
      <StatCard
        :title="$t('deduplication.safeToClean')"
        :value="dedupSummary?.safe_to_clean_count ?? 0"
        :subtitle="$t('deduplication.safeToCleanSub')"
        :icon="icons.clean"
        trend="neutral"
        color="var(--color-success)"
      />
    </section>

    <section class="scan-options glass-panel">
      <label for="dedup-root">{{ t('deduplication.sourceDirectory') }}</label>
      <div class="scan-directory-row">
        <UiInput id="dedup-root" v-model="resourceRoot" :placeholder="defaultRoot" :disabled="busy" />
        <UiButton :disabled="busy" @click="chooseRoot" variant="secondary">{{ t('deduplication.browse') }}</UiButton>
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


    <!-- Keep groups at their natural height; paginate large scan results. -->
    <section v-if="duplicateGroups.length > 0" class="dedup-content glass-panel">
      <div class="dedup-header">
        <h3 class="dedup-header-title">{{ t('deduplication.duplicates') }}</h3>
        <span class="text-xs text-tertiary">{{ t('deduplication.filteredGroups', { count: filteredGroups.length, total: duplicateGroups.length }) }}</span>
      </div>
      <div class="dedup-search">
        <UiInput v-model="groupSearch" type="search" :aria-label="t('deduplication.searchPaths')" :placeholder="t('deduplication.searchPaths')" search />
        <span class="text-xs text-tertiary">{{ t('deduplication.pathsHint') }}</span>
      </div>
      <div ref="groupListEl" class="dedup-body">
        <article v-for="group in visibleGroups" :key="group.id" class="dup-group">
          <div class="dup-group-header">
            <UiButton class="dup-group-toggle" :aria-expanded="!collapsedGroups.has(group.id)" @click="toggleGroup(group.id)" variant="secondary">
              <span class="dup-expand-icon">{{ collapsedGroups.has(group.id) ? '▸' : '▾' }}</span>
              <span class="dup-group-info">
                <span class="dup-group-path text-sm">{{ group.resource_path }}</span>
                <span class="dup-group-meta text-xs text-tertiary">
                  {{ t('deduplication.copies', { count: group.file_count }) }} · {{ t('deduplication.wasted', { size: formatSize(group.total_wasted_bytes) }) }}
                </span>
              </span>
            </UiButton>
            <div class="dup-group-actions">
              <UiButton class="text-xs" @click="copyPaths(group.instances.map(i => i.file_path))" variant="secondary">{{ t('deduplication.copyGroupPaths') }}</UiButton>
              <UiButton v-if="!allSelectedFor(group)" class="dup-action-keep text-xs" :disabled="busy" @click="selectAllForKeep(group)" variant="secondary">{{ t('deduplication.keepAll') }}</UiButton>
              <UiButton v-else class="dup-action-clean text-xs" :disabled="busy" @click="selectRecommendedForKeep(group)" variant="secondary">{{ t('deduplication.keepRecommended') }}</UiButton>
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
              <UiButton class="text-xs" :aria-label="t('deduplication.copyFilePath', { path: instance.file_path })" @click="copyPaths([instance.file_path])" variant="ghost">{{ t('deduplication.copyPath') }}</UiButton>
            </div>
          </div>
        </article>
        <p v-if="!filteredGroups.length" class="dedup-no-match text-sm text-secondary">{{ t('deduplication.noMatchingGroups') }}</p>
      </div>
      <nav class="dedup-pagination" :aria-label="t('deduplication.groupPages')">
        <span class="text-xs text-tertiary">{{ t('deduplication.pageInfo', { page: currentPage, total: pageCount, size: pageSize }) }}</span>
        <div class="dup-group-actions">
          <UiButton :disabled="currentPage <= 1" @click="currentPage--" variant="ghost">{{ t('deduplication.previousPage') }}</UiButton>
          <UiButton :disabled="currentPage >= pageCount" @click="currentPage++" variant="ghost">{{ t('deduplication.nextPage') }}</UiButton>
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
    <UiModal v-if="showCleanupPreview" :show-close="false"  :close-disabled="isCleaning" @close="showCleanupPreview = false">
          <div class="modal-header">
            <h3 class="modal-title">
              <Icon class="title-icon" :size="18" name="outline-18" />
              {{ $t('deduplication.confirmModalTitle') }}
            </h3>
            <UiButton :disabled="isCleaning" @click="showCleanupPreview = false" variant="ghost" icon-only :aria-label="$t('common.close')">
              <Icon  :size="16" name="outline-9" />
            </UiButton>
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
              <UiButton :disabled="isCleaning" @click="showCleanupPreview = false" variant="secondary">
                {{ $t('common.cancel') }}
              </UiButton>
              <UiButton :disabled="isCleaning" @click="handleCleanup" variant="danger">
                <span v-if="isCleaning" class="spinner"></span>
                <span>{{ isCleaning ? $t('deduplication.cleaning') : $t('deduplication.confirmCleanup') }}</span>
              </UiButton>
            </div>
          </div>
        </UiModal>
  </div></div>
</template>

<script setup lang="ts">
import UiModal from '@/components/ui/UiModal.vue'
import PageHeader from '@/components/layout/PageHeader.vue'
import Icon from '@/components/ui/Icon.vue'
import UiButton from '@/components/ui/UiButton.vue'
import UiInput from '@/components/ui/UiInput.vue'
import { formatSize } from '@/utils/bytes'
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
.scan-directory-row input { flex: 1; min-width: 0; padding: 9px 12px; background: var(--bg-base); border: 1px solid var(--border-subtle); border-radius: var(--radius-xs); color: var(--text-primary); }
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

.spin-icon {
  animation: spin 1s linear infinite;
  transform-origin: center;
}

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
.dup-group { border: none; border-radius: var(--radius-md); overflow: hidden; }
.dup-group + .dup-group { margin-top: 12px; }
.dup-group-header { display: flex; align-items: center; justify-content: space-between; gap: 12px; padding: 12px; background: var(--bg-hover); flex-wrap: wrap; }
.dup-group-toggle { display: flex; align-items: center; gap: 8px; text-align: left; flex: 1 1 260px; min-width: 0; }
.dup-group-info { display: flex; flex-direction: column; gap: 4px; min-width: 0; }
.dup-group-path { color: var(--text-primary); font-weight: var(--font-semibold); overflow-wrap: anywhere; }
.dup-group-meta, .dup-inst-meta { font-variant-numeric: tabular-nums; }
.dup-group-actions { display: flex; align-items: center; gap: 8px; flex-wrap: wrap; }
.dup-action-keep { color: var(--color-success); }
.dup-action-clean { color: var(--accent-primary); }
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
.dup-status, .dup-source-badge { padding: 2px 7px; border-radius: var(--radius-progress); font-size: var(--text-xs); white-space: nowrap; }
.dup-status.keep { color: var(--color-success); background: var(--color-success-bg); }
.dup-status.recycle { color: var(--color-error); background: var(--color-error-bg); }
.dup-status.archive, .dup-source-badge { color: var(--color-info); background: var(--color-info-bg); }
.dedup-pagination { display: flex; align-items: center; justify-content: space-between; gap: 12px; flex-wrap: wrap; padding: 12px 16px; border-top: 1px solid var(--border-subtle); }
.dedup-no-match { padding: 24px; text-align: center; }
@media (max-width: 700px) {
  .dup-instance { grid-template-columns: 16px minmax(0, 1fr); }
}

/* ── Cleanup Modal ────────────────────────────────────────── */

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
  border: none;
  border-radius: var(--radius-md);
  background: var(--border-subtle);
  padding: var(--space-1) 0;
}

.cleanup-preview-item {
  display: flex;
  flex-direction: column;
  gap: 2px;
  padding: var(--space-2) var(--space-3);
  border-bottom: 1px solid var(--bg-subtle);
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

.spinner {
  width: 16px;
  height: 16px;
  border: none;
  border-top-color: var(--text-primary);
  border-radius: var(--radius-xl)%;
  animation: spin 1s linear infinite;
}

.list-scrollbar::-webkit-scrollbar {
  width: 6px;
}

.list-scrollbar::-webkit-scrollbar-track {
  background: transparent;
}

.list-scrollbar::-webkit-scrollbar-thumb {
  background: var(--border-default);
  border-radius: var(--radius-full);
}

.list-scrollbar::-webkit-scrollbar-thumb:hover {
  background: var(--border-strong);
}

@keyframes spin {
  to { transform: rotate(360deg); }
}

/* Modal Animations */

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

.b2-page { display: flex; flex-direction: column; height: 100%; min-height: 0; max-width: none; margin: 0; padding: 0; gap: 0; overflow: hidden; }
.b2-page-body { flex: 1; min-height: 0; min-width: 0; overflow: auto; padding: 20px 24px; display: flex; flex-direction: column; gap: 20px; }
.b2-page :deep(.page-header) { width: 100%; }
@media(max-width: 800px) { .b2-page-body { padding: 12px; } }
</style>
