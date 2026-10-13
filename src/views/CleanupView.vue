<template>
  <div class="cleanup-view b2-page">
    <PageHeader :title="$t('ui.cleanup')">
      <template #subtitle>
        <p v-if="indexText" class="index-status" :class="cleanup.indexStatus?.state" role="status">
          {{ indexText }}
        </p>
      </template>
      <template #actions>
        <UiButton icon="trash" @click="router.push('/trash')">{{ t('cleanup.openTrash') }}</UiButton>
      </template>
      <template #filters>
        <nav class="chip-row" :aria-label="t('cleanup.title')">
          <UiChip
            v-for="item in tabs"
            :key="item.key"
            :class="{ active: tab === item.key }"
            :aria-pressed="tab === item.key"
            @click="tab = item.key"
            :selected="tab === item.key"
          >
            {{ item.title }}
          </UiChip>
        </nav>
      </template>
    </PageHeader>
    <div class="b2-page-body">
      <div v-if="tab !== 'dedup'" class="resource-surface">
        <ResourceDisplay
          :packages="filteredPackages"
          :view-mode="viewMode"
          :organizing="true"
          :result-count-label="t('packages.packagesCount', { count: filteredPackages.length })"
          :empty-title="t(library.loading || (standaloneMode && cleanup.healthLoading && !cleanup.health) ? 'common.loading' : 'cleanup.noResources')"
          :empty-description="emptyDescription"
          :status-tags="statusTags"
          @update:view-mode="viewMode = $event"
        >
          <template #actions>
            <SearchInput
              v-model="query"
              class="cleanup-search"
              :placeholder="t('cleanup.search')"
            />
            <select v-if="!standaloneMode" v-model="selectedType" :aria-label="t('packages.filter')">
              <option value="all">{{ t('packages.filterAll') }}</option>
              <option v-for="type in resourceTypes" :key="type" :value="type">
                {{ t(type === 'other' ? 'packages.filterOther' : `resourceType.${type}`) }}
              </option>
            </select>
            <select v-model="markFilter" :aria-label="t('cleanup.stateFilter')">
              <option value="all">{{ t('cleanup.all') }}</option>
              <option value="pending">{{ t('cleanup.pending') }}</option>
              <option value="protected">{{ t('cleanup.protected') }}</option>
              <option value="normal">{{ t('cleanup.normal') }}</option>
            </select>
            <select v-model="sortBy" :aria-label="t('cleanup.sort')">
              <option value="size">{{ t('cleanup.largestFirst') }}</option>
              <option value="imported">{{ t('cleanup.newestFirst') }}</option>
              <option value="name">{{ t('packages.sortByName') }}</option>
            </select>
            <UiButton
              :disabled="app.isScanning || library.loading || !app.vamRootPath"
              @click="scan"
              variant="secondary"
            >
              {{ t(app.isScanning ? 'toolbar.scanning' : 'cleanup.rescan') }}
            </UiButton>
            <UiButton
              v-if="standaloneMode && filteredPackages.length"
              :disabled="app.isScanning || library.loading || cleanup.healthLoading"
              @click="cleanup.openPreview(filteredPackages.map(pkg => pkg.id))"
            >
              {{ t('cleanup.standalone.previewAll', { count: filteredPackages.length }) }}
            </UiButton>
          </template>
          <template #filters>
            <div v-if="standaloneMode" class="chip-row standalone-exclusions" role="group" :aria-label="t('cleanup.standalone.excludeTypes')">
              <span class="issue-group-label">{{ t('cleanup.standalone.excludeTypes') }}</span>
              <UiChip
                v-for="type in resourceTypes"
                :key="type"
                :class="`exclude-${type}`"
                :selected="excludedTypes.has(type)"
                @click="toggleExcludedType(type)"
              >
                {{ t(type === 'other' ? 'packages.filterOther' : `resourceType.${type}`) }}
              </UiChip>
            </div>
            <div v-if="!standaloneMode || (cleanup.healthLoading && !cleanup.health) || (cleanup.healthError && !cleanup.health)" class="issue-filters">
              <div class="chip-row" role="group" :aria-label="t('cleanup.issues.label')">
                <template v-for="group in standaloneMode ? [] : issueGroups" :key="group.key">
                  <span v-if="group.filters.length" class="issue-group-label">{{ group.title }}</span>
                  <UiChip
                    v-for="filter in group.filters"
                    :key="filter"
                    class="issue-chip"
                    :class="`issue-${filter}`"
                    :selected="issueFilter === filter"
                    :count="issueTotals[filter]"
                    @click="toggleIssue(filter)"
                  >
                    {{ t(`cleanup.issues.${filter}`) }}
                  </UiChip>
                </template>
                <span v-if="cleanup.healthLoading && !cleanup.health" class="issue-status" role="status">
                  <span class="spinner-ring sm" aria-hidden="true" />
                  {{ t('cleanup.issues.checking') }}
                </span>
                <span v-else-if="cleanup.healthError && !cleanup.health" class="issue-status error" role="alert">
                  {{ t('cleanup.issues.checkFailed') }}
                  <UiButton size="sm" variant="ghost" @click="cleanup.loadHealth()">{{ t('cleanup.issues.retry') }}</UiButton>
                </span>
                <span v-else-if="!standaloneMode && cleanup.health && !issueGroups.some(group => group.filters.length)" class="issue-status">
                  {{ t('cleanup.issues.none') }}
                </span>
              </div>
              <div v-if="!standaloneMode && issueFilter" class="issue-hint">
                <p class="issue-hint-text">{{ t(`cleanup.issues.help.${issueFilter}`) }}</p>
                <UiButton
                  v-if="(issueFilter === 'invalid' || issueFilter === 'invalidName' || issueFilter === 'damaged') && filteredPackages.length"
                  :disabled="app.isScanning || library.loading"
                  @click="cleanup.openPreview(filteredPackages.map(pkg => pkg.id))"
                >
                  {{ t('cleanup.issues.cleanAll', { count: filteredPackages.length }) }}
                </UiButton>
                <UiButton v-else-if="issueFilter === 'copies'" @click="tab = 'dedup'">
                  {{ t('cleanup.issues.openDedup') }}
                </UiButton>
                <UiButton v-else-if="issueFilter === 'missingDeps'" @click="router.push('/dependency-completion')">
                  {{ t('cleanup.issues.completeDependencies') }}
                </UiButton>
              </div>
            </div>
          </template>
          <template #empty-action>
            <UiButton v-if="hasFilters" @click="resetFilters" variant="secondary">
              {{ t('cleanup.resetFilters') }}
            </UiButton>
            <UiButton
              v-else-if="!app.vamRootPath"
              @click="router.push('/settings')"
              variant="secondary"
            >
              {{ t('packages.selectDirectory') }}
            </UiButton>
            <UiButton
              v-else
              :disabled="app.isScanning || library.loading"
              @click="scan"
              variant="secondary"
            >
              {{ t(app.isScanning ? 'toolbar.scanning' : 'cleanup.rescan') }}
            </UiButton>
          </template>
        </ResourceDisplay>
      </div>
      <div v-else class="dedup-surface"><DeduplicationView /></div>
    </div>
  </div>
