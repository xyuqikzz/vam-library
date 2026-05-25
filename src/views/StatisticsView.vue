<template>
  <div class="statistics-view animate-fadeIn">
    <div class="statistics-header">
      <div>
        <h1 class="statistics-title">数据统计</h1>
        <p class="statistics-subtitle">基于当前扫描入库的 .var 包生成统计概览。</p>
      </div>
      <button class="refresh-btn" :disabled="loading" @click="loadPackages">
        <svg width="14" height="14" viewBox="0 0 24 24" fill="none">
          <path d="M21 12a9 9 0 1 1-6.219-8.56" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" />
        </svg>
        <span>{{ loading ? '加载中' : '刷新' }}</span>
      </button>
    </div>

    <div class="statistics-tabs">
      <button :class="['tab-btn', { active: activeTab === 'overview' }]" @click="activeTab = 'overview'">总览概况</button>
      <button :class="['tab-btn', { active: activeTab === 'analysis' }]" @click="activeTab = 'analysis'">资源分析</button>
    </div>

    <div v-if="loading" class="loading-panel glass-panel">正在加载统计数据...</div>

    <div v-else-if="packages.length === 0" class="empty-panel glass-panel">
      <h2>暂无统计数据</h2>
      <p>请先选择 VAM 目录并完成扫描，数据统计会在这里显示。</p>
    </div>

    <template v-else>
      <div v-if="activeTab === 'overview'" class="statistics-content">
        <section class="stats-card glass-panel">
          <h2 class="section-title">概况</h2>
          <div class="summary-grid">
            <div class="summary-item">
              <div class="summary-icon icon-green">
                <svg width="18" height="18" viewBox="0 0 24 24" fill="none">
                  <path d="M12 2L3 7v10l9 5 9-5V7l-9-5Z" stroke="currentColor" stroke-width="1.5" stroke-linejoin="round" />
                  <path d="M12 12l9-5M12 12v10M12 12L3 7" stroke="currentColor" stroke-width="1.5" />
                </svg>
              </div>
              <div>
                <span class="summary-label">总资源包数</span>
                <strong class="summary-value">{{ formatNumber(totalPackages) }}</strong>
              </div>
            </div>
            <div class="summary-item">
              <div class="summary-icon icon-blue">
                <svg width="18" height="18" viewBox="0 0 24 24" fill="none">
                  <path d="M4 7a8 3 0 1 0 16 0 8 3 0 1 0-16 0ZM4 7v10a8 3 0 1 0 16 0V7" stroke="currentColor" stroke-width="1.5" />
                  <path d="M4 12a8 3 0 1 0 16 0" stroke="currentColor" stroke-width="1.5" />
                </svg>
              </div>
              <div>
                <span class="summary-label">总文件大小</span>
                <strong class="summary-value highlight">{{ formatSize(totalSizeBytes) }}</strong>
              </div>
            </div>
            <div class="summary-item">
              <div class="summary-icon icon-purple">
                <svg width="18" height="18" viewBox="0 0 24 24" fill="none">
                  <path d="M8 4h8v16H8zM4 8h4v12H4zM16 11h4v9h-4z" stroke="currentColor" stroke-width="1.5" stroke-linejoin="round" />
                </svg>
              </div>
              <div>
                <span class="summary-label">平均包大小</span>
                <strong class="summary-value">{{ formatSize(averageSizeBytes) }}</strong>
              </div>
            </div>
          </div>
        </section>

        <section class="stats-card glass-panel">
          <h2 class="section-title">存储空间分布</h2>
          <div class="directory-table">
            <div class="directory-head">
              <span>目录名称</span>
              <span>文件数</span>
              <span>大小</span>
              <span>占比</span>
            </div>
            <div class="directory-row root-row">
              <span class="directory-name">
                <svg width="14" height="14" viewBox="0 0 24 24" fill="none">
                  <path d="M6 9l6 6 6-6" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round" />
                </svg>
                AddonPackages
              </span>
              <span>{{ totalPackages }}</span>
              <span>{{ formatSize(totalSizeBytes) }}</span>
              <span>100%</span>
            </div>
            <div v-for="row in directoryRows" :key="row.name" class="directory-row">
              <span class="directory-name child-name">
                <svg width="14" height="14" viewBox="0 0 24 24" fill="none">
                  <path d="M9 6l6 6-6 6" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round" />
                </svg>
                {{ row.name }}
              </span>
              <span>{{ row.count }}</span>
              <span>{{ formatSize(row.sizeBytes) }}</span>
              <span>{{ formatPercent(row.sizeBytes / totalSizeBytes) }}</span>
            </div>
          </div>
        </section>

        <section class="stats-card glass-panel">
          <h2 class="section-title">资源库健康度</h2>
          <div class="health-layout">
            <div class="health-ring" :style="healthRingStyle">
              <div class="health-ring-inner">
                <strong>{{ totalHealthIssues }}</strong>
                <span>个异常</span>
                <small>{{ formatPercent(healthIssueRate) }}</small>
              </div>
            </div>
            <div class="health-bars">
              <div v-for="item in healthItems" :key="item.label" class="health-item">
                <div class="bar-label">
                  <span>{{ item.label }}</span>
                  <strong>{{ item.count }} 个</strong>
                </div>
                <div class="track">
                  <div class="fill" :class="item.className" :style="{ width: `${item.percent}%` }" />
                </div>
              </div>
            </div>
          </div>
        </section>

        <section class="stats-card glass-panel">
          <h2 class="section-title">类别分布</h2>
          <div class="category-layout">
            <div class="donut" :style="{ background: categoryDonutGradient }">
              <div class="donut-inner">
                <strong>{{ totalPackages }}</strong>
                <span>总计</span>
              </div>
            </div>
            <div class="category-grid">
              <div v-for="item in categoryRows" :key="item.type" class="category-row">
                <div class="category-name">
                  <span class="type-dot" :style="{ background: item.color }" />
                  <span>{{ item.label }}</span>
                  <strong>{{ item.count }}</strong>
                </div>
                <div class="category-track">
                  <div class="category-fill" :style="{ width: `${item.percent}%`, background: item.color }" />
                </div>
                <span class="category-percent">{{ item.percent.toFixed(1) }}%</span>
              </div>
            </div>
          </div>
        </section>
      </div>

      <div v-else class="statistics-content">
        <section class="stats-card glass-panel">
          <h2 class="section-title">作者 TOP 20</h2>
          <div class="rank-list">
            <div v-for="(item, index) in authorTopRows" :key="item.creator" class="rank-row">
              <span class="rank-index">{{ index + 1 }}</span>
              <span class="rank-name" :title="item.creator">{{ item.creator }}</span>
              <div class="rank-track">
                <div class="rank-fill" :class="{ top: index === 0 }" :style="{ width: `${item.percent}%` }">
                  <span>{{ item.count }}</span>
                </div>
              </div>
              <span class="rank-percent">{{ item.percent.toFixed(1) }}%</span>
            </div>
          </div>
        </section>

        <section class="stats-card glass-panel">
          <h2 class="section-title">文件大小分布</h2>
          <div class="size-list">
            <div v-for="item in sizeDistributionRows" :key="item.label" class="size-row">
              <span class="size-label">{{ item.label }}</span>
              <div class="size-track">
                <div class="size-fill" :style="{ width: `${item.percent}%` }">
                  <span>{{ item.percent.toFixed(0) }}%</span>
                </div>
              </div>
              <span class="size-count">{{ item.count }}</span>
            </div>
          </div>
        </section>
      </div>
    </template>
  </div>
