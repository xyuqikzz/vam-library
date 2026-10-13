<template>
  <div class="deduplication-view animate-fadeIn b2-page">
    <PageHeader :title="$t('sidebar.deduplication')" />
    <div class="b2-page-body">
      <section class="dedup-setup" :aria-label="t('deduplication.sourceDirectory')">
        <div class="dedup-setup-row">
          <label class="dedup-setup-label" for="dedup-root">{{ t('deduplication.sourceDirectory') }}</label>
          <UiInput id="dedup-root" v-model="resourceRoot" class="dedup-root" size="md" :placeholder="defaultRoot" :disabled="busy" />
          <UiButton :disabled="busy" icon="folder" @click="chooseRoot">{{ t('deduplication.browse') }}</UiButton>
          <UiSegmented v-model="analysisMode" class="dedup-mode-filter" :label="t('deduplication.analysisMode')" :options="modeOptions" />
          <UiButton v-if="isDedupScanning" :disabled="isCancelling" @click="cancelScan">
            {{ t(isCancelling ? 'deduplication.stopping' : 'deduplication.stopAnalysis') }}
          </UiButton>
          <UiButton v-else :disabled="busy || !effectiveRoot" variant="primary" icon="search" @click="handleScan">
            {{ t(hasScanned ? 'deduplication.rescanButton' : 'deduplication.scanButton') }}
          </UiButton>
        </div>
        <div v-if="!hasScanned && (analysisMode === 'deep' || trashList.length)" class="dedup-setup-foot">
          <p v-if="analysisMode === 'deep'" class="dedup-help">{{ t('deduplication.deepHelp') }}</p>
          <UiButton v-if="trashList.length" variant="ghost" size="sm" icon="refresh" :disabled="busy" @click="handleRestoreLatest">
            {{ t('deduplication.restoreLatest', { count: trashList.length }) }}
          </UiButton>
        </div>
      </section>

      <section v-if="isDedupScanning" class="dedup-status" role="status" aria-live="polite">
        <div class="dedup-status-row">
          <span class="spinner-ring sm" aria-hidden="true"></span>
          <strong>{{ t(`deduplication.stages.${scanProgress.stage}`) }}</strong>
          <span v-if="scanProgress.total" class="dedup-status-count">{{ scanProgress.current }} / {{ scanProgress.total }}</span>
        </div>
        <UiProgress :value="scanProgress.current" :max="scanProgress.total || 1" :indeterminate="!scanProgress.total" :label="t('deduplication.progressLabel')" />
      </section>
      <section v-else-if="scanCancelled" class="dedup-status" role="status"><p class="dedup-help">{{ t('deduplication.cancelled') }}</p></section>
      <section v-if="scanError || cleanupError" class="dedup-status dedup-alert">
        <p role="alert">{{ t(cleanupError ? 'deduplication.cleanupFailed' : 'deduplication.analysisFailed') }}</p>
        <details><summary>{{ t('deduplication.errorDetails') }}</summary><p class="dedup-mono">{{ cleanupError || scanError }}</p></details>
      </section>
      <section v-if="!hasScanned && !isDedupScanning && !scanError && !cleanupError && !scanCancelled" class="dedup-empty">
        <EmptyState :icon="icons.empty" :title="t('deduplication.readyTitle')" :description="t('deduplication.readyHint')" hide-action />
      </section>
      <section v-else-if="hasScanned && !duplicateGroups.length" class="dedup-empty">
        <EmptyState :icon="icons.empty" :title="t('deduplication.noDuplicates')" :description="emptyStateDescription" hide-action />
      </section>

      <section v-else-if="hasScanned" class="dedup-results" :aria-label="t('deduplication.duplicates')">
        <div class="dedup-workspace">
          <aside class="dedup-browser" :aria-label="t('deduplication.groupList')">
            <UiSegmented v-model="groupFilter" class="dedup-group-filter" :label="t('deduplication.groupFilter')" :options="groupFilterOptions" />
            <UiInput v-model="groupSearch" class="dedup-search" type="search" :aria-label="t('deduplication.searchPaths')" :placeholder="t('deduplication.searchPaths')" search />
            <div ref="groupListEl" class="dedup-groups">
              <button v-for="group in visibleGroups" :key="group.id" type="button" class="dedup-group-row"
                :class="{ selected: activeGroupId === group.id }" :aria-pressed="activeGroupId === group.id" @click="activeGroupId = group.id">
                <span class="dup-thumb" aria-hidden="true">
                  <img data-resource-preview v-if="thumbUrl(groupCover(group))" :src="thumbUrl(groupCover(group))!" alt="" loading="lazy" @error="markThumbFailed(groupCover(group))" />
                  <Icon v-else name="package" :size="22" />
                </span>
                <span class="dedup-group-copy">
                  <strong class="dedup-group-name">{{ group.resource_path }}</strong>
                  <span class="dedup-meta">{{ t('deduplication.copies', { count: group.instances.length }) }} · {{ groupSpace(group) ? t('deduplication.wasted', { size: formatSize(groupSpace(group)) }) : t('deduplication.keepStatus') }}</span>
                </span>
                <Icon name="chevron-right" :size="16" />
              </button>
              <p v-if="!filteredGroups.length" class="dedup-no-match">{{ t('deduplication.noMatchingGroups') }}</p>
            </div>
            <nav v-if="pageCount > 1" class="dedup-pagination" :aria-label="t('deduplication.groupPages')">
              <span class="dedup-meta">{{ currentPage }} / {{ pageCount }}</span>
              <div class="dedup-pager">
                <UiButton size="sm" variant="ghost" icon="chevrons-left" icon-only :aria-label="t('deduplication.firstPage')" :disabled="currentPage <= 1" @click="currentPage = 1" />
                <UiButton size="sm" variant="ghost" icon="chevron-left" icon-only :aria-label="t('deduplication.previousPage')" :disabled="currentPage <= 1" @click="currentPage--" />
                <UiButton size="sm" variant="ghost" icon="chevron-right" icon-only :aria-label="t('deduplication.nextPage')" :disabled="currentPage >= pageCount" @click="currentPage++" />
                <UiButton size="sm" variant="ghost" icon="chevrons-right" icon-only :aria-label="t('deduplication.lastPage')" :disabled="currentPage >= pageCount" @click="currentPage = pageCount" />
              </div>
            </nav>
          </aside>

          <section v-if="activeGroup" class="dedup-comparison" :aria-label="activeGroup.resource_path">
            <header class="comparison-heading">
              <div class="comparison-copy">
                <h2 class="comparison-title">{{ activeGroup.resource_path }}</h2>
                <p class="dedup-help">{{ t(`deduplication.comparisonHints.${activeGroup.strategy}`) }}</p>
              </div>
              <UiButton :disabled="busy" @click="allSelectedFor(activeGroup) ? selectRecommendedForKeep(activeGroup) : selectAllForKeep(activeGroup)">
                {{ t(allSelectedFor(activeGroup) ? 'deduplication.keepRecommended' : 'deduplication.keepAll') }}
              </UiButton>
            </header>
            <div class="comparison-scroll">
              <div class="comparison-grid resource-grid">
                <article v-for="instance in comparedInstances" :key="instance.file_path" class="dup-file" :class="instanceStatus(activeGroup, instance)">
                  <div class="comparison-card">
                    <button type="button" class="comparison-image" :disabled="busy || !isPackageFile(instance)"
                      :aria-label="t('deduplication.viewDetails', { name: fileName(instance.file_path) })" @click="openDetails(instance)">
                      <img v-if="thumbUrl(instance)" data-resource-preview :src="thumbUrl(instance)!" :alt="fileName(instance.file_path)" @error="markThumbFailed(instance)" />
                      <Icon v-else :name="isPackageFile(instance) ? 'package' : 'copy'" :size="40" />
                    </button>
                    <span class="dup-badge comparison-badge" :class="instanceStatus(activeGroup, instance)">
                      <Icon :name="instanceStatus(activeGroup, instance) === 'recycle' ? 'trash' : 'check'" :size="14" />
                      {{ t(instanceStatus(activeGroup, instance) === 'archive' ? 'deduplication.willArchive' : instanceStatus(activeGroup, instance) === 'recycle' ? 'deduplication.recycleStatus' : instance.is_recommended_keep ? 'deduplication.lockedKeep' : 'deduplication.keepStatus') }}
                    </span>
                    <div class="comparison-card-info">
                      <strong class="comparison-file-name" :title="fileName(instance.file_path)">{{ fileName(instance.file_path) }}</strong>
                      <span class="comparison-location" :title="instance.file_path"><Icon name="folder" :size="14" /><span>{{ relativeFolder(instance.file_path) }}</span><span class="comparison-size">{{ formatSize(instance.size_bytes) }}</span></span>
                    </div>
                  </div>
                  <div class="comparison-file-actions">
                    <UiCheckbox :model-value="isKept(activeGroup, instance)" :disabled="busy || instance.is_recommended_keep || (isKept(activeGroup, instance) && (selectedInstances[activeGroup.id]?.size ?? 0) <= 1)"
                      :aria-label="t('deduplication.keepFile', { path: instance.file_path })" @update:model-value="toggleInstance(activeGroup.id, instance.file_path)">{{ t('deduplication.keepStatus') }}</UiCheckbox>
                    <UiButton size="sm" variant="ghost" icon="copy" icon-only :aria-label="t('deduplication.copyFilePath', { path: instance.file_path })" @click="copyPaths([instance.file_path])" />
                  </div>
                  <p v-if="instance.protection_reason" class="comparison-reason">{{ t(`deduplication.protection.${instance.protection_reason}`) }}</p>
                  <p v-else-if="instance.referenced_by.length" class="comparison-reason" :title="instance.referenced_by.join('\n')">{{ t('deduplication.pinned') }}</p>
                  <p v-if="instance.archive_destination" class="comparison-reason" :title="instance.archive_destination">{{ t('deduplication.archiveExplanation') }}</p>
                </article>
              </div>
              <UiButton v-if="activeGroup.instances.length > comparisonLimit" variant="ghost" class="comparison-more" @click="comparisonLimit += 12">{{ t('deduplication.planShowMore', { count: Math.min(12, activeGroup.instances.length - comparisonLimit) }) }}</UiButton>
              <details v-if="dedupSummary?.warnings.length" class="dedup-rules">
                <summary>{{ t('deduplication.scanWarnings', { count: dedupSummary.warnings.length }) }}</summary>
                <p v-for="warning in dedupSummary.warnings" :key="warning" class="dedup-mono">{{ warning }}</p>
              </details>
            </div>
            <p class="comparison-safety"><Icon name="info" :size="18" />{{ t('deduplication.safetyHint') }}</p>
          </section>
          <div v-else class="dedup-comparison dedup-no-match">{{ t('deduplication.noMatchingGroups') }}</div>
        </div>
        <div class="dedup-planbar" role="region" :aria-label="t('deduplication.reviewPlan')">
          <div class="dedup-planbar-copy">
            <strong>{{ cleanupList.length ? t('deduplication.planBarRecycle', { count: cleanupList.length }) : t('deduplication.planBarNothing') }}</strong>
            <span><template v-if="archiveList.length">{{ t('deduplication.planBarArchive', { count: archiveList.length }) }} · </template>{{ t('deduplication.planBarSpace', { size: formatSize(freedBytes) }) }}</span>
          </div>
          <div class="dedup-planbar-actions">
            <UiButton v-if="trashList.length" variant="ghost" icon="refresh" :disabled="busy" @click="handleRestoreLatest">{{ t('deduplication.restoreLatest', { count: trashList.length }) }}</UiButton>
            <UiButton variant="primary" :disabled="busy || (!cleanupList.length && !archiveList.length)" @click="openPlan">{{ t('deduplication.reviewPlan') }}</UiButton>
          </div>
        </div>
      </section>

      <!-- 整理方案：带预览图，点开可看资源详情 -->
      <UiModal
        v-if="showCleanupPreview"
        large
        :title="t('deduplication.planTitle')"
        :description="t('deduplication.planDesc')"
        :close-disabled="isCleaning"
        @close="showCleanupPreview = false"
      >
        <section v-if="isCleaning" class="cleanup-progress" role="status" aria-live="polite">
          <div class="dedup-status-row"><strong>{{ t(`deduplication.cleanupStages.${cleanupProgress.stage}`) }}</strong><span v-if="cleanupProgress.total">{{ cleanupProgress.current }} / {{ cleanupProgress.total }}</span></div>
          <UiProgress :value="cleanupProgress.current" :max="cleanupProgress.total || 1" :indeterminate="!cleanupProgress.total" :label="t('deduplication.cleanupProgress')" />
          <p v-if="cleanupProgress.file_name" class="cleanup-current-file">{{ cleanupProgress.file_name }}</p>
        </section>
        <dl v-else class="plan-stats">
          <div class="plan-stat">
            <dt>{{ t('deduplication.planStatRecycle') }}</dt>
            <dd>{{ cleanupList.length }}</dd>
          </div>
          <div class="plan-stat">
            <dt>{{ t('deduplication.planStatArchive') }}</dt>
            <dd>{{ archiveList.length }}</dd>
          </div>
          <div class="plan-stat">
            <dt>{{ t('deduplication.planStatSpace') }}</dt>
            <dd>{{ formatSize(freedBytes) }}</dd>
          </div>
        </dl>

        <p v-if="!cleanupList.length && !archiveList.length" class="dedup-help">{{ t('deduplication.planEmpty') }}</p>

        <section v-for="section in (isCleaning ? [] : planSections)" :key="section.key" class="plan-section">
          <h3 class="plan-section-title">
            {{ section.title }} <span class="plan-count">{{ section.items.length }}</span>
          </h3>
          <ul class="plan-list">
            <li v-for="item in section.items.slice(0, planLimit[section.key])" :key="item.file_path" class="plan-item">
              <button
                v-if="isPackageFile(item)"
                type="button"
                class="dup-thumb lg"
                :disabled="isCleaning"
                :aria-label="t('deduplication.viewDetails', { name: fileName(item.file_path) })"
                @click="openDetails(item)"
              >
                <img
                  v-if="thumbUrl(item)"
                  data-resource-preview
                  :src="thumbUrl(item)!"
                  :alt="fileName(item.file_path)"
                  loading="lazy"
                  @error="markThumbFailed(item)"
                />
                <Icon v-else name="package" :size="22" />
              </button>
              <span v-else class="dup-thumb lg" aria-hidden="true"><Icon name="copy" :size="22" /></span>
              <div class="dup-file-info">
                <span class="dup-file-name">{{ fileName(item.file_path) }}</span>
                <span class="dup-file-path">{{ item.file_path }}</span>
                <span v-if="section.key === 'archive'" class="dup-file-path">→ {{ item.archive_destination }}</span>
                <span class="dup-file-meta">{{ formatSize(item.size_bytes) }}</span>
              </div>
              <UiButton
                v-if="section.key === 'recycle'"
                size="sm"
                variant="ghost"
                :disabled="isCleaning"
                @click="keepInstead(item)"
              >
                {{ t('deduplication.planKeepInstead') }}
              </UiButton>
            </li>
          </ul>
          <UiButton
            v-if="section.items.length > planLimit[section.key]"
            size="sm"
            variant="ghost"
            @click="planLimit[section.key] += PLAN_STEP"
          >
            {{ t('deduplication.planShowMore', { count: Math.min(PLAN_STEP, section.items.length - planLimit[section.key]) }) }}
          </UiButton>
        </section>

        <template #footer>
          <UiButton :disabled="isCleaning" @click="showCleanupPreview = false">
            {{ $t('common.cancel') }}
          </UiButton>
          <UiButton
            variant="primary"
            :loading="isCleaning"
            :loading-text="t('deduplication.cleaning')"
            :disabled="!cleanupList.length && !archiveList.length"
            @click="handleCleanup"
          >
            {{ t('deduplication.organizeCount', { count: cleanupList.length + archiveList.length }) }}
          </UiButton>
        </template>
      </UiModal>

      <PackageDetails
        v-if="detailItem"
        :key="detailItem.id"
        :item="detailItem"
        :image="thumbnails[detailItem.id.toLowerCase()]"
        :expanded="true"
        @close="detailItem = null"
        @update:expanded="value => { if (!value) detailItem = null }"
      />
    </div>
  </div>
