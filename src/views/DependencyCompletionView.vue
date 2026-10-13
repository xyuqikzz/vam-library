<template>
  <div class="dependency-completion-view animate-fadeIn b2-page">
    <PageHeader :title="t('sidebar.dependencyCompletion')">
      <template #actions>
        <span v-if="lastRefreshedText" class="toolbar-hint">{{ lastRefreshedText }}</span>
        <UiButton
          variant="secondary"
          icon="refresh"
          :loading="localLibraryStore.loading"
          :loading-text="t('depCompletion.checking')"
          :disabled="busy || !appStore.vamRootPath"
          @click="refreshLibrary"
        >
          {{ t('depCompletion.recheck') }}
        </UiButton>
        <UiButton
          variant="primary"
          icon="download"
          :disabled="!dependencyGroups.length || busy"
          @click="openPreview"
        >
          {{ t('depCompletion.findDownloads') }}
        </UiButton>
      </template>
    </PageHeader>
    <div class="b2-page-body">
      <section v-if="!localLibraryStore.loading" class="stats-row">
        <StatCard
          :title="t('depCompletion.stats.missing')"
          :value="missingEntryCount"
          :subtitle="t('depCompletion.stats.missingHint')"
          icon="M12 8v4m0 4h.01M10.29 3.86l-7.43 12.88A2 2 0 0 0 4.58 19h14.84a2 2 0 0 0 1.72-3.02L13.71 3.86a2 2 0 0 0-3.42 0z"
          trend="neutral"
          color="var(--color-error)"
        />
        <StatCard
          :title="t('depCompletion.stats.packages')"
          :value="dependencyGroups.length"
          :subtitle="t('depCompletion.stats.packagesHint')"
          icon="M4 7h16M4 12h10M4 17h7"
          trend="neutral"
          color="var(--accent-primary)"
        />
        <StatCard
          :title="t('depCompletion.stats.affected')"
          :value="affectedPackageCount"
          :subtitle="t('depCompletion.stats.affectedHint')"
          icon="M12 3l8 4v10l-8 4-8-4V7l8-4z"
          trend="neutral"
          color="var(--color-info)"
        />
        <StatCard
          :title="t('depCompletion.stats.outdated')"
          :value="lowerVersionCount"
          :subtitle="t('depCompletion.stats.outdatedHint')"
          icon="M12 19V5m0 0 4 4m-4-4-4 4"
          trend="neutral"
          color="var(--color-warning)"
        />
      </section>

      <section v-else class="stats-row">
        <div v-for="idx in 4" :key="idx" class="stat-skeleton glass-panel">
          <div class="skeleton-line short"></div>
          <div class="skeleton-line big"></div>
          <div class="skeleton-line tiny"></div>
        </div>
      </section>

      <div class="content-panel glass-panel">
        <div class="panel-header">
          <div>
            <h2 class="panel-title">{{ t('depCompletion.listTitle') }}</h2>
            <p class="panel-desc">{{ t('depCompletion.listDesc') }}</p>
          </div>
          <span class="panel-count">{{ dependencyGroups.length }}</span>
        </div>

        <div v-if="dependencyGroups.length > 0" class="row-list">
          <div v-for="group in dependencyGroups" :key="group.key" class="dep-row">
            <div class="row-main">
              <div class="row-title">
                <h3 class="row-name" :title="group.fullId">{{ group.fullId }}</h3>
                <span class="status-tag" :class="group.statusClass">{{ group.statusLabel }}</span>
              </div>
              <div class="row-meta">
                <span>{{ t('depCompletion.meta.required', { version: group.requiredVersionLabel }) }}</span>
                <span>{{ t('depCompletion.meta.references', { count: group.occurrenceCount }) }}</span>
                <span>{{ t('depCompletion.meta.usedBy', { count: group.dependentPackageCount }) }}</span>
                <span v-if="group.installedVersion !== null">
                  {{ t('depCompletion.meta.installed', { version: group.installedVersion }) }}
                </span>
              </div>
              <div v-if="group.dependentPackages.length > 0" class="row-sources">
                <span
                  v-for="pkg in group.dependentPackages.slice(0, 4)"
                  :key="pkg"
                  class="source-pill"
                  :title="pkg"
                >
                  {{ pkg }}
                </span>
                <span v-if="group.dependentPackages.length > 4" class="source-pill more">
                  {{ t('depCompletion.more', { count: group.dependentPackages.length - 4 }) }}
                </span>
              </div>
            </div>

            <UiButton variant="ghost" size="sm" icon="copy" @click="copyIdentifier(group)">
              {{ t('depCompletion.copyName') }}
            </UiButton>
          </div>
        </div>

        <EmptyState
          v-else
          icon="M4 7h16M4 12h16M4 17h10"
          :title="t('depCompletion.emptyTitle')"
          :description="emptyDescription"
          :hide-action="Boolean(appStore.vamRootPath)"
        >
          <template v-if="!appStore.vamRootPath" #action>
            <UiButton variant="primary" size="lg" @click="goSettings">
              {{ t('depCompletion.chooseDirectory') }}
            </UiButton>
          </template>
        </EmptyState>
      </div>

      <UiModal
        v-if="previewOpen"
        large
        :title="t('depCompletion.modal.title')"
        :description="t('depCompletion.modal.description')"
        @close="closePreview"
      >
        <div class="preview-body">
          <div class="mode-bar">
            <UiSegmented
              :model-value="resolutionMode"
              :options="modeOptions"
              :label="t('depCompletion.modal.modeLabel')"
              @update:model-value="setResolutionMode"
            />
            <span class="mode-hint">{{ t('depCompletion.modal.modeHint') }}</span>
          </div>

          <div class="preview-summary">
            <span class="summary-chip">
              {{ t('depCompletion.modal.available', { count: downloadCandidates.length }) }}
            </span>
            <span class="summary-chip" :class="{ warn: missingSourceCount > 0 }">
              {{ t('depCompletion.modal.notFound', { count: missingSourceCount }) }}
            </span>
            <span class="summary-chip" :class="{ warn: versionMismatchCount > 0 }">
              {{ t('depCompletion.modal.mismatch', { count: versionMismatchCount }) }}
            </span>
            <span v-if="lookupFailedCount > 0" class="summary-chip warn">
              {{ t('depCompletion.modal.failed', { count: lookupFailedCount }) }}
            </span>
            <span class="summary-chip">
              {{ t('depCompletion.modal.total', { count: previewGroups.length }) }}
            </span>
          </div>

          <div v-if="previewLoading" class="preview-loading" role="status">
            <span class="spinner-ring" aria-hidden="true" />
            <p>
              {{
                t('depCompletion.modal.resolving', {
                  current: resolvingProgress.current,
                  total: resolvingProgress.total,
                })
              }}
            </p>
          </div>

          <div v-else class="row-list preview-list">
            <div v-for="group in previewGroupsView" :key="group.key" class="dep-row">
              <div class="row-main">
                <div class="row-title">
                  <h4 class="row-name" :title="group.fullId">{{ group.fullId }}</h4>
                  <span class="status-tag" :class="group.previewStatusClass">
                    {{ group.previewStatusLabel }}
                  </span>
                </div>
                <div class="row-meta">
                  <span>{{ t('depCompletion.meta.required', { version: group.requiredVersionLabel }) }}</span>
                  <span>
                    {{
                      t('depCompletion.meta.available', {
                        version: group.selectedFile
                          ? `v${group.selectedFile.version}`
                          : t('depCompletion.none'),
                      })
                    }}
                  </span>
                  <span>{{ t('depCompletion.meta.references', { count: group.occurrenceCount }) }}</span>
                </div>
                <div v-if="group.selectedFile" class="preview-file">
                  {{ group.selectedFile.filename }} · {{ formatSize(group.selectedFile.sizeBytes) }}
                </div>
                <div v-else class="preview-file muted" :title="group.lookupError ?? undefined">
                  {{ group.previewReason }}
                </div>
              </div>

              <UiButton variant="ghost" size="sm" icon="copy" @click="copyIdentifier(group)">
                {{ t('depCompletion.copyName') }}
              </UiButton>
            </div>
          </div>
        </div>

        <template #footer>
          <span class="preview-footer-summary">
            <template v-if="!previewLoading">{{ footerSummary }}</template>
          </span>
          <UiButton @click="closePreview">{{ t('common.cancel') }}</UiButton>
          <UiButton
            variant="primary"
            :disabled="!downloadCandidates.length || previewLoading"
            @click="downloadCandidatesToQueue"
          >
            {{
              resolutionMode === 'latest'
                ? t('depCompletion.modal.downloadLatest')
                : t('depCompletion.modal.downloadRequired')
            }}
          </UiButton>
        </template>
      </UiModal>
    </div>
  </div>