</template>

<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import { storeToRefs } from 'pinia'
import { useAppStore } from '@/stores/app'
import { useLocalLibraryStore } from '@/stores/localLibrary'
import type { PackageDisplayItem } from '@/types/package'

interface DirectoryRow {
  name: string
  count: number
  sizeBytes: number
}

interface CategoryDefinition {
  type: string
  label: string
  color: string
}

const appStore = useAppStore()
const localLibraryStore = useLocalLibraryStore()
const { vamRootPath } = storeToRefs(appStore)
const { packages, loading } = storeToRefs(localLibraryStore)

const activeTab = ref<'overview' | 'analysis'>('overview')

const categoryDefinitions: CategoryDefinition[] = [
  { type: 'scene', label: '场景', color: '#ff8058' },
  { type: 'plugin', label: '插件', color: '#56bd63' },
  { type: 'asset', label: '资产', color: '#ffb64d' },
  { type: 'appearance', label: '外观', color: '#f27aa6' },
  { type: 'clothing', label: '服装', color: '#9aa4db' },
  { type: 'hair', label: '发型', color: '#5fb2f2' },
  { type: 'morph', label: '变形', color: '#43c6e8' },
  { type: 'other', label: '其他', color: '#b8c4cc' },
  { type: 'unknown', label: 'Unknown', color: '#c7d0d5' },
]

