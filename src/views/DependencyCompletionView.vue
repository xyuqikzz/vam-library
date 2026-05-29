<template>
  <div class="dependency-completion-view animate-fadeIn">
    <section v-if="!localLibraryStore.loading" class="stats-row">
      <StatCard
        title="缺失依赖条目"
        :value="missingEntryCount"
        subtitle="未满足的依赖记录总数"
        icon="M12 8v4m0 4h.01M10.29 3.86l-7.43 12.88A2 2 0 0 0 4.58 19h14.84a2 2 0 0 0 1.72-3.02L13.71 3.86a2 2 0 0 0-3.42 0z"
        trend="neutral"
        color="#f59e0b"
      />
      <StatCard
        title="去重后包数"
        :value="dependencyGroups.length"
        subtitle="按 creator + package 去重"
        icon="M4 7h16M4 12h10M4 17h7"
        trend="neutral"
        color="#6e6bf0"
      />
      <StatCard
        title="受影响资源包"
        :value="affectedPackageCount"
        subtitle="这些包引用了缺失依赖"
        icon="M12 3l8 4v10l-8 4-8-4V7l8-4z"
        trend="neutral"
        color="#5b8def"
      />
      <StatCard
        title="版本过低"
        :value="lowerVersionCount"
        subtitle="本地已装但版本不够"
        icon="M12 19V5m0 0 4 4m-4-4-4 4"
        trend="neutral"
        color="#34d399"
      />
    </section>

    <section v-else class="stats-row">
      <div v-for="idx in 4" :key="idx" class="stat-skeleton glass-panel">
        <div class="skeleton-line short"></div>
        <div class="skeleton-line big"></div>
        <div class="skeleton-line tiny"></div>
      </div>
    </section>

    <div class="toolbar glass-panel">
      <div class="toolbar-left">
        <span class="toolbar-label">本地缺失依赖</span>
        <span class="toolbar-count">{{ dependencyGroups.length }} 组</span>
      </div>
      <div class="toolbar-right">
        <span v-if="lastRefreshedText" class="toolbar-hint">{{ lastRefreshedText }}</span>
        <button class="toolbar-btn" :disabled="busy || !appStore.vamRootPath" @click="refreshLibrary">
          <svg width="14" height="14" viewBox="0 0 24 24" fill="none">
            <path d="M21 12a9 9 0 1 1-6.219-8.56" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" />
          </svg>
          <span>{{ localLibraryStore.loading ? '加载中...' : '重新读取' }}</span>
        </button>
        <button class="toolbar-btn primary" :disabled="!dependencyGroups.length || busy" @click="openPreview">
          <svg width="14" height="14" viewBox="0 0 24 24" fill="none">
            <path d="M4 12h10m0 0-4-4m4 4-4 4M14 7h4a2 2 0 0 1 2 2v6" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" />
          </svg>
          <span>生成补全方案</span>
        </button>
      </div>
    </div>

    <div class="content-panel glass-panel">
      <div class="panel-header">
        <div>
          <h2 class="panel-title">去重后的缺失依赖</h2>
          <p class="panel-desc">每条记录代表一个包名，右侧显示它被多少个资源包引用。</p>
        </div>
        <span class="panel-count">{{ dependencyGroups.length }}</span>
      </div>

      <div v-if="dependencyGroups.length > 0" class="group-list">
        <div v-for="group in dependencyGroups" :key="group.key" class="group-row">
          <div class="group-main">
            <div class="group-title-row">
              <h3 class="group-title" :title="group.fullId">{{ group.fullId }}</h3>
              <span class="group-tag" :class="group.statusClass">{{ group.statusLabel }}</span>
            </div>
            <div class="group-meta">
              <span>要求版本：{{ group.requiredVersionLabel }}</span>
              <span>缺失记录：{{ group.occurrenceCount }} 条</span>
              <span>受影响包：{{ group.dependentPackageCount }} 个</span>
              <span v-if="group.installedVersion !== null">本地版本：v{{ group.installedVersion }}</span>
            </div>
            <div class="group-sources" v-if="group.dependentPackages.length > 0">
              <span v-for="pkg in group.dependentPackages.slice(0, 4)" :key="pkg" class="source-pill" :title="pkg">
                {{ pkg }}
              </span>
              <span v-if="group.dependentPackages.length > 4" class="source-more">
                还有 {{ group.dependentPackages.length - 4 }} 个
              </span>
            </div>
          </div>

          <div class="group-actions">
            <button class="copy-btn" @click="copyIdentifier(group)">
              <svg width="14" height="14" viewBox="0 0 24 24" fill="none">
                <rect x="9" y="9" width="10" height="10" rx="2" stroke="currentColor" stroke-width="1.5" />
                <rect x="5" y="5" width="10" height="10" rx="2" stroke="currentColor" stroke-width="1.5" opacity="0.45" />
              </svg>
              <span>复制标识</span>
            </button>
          </div>
        </div>
      </div>

      <EmptyState
        v-else
        icon="M4 7h16M4 12h16M4 17h10"
        title="暂无缺失依赖"
        :description="emptyDescription"
      >
        <template v-if="!appStore.vamRootPath" #action>
          <button class="toolbar-btn primary" @click="goSettings">
            去设置目录
          </button>
        </template>
      </EmptyState>
    </div>

    <Teleport to="body">
      <Transition name="fade">
        <div v-if="previewOpen" class="preview-overlay" @click.self="closePreview">
          <div class="preview-modal glass-panel">
            <div class="preview-header">
              <div>
                <h3 class="preview-title">补全依赖弹窗</h3>
                <p class="preview-desc">
                  已按包名去重，并按当前策略从 VaM Hub 查询可下载版本。没有来源的包会保留在列表里，但不会进入下载队列。
                </p>
              </div>
              <button class="preview-close" @click="closePreview">
                <svg width="16" height="16" viewBox="0 0 24 24" fill="none">
                  <path d="M18 6L6 18M6 6l12 12" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" />
                </svg>
              </button>
            </div>

            <div class="preview-body">
              <div class="mode-bar">
                <button :class="['mode-btn', { active: resolutionMode === 'required' }]" @click="resolutionMode = 'required'">
                  按要求版本
                </button>
                <button :class="['mode-btn', { active: resolutionMode === 'latest' }]" @click="resolutionMode = 'latest'">
                  直接下载最新
                </button>
                <span class="mode-hint">
                  当前会排除 Hub 找不到的包。
                </span>
              </div>

              <div class="preview-summary">
                <div class="summary-chip">可下载 {{ downloadCandidates.length }} 组</div>
                <div class="summary-chip warn">无来源 {{ missingSourceCount }} 组</div>
                <div class="summary-chip warn">版本不符 {{ versionMismatchCount }} 组</div>
                <div class="summary-chip">总计 {{ previewGroups.length }} 组</div>
              </div>

              <div v-if="previewLoading" class="preview-loading">
                <div class="loader"></div>
                <p>正在查询 Hub 资源... {{ resolvingProgress.current }} / {{ resolvingProgress.total }}</p>
              </div>

              <div v-else class="preview-list">
                <div v-for="group in previewGroupsView" :key="group.key" class="preview-row">
                  <div class="preview-main">
                    <div class="preview-row-title">
                      <h4 class="preview-name">{{ group.fullId }}</h4>
                      <span class="preview-badge" :class="group.previewStatusClass">{{ group.previewStatusLabel }}</span>
                    </div>
                    <div class="preview-meta">
                      <span>要求版本：{{ group.requiredVersionLabel }}</span>
                      <span>可用版本：{{ group.selectedFile ? `v${group.selectedFile.version}` : '无' }}</span>
                      <span>缺失记录：{{ group.occurrenceCount }} 条</span>
                    </div>
                    <div class="preview-file" v-if="group.selectedFile">
                      {{ group.selectedFile.filename }} · {{ formatSize(group.selectedFile.sizeBytes) }}
                    </div>
                    <div class="preview-file muted" v-else>
                      {{ group.previewReason }}
                    </div>
                  </div>

                  <div class="preview-side">
                    <button class="copy-btn" @click="copyIdentifier(group)">
                      <svg width="14" height="14" viewBox="0 0 24 24" fill="none">
                        <rect x="9" y="9" width="10" height="10" rx="2" stroke="currentColor" stroke-width="1.5" />
                        <rect x="5" y="5" width="10" height="10" rx="2" stroke="currentColor" stroke-width="1.5" opacity="0.45" />
                      </svg>
                      <span>复制标识</span>
                    </button>
                    <span v-if="group.selectedFile" class="download-ready">可加入队列</span>
                    <span v-else class="download-empty">不会下载</span>
                  </div>
                </div>
              </div>
            </div>

            <div class="preview-footer">
              <div class="footer-stats">
                <span>将加入：{{ downloadCandidates.length }} 组</span>
                <span v-if="missingSourceCount > 0">已排除：{{ missingSourceCount }} 组无来源包</span>
              </div>
              <div class="footer-actions">
                <button class="toolbar-btn" @click="closePreview">取消</button>
                <button class="toolbar-btn primary" :disabled="!downloadCandidates.length || previewLoading" @click="downloadCandidatesToQueue">
                  {{ resolutionMode === 'latest' ? '下载最新版本' : '下载要求版本' }}
                </button>
              </div>
            </div>
          </div>
        </div>
      </Transition>
    </Teleport>
  </div>