</template>

<script setup lang="ts">
import UiModal from '@/components/ui/UiModal.vue'
import PageHeader from '@/components/layout/PageHeader.vue'
import UiButton from '@/components/ui/UiButton.vue'
import UiSegmented from '@/components/ui/UiSegmented.vue'
import { parseHubFiles, parsePackageId as parseDependencyId, selectHubFile as selectCandidate, type HubFile, type HubFileCandidate } from '@/utils/hubFiles'
import { formatSize } from '@/utils/bytes'
import { computed, onMounted, ref } from 'vue'
import { useRouter } from 'vue-router'
import { useI18n } from 'vue-i18n'
import { invoke } from '@tauri-apps/api/core'
import { useAppStore } from '@/stores/app'
import { useDownloadStore } from '@/stores/download'
import { useLocalLibraryStore } from '@/stores/localLibrary'
import { useNotification } from '@/composables/useNotification'
import StatCard from '@/components/common/StatCard.vue'
import EmptyState from '@/components/common/EmptyState.vue'

interface HubPackageInfo {
  title?: string | null
  username?: string | null
  resource_id?: string | null
  hubFiles?: HubFile[] | null
}


interface DownloadQueueItem {
  id: string
  url: string
  filename: string
  creator: string
  name: string
  version: number
  total_bytes: number
}