const totalPackages = computed(() => packages.value.length)
const totalSizeBytes = computed(() => packages.value.reduce((sum, pkg) => sum + pkg.size_bytes, 0))
const averageSizeBytes = computed(() => totalPackages.value > 0 ? totalSizeBytes.value / totalPackages.value : 0)

const directoryRows = computed<DirectoryRow[]>(() => {
  const map = new Map<string, DirectoryRow>()
  for (const pkg of packages.value) {
    const name = topLevelDirectory(pkg)
    if (name === 'AddonPackages') continue
    const row = map.get(name) || { name, count: 0, sizeBytes: 0 }
    row.count += 1
    row.sizeBytes += pkg.size_bytes
    map.set(name, row)
  }
  return [...map.values()]
    .sort((a, b) => b.sizeBytes - a.sizeBytes || a.name.localeCompare(b.name))
    .slice(0, 10)
})

const categoryRows = computed(() => {
  const counts = new Map<string, number>()
  for (const pkg of packages.value) {
    const type = primaryType(pkg)
    counts.set(type, (counts.get(type) || 0) + 1)
  }

  return categoryDefinitions
    .map(item => {
      const count = counts.get(item.type) || 0
      return {
        ...item,
        count,
        percent: totalPackages.value > 0 ? count / totalPackages.value * 100 : 0,
      }
    })
    .filter(item => item.count > 0)
    .sort((a, b) => b.count - a.count)
})

const categoryDonutGradient = computed(() => {
  if (categoryRows.value.length === 0) return '#e9edf2'
  let cursor = 0
  const segments = categoryRows.value.map((item) => {
    const start = cursor
    const end = cursor + item.percent
    cursor = end
    return `${item.color} ${start}% ${end}%`
  })
  return `conic-gradient(${segments.join(', ')})`
})

const fileNameIssueCount = computed(() => packages.value.filter(pkg => normalizedBaseName(pkg.file_path) !== `${pkg.id.toLowerCase()}.var`).length)
const unknownTypeCount = computed(() => packages.value.filter(pkg => primaryType(pkg) === 'unknown').length)
const emptyContentCount = computed(() => packages.value.filter(pkg => pkg.content_count <= 0).length)
const totalHealthIssues = computed(() => fileNameIssueCount.value + unknownTypeCount.value + emptyContentCount.value)
const healthIssueRate = computed(() => totalPackages.value > 0 ? totalHealthIssues.value / totalPackages.value : 0)
const healthRingStyle = computed(() => ({
  '--health-angle': `${Math.min(healthIssueRate.value, 1) * 360}deg`,
}))
const healthItems = computed(() => [
  {
    label: '文件名格式错误',
    count: fileNameIssueCount.value,
    percent: healthPercent(fileNameIssueCount.value),
    className: 'warning',
  },
  {
    label: '类型未识别',
    count: unknownTypeCount.value,
    percent: healthPercent(unknownTypeCount.value),
    className: 'info',
  },
  {
    label: '内容索引为空',
    count: emptyContentCount.value,
    percent: healthPercent(emptyContentCount.value),
    className: 'danger',
  },
])

const authorTopRows = computed(() => {
  const map = new Map<string, number>()
  for (const pkg of packages.value) {
    const creator = pkg.creator || 'Unknown'
    map.set(creator, (map.get(creator) || 0) + 1)
  }

  return [...map.entries()]
    .map(([creator, count]) => ({
      creator,
      count,
      percent: totalPackages.value > 0 ? count / totalPackages.value * 100 : 0,
    }))
    .sort((a, b) => b.count - a.count || a.creator.localeCompare(b.creator))
    .slice(0, 20)
})