</template>

<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import { useRouter } from 'vue-router'
import { invoke } from '@tauri-apps/api/core'
import { useAppStore } from '@/stores/app'
import { useDownloadStore } from '@/stores/download'
import { useLocalLibraryStore } from '@/stores/localLibrary'
import { useNotification } from '@/composables/useNotification'
import StatCard from '@/components/common/StatCard.vue'
import EmptyState from '@/components/common/EmptyState.vue'

/*
interface MissingDependency {
  package_id: string
  depends_on_id: string
  required_version: string
  dependent_package: string
  status: 'missing' | 'lower_version' | string
  installed_version: number | null
}
*/

interface HubFile {
  filename?: string | null
  file_size?: string | null
  creatorName?: string | null
  licenseType?: string | null
  urlHosted?: string | null
}

interface HubPackageInfo {
  title?: string | null
  username?: string | null
  resource_id?: string | null
  hubFiles?: HubFile[] | null
}

interface HubFileCandidate {
  filename: string
  version: number
  sizeBytes: number
  url: string
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
  previewStatusLabel: string
  previewStatusClass: string
  previewReason: string
  selectedFile: HubFileCandidate | null
}

const router = useRouter()
const appStore = useAppStore()
const downloadStore = useDownloadStore()
const localLibraryStore = useLocalLibraryStore()
const notify = useNotification()