</template>

<script setup lang="ts">
import UiModal from '@/components/ui/UiModal.vue'
import PageHeader from '@/components/layout/PageHeader.vue'
import Icon from '@/components/ui/Icon.vue'
import UiButton from '@/components/ui/UiButton.vue'
import UiCheckbox from '@/components/ui/UiCheckbox.vue'
import UiInput from '@/components/ui/UiInput.vue'
import UiProgress from '@/components/ui/UiProgress.vue'
import UiSegmented from '@/components/ui/UiSegmented.vue'
import PackageDetails from '@/components/packages/PackageDetails.vue'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import { formatSize } from '@/utils/bytes'
import { loadPackageThumbnails } from '@/utils/resourceCache'
import type { ResourceDetailItem } from '@/types/package'
import { nextTick, onMounted, onBeforeUnmount, onDeactivated, ref, reactive, computed, watch } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { open } from '@tauri-apps/plugin-dialog'
import { storeToRefs } from 'pinia'
import { useI18n } from 'vue-i18n'
import { useAppStore } from '@/stores/app'
import { useLocalLibraryStore } from '@/stores/localLibrary'
import { useNotification } from '@/composables/useNotification'
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
  protection_reason?: string | null
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
  mode: 'standard' | 'deep'
  cache_hits: number
  bytes_read: number
  elapsed_ms: number
}