const sizeDistributionRows = computed(() => {
  const mb = 1024 * 1024
  const gb = 1024 * mb
  const bins = [
    { label: '<1MB', min: 0, max: mb },
    { label: '1-10MB', min: mb, max: 10 * mb },
    { label: '10-50MB', min: 10 * mb, max: 50 * mb },
    { label: '50-100MB', min: 50 * mb, max: 100 * mb },
    { label: '100-500MB', min: 100 * mb, max: 500 * mb },
    { label: '500MB-1GB', min: 500 * mb, max: gb },
    { label: '>1GB', min: gb, max: Number.POSITIVE_INFINITY },
  ]

  return bins.map((bin) => {
    const count = packages.value.filter(pkg => pkg.size_bytes >= bin.min && pkg.size_bytes < bin.max).length
    return {
      label: bin.label,
      count,
      percent: totalPackages.value > 0 ? count / totalPackages.value * 100 : 0,
    }
  })
})

onMounted(async () => {
  await localLibraryStore.ensureLoaded()
})

async function loadPackages() {
  await localLibraryStore.refreshAll('refreshing')
}

function topLevelDirectory(pkg: PackageDisplayItem): string {
  const normalizedPath = normalizeSlashes(pkg.file_path)
  const root = vamRootPath.value ? normalizeSlashes(`${vamRootPath.value}/AddonPackages`) : ''
  if (root && normalizedPath.toLowerCase().startsWith(`${root.toLowerCase()}/`)) {
    const relativePath = normalizedPath.slice(root.length).replace(/^\/+/, '')
    const segments = relativePath.split('/').filter(Boolean)
    return segments.length > 1 ? segments[0] : 'AddonPackages'
  }

  const marker = '/AddonPackages/'
  const markerIndex = normalizedPath.toLowerCase().lastIndexOf(marker.toLowerCase())
  if (markerIndex >= 0) {
    const relativePath = normalizedPath.slice(markerIndex + marker.length)
    const segments = relativePath.split('/').filter(Boolean)
    return segments.length > 1 ? segments[0] : 'AddonPackages'
  }

  return '未归档'
}

function primaryType(pkg: PackageDisplayItem): string {
  const type = pkg.resource_types[0]
  if (!type) return 'unknown'
  return categoryDefinitions.some(item => item.type === type) ? type : 'unknown'
}

function normalizedBaseName(path: string): string {
  return normalizeSlashes(path).split('/').pop()?.toLowerCase() || ''
}

function normalizeSlashes(path: string): string {
  return path.replace(/\\/g, '/').replace(/\/+$/, '')
}

function healthPercent(count: number): number {
  if (count <= 0) return 0
  if (totalHealthIssues.value === 0) return 0
  return Math.max(4, count / totalHealthIssues.value * 100)
}

function formatNumber(value: number): string {
  return value.toLocaleString('zh-CN')
}

function formatSize(bytes: number): string {
  if (!Number.isFinite(bytes) || bytes <= 0) return '0B'
  const units = ['B', 'KB', 'MB', 'GB', 'TB']
  const index = Math.min(Math.floor(Math.log(bytes) / Math.log(1024)), units.length - 1)
  const value = bytes / Math.pow(1024, index)
  return `${value.toFixed(index === 0 ? 0 : 2)}${units[index]}`
}

function formatPercent(value: number): string {
  if (!Number.isFinite(value) || value <= 0) return '0%'
  return `${(value * 100).toFixed(1)}%`
}
</script>

<style scoped>
.statistics-view {
  display: flex;
  flex-direction: column;
  gap: var(--space-4);
  min-height: 0;
}

.statistics-header {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: var(--space-4);
}

.statistics-title {
  margin: 0;
  color: var(--text-primary);
  font-size: var(--text-3xl);
  font-weight: var(--font-bold);
}

.statistics-title::after {
  content: '';
  display: block;
  width: 92px;
  height: 10px;
  margin-top: -8px;
  border-radius: var(--radius-full);
  background: linear-gradient(90deg, rgba(67, 198, 232, 0.8), rgba(67, 198, 232, 0));
}

.statistics-subtitle {
  margin-top: var(--space-2);
  color: var(--text-tertiary);
  font-size: var(--text-sm);
}

.refresh-btn {
  display: inline-flex;
  align-items: center;
  gap: var(--space-2);
  height: 32px;
  padding: 0 var(--space-3);
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-sm);
  color: var(--text-secondary);
  background: var(--glass-bg);
  font-size: var(--text-sm);
  transition: color var(--duration-fast) var(--ease), background var(--duration-fast) var(--ease);
}