</template>
<script setup lang="ts">
import PageHeader from '@/components/layout/PageHeader.vue'
import UiButton from '@/components/ui/UiButton.vue'
import UiChip from '@/components/ui/UiChip.vue'
import type { StatusTag } from '@/components/ResourceDisplay.vue'
import {
  ISSUE_FILTERS,
  issueCounts,
  isStandalonePackage,
  matchesIssue,
  packageIssues,
  PROBLEM_FILTERS,
  SUGGESTION_FILTERS,
  type IssueFilter,
} from '@/utils/cleanupIssues'
import { computed, onMounted, onUnmounted, ref, watch } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { useI18n } from 'vue-i18n'
import { useCleanupStore } from '@/stores/cleanup'
import { useLocalLibraryStore } from '@/stores/localLibrary'
import { useAppStore } from '@/stores/app'
import { useNotification } from '@/composables/useNotification'
import ResourceDisplay from '@/components/ResourceDisplay.vue'
import SearchInput from '@/components/common/SearchInput.vue'
import DeduplicationView from './DeduplicationView.vue'
const { t } = useI18n()
const router = useRouter()
const route = useRoute()
const cleanup = useCleanupStore()
const library = useLocalLibraryStore()
const app = useAppStore()
const notify = useNotification()
const tab = ref('resources')
const viewMode = ref<'large-card' | 'small-card' | 'list'>('large-card')
const query = ref('')
const selectedType = ref('all')
const excludedTypes = ref(new Set(['scene']))
const standaloneMode = computed(() => tab.value === 'standalone')
function toggleExcludedType(type: string) {
  const next = new Set(excludedTypes.value)
  if (next.has(type)) next.delete(type)
  else next.add(type)
  excludedTypes.value = next
}
const markFilter = ref('all')
const sortBy = ref('size')
const issueFilter = ref<IssueFilter | null>(null)
const resourceTypes = [
  'scene',
  'appearance',
  'clothing',
  'hair',
  'morph',
  'plugin',
  'texture',
  'asset',
  'sound',
  'other',
]
const tabs = computed(() => [
  { key: 'resources', title: t('cleanup.resources') },
  { key: 'standalone', title: t('cleanup.standalone.title') },
  { key: 'dedup', title: t('cleanup.dedup') },
])
const hasFilters = computed(
  () =>
    Boolean(query.value.trim()) ||
    (standaloneMode.value
      ? excludedTypes.value.size !== 1 || !excludedTypes.value.has('scene')
      : selectedType.value !== 'all') ||
    markFilter.value !== 'all' ||
    (!standaloneMode.value && issueFilter.value !== null),
)
const emptyDescription = computed(() => {
  if (standaloneMode.value && app.vamRootPath) {
    if (cleanup.healthLoading && !cleanup.health) return t('cleanup.issues.checking')
    if (cleanup.healthError && !cleanup.health) return t('cleanup.issues.checkFailed')
    return t(hasFilters.value ? 'cleanup.noMatches' : 'cleanup.standalone.none')
  }
  return t(hasFilters.value ? 'cleanup.noMatches' : 'cleanup.emptyResources')
})
// Problems and cleanup suggestions per package (lower-case id).
const issues = computed(() =>
  packageIssues(library.packages, {
    health: cleanup.health,
    missing: library.missingDependencies ?? [],
    corrupted: library.corruptedPackages ?? [],
    protectedIds: cleanup.protectedIds,
  }),
)
const issueTotals = computed(() => issueCounts(issues.value))
const issueGroups = computed(() =>
  [
    { key: 'problems', title: t('cleanup.issues.problems'), filters: PROBLEM_FILTERS },
    { key: 'suggestions', title: t('cleanup.issues.suggestions'), filters: SUGGESTION_FILTERS },
  ].map((group) => ({
    ...group,
    // Only filters with matches, plus the active one so it can be turned off.
    filters: group.filters.filter(
      (filter) => issueTotals.value[filter] > 0 || issueFilter.value === filter,
    ),
  })),
)
function toggleIssue(filter: IssueFilter) {
  issueFilter.value = issueFilter.value === filter ? null : filter
}
// Other pages link here with a filter, e.g. the dashboard's damaged packages.
watch(
  () => route.query?.issue,
  (issue) => {
    if (typeof issue === 'string' && (ISSUE_FILTERS as readonly string[]).includes(issue)) {
      tab.value = 'resources'
      issueFilter.value = issue as IssueFilter
    }
  },
  { immediate: true },
)
const statusTags = computed(() => {
  const result: Record<string, StatusTag[]> = {}
  for (const pkg of library.packages) {
    const found = issues.value.get(pkg.id.toLowerCase())
    if (!found) continue
    const tags: StatusTag[] = []
    if (found.invalid) tags.push({ label: t('cleanup.issues.tag.invalid'), tone: 'error' })
    if (found.invalidName) tags.push({ label: t('cleanup.issues.tag.invalidName'), tone: 'warning' })
    if (found.damaged)
      tags.push({
        label: t(found.damaged === 'info' ? 'cleanup.issues.tag.brokenInfo' : 'cleanup.issues.tag.damaged'),
        tone: 'error',
      })
    if (found.missing)
      tags.push({ label: t('cleanup.issues.tag.missing', { count: found.missing }), tone: 'warning' })
    if (!standaloneMode.value && issueFilter.value === 'oldVersion' && found.newerVersion)
      tags.push({ label: t('cleanup.issues.tag.newer', { version: found.newerVersion }), tone: 'neutral' })
    if (!standaloneMode.value && issueFilter.value === 'copies' && found.copies)
      tags.push({ label: t('cleanup.issues.tag.copies', { count: found.copies }), tone: 'neutral' })
    if (tags.length) result[pkg.id] = tags
  }
  return result
})
const filteredPackages = computed(() => {
  // Start from the complete library, independent of package-browser filters.
  const search = query.value.trim().toLowerCase()
  const packages = library.packages.filter((pkg) => {
    if (
      search &&
      ![pkg.name, pkg.creator, `${pkg.id}.var`].some((value) =>
        value.toLowerCase().includes(search),
      )
    )
      return false
    if (standaloneMode.value) {
      if (pkg.resource_types.some(type => excludedTypes.value.has(type))) return false
    } else if (selectedType.value !== 'all' && !pkg.resource_types.includes(selectedType.value))
      return false
    const id = pkg.id.toLowerCase()
    const mark = cleanup.markMap.get(id)
    if (markFilter.value === 'pending' && !mark?.pending) return false
    if (markFilter.value === 'protected' && !mark?.protected) return false
    if (markFilter.value === 'normal' && (mark?.pending || mark?.protected)) return false
    if (standaloneMode.value) {
      if (!isStandalonePackage(cleanup.health?.get(id), issues.value.get(id), cleanup.protectedIds.has(id))) return false
    } else if (issueFilter.value && !matchesIssue(issues.value.get(id), issueFilter.value))
      return false
    return true
  })
  const byName = (a: (typeof packages)[number], b: (typeof packages)[number]) =>
    a.id.localeCompare(b.id)
  const time = (value: string) => Date.parse(value) || 0
  return packages.sort((a, b) =>
    sortBy.value === 'size'
      ? b.size_bytes - a.size_bytes || byName(a, b)
      : sortBy.value === 'imported'
        ? time(b.scan_time) - time(a.scan_time) || byName(a, b)
        : byName(a, b),
  )
})