interface CleanupTrashEntry {
  id: number
  package_id: string
  original_path: string
  trash_path: string
  size_bytes: number
  created_at: string
}

type InstanceStatus = 'keep' | 'recycle' | 'archive'

const appStore = useAppStore()
const localLibraryStore = useLocalLibraryStore()
const { t } = useI18n()
const { vamRootPath, isScanning } = storeToRefs(appStore)
const { revision } = storeToRefs(localLibraryStore)
const notify = useNotification()

const resourceRoot = ref('')
const analysisMode = ref('standard')
const modeOptions = computed(() => ['standard', 'deep'].map(value => ({ value, label: t(`deduplication.${value}Mode`), disabled: busy.value })))
const scanProgress = ref({ stage: 'inventory', current: 0, total: 0 })
const scanError = ref('')
const scanCancelled = ref(false)
const isCancelling = ref(false)
let scanEpoch = 0
let activeRequest: string | null = null
let unlistenProgress: UnlistenFn | null = null
let disposed = false
async function cancelScan() {
  if (!activeRequest || isCancelling.value) return
  isCancelling.value = true
  try { await invoke('cancel_dedup_scan', { requestId: activeRequest }) }
  catch (err) { isCancelling.value = false; scanError.value = String(err) }
}
function leaveScan() {
  ++scanEpoch
  void cancelScan()
  unlistenProgress?.(); unlistenProgress = null
}
onBeforeUnmount(() => { disposed = true; leaveScan(); unlistenCleanup?.() })
onDeactivated(leaveScan)
const defaultRoot = computed(() => vamRootPath.value ? `${vamRootPath.value}/AddonPackages` : '')
const effectiveRoot = computed(() => resourceRoot.value.trim() || defaultRoot.value)
const busy = computed(() => isDedupScanning.value || isScanning.value || isCleaning.value || isRestoring.value)
const dedupSummary = ref<DedupSummary | null>(null)
const duplicateGroups = ref<DuplicateGroup[]>([])
const hasScanned = ref(false)
const groupFilter = ref('cleanup')
const activeGroupId = ref('')
const comparisonLimit = ref(12)
const groupSearch = ref('')
const currentPage = ref(1)
const pageSize = 20
const groupListEl = ref<HTMLElement | null>(null)
const filteredGroups = computed(() => {
  const query = groupSearch.value.trim().toLocaleLowerCase().replace(/\\/g, '/')
  const groups = duplicateGroups.value.filter(group => canCleanGroup(group) === (groupFilter.value === 'cleanup'))
  if (!query) return groups
  const matches = (value: string) => value.toLocaleLowerCase().replace(/\\/g, '/').includes(query)
  return groups.filter(group => matches(group.resource_path) || group.instances.some(i => matches(i.file_path)))
})
const pageCount = computed(() => Math.max(1, Math.ceil(filteredGroups.value.length / pageSize)))
const visibleGroups = computed(() => filteredGroups.value.slice((currentPage.value - 1) * pageSize, currentPage.value * pageSize))
const activeGroup = computed(() => visibleGroups.value.find(group => group.id === activeGroupId.value) ?? null)
const comparedInstances = computed(() => [...(activeGroup.value?.instances ?? [])]
  .sort((a, b) => Number(b.is_recommended_keep) - Number(a.is_recommended_keep)).slice(0, comparisonLimit.value))