.refresh-btn:hover:not(:disabled) {
  color: var(--text-primary);
  background: var(--bg-hover);
}

.refresh-btn:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}

.statistics-tabs {
  display: flex;
  gap: var(--space-5);
  border-bottom: 1px solid var(--border-subtle);
}

.tab-btn {
  position: relative;
  height: 36px;
  color: var(--text-secondary);
  font-size: var(--text-sm);
}

.tab-btn.active {
  color: #43c6e8;
}

.tab-btn.active::after {
  content: '';
  position: absolute;
  left: 0;
  right: 0;
  bottom: -1px;
  height: 2px;
  border-radius: var(--radius-full);
  background: #43c6e8;
}

.statistics-content {
  display: flex;
  flex-direction: column;
  gap: var(--space-4);
  min-height: 0;
}

.stats-card {
  padding: var(--space-5);
  border-radius: var(--radius-md);
}

.section-title {
  margin: 0 0 var(--space-4);
  color: var(--text-primary);
  font-size: var(--text-lg);
  font-weight: var(--font-semibold);
}

.summary-grid {
  display: grid;
  grid-template-columns: repeat(3, minmax(0, 1fr));
  gap: var(--space-3);
}

.summary-item {
  display: flex;
  align-items: center;
  gap: var(--space-3);
  min-height: 86px;
  padding: var(--space-4);
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-sm);
  background: rgba(255, 255, 255, 0.03);
}

.summary-icon {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 36px;
  height: 36px;
  border-radius: var(--radius-md);
}

.icon-green {
  color: var(--color-success);
  background: var(--color-success-bg);
}

.icon-blue {
  color: var(--accent-secondary);
  background: var(--color-info-bg);
}

.icon-purple {
  color: var(--accent-primary);
  background: rgba(124, 92, 252, 0.16);
}

.summary-label {
  display: block;
  margin-bottom: var(--space-1);
  color: var(--text-tertiary);
  font-size: var(--text-xs);
}

.summary-value {
  display: block;
  color: var(--text-primary);
  font-size: var(--text-2xl);
  font-variant-numeric: tabular-nums;
}

.summary-value.highlight {
  background: linear-gradient(90deg, var(--accent-secondary), #f27aa6);
  -webkit-background-clip: text;
  background-clip: text;
  color: transparent;
}

.directory-table {
  overflow: hidden;
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-sm);
}

.directory-head,
.directory-row {
  display: grid;
  grid-template-columns: minmax(0, 1fr) 100px 120px 80px;
  align-items: center;
  gap: var(--space-3);
  min-height: 34px;
  padding: 0 var(--space-3);
}

.directory-head {
  color: var(--text-tertiary);
  background: rgba(255, 255, 255, 0.03);
  font-size: var(--text-xs);
  font-weight: var(--font-semibold);
}

.directory-row {
  color: var(--text-secondary);
  font-size: var(--text-sm);
  font-variant-numeric: tabular-nums;
}

.directory-row + .directory-row {
  border-top: 1px solid var(--border-subtle);
}

.root-row {
  color: var(--text-primary);
}