interface DependencyGroup {
  key: string
  creator: string
  name: string
  fullId: string
  requiredVersion: number | null
  requiredVersionLabel: string
  dependentPackages: string[]
  dependentPackageCount: number
  occurrenceCount: number
  installedVersion: number | null
  statusLabel: string
  statusClass: string
}

interface PreviewGroup extends DependencyGroup {
  hubQuery: string
  hubTitle: string
  hubUsername: string
  hubFiles: HubFileCandidate[]
  /** 查询 VaM Hub 失败时的原始错误（网络等），null 表示查询成功或尚未查询 */
  lookupError: string | null
  selectedFile: HubFileCandidate | null
}

interface PreviewStatus {
  previewStatusLabel: string
  previewStatusClass: 'ok' | 'warn' | 'danger'
  previewReason: string
}

type ResolutionMode = 'required' | 'latest'

const { t, locale } = useI18n()
const router = useRouter()
const appStore = useAppStore()
const downloadStore = useDownloadStore()
const localLibraryStore = useLocalLibraryStore()
const notify = useNotification()

const previewOpen = ref(false)
const previewLoading = ref(false)
const previewGroups = ref<PreviewGroup[]>([])
const resolutionMode = ref<ResolutionMode>('required')
const resolvingProgress = ref({ current: 0, total: 0 })
const lastRefreshedAt = ref<Date | null>(null)

const modeOptions = computed(() => [
  { value: 'required', label: t('depCompletion.modal.modeRequired') },
  { value: 'latest', label: t('depCompletion.modal.modeLatest') },
])

const busy = computed(() => localLibraryStore.loading || previewLoading.value)

const missingEntryCount = computed(() => localLibraryStore.missingDependencies.length)