const groupFilterOptions = computed(() => [
  { value: 'cleanup', label: t('deduplication.cleanableGroups'), count: duplicateGroups.value.filter(canCleanGroup).length },
  { value: 'keep', label: t('deduplication.retainedGroups'), count: duplicateGroups.value.filter(g => !canCleanGroup(g)).length },
])
watch([groupSearch, groupFilter], () => { currentPage.value = 1 })
watch(visibleGroups, groups => {
  if (!groups.some(group => group.id === activeGroupId.value)) activeGroupId.value = groups[0]?.id ?? ''
})
watch(activeGroupId, () => { comparisonLimit.value = 12 })
watch(pageCount, count => { currentPage.value = Math.min(currentPage.value, count) })
watch(currentPage, async () => {
  await nextTick()
  if (groupListEl.value) groupListEl.value.scrollTop = 0
})
const showCleanupPreview = ref(false)
const isCleaning = ref(false)
const isRestoring = ref(false)
const isDedupScanning = ref(false)
const trashList = ref<CleanupTrashEntry[]>([])
const cleanupError = ref('')
interface CleanupProgress { scan_id: string; stage: string; current: number; total: number; file_name: string }
const cleanupProgress = ref<CleanupProgress>({ scan_id: '', stage: 'waiting', current: 0, total: 0, file_name: '' })
let unlistenCleanup: UnlistenFn | null = null

// Track selected (keep) instances per group
const selectedInstances = reactive<Record<string, Set<string>>>({})