const previewOpen = ref(false)
const previewLoading = ref(false)
const previewGroups = ref<PreviewGroup[]>([])
const resolutionMode = ref<'required' | 'latest'>('required')
const resolvingProgress = ref({ current: 0, total: 0 })
const lastRefreshedAt = ref<Date | null>(null)

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
        requiredVersionLabel: typeof requiredVersion === 'number' ? `v${requiredVersion}` : '最新',
        dependentPackages: item.dependent_package ? [item.dependent_package] : [],
        dependentPackageCount: item.dependent_package ? 1 : 0,
        occurrenceCount: 1,
        installedVersion: item.installed_version,
        statusLabel: item.status === 'lower_version' ? '版本过低' : '缺失',
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
      existing.statusLabel = '版本过低'
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
    let previewStatusLabel = '可下载'
    let previewStatusClass = 'ok'
    let previewReason = ''

    if (group.hubFiles.length === 0) {
      previewStatusLabel = '无来源'
      previewStatusClass = 'danger'
      previewReason = 'Hub 没有可下载的包'
    } else if (!selectedFile) {
      previewStatusLabel = resolutionMode.value === 'latest' ? '无可用版本' : '无对应版本'
      previewStatusClass = 'warn'
      previewReason = resolutionMode.value === 'latest'
        ? 'Hub 有包，但无法解析成可下载版本'
        : 'Hub 有包，但没有精确匹配当前要求版本的文件'
    } else if (resolutionMode.value === 'latest') {
      previewStatusLabel = '最新版本'
      previewStatusClass = 'ok'
      previewReason = '将下载 Hub 中最高版本'
    } else {
      previewStatusLabel = selectedFile && group.requiredVersion !== null && selectedFile.version > group.requiredVersion
        ? '更高版本'
        : '要求版本'
      previewStatusClass = 'ok'
      previewReason = selectedFile && group.requiredVersion !== null && selectedFile.version > group.requiredVersion
        ? '将下载满足最低要求的更高版本'
        : '将下载当前要求的版本'
    }

    return {
      ...group,
      selectedFile,
      previewStatusLabel,
      previewStatusClass,
      previewReason,
    }
  })
})