const affectedPackageCount = computed(() => {
  return new Set(localLibraryStore.missingDependencies.map(item => item.package_id)).size
})

const lowerVersionCount = computed(() => (
  localLibraryStore.missingDependencies.filter(item => item.status === 'lower_version').length
))

const dependencyGroups = computed<DependencyGroup[]>(() => {
  const map = new Map<string, DependencyGroup>()

  for (const item of localLibraryStore.missingDependencies) {
    const parsed = parseDependencyId(item.depends_on_id)
    if (!parsed) continue

    const key = `${parsed.creator.toLowerCase()}::${parsed.name.toLowerCase()}`
    const existing = map.get(key)
    const requiredVersion = parseRequiredVersion(item.required_version)

    if (!existing) {
      map.set(key, {
        key,
        creator: parsed.creator,
        name: parsed.name,
        fullId: `${parsed.creator}.${parsed.name}`,
        requiredVersion: typeof requiredVersion === 'number' ? requiredVersion : null,
        requiredVersionLabel: typeof requiredVersion === 'number' ? `v${requiredVersion}` : t('depCompletion.latestVersion'),
        dependentPackages: item.dependent_package ? [item.dependent_package] : [],
        dependentPackageCount: item.dependent_package ? 1 : 0,
        occurrenceCount: 1,
        installedVersion: item.installed_version,
        statusLabel: item.status === 'lower_version' ? t('depCompletion.status.outdated') : t('depCompletion.status.missing'),
        statusClass: item.status === 'lower_version' ? 'warn' : 'danger',
      })
      continue
    }

    existing.occurrenceCount += 1
    if (item.dependent_package && !existing.dependentPackages.includes(item.dependent_package)) {
      existing.dependentPackages.push(item.dependent_package)
    }
    existing.dependentPackageCount = existing.dependentPackages.length

    const nextRequired = typeof requiredVersion === 'number' ? requiredVersion : null
    if (nextRequired !== null) {
      existing.requiredVersion = existing.requiredVersion === null
        ? nextRequired
        : Math.max(existing.requiredVersion, nextRequired)
      existing.requiredVersionLabel = `v${existing.requiredVersion}`
    }

    if (item.installed_version !== null) {
      existing.installedVersion = existing.installedVersion === null
        ? item.installed_version
        : Math.max(existing.installedVersion, item.installed_version)
    }

    if (item.status === 'lower_version') {
      existing.statusLabel = t('depCompletion.status.outdated')
      existing.statusClass = 'warn'
    }
  }

  return [...map.values()].sort((a, b) => {
    const creatorDiff = a.creator.localeCompare(b.creator, 'zh-CN')
    if (creatorDiff !== 0) return creatorDiff
    return a.name.localeCompare(b.name, 'zh-CN')
  })
})

const previewGroupsView = computed(() => {
  return previewGroups.value.map((group) => {
    const selectedFile = selectHubFile(group)
    return {
      ...group,
      selectedFile,
      ...describePreview(group, selectedFile),
    }
  })
})

const downloadCandidates = computed(() => (
  previewGroupsView.value.filter(group => Boolean(group.selectedFile))
))

const lookupFailedCount = computed(() => (
  previewGroupsView.value.filter(group => group.lookupError !== null).length
))

const missingSourceCount = computed(() => (
  previewGroupsView.value.filter(group => group.lookupError === null && group.hubFiles.length === 0).length
))

const versionMismatchCount = computed(() => (
  previewGroupsView.value.filter(group => group.hubFiles.length > 0 && !group.selectedFile).length
))

const footerSummary = computed(() => {
  const count = downloadCandidates.value.length
  const skipped = previewGroupsView.value.length - count
  return skipped > 0
    ? t('depCompletion.modal.summarySkipped', { count, skipped })
    : t('depCompletion.modal.summary', { count })
})

const lastRefreshedText = computed(() => {
  if (!lastRefreshedAt.value) return ''
  const time = lastRefreshedAt.value.toLocaleTimeString(locale.value, { hour: '2-digit', minute: '2-digit' })
  return t('depCompletion.lastChecked', { time })
})