// Every duplicate instance that is NOT kept goes to the recycle bin.
const cleanupList = computed(() => {
  const list: DuplicateInstance[] = []
  for (const group of duplicateGroups.value) {
    const selected = selectedInstances[group.id]
    if (!selected) continue
    for (const instance of group.instances) {
      if (!selected.has(instance.file_path)) list.push(instance)
    }
  }
  return list
})
const archiveList = computed(() => duplicateGroups.value.flatMap(group => group.instances.filter(i => i.archive_destination && selectedInstances[group.id]?.has(i.file_path))))
const freedBytes = computed(() => cleanupList.value.reduce((sum, item) => sum + item.size_bytes, 0))
const groupByPath = computed(() => {
  const map = new Map<string, string>()
  for (const group of duplicateGroups.value) for (const instance of group.instances) map.set(instance.file_path, group.id)
  return map
})

function isKept(group: DuplicateGroup, instance: DuplicateInstance) {
  return Boolean(selectedInstances[group.id]?.has(instance.file_path))
}
function instanceStatus(group: DuplicateGroup, instance: DuplicateInstance): InstanceStatus {
  if (!isKept(group, instance)) return 'recycle'
  return instance.archive_destination ? 'archive' : 'keep'
}
function canCleanGroup(group: DuplicateGroup) { return group.instances.some(i => !i.is_recommended_keep) }
function groupSpace(group: DuplicateGroup) {
  return group.instances.reduce((size, item) => size + (isKept(group, item) ? 0 : item.size_bytes), 0)
}
function groupCover(group: DuplicateGroup) { return group.instances.find(i => i.is_recommended_keep) ?? group.instances[0] }
function relativeFolder(path: string) {
  const normalized = path.replace(/\\/g, '/')
  const root = (dedupSummary.value?.root ?? effectiveRoot.value).replace(/\\/g, '/').replace(/\/$/, '')
  const relative = normalized.toLowerCase().startsWith(root.toLowerCase() + '/') ? normalized.slice(root.length + 1) : normalized
  return relative.split('/').slice(0, -1).join(' / ') || fileName(root)
}

function invalidateScan() {
  duplicateGroups.value = []
  activeGroupId.value = ''
  groupFilter.value = 'cleanup'
  groupSearch.value = ''
  currentPage.value = 1
  dedupSummary.value = null
  hasScanned.value = false
  showCleanupPreview.value = false
  detailItem.value = null
  for (const key of Object.keys(selectedInstances)) delete selectedInstances[key]
}
watch([resourceRoot, vamRootPath, analysisMode], () => {
  ++scanEpoch
  void cancelScan()
  invalidateScan()
})
async function chooseRoot() {
  const path = await open({ directory: true, multiple: false })
  if (typeof path === 'string') resourceRoot.value = path
}
function fileName(path: string) { return path.split(/[\\/]/).pop() || path }

const emptyStateDescription = computed(() => {
  if (!hasScanned.value || !dedupSummary.value) return t('deduplication.noDuplicatesDesc')
  return t('deduplication.noDuplicatesScannedDesc', { count: dedupSummary.value.total_files })
})

// ── Previews & details ─────────────────────────────────────────
const detailItem = ref<ResourceDetailItem | null>(null)
const thumbnails = ref<Record<string, string | null>>({})
const failedThumbs = ref(new Set<string>())
const libraryById = computed(() => new Map(
  (localLibraryStore.packages ?? []).map(pkg => [pkg.id.toLowerCase(), pkg as ResourceDetailItem]),
))
function resourceItem(id: string): ResourceDetailItem {
  return libraryById.value.get(id.toLowerCase()) ?? { id, name: id, creator: id.split('.')[0], resource_types: [] }
}
function isPackageFile(instance: DuplicateInstance) { return /\.var$/i.test(instance.file_path) && Boolean(instance.package_id) }
function thumbUrl(instance: DuplicateInstance) {
  const key = instance.package_id.toLowerCase()
  const url = thumbnails.value[key]
  return url && !failedThumbs.value.has(key) ? url : null
}
function markThumbFailed(instance: DuplicateInstance) {
  failedThumbs.value = new Set(failedThumbs.value).add(instance.package_id.toLowerCase())
}
function openDetails(instance: DuplicateInstance) {
  if (busy.value || !isPackageFile(instance)) return
  detailItem.value = resourceItem(instance.package_id)
}

const PLAN_STEP = 50
const planLimit = reactive({ recycle: PLAN_STEP, archive: PLAN_STEP })
const planSections = computed(() => [
  { key: 'recycle' as const, title: t('deduplication.planStatRecycle'), items: cleanupList.value },
  { key: 'archive' as const, title: t('deduplication.archivePreview'), items: archiveList.value },
].filter(section => section.items.length))
function openPlan() {
  planLimit.recycle = PLAN_STEP
  planLimit.archive = PLAN_STEP
  showCleanupPreview.value = true
}
function keepInstead(instance: DuplicateInstance) {
  const groupId = groupByPath.value.get(instance.file_path)
  if (groupId && !selectedInstances[groupId]?.has(instance.file_path)) toggleInstance(groupId, instance.file_path)
}

// Only load previews for what is on screen: the current page, plus the open plan.
const thumbnailIds = computed(() => {
  const instances = [...visibleGroups.value.map(groupCover), ...comparedInstances.value]
  if (showCleanupPreview.value) {
    instances.push(...cleanupList.value.slice(0, planLimit.recycle), ...archiveList.value.slice(0, planLimit.archive))
  }
  return [...new Set(instances.filter(isPackageFile).map(i => i.package_id))]
})
let thumbnailEpoch = 0
watch(thumbnailIds, ids => {
  const generation = ++thumbnailEpoch
  const missing = ids.filter(id => !(id.toLowerCase() in thumbnails.value))
  if (!missing.length) return
  void loadPackageThumbnails(missing.map(resourceItem), (id, url) => {
    thumbnails.value = { ...thumbnails.value, [id.toLowerCase()]: url }
  }, () => !disposed && generation === thumbnailEpoch).catch(() => {})
})