.directory-name {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.child-name {
  padding-left: var(--space-5);
}

.health-layout,
.category-layout {
  display: grid;
  grid-template-columns: 150px minmax(0, 1fr);
  gap: var(--space-5);
  align-items: center;
}

.health-ring,
.donut {
  display: grid;
  place-items: center;
  width: 126px;
  height: 126px;
  border-radius: 50%;
  background: conic-gradient(#d9345f 0 var(--health-angle), rgba(160, 160, 200, 0.15) var(--health-angle) 360deg);
}

.health-ring-inner,
.donut-inner {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  width: 98px;
  height: 98px;
  border-radius: 50%;
  background: var(--bg-surface);
  color: var(--text-primary);
}

.health-ring-inner strong,
.donut-inner strong {
  font-size: var(--text-3xl);
  font-variant-numeric: tabular-nums;
}

.health-ring-inner span,
.donut-inner span,
.health-ring-inner small {
  color: var(--text-tertiary);
  font-size: var(--text-xs);
}

.health-bars {
  display: flex;
  flex-direction: column;
  gap: var(--space-4);
}

.health-item {
  display: grid;
  gap: var(--space-2);
}

.bar-label {
  display: flex;
  align-items: center;
  justify-content: space-between;
  color: var(--text-secondary);
  font-size: var(--text-sm);
}

.bar-label strong {
  color: var(--text-primary);
  font-weight: var(--font-semibold);
}

.track,
.category-track,
.rank-track,
.size-track {
  height: 12px;
  overflow: hidden;
  border-radius: var(--radius-full);
  background: rgba(160, 160, 200, 0.16);
}

.fill,
.category-fill,
.rank-fill,
.size-fill {
  height: 100%;
  border-radius: var(--radius-full);
  min-width: 0;
}

.fill.warning {
  background: #f5a623;
}

.fill.info {
  background: #43c6e8;
}

.fill.danger {
  background: #d9345f;
}

.category-grid {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: var(--space-2);
}

.category-row {
  display: grid;
  grid-template-columns: 150px minmax(0, 1fr) 52px;
  align-items: center;
  gap: var(--space-3);
  min-height: 34px;
  padding: 0 var(--space-3);
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-sm);
}

.category-name {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  min-width: 0;
  color: var(--text-secondary);
  font-size: var(--text-sm);
}

.category-name strong {
  color: var(--text-primary);
  font-variant-numeric: tabular-nums;
}

.type-dot {
  width: 9px;
  height: 9px;
  border-radius: 50%;
  flex-shrink: 0;
}

.category-percent,
.rank-percent {
  color: var(--text-tertiary);
  font-size: var(--text-xs);
  text-align: right;
  font-variant-numeric: tabular-nums;
}

.rank-list,
.size-list {
  display: flex;
  flex-direction: column;
  gap: var(--space-3);
}

.rank-row {
  display: grid;
  grid-template-columns: 36px 180px minmax(0, 1fr) 64px;
  align-items: center;
  gap: var(--space-3);
}

.rank-index {
  color: #f5a623;
  font-size: var(--text-sm);
  text-align: right;
  font-variant-numeric: tabular-nums;
}

.rank-name {
  min-width: 0;
  overflow: hidden;
  color: var(--text-secondary);
  font-size: var(--text-sm);
  text-overflow: ellipsis;
  white-space: nowrap;
}

.rank-track {
  height: 14px;
}

.rank-fill {
  display: flex;
  align-items: center;
  justify-content: flex-end;
  min-width: 20px;
  padding-right: var(--space-2);
  background: linear-gradient(90deg, rgba(67, 198, 232, 0.25), #43c6e8);
  color: white;
  font-size: 10px;
  font-variant-numeric: tabular-nums;
}

.rank-fill.top {
  background: linear-gradient(90deg, #b57d15, #ffcf26);
}

.size-row {
  display: grid;
  grid-template-columns: 90px minmax(0, 1fr) 48px;
  align-items: center;
  gap: var(--space-3);
}

.size-label,
.size-count {
  color: var(--text-secondary);
  font-size: var(--text-sm);
  font-variant-numeric: tabular-nums;
}

.size-count {
  text-align: right;
}

.size-fill {
  display: flex;
  align-items: center;
  justify-content: flex-end;
  min-width: 18px;
  padding-right: var(--space-2);
  background: #2785ee;
  color: white;
  font-size: 10px;
  font-variant-numeric: tabular-nums;
}

.loading-panel,
.empty-panel {
  display: grid;
  place-items: center;
  min-height: 260px;
  padding: var(--space-6);
  color: var(--text-secondary);
  text-align: center;
}

.empty-panel h2 {
  margin: 0 0 var(--space-2);
  color: var(--text-primary);
  font-size: var(--text-xl);
}

.empty-panel p {
  margin: 0;
  color: var(--text-tertiary);
  font-size: var(--text-sm);
}

@media (max-width: 1100px) {
  .summary-grid,
  .health-layout,
  .category-layout {
    grid-template-columns: 1fr;
  }

  .category-grid {
    grid-template-columns: 1fr;
  }

  .rank-row {
    grid-template-columns: 28px 120px minmax(0, 1fr) 52px;
  }
}

@media (max-width: 760px) {
  .statistics-header {
    flex-direction: column;
  }

  .summary-grid {
    grid-template-columns: 1fr;
  }

  .directory-head,
  .directory-row {
    grid-template-columns: minmax(0, 1fr) 68px 88px 58px;
  }
}
</style>