const emptyDescription = computed(() => (
  appStore.vamRootPath
    ? t('depCompletion.emptyReady')
    : t('depCompletion.emptyNoRoot')
))

onMounted(async () => {
  await localLibraryStore.ensureLoaded()
  lastRefreshedAt.value = new Date()
})

async function refreshLibrary() {
  if (!appStore.vamRootPath) {
    notify.warning(t('depCompletion.toast.needDirectory'))
    router.push('/settings')
    return
  }

  try {
    await localLibraryStore.refreshAll('refreshing')
    lastRefreshedAt.value = new Date()
    notify.success(t('depCompletion.toast.refreshed'))
  } catch (error) {
    notify.error(t('depCompletion.toast.refreshFailed', { error: String(error) }))
  }
}

async function openPreview() {
  if (!dependencyGroups.value.length) {
    notify.warning(t('depCompletion.toast.nothingToComplete'))
    return
  }

  previewOpen.value = true
  previewLoading.value = true
  resolutionMode.value = 'required'
  previewGroups.value = dependencyGroups.value.map(group => ({
    ...group,
    hubQuery: `${group.creator}.${group.name}.latest`,
    hubTitle: '',
    hubUsername: '',
    hubFiles: [],
    lookupError: null,
    selectedFile: null,
  }))

  resolvingProgress.value = { current: 0, total: previewGroups.value.length }

  try {
    const nextGroups: PreviewGroup[] = []
    for (const chunk of chunkArray(previewGroups.value, 4)) {
      const results = await Promise.all(chunk.map(async (group) => {
        try {
          const detail = await invoke<HubPackageInfo>('fetch_hub_package_info', {
            packageName: group.hubQuery,
          })
          const files = parseHubFiles(detail.hubFiles || []).sort((a, b) => b.version - a.version)
          return {
            ...group,
            hubTitle: detail.title || '',
            hubUsername: detail.username || '',
            hubFiles: files,
            lookupError: null,
          }
        } catch (error) {
          return {
            ...group,
            hubFiles: [],
            lookupError: String(error),
          }
        } finally {
          resolvingProgress.value.current += 1
        }
      }))
      nextGroups.push(...results)
      previewGroups.value = nextGroups.slice()
    }
    previewGroups.value = nextGroups
  } finally {
    previewLoading.value = false
  }
}

function closePreview() {
  previewOpen.value = false
}

function setResolutionMode(value: string) {
  if (value === 'required' || value === 'latest') resolutionMode.value = value
}

async function downloadCandidatesToQueue() {
  const items: DownloadQueueItem[] = downloadCandidates.value
    .map((group) => {
      const file = group.selectedFile
      if (!file) return null
      return {
        id: `${group.creator}.${group.name}.${file.version}`,
        url: file.url,
        filename: file.filename,
        creator: group.creator,
        name: group.name,
        version: file.version,
        total_bytes: file.sizeBytes,
      }
    })
    .filter((item): item is DownloadQueueItem => Boolean(item && item.url))

  if (!items.length) {
    notify.warning(t('depCompletion.toast.nothingToDownload'))
    return
  }

  try {
    const results = await downloadStore.addItems(items)
    let addedCount = 0
    let skippedCount = 0
    if (Array.isArray(results)) {
      for (const result of results) {
        if (result && typeof result === 'object' && 'Skipped' in result) {
          skippedCount += 1
        } else {
          addedCount += 1
        }
      }
    } else {
      addedCount = items.length
    }

    if (addedCount > 0) {
      notify.success(t('depCompletion.toast.added', { count: addedCount }))
    }
    if (skippedCount > 0) {
      notify.info(t('depCompletion.toast.skippedExisting', { count: skippedCount }))
    }

    previewOpen.value = false
    router.push('/download')
  } catch (error) {
    notify.error(t('depCompletion.toast.addFailed', { error: String(error) }))
  }
}