function resetFilters() {
  query.value = ''
  selectedType.value = 'all'
  excludedTypes.value = new Set(['scene'])
  markFilter.value = 'all'
  issueFilter.value = null
}

async function load(refresh = false) {
  try {
    await Promise.all([
      refresh ? library.refreshAll('loading') : library.ensureLoaded(),
      cleanup.load(),
    ])
  } catch (e) {
    notify.error(String(e))
  }
}

async function scan() {
  if (!app.vamRootPath || app.isScanning || library.loading) return
  try {
    await app.startScan(app.vamRootPath)
  } catch (e) {
    notify.error(String(e))
  }
}

const indexText = computed(() => {
  const status = cleanup.indexStatus
  if (!status || !app.vamRootPath) return ''
  switch (status.state) {
    case 'running':
      return status.total
        ? t('cleanup.index.running', { current: status.current, total: status.total })
        : t('cleanup.index.runningPlain')
    case 'waiting':
      return t('cleanup.index.waiting')
    case 'paused_game':
      return t('cleanup.index.pausedGame')
    case 'ready':
      return t('cleanup.index.ready')
    case 'error':
      return t('cleanup.index.error', { message: status.message ?? '' })
    default:
      return ''
  }
})

watch(
  () => app.vamRootPath,
  () => {
    resetFilters()
    void load(true)
    void cleanup.loadHealth()
  },
)
onMounted(() => {
  void load()
  void cleanup.watchIndexStatus()
  void cleanup.loadHealth()
})
onUnmounted(() => cleanup.unwatchIndexStatus())
</script>
<style scoped>
.cleanup-view {
  display: flex;
  flex-direction: column;
}
h1 {
  font-size: var(--text-xl);
}
.index-status {
  flex: 1;
  min-width: 0;
  margin: 0;
  font-size: var(--text-sm);
  color: var(--text-secondary);
  overflow-wrap: anywhere;
}
.index-status.paused_game,
.index-status.waiting {
  color: var(--color-warning);
}
.index-status.error {
  color: var(--color-error);
}
nav {
  display: flex;
  gap: var(--space-2);
}
.cleanup-search {
  width: 220px;
  max-width: 100%;
}
.resource-surface {
  flex: 1;
  min-height: 0;
  display: flex;
}
.resource-surface > :deep(.resource-display) {
  flex: 1;
}
.issue-filters {
  display: flex;
  flex-direction: column;
  gap: var(--space-item);
  flex-shrink: 0;
}
.issue-group-label {
  font-size: var(--text-sm);
  color: var(--text-tertiary);
}
.issue-group-label + .issue-chip,
.issue-chip + .issue-group-label {
  margin-left: var(--space-1);
}
.issue-status {
  display: inline-flex;
  align-items: center;
  gap: var(--space-2);
  font-size: var(--text-sm);
  color: var(--text-tertiary);
}
.issue-status.error {
  color: var(--color-error);
}
.issue-hint {
  display: flex;
  align-items: center;
  justify-content: space-between;
  flex-wrap: wrap;
  gap: var(--space-item) var(--space-grid);
  padding: var(--space-stack) var(--space-panel);
  border-radius: var(--radius-lg);
  background: var(--bg-elevated);
}
.issue-hint-text {
  flex: 1;
  min-width: 240px;
  margin: 0;
  font-size: var(--text-base);
  line-height: var(--leading-relaxed);
  color: var(--text-secondary);
}
.dedup-surface {
  flex: 1;
  min-height: 0;
  overflow: auto;
}
/* 去重分析以标签页形式嵌入：去掉它自己的页头和第二层页面内边距，
   操作都在去重页内容里，内容与上方筛选片左缘对齐。 */
.dedup-surface > :deep(.b2-page) {
  height: 100%;
  overflow: hidden;
}
.dedup-surface :deep(.page-header) {
  display: none;
}
.dedup-surface :deep(.b2-page-body) {
  padding: 0;
  overflow: hidden;
}

</style>