onMounted(async () => {
  await localLibraryStore.ensureLoaded()
  await loadTrash()
})

watch(revision, async () => {
  if (isCleaning.value || isRestoring.value || isDedupScanning.value) return
  invalidateScan()
  thumbnails.value = {}
  failedThumbs.value = new Set()
  await loadTrash()
})

async function loadTrash() {
  try {
    trashList.value = await invoke<CleanupTrashEntry[]>('list_cleanup_trash')
  } catch {
    trashList.value = []
  }
}

async function handleScan() {
  if (!effectiveRoot.value || isScanning.value || isDedupScanning.value) return
  const epoch = ++scanEpoch
  const requestId = window.crypto.randomUUID()
  const root = effectiveRoot.value
  isDedupScanning.value = true
  isCancelling.value = false
  scanCancelled.value = false
  scanError.value = ''
  cleanupError.value = ''
  scanProgress.value = { stage: 'inventory', current: 0, total: 0 }
  invalidateScan()
  try {
    const stop = await listen<{ request_id: string; stage: string; current: number; total: number }>('dedup-progress', event => {
      if (epoch === scanEpoch && event.payload.request_id === requestId) scanProgress.value = event.payload
    })
    if (epoch !== scanEpoch) { stop(); return }
    unlistenProgress = stop
    activeRequest = requestId
    const summary = await invoke<DedupSummary>('scan_for_duplicates', { vamRoot: root, mode: analysisMode.value, requestId })
    if (epoch !== scanEpoch) return
    if (isCancelling.value) { scanCancelled.value = true; return }
    const groups = await invoke<DuplicateGroup[]>('get_duplicate_groups', { scanId: summary.scan_id })
    if (epoch !== scanEpoch) return
    if (isCancelling.value) { scanCancelled.value = true; return }
    dedupSummary.value = summary
    duplicateGroups.value = groups
    hasScanned.value = true
    groupFilter.value = groups.some(canCleanGroup) ? 'cleanup' : 'keep'
    for (const group of groups) selectedInstances[group.id] = new Set(group.instances.filter(i => i.is_recommended_keep).map(i => i.file_path))
  } catch (err) {
    if (epoch === scanEpoch) {
      if (isCancelling.value || String(err).includes('DEDUP_CANCELLED')) scanCancelled.value = true
      else scanError.value = String(err)
    }
  } finally {
    unlistenProgress?.(); unlistenProgress = null
    activeRequest = null
    isDedupScanning.value = false
    isCancelling.value = false
  }
}

async function handleCleanup() {
  if ((!cleanupList.value.length && !archiveList.value.length) || isCleaning.value || !dedupSummary.value) return
  isCleaning.value = true
  cleanupError.value = ''
  const scanId = dedupSummary.value.scan_id
  cleanupProgress.value = { scan_id: scanId, stage: 'waiting', current: 0, total: 0, file_name: '' }
  try {
    unlistenCleanup = await listen<CleanupProgress>('dedup-cleanup-progress', event => {
      if (!disposed && isCleaning.value && event.payload.scan_id === scanId) cleanupProgress.value = event.payload
    })
    if (disposed) return
    const result = await invoke<{ cleaned: number; archived: number; removed_directories: number; errors: string[] }>('execute_cleanup', {
      scanId,
      filePaths: cleanupList.value.map(i => i.file_path),
    })
    notify.success(t('deduplication.organized', { count: result.cleaned, archive: result.archived, folders: result.removed_directories }))
    if (result.errors.length) cleanupError.value = result.errors.join('\n')
    showCleanupPreview.value = false
    isCleaning.value = false
    if (!disposed) {
      if (result.errors.length) invalidateScan()
      else await handleScan()
      await loadTrash()
    }
  } catch (err) {
    cleanupError.value = String(err)
    notify.error(t('deduplication.cleanupFailed'), t('common.error'))
    invalidateScan()
  } finally {
    unlistenCleanup?.(); unlistenCleanup = null
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
    isRestoring.value = false
    if (hasScanned.value) await handleScan()
  } catch (err) {
    notify.error(t('deduplication.restoreFailed', { error: String(err) }), t('common.error'))
  } finally {
    isRestoring.value = false
  }
}

async function copyPaths(paths: string[]) {
  try {
    await navigator.clipboard.writeText(paths.join('\n'))
    notify.success(t('deduplication.pathsCopied', { count: paths.length }))
  } catch (err) {
    notify.error(t('deduplication.copyFailed', { error: String(err) }))
  }
}