async function copyIdentifier(group: DependencyGroup | PreviewGroup) {
  const text = getCopyIdentifier(group)
  try {
    await navigator.clipboard.writeText(text)
    notify.success(t('depCompletion.toast.copied', { name: text }))
  } catch (error) {
    notify.error(t('depCompletion.toast.copyFailed', { error: String(error) }))
  }
}

function goSettings() {
  router.push('/settings')
}

function describePreview(group: PreviewGroup, selectedFile: HubFileCandidate | null): PreviewStatus {
  if (group.lookupError !== null) {
    return {
      previewStatusLabel: t('depCompletion.status.failed'),
      previewStatusClass: 'danger',
      previewReason: t('depCompletion.reason.failed'),
    }
  }
  if (group.hubFiles.length === 0) {
    return {
      previewStatusLabel: t('depCompletion.status.notFound'),
      previewStatusClass: 'danger',
      previewReason: t('depCompletion.reason.notFound'),
    }
  }
  // Hub 上有文件却选不出来：只会出现在“按需要的版本”下，Hub 上的版本都比需要的旧
  if (!selectedFile) {
    return {
      previewStatusLabel: t('depCompletion.status.noMatch'),
      previewStatusClass: 'warn',
      previewReason: t('depCompletion.reason.noMatch'),
    }
  }
  // 可以下载的行显示文件名和大小，不显示原因
  if (resolutionMode.value === 'latest') {
    return { previewStatusLabel: t('depCompletion.status.latest'), previewStatusClass: 'ok', previewReason: '' }
  }
  const isNewer = group.requiredVersion !== null && selectedFile.version > group.requiredVersion
  return {
    previewStatusLabel: isNewer ? t('depCompletion.status.newer') : t('depCompletion.status.required'),
    previewStatusClass: 'ok',
    previewReason: '',
  }
}

function selectHubFile(group: PreviewGroup): HubFileCandidate | null {
  return selectCandidate(group.hubFiles, resolutionMode.value === 'latest' ? null : group.requiredVersion)
}

function getCopyIdentifier(group: DependencyGroup | PreviewGroup): string {
  const selectedVersion = 'selectedFile' in group ? group.selectedFile?.version : null
  const version = resolutionMode.value === 'latest'
    ? 'latest'
    : selectedVersion !== null && selectedVersion !== undefined
      ? String(selectedVersion)
      : group.requiredVersion !== null
        ? String(group.requiredVersion)
        : 'latest'
  return `${group.creator}.${group.name}.${version}`
}


function parseRequiredVersion(requiredVersion: string) {
  const text = requiredVersion.trim()
  if (!text || text.toLowerCase() === 'latest') return 'latest' as const
  const version = Number.parseInt(text, 10)
  return Number.isFinite(version) ? version : 'latest'
}

function chunkArray<T>(list: T[], size: number) {
  const chunks: T[][] = []
  for (let index = 0; index < list.length; index += size) {
    chunks.push(list.slice(index, index + size))
  }
  return chunks
}
</script>

<style scoped>
.dependency-completion-view {
  display: flex;
  flex-direction: column;
  min-height: 100%;
}

.toolbar-hint {
  font-size: var(--text-xs);
  color: var(--text-tertiary);
}

/* 统计卡片 */
.stats-row {
  display: grid;
  grid-template-columns: repeat(4, minmax(0, 1fr));
  gap: var(--space-4);
}

.stat-skeleton {
  padding: var(--space-5);
}

.skeleton-line {
  height: 10px;
  border-radius: var(--radius-full);
  background: var(--bg-active);
}

.skeleton-line.short {
  width: 40%;
}

.skeleton-line.big {
  margin-top: var(--space-4);
  width: 55%;
  height: 24px;
}

.skeleton-line.tiny {
  margin-top: var(--space-2);
  width: 70%;
}

/* 列表面板 */
.content-panel {
  display: flex;
  flex-direction: column;
  min-height: 0;
  overflow: hidden;
  border-radius: var(--radius-lg);
}