const downloadCandidates = computed(() => (
  previewGroupsView.value.filter(group => Boolean(group.selectedFile))
))

const missingSourceCount = computed(() => (
  previewGroupsView.value.filter(group => group.hubFiles.length === 0).length
))

const versionMismatchCount = computed(() => (
  previewGroupsView.value.filter(group => group.hubFiles.length > 0 && !group.selectedFile).length
))

const lastRefreshedText = computed(() => {
  if (!lastRefreshedAt.value) return ''
  return `上次刷新：${lastRefreshedAt.value.toLocaleTimeString('zh-CN', { hour: '2-digit', minute: '2-digit' })}`
})

const emptyDescription = computed(() => (
  appStore.vamRootPath
    ? '当前没有需要补全的缺失依赖。'
    : '请先设置 VAM 目录并完成扫描。'
))

onMounted(async () => {
  await localLibraryStore.ensureLoaded()
  lastRefreshedAt.value = new Date()
})

async function refreshLibrary() {
  if (!appStore.vamRootPath) {
    notify.warning('请先在设置里选择 VAM 目录')
    router.push('/settings')
    return
  }

  try {
    await localLibraryStore.refreshAll('refreshing')
    lastRefreshedAt.value = new Date()
    notify.success('已刷新依赖统计')
  } catch (error) {
    notify.error(`刷新失败：${String(error)}`)
  }
}

async function openPreview() {
  if (!dependencyGroups.value.length) {
    notify.warning('当前没有可补全的依赖')
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
    previewStatusLabel: '查询中',
    previewStatusClass: 'pending',
    previewReason: '正在查询 Hub...',
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
          const files = parseHubFiles(detail.hubFiles || [])
          return {
            ...group,
            hubTitle: detail.title || '',
            hubUsername: detail.username || '',
            hubFiles: files,
            previewStatusLabel: files.length > 0 ? '已找到' : '无来源',
            previewStatusClass: files.length > 0 ? 'ok' : 'danger',
            previewReason: files.length > 0 ? '已从 Hub 读取可下载文件' : 'Hub 没有可下载文件',
          }
        } catch (error) {
          return {
            ...group,
            hubFiles: [],
            previewStatusLabel: '查询失败',
            previewStatusClass: 'danger',
            previewReason: String(error),
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
    notify.warning('没有可加入下载队列的依赖')
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
      notify.success(`已加入 ${addedCount} 个下载任务`)
    }
    if (skippedCount > 0) {
      notify.info(`已跳过 ${skippedCount} 个本地已存在的任务`)
    }

    previewOpen.value = false
    router.push('/download')
  } catch (error) {
    notify.error(`加入下载队列失败：${String(error)}`)
  }
}