function toggleInstance(groupId: string, path: string) {
  if (!selectedInstances[groupId]) return
  if (duplicateGroups.value.find(g => g.id === groupId)?.instances.find(i => i.file_path === path)?.is_recommended_keep) return
  if (selectedInstances[groupId].has(path)) {
    // Don't allow deselecting the last instance
    if (selectedInstances[groupId].size <= 1) return
    selectedInstances[groupId].delete(path)
  } else {
    selectedInstances[groupId].add(path)
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
  empty: 'M3 3h12v12H3zM9 9h12v12H9',
}
</script>

<style scoped>
.deduplication-view { display: flex; flex-direction: column; min-height: 0; }
.dedup-setup { flex-shrink: 0; background: var(--bg-elevated); border-radius: var(--radius-lg); padding: var(--space-stack) var(--space-panel); }
.dedup-setup-row, .dedup-setup-foot { display: flex; align-items: center; gap: var(--space-item); }
.dedup-setup-row { flex-wrap: wrap; }
.dedup-setup-row > .ui-button:last-child { margin-left: auto; }
.dedup-setup-label { color: var(--text-primary); font-size: var(--text-base); font-weight: var(--font-bold); }
.dedup-root { flex: 1; min-width: 180px; }
.dedup-setup-foot { justify-content: space-between; margin-top: var(--space-item); min-height: var(--space-6); }
.dedup-help { margin: 0; color: var(--text-secondary); font-size: var(--text-base); line-height: var(--leading-relaxed); }
.dedup-setup-foot .dedup-help { font-size: var(--text-sm); }
.dedup-meta { color: var(--text-tertiary); font-size: var(--text-sm); }
.dedup-mono { white-space: pre-wrap; overflow-wrap: anywhere; font-family: var(--font-mono); font-size: var(--text-sm); }
.dedup-status { padding: var(--space-panel); border-radius: var(--radius-lg); background: var(--bg-elevated); }
.dedup-status-row { display: flex; align-items: center; gap: var(--space-item); margin-bottom: var(--space-stack); font-size: var(--text-base); }
.dedup-status-row strong { flex: 1; }
.dedup-status-count { margin-left: auto; color: var(--text-secondary); }
.dedup-alert { color: var(--color-error); }
.dedup-empty { display: grid; place-items: center; flex: 1; }
.dedup-results { flex: 1; min-height: 0; display: flex; flex-direction: column; gap: var(--space-stack); }
.dedup-workspace { display: grid; grid-template-columns: minmax(280px, 30%) minmax(0, 1fr); gap: var(--space-grid); flex: 1; min-height: 0; }
.dedup-browser, .dedup-comparison { background: var(--bg-elevated); border-radius: var(--radius-lg); min-width: 0; min-height: 0; }
.dedup-browser { display: flex; flex-direction: column; padding: var(--space-stack); gap: var(--space-stack); }
.dedup-mode-filter, .dedup-group-filter { gap: var(--space-item); }
.dedup-group-filter { display: flex; }
.dedup-group-filter :deep(.ui-button) { flex: 1; min-width: 0; padding-inline: var(--space-item); }
.dedup-group-filter :deep(.ui-button.selected), .dedup-group-filter :deep(.ui-button.selected:hover) { color: var(--accent-primary); background: var(--accent-subtle); }
.dedup-groups { flex: 1; min-height: 0; overflow-y: auto; }
.dedup-group-row { display: flex; align-items: center; gap: var(--space-stack); width: 100%; padding: var(--space-stack); border: 0; border-radius: var(--radius-control); background: transparent; color: var(--text-secondary); text-align: left; cursor: pointer; position: relative; }
.dedup-group-row + .dedup-group-row { margin-top: var(--space-1); }
.dedup-group-row:hover { background: var(--bg-hover); }
.dedup-group-row.selected, .dedup-group-row.selected:hover { background: var(--accent-subtle); }
.dedup-group-row.selected::before { content: ''; position: absolute; inset: var(--space-item) auto var(--space-item) 0; width: 3px; background: var(--accent-primary); border-radius: var(--radius-full); }
.dedup-group-copy { flex: 1; min-width: 0; display: flex; flex-direction: column; gap: var(--space-item); }
.dedup-group-name { color: var(--text-primary); font-size: var(--text-base); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.dedup-group-row .dedup-meta { line-height: var(--leading-normal); }
.dedup-pagination, .dedup-pager { display: flex; align-items: center; gap: var(--space-item); }
.dedup-pagination { justify-content: space-between; }
.dedup-pager { gap: var(--space-1); }
.dedup-no-match { padding: var(--space-section); color: var(--text-secondary); font-size: var(--text-base); text-align: center; }
.dedup-comparison { display: flex; flex-direction: column; padding: var(--space-panel); gap: var(--space-item); }
.comparison-heading { display: flex; align-items: flex-start; justify-content: space-between; gap: var(--space-stack); }
.comparison-copy { min-width: 0; }
.comparison-title { margin: 0 0 var(--space-item); font-size: var(--text-xl); font-weight: var(--font-bold); color: var(--text-primary); overflow-wrap: anywhere; }
.comparison-scroll { flex: 1; min-height: 0; overflow-y: auto; }
.comparison-grid { align-items: start; grid-template-columns: repeat(2, minmax(0, 1fr)); }
.dup-file { min-width: 0; }
.comparison-card { position: relative; display: grid; grid-template-rows: minmax(0, 1fr) var(--resource-card-info-h); aspect-ratio: var(--resource-card-ratio); background: var(--bg-input); border-radius: var(--radius-card); overflow: hidden; }
.comparison-image { position: relative; display: grid; place-items: center; width: 100%; height: 100%; min-height: 0; padding: 0; border: 0; background: var(--bg-base); color: var(--text-tertiary); cursor: zoom-in; }
.comparison-image:hover:not(:disabled) { background: var(--bg-hover); }
.comparison-image:disabled { cursor: default; }
.comparison-image img { position: absolute; inset: 0; width: 100%; height: 100%; object-fit: contain; }
.comparison-card-info { min-width: 0; display: flex; flex-direction: column; justify-content: center; gap: var(--space-1); padding: 0 var(--space-stack); }
.comparison-file-name { color: var(--text-primary); font-size: var(--text-base); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.comparison-location { display: flex; align-items: center; gap: var(--space-item); color: var(--text-secondary); font-size: var(--text-sm); min-width: 0; }
.comparison-location > span:first-of-type { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.comparison-size { margin-left: auto; flex-shrink: 0; }
.comparison-file-actions { display: flex; align-items: center; justify-content: space-between; gap: var(--space-item); }
.comparison-reason { margin: var(--space-item) 0 0; font-size: var(--text-sm); color: var(--text-secondary); }
.comparison-safety { display: flex; align-items: flex-start; gap: var(--space-stack); margin: 0; flex-shrink: 0; padding: var(--space-stack); background: var(--bg-input); border-radius: var(--radius-control); color: var(--text-secondary); font-size: var(--text-base); line-height: var(--leading-normal); }
.comparison-safety :deep(svg) { flex-shrink: 0; }
.comparison-more { margin-top: var(--space-stack); }
.dedup-rules { margin-top: var(--space-stack); font-size: var(--text-sm); color: var(--text-secondary); }
.dedup-planbar { flex-shrink: 0; display: flex; align-items: center; justify-content: space-between; gap: var(--space-stack); padding: var(--space-stack) var(--space-panel); background: var(--bg-menu); border-radius: var(--radius-lg); }
.dedup-planbar-actions { display: flex; gap: var(--space-item); flex-wrap: wrap; }
.dedup-planbar-copy { display: flex; flex-direction: column; gap: var(--space-1); }
.dedup-planbar-copy strong { font-size: var(--text-md); color: var(--text-primary); }
.dedup-planbar-copy span { font-size: var(--text-sm); color: var(--text-secondary); }
.dup-thumb { position: relative; display: grid; place-items: center; flex-shrink: 0; width: 64px; height: 46px; padding: 0; border: 0; border-radius: var(--radius-xs); background: var(--bg-control); color: var(--text-tertiary); overflow: hidden; }
.dup-thumb.lg { width: 84px; height: 60px; cursor: zoom-in; }
.dup-thumb.lg:hover:not(:disabled) { background: var(--bg-hover); }
.dup-thumb:disabled { cursor: default; }
.dup-thumb img { position: absolute; inset: 0; width: 100%; height: 100%; object-fit: contain; background: var(--bg-base); }
.dup-badge { display: inline-flex; align-items: center; gap: var(--space-1); min-height: 22px; padding: 0 var(--space-item); border-radius: var(--radius-full); font-size: var(--text-xs); font-weight: var(--font-bold); }
.comparison-badge { position: absolute; top: var(--space-item); left: var(--space-item); pointer-events: none; }
.dup-badge.keep { background: var(--color-success-bg); color: var(--color-success); }
.dup-badge.recycle { background: var(--color-error-bg); color: var(--color-error); }
.dup-badge.archive { background: var(--color-info-bg); color: var(--color-info); }
.dup-file-info { display: flex; flex-direction: column; gap: var(--space-1); min-width: 0; }
.dup-file-name { font-size: var(--text-base); font-weight: var(--font-bold); color: var(--text-primary); overflow-wrap: anywhere; user-select: text; }
.dup-file-path { font-family: var(--font-mono); font-size: var(--text-xs); color: var(--text-tertiary); overflow-wrap: anywhere; user-select: text; }
.dup-file-meta { font-size: var(--text-sm); color: var(--text-tertiary); }
.cleanup-progress { padding: var(--space-section) 0; }
.cleanup-current-file { margin: var(--space-stack) 0 0; color: var(--text-secondary); font-size: var(--text-sm); overflow-wrap: anywhere; }
.plan-stats {
  display: grid;
  grid-template-columns: repeat(3, minmax(0, 1fr));
  gap: var(--space-item);
  margin: 0 0 var(--space-5);
}
.plan-stat {
  padding: var(--space-3) var(--space-4);
  border-radius: var(--radius-md);
  background: var(--bg-input);
}
.plan-stat dt {
  font-size: var(--text-sm);
  color: var(--text-tertiary);
}
.plan-stat dd {
  margin: var(--space-1) 0 0;
  font-size: var(--text-xl);
  font-weight: var(--font-heavy);
  color: var(--text-primary);
  font-variant-numeric: tabular-nums;
}
.plan-section + .plan-section {
  margin-top: var(--space-5);
}
.plan-section-title {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  margin: 0 0 var(--space-2);
  font-size: var(--text-base);
  font-weight: var(--font-bold);
  color: var(--text-primary);
}
.plan-count {
  color: var(--text-tertiary);
  font-weight: var(--font-medium);
}
.plan-list {
  margin: 0 0 var(--space-2);
  padding: 0;
  list-style: none;
  max-height: 40vh;
  overflow-y: auto;
  border-radius: var(--radius-md);
  background: var(--bg-input);
}
.plan-item {
  display: grid;
  grid-template-columns: auto minmax(0, 1fr) auto;
  align-items: center;
  gap: var(--space-3);
  padding: var(--space-item) var(--space-3);
}
.plan-item + .plan-item {
  border-top: 1px solid var(--border-subtle);
}

@media (max-width: 1000px) {
  .comparison-grid { grid-template-columns: 1fr; }
  .dedup-workspace { grid-template-columns: minmax(240px, 32%) minmax(0, 1fr); }
  .dedup-group-row { padding: var(--space-item); gap: var(--space-item); }
  .dedup-group-row .dup-thumb { width: 48px; height: 40px; }
  .comparison-heading { flex-wrap: wrap; }
}
@media (max-width: 760px) {
  .dedup-workspace { display: flex; flex-direction: column; overflow-y: auto; }
  .dedup-browser { flex: 0 0 240px; }
  .dedup-comparison { flex-shrink: 0; overflow: visible; }
  .comparison-scroll { overflow: visible; }
  .dedup-planbar { flex-wrap: wrap; }
  .plan-item { grid-template-columns: auto minmax(0, 1fr); }
  .plan-stats { grid-template-columns: 1fr; }
  .dedup-root { min-width: min(220px, 100%); }
}
</style>