.panel-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--space-3);
  padding: var(--space-4) var(--space-5);
  border-bottom: 1px solid var(--border-subtle);
}

.panel-title {
  font-size: var(--text-md);
  font-weight: var(--font-semibold);
  color: var(--text-primary);
}

.panel-desc {
  margin-top: 2px;
  font-size: var(--text-xs);
  color: var(--text-tertiary);
}

/* 标签 / 计数片：统一胶囊尺寸 */
.panel-count,
.status-tag,
.summary-chip,
.source-pill {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  flex-shrink: 0;
  min-height: 22px;
  padding: 0 var(--space-2);
  border-radius: var(--radius-full);
  font-size: var(--text-xs);
  font-weight: var(--font-bold);
  line-height: 1.2;
  white-space: nowrap;
}

.panel-count,
.summary-chip {
  background: var(--bg-control);
  color: var(--text-secondary);
  font-variant-numeric: tabular-nums;
}

.summary-chip.warn {
  background: var(--color-warning-bg);
  color: var(--color-warning);
}

.status-tag.ok {
  background: var(--color-success-bg);
  color: var(--color-success);
}

.status-tag.warn {
  background: var(--color-warning-bg);
  color: var(--color-warning);
}

.status-tag.danger {
  background: var(--color-error-bg);
  color: var(--color-error);
}

/* 依赖行：左侧名称与信息，右侧操作，垂直居中 */
.row-list {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
  padding: var(--space-4);
  overflow-y: auto;
}

.dep-row {
  display: flex;
  align-items: center;
  gap: var(--space-4);
  padding: var(--space-4);
  border-radius: var(--radius-md);
  background: var(--bg-subtle);
}

.row-main {
  flex: 1;
  min-width: 0;
}

.row-title {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  min-width: 0;
}

.row-name {
  min-width: 0;
  font-size: var(--text-sm);
  font-weight: var(--font-semibold);
  color: var(--text-primary);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.row-meta {
  display: flex;
  align-items: center;
  gap: var(--space-3);
  margin-top: var(--space-2);
  flex-wrap: wrap;
  font-size: var(--text-xs);
  color: var(--text-secondary);
}

.row-sources {
  display: flex;
  flex-wrap: wrap;
  gap: var(--space-2);
  margin-top: var(--space-3);
}

.source-pill {
  flex-shrink: 1;
  max-width: 100%;
  background: var(--bg-control);
  color: var(--text-secondary);
  font-weight: var(--font-medium);
  overflow: hidden;
  text-overflow: ellipsis;
}

.source-pill.more {
  flex-shrink: 0;
  color: var(--text-tertiary);
}

/* 补全对话框 */
.preview-body {
  display: flex;
  flex-direction: column;
  gap: var(--space-4);
}

.mode-bar {
  display: flex;
  align-items: center;
  gap: var(--space-3);
  flex-wrap: wrap;
}

.mode-hint {
  font-size: var(--text-xs);
  color: var(--text-tertiary);
}

.preview-summary {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  flex-wrap: wrap;
}

.preview-loading {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: var(--space-4);
  min-height: 280px;
  font-size: var(--text-sm);
  color: var(--text-secondary);
  font-variant-numeric: tabular-nums;
}

.preview-list {
  min-height: 240px;
  max-height: 50vh;
  padding: 0 2px 0 0;
}

.preview-file {
  margin-top: var(--space-2);
  font-size: var(--text-xs);
  color: var(--text-secondary);
  word-break: break-all;
}

.preview-file.muted {
  color: var(--text-tertiary);
}

.preview-footer-summary {
  margin-right: auto;
  align-self: center;
  color: var(--text-secondary);
  font-size: var(--text-sm);
  font-variant-numeric: tabular-nums;
}

@media (max-width: 1200px) {
  .stats-row {
    grid-template-columns: repeat(2, minmax(0, 1fr));
  }
}

@media (max-width: 720px) {
  .stats-row {
    grid-template-columns: 1fr;
  }
}
</style>