async function copyIdentifier(group: DependencyGroup | PreviewGroup) {
  const text = getCopyIdentifier(group)
  try {
    await navigator.clipboard.writeText(text)
    notify.success(`已复制 ${text}`)
  } catch (error) {
    notify.error(`复制失败：${String(error)}`)
  }
}

function goSettings() {
  router.push('/settings')
}

function selectHubFile(group: PreviewGroup): HubFileCandidate | null {
  if (!group.hubFiles.length) return null

  const sortedFiles = [...group.hubFiles].sort((a, b) => a.version - b.version)
  if (resolutionMode.value === 'latest' || group.requiredVersion === null) {
    return sortedFiles[sortedFiles.length - 1] || null
  }

  return sortedFiles.find(file => file.version >= group.requiredVersion!) || null
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

function parseDependencyId(dependsOnId: string) {
  const cleaned = dependsOnId.replace(/\.var$/i, '')
  const parts = cleaned.split('.')
  if (parts.length < 2) return null

  const creator = parts[0]
  if (parts.length === 2) {
    return {
      creator,
      name: parts[1],
      version: null as number | null,
    }
  }

  const versionPart = parts[parts.length - 1]
  const name = parts.slice(1, -1).join('.')
  if (!versionPart || versionPart.toLowerCase() === 'latest') {
    return {
      creator,
      name,
      version: null as number | null,
    }
  }

  const version = Number.parseInt(versionPart, 10)
  return {
    creator,
    name,
    version: Number.isFinite(version) ? version : null,
  }
}

function parseRequiredVersion(requiredVersion: string) {
  const text = requiredVersion.trim()
  if (!text || text.toLowerCase() === 'latest') return 'latest' as const
  const version = Number.parseInt(text, 10)
  return Number.isFinite(version) ? version : 'latest'
}

function parseHubFiles(files: HubFile[]) {
  return files
    .map((file) => {
      if (!file.filename || !file.urlHosted) return null
      const parsed = parseDependencyId(file.filename)
      if (!parsed || parsed.version === null) return null
      return {
        filename: file.filename,
        version: parsed.version,
        sizeBytes: parseSizeValue(file.file_size),
        url: file.urlHosted,
      } satisfies HubFileCandidate
    })
    .filter((item): item is HubFileCandidate => Boolean(item))
    .sort((a, b) => b.version - a.version)
}

function parseSizeValue(value: string | null | undefined) {
  if (!value) return 0
  const text = String(value).trim().toUpperCase()
  const match = text.match(/^([\d.]+)\s*(B|KB|MB|GB|TB)?$/)
  if (!match) return 0
  const size = Number.parseFloat(match[1])
  const unit = match[2] || 'B'
  const map: Record<string, number> = {
    B: 1,
    KB: 1024,
    MB: 1024 * 1024,
    GB: 1024 * 1024 * 1024,
    TB: 1024 * 1024 * 1024 * 1024,
  }
  return Math.round(size * (map[unit] || 1))
}

function formatSize(bytes: number) {
  if (!bytes) return '0 B'
  const units = ['B', 'KB', 'MB', 'GB', 'TB']
  const index = Math.min(Math.floor(Math.log(bytes) / Math.log(1024)), units.length - 1)
  return `${(bytes / Math.pow(1024, index)).toFixed(index > 0 ? 1 : 0)} ${units[index]}`
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
  gap: var(--space-4);
  min-height: 100%;
}

.toolbar,
.content-panel {
  border-radius: var(--radius-lg);
}

.toolbar-right,
.footer-actions {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  flex-wrap: wrap;
}

.toolbar-btn,
.copy-btn,
.mode-btn {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  gap: var(--space-2);
  border-radius: var(--radius-md);
  font-size: var(--text-sm);
  font-weight: var(--font-medium);
  transition: all var(--duration-fast) var(--ease);
  white-space: nowrap;
}

.toolbar-btn {
  height: 34px;
  padding: 0 var(--space-4);
  border: 1px solid var(--border-subtle);
  background: var(--bg-hover);
  color: var(--text-primary);
}

.toolbar-btn.primary {
  background: var(--accent-gradient);
  color: #fff;
  border-color: transparent;
}

.toolbar-btn:hover:not(:disabled),
.copy-btn:hover {
  transform: translateY(-1px);
  border-color: var(--border-strong);
}

.toolbar-btn:disabled {
  opacity: 0.45;
  cursor: not-allowed;
  transform: none;
}

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
  background: rgba(255, 255, 255, 0.06);
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

.toolbar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--space-3);
  padding: var(--space-4) var(--space-5);
}

.toolbar-left {
  display: flex;
  align-items: center;
  gap: var(--space-3);
  min-width: 0;
}

.toolbar-label {
  font-size: var(--text-sm);
  font-weight: var(--font-semibold);
  color: var(--text-primary);
}

.toolbar-count,
.toolbar-hint,
.panel-count {
  font-size: var(--text-xs);
  color: var(--text-tertiary);
}

.content-panel {
  display: flex;
  flex-direction: column;
  min-height: 0;
  overflow: hidden;
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

.panel-count {
  padding: 2px 8px;
  border-radius: var(--radius-full);
  background: var(--bg-hover);
}

.group-list {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
  padding: var(--space-4);
  overflow-y: auto;
}

.group-row {
  display: flex;
  align-items: stretch;
  justify-content: space-between;
  gap: var(--space-4);
  padding: var(--space-4);
  border-radius: var(--radius-md);
  background: rgba(255, 255, 255, 0.02);
  border: 1px solid rgba(255, 255, 255, 0.05);
}

.group-main {
  flex: 1;
  min-width: 0;
}

.group-title-row,
.preview-row-title {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--space-3);
}

.group-title,
.preview-name {
  min-width: 0;
  font-size: var(--text-sm);
  font-weight: var(--font-semibold);
  color: var(--text-primary);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.group-tag,
.preview-badge {
  flex-shrink: 0;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  padding: 2px 8px;
  border-radius: var(--radius-full);
  font-size: 10px;
  font-weight: var(--font-semibold);
}

.group-tag.ok,
.preview-badge.ok {
  background: rgba(52, 211, 153, 0.12);
  color: var(--color-success);
}

.group-tag.warn,
.preview-badge.warn {
  background: rgba(251, 191, 36, 0.12);
  color: var(--color-warning);
}

.group-tag.danger,
.preview-badge.danger {
  background: rgba(248, 113, 113, 0.12);
  color: var(--color-error);
}

.group-tag.pending,
.preview-badge.pending {
  background: rgba(96, 165, 250, 0.12);
  color: var(--color-info);
}

.group-meta,
.preview-meta {
  display: flex;
  align-items: center;
  gap: var(--space-3);
  margin-top: var(--space-2);
  flex-wrap: wrap;
  font-size: var(--text-xs);
  color: var(--text-secondary);
}

.group-sources {
  display: flex;
  flex-wrap: wrap;
  gap: var(--space-2);
  margin-top: var(--space-3);
}

.source-pill,
.source-more {
  display: inline-flex;
  align-items: center;
  max-width: 100%;
  padding: 2px 8px;
  border-radius: var(--radius-full);
  background: var(--bg-hover);
  color: var(--text-secondary);
  font-size: 10px;
}

.source-pill {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.group-actions,
.preview-side {
  display: flex;
  flex-direction: column;
  align-items: flex-end;
  justify-content: space-between;
  gap: var(--space-2);
  flex-shrink: 0;
}

.copy-btn {
  height: 32px;
  padding: 0 var(--space-3);
  border: 1px solid var(--border-subtle);
  background: var(--bg-hover);
  color: var(--text-primary);
}

.preview-overlay {
  position: fixed;
  inset: 0;
  z-index: calc(var(--z-overlay) + 5);
  background: rgba(22, 22, 24, 0.6);
  /* backdrop-filter removed */
  display: flex;
  align-items: center;
  justify-content: center;
  padding: var(--space-6);
}

.preview-modal {
  width: min(1080px, 100%);
  max-height: min(88vh, 960px);
  display: flex;
  flex-direction: column;
  overflow: hidden;
}

.preview-header {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: var(--space-3);
  padding: var(--space-5);
  border-bottom: 1px solid var(--border-subtle);
}

.preview-title {
  font-size: var(--text-lg);
  font-weight: var(--font-semibold);
  color: var(--text-primary);
}

.preview-desc {
  margin-top: 4px;
  max-width: 760px;
  font-size: var(--text-sm);
  color: var(--text-secondary);
  line-height: 1.6;
}

.preview-close {
  width: 32px;
  height: 32px;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  border-radius: var(--radius-full);
  color: var(--text-secondary);
  background: var(--bg-hover);
}

.preview-body {
  display: flex;
  flex-direction: column;
  gap: var(--space-4);
  padding: var(--space-5);
  overflow: hidden;
}

.mode-bar {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  flex-wrap: wrap;
}

.mode-btn {
  height: 34px;
  padding: 0 var(--space-4);
  border: 1px solid var(--border-subtle);
  background: var(--bg-hover);
  color: var(--text-primary);
}

.mode-btn.active {
  background: rgba(110, 107, 240, 0.16);
  color: var(--accent-primary);
  border-color: rgba(110, 107, 240, 0.3);
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

.summary-chip {
  display: inline-flex;
  align-items: center;
  padding: 4px 10px;
  border-radius: var(--radius-full);
  background: rgba(52, 211, 153, 0.1);
  color: var(--color-success);
  font-size: 10px;
  font-weight: var(--font-semibold);
}

.summary-chip.warn {
  background: rgba(251, 191, 36, 0.12);
  color: var(--color-warning);
}

.preview-loading {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: var(--space-4);
  min-height: 280px;
}

.loader {
  width: 28px;
  height: 28px;
  border-radius: 50%;
  border: 2px solid var(--accent-primary);
  border-top-color: transparent;
  animation: spin 0.9s linear infinite;
}

.preview-list {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
  overflow-y: auto;
  min-height: 320px;
  max-height: 56vh;
  padding-right: 2px;
}

.preview-row {
  display: flex;
  align-items: stretch;
  justify-content: space-between;
  gap: var(--space-4);
  padding: var(--space-4);
  border-radius: var(--radius-md);
  background: rgba(255, 255, 255, 0.02);
  border: 1px solid rgba(255, 255, 255, 0.05);
}

.preview-main {
  flex: 1;
  min-width: 0;
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

.download-ready {
  color: var(--color-success);
  font-size: 10px;
  font-weight: var(--font-semibold);
}

.download-empty {
  color: var(--color-warning);
  font-size: 10px;
  font-weight: var(--font-semibold);
}

.preview-footer {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--space-4);
  padding: var(--space-4) var(--space-5);
  border-top: 1px solid var(--border-subtle);
}

.footer-stats {
  display: flex;
  align-items: center;
  gap: var(--space-3);
  flex-wrap: wrap;
  font-size: var(--text-xs);
  color: var(--text-secondary);
}

@keyframes spin {
  to {
    transform: rotate(360deg);
  }
}

@media (max-width: 1200px) {
  .stats-row {
    grid-template-columns: repeat(2, minmax(0, 1fr));
  }

  .group-row,
  .preview-row {
    flex-direction: column;
  }

  .group-actions,
  .preview-side {
    align-items: flex-start;
  }
}

@media (max-width: 720px) {
  .toolbar,
  .preview-header,
  .preview-footer {
    flex-direction: column;
    align-items: flex-start;
  }

  .stats-row {
    grid-template-columns: 1fr;
  }

  .toolbar-right,
  .footer-actions {
    width: 100%;
  }

  .toolbar-btn,
  .copy-btn {
    width: 100%;
  }
}
</style>
