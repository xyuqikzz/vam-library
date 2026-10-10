<script setup lang="ts">
import PageHeader from '@/components/layout/PageHeader.vue'
import UiButton from '@/components/ui/UiButton.vue'
import UiChip from '@/components/ui/UiChip.vue'
import UiSegmented from '@/components/ui/UiSegmented.vue'
import UiMenu, { type MenuItem } from '@/components/ui/UiMenu.vue'
import Icon from '@/components/ui/Icon.vue'
import { formatSize } from '@/utils/bytes'
import { resourceColor } from '@/utils/resourcePresentation'

import { computed, onMounted, onUnmounted, onActivated, onDeactivated, ref, watch } from 'vue'
import { useRouter } from 'vue-router'
import { useI18n } from 'vue-i18n'
import { storeToRefs } from 'pinia'
import { useAppStore } from '@/stores/app'
import { useLocalLibraryStore } from '@/stores/localLibrary'
import SearchInput from '@/components/common/SearchInput.vue'
import ResourceDisplay from '@/components/ResourceDisplay.vue'
import type { PackageDisplayItem } from '@/types/package'

const { t } = useI18n()
const router = useRouter()
const appStore = useAppStore()
const localLibraryStore = useLocalLibraryStore()
const { vamRootPath, isScanning, searchQuery } = storeToRefs(appStore)
const { packages, packageFolders: folderPaths } = storeToRefs(localLibraryStore)

const viewMode = ref<'large-card' | 'small-card' | 'list'>('large-card')
const sortBy = ref('imported')
const selectedType = ref('all')
const selectedFolder = ref('')
const sortMenuOpen = ref(false)
const packageNameQuery = ref('')
const isFolderPanelOpen = ref(false)
const expandedFolders = ref(new Set<string>())

interface FolderNode {
  label: string
  path: string
  count: number
  children: Map<string, FolderNode>
}

interface FlatFolderNode {
  label: string
  fullLabel: string
  path: string
  count: number
  depth: number
  hasChildren: boolean
  isExpanded: boolean
}

const typeFilters = computed(() => [
  { value: 'all', label: t('packages.filterAll') },
  { value: 'scene', label: t('resourceType.scene') },
  { value: 'appearance', label: t('packages.filterCharacter') },
  { value: 'clothing', label: t('resourceType.clothing') },
  { value: 'hair', label: t('resourceType.hair') },
  { value: 'morph', label: t('resourceType.morph') },
  { value: 'plugin', label: t('resourceType.plugin') },
  { value: 'texture', label: t('resourceType.texture') },
  { value: 'asset', label: t('resourceType.asset') },
  { value: 'sound', label: t('resourceType.sound') },
  { value: 'other', label: t('packages.filterOther') },
])


const filteredPackages = computed(() => {
  let list = packages.value
  if (selectedFolder.value) {
    list = list.filter(p => isPackageInSelectedFolder(p))
  }
  if (selectedType.value !== 'all') {
    list = list.filter(p => p.resource_types.includes(selectedType.value))
  }
  const nameQuery = packageNameQuery.value.toLowerCase().trim()
  if (nameQuery) {
    list = list.filter(p =>
      p.name.toLowerCase().includes(nameQuery) ||
      p.id.toLowerCase().includes(nameQuery) ||
      `${p.id.toLowerCase()}.var`.includes(nameQuery) ||
      p.creator.toLowerCase().includes(nameQuery)
    )
  }
  const query = searchQuery.value.toLowerCase().trim()
  if (query) {
    list = list.filter(p =>
      p.creator.toLowerCase().includes(query) ||
      p.name.toLowerCase().includes(query) ||
      p.id.toLowerCase().includes(query) ||
      `${p.id.toLowerCase()}.var`.includes(query)
    )
  }
  return list
})

const addonPackagesPath = computed(() => (
  vamRootPath.value ? normalizeSlashes(`${vamRootPath.value}/AddonPackages`) : ''
))

const libraryPackagesPath = computed(() => (
  vamRootPath.value ? normalizeSlashes(`${vamRootPath.value}/VAMBoxLibrary/AddonPackages`) : ''
))

const packageRootPaths = computed(() => (
  [addonPackagesPath.value, libraryPackagesPath.value].filter(Boolean)
))

const flatFolderNodes = computed(() => {
  const root: FolderNode = {
    label: t('packages.addonPackages'),
    path: '',
    count: packages.value.length,
    children: new Map(),
  }

  for (const folderPath of folderPaths.value) {
    ensureFolderPath(root, folderPath)
  }

  for (const pkg of packages.value) {
    const dir = packageRelativeDir(pkg)
    if (!dir) continue

    const nodes = ensureFolderPath(root, dir)
    for (const node of nodes) node.count += 1
  }

  const nodes: FlatFolderNode[] = []
  flattenFolder(root, 0, nodes)
  return nodes
})

const sortedPackages = computed(() => {
  const sorted = [...filteredPackages.value]
  switch (sortBy.value) {
    case 'size':
      sorted.sort((a, b) => b.size_bytes - a.size_bytes || comparePackageName(a, b))
      break
    case 'creator':
      sorted.sort(comparePackageName)
      break
    case 'created':
      sorted.sort((a, b) => timeValue(b.created_time) - timeValue(a.created_time) || comparePackageName(a, b))
      break
    case 'imported':
      sorted.sort((a, b) => timeValue(b.scan_time) - timeValue(a.scan_time) || comparePackageName(a, b))
      break
    case 'name':
    default:
      sorted.sort(comparePackageName)
      break
  }
  return sorted
})

onMounted(async () => {
  await localLibraryStore.ensureLoaded()
})

watch(vamRootPath, async () => {
  await localLibraryStore.refreshAll('loading')
})

async function handleScan() {
  if (!vamRootPath.value || isScanning.value) return
  try {
    await appStore.startScan(vamRootPath.value)
  } catch {
    // Handle silently
  }
}

function goToSettings() {
  router.push('/settings')
}

function selectType(type: string) {
  selectedType.value = type

}

function toggleFolder(path: string) {
  const next = new Set(expandedFolders.value)
  if (next.has(path)) {
    next.delete(path)
  } else {
    next.add(path)
  }
  expandedFolders.value = next
}

function normalizeSlashes(path: string): string {
  return path.replace(/\\/g, '/').replace(/\/+$/, '')
}

function comparePath(path: string): string {
  return normalizeSlashes(path).toLowerCase()
}

function packageRelativeDir(pkg: PackageDisplayItem): string {
  if (packageRootPaths.value.length === 0) return ''
  const filePath = normalizeSlashes(pkg.file_path)
  const comparableFilePath = comparePath(filePath)

  for (const rootPath of packageRootPaths.value) {
    const comparableRootPath = comparePath(rootPath)
    if (comparableFilePath === comparableRootPath || comparableFilePath.startsWith(`${comparableRootPath}/`)) {
      const relativeFile = filePath.slice(rootPath.length).replace(/^\/+/, '')
      const lastSlash = relativeFile.lastIndexOf('/')
      return lastSlash >= 0 ? relativeFile.slice(0, lastSlash) : ''
    }
  }

  return ''
}

// Check if package is inside selected folder or its subfolders
function isPackageInSelectedFolder(pkg: PackageDisplayItem): boolean {
  const dir = packageRelativeDir(pkg)
  return dir === selectedFolder.value || dir.startsWith(`${selectedFolder.value}/`)
}

function flattenFolder(node: FolderNode, depth: number, output: FlatFolderNode[]) {
  const isExpanded = expandedFolders.value.has(node.path)
  const hasChildren = node.children.size > 0

  output.push({
    label: node.label,
    fullLabel: node.path || node.label,
    path: node.path,
    count: node.count,
    depth,
    hasChildren,
    isExpanded,
  })

  if (!isExpanded) return

  const children = [...node.children.values()].sort((a, b) => a.label.localeCompare(b.label))
  for (const child of children) {
    flattenFolder(child, depth + 1, output)
  }
}

function ensureFolderPath(root: FolderNode, folderPath: string): FolderNode[] {
  let current = root
  let currentPath = ''
  const nodes: FolderNode[] = []

  for (const segment of folderPath.split('/').filter(Boolean)) {
    currentPath = currentPath ? `${currentPath}/${segment}` : segment
    if (!current.children.has(segment)) {
      current.children.set(segment, {
        label: segment,
        path: currentPath,
        count: 0,
        children: new Map(),
      })
    }
    current = current.children.get(segment)!
    nodes.push(current)
  }

  return nodes
}


async function handlePackageDeleted() {
  await localLibraryStore.refreshAll('refreshing')
}

function timeValue(value?: string): number {
  const time = value ? new Date(value).getTime() : 0
  return Number.isFinite(time) ? time : 0
}

function comparePackageName(a: PackageDisplayItem, b: PackageDisplayItem): number {
  return a.creator.localeCompare(b.creator)
    || a.name.localeCompare(b.name)
    || b.version - a.version
}

const searchInput = ref<InstanceType<typeof SearchInput> | null>(null)
const folderAnchor = ref<HTMLElement | null>(null)
const sortAnchor = ref<HTMLElement | null>(null)
const issuesOnly = ref(false)
const missingCounts = computed(() => { const result: Record<string, number> = {}; for (const item of localLibraryStore.missingDependencies) result[item.package_id] = (result[item.package_id] || 0) + 1; return result })
const problemIds = computed(() => new Set([...Object.keys(missingCounts.value), ...localLibraryStore.corruptedPackages.map(item => item.package_id)]))
const displayPackages = computed(() => issuesOnly.value ? sortedPackages.value.filter(item => problemIds.value.has(item.id)) : sortedPackages.value)
const latestScanTime = computed(() => packages.value.reduce((latest, item) => Math.max(latest, Date.parse(item.scan_time) || 0), 0))
const packageSubtitle = computed(() => [t('ui.total', { count: packages.value.length }), formatSize(packages.value.reduce((sum, item) => sum + item.size_bytes, 0)), latestScanTime.value ? t('ui.synced', { minutes: Math.max(0, Math.floor((Date.now() - latestScanTime.value) / 60000)) }) : t('ui.notSynced')])
const viewOptions = computed(() => [{ value: 'large-card', label: t('packages.largeCardView'), icon: 'grid' as const }, { value: 'small-card', label: t('packages.smallCardView'), icon: 'landscape' as const }, { value: 'list', label: t('packages.listView'), icon: 'list' as const }])
const sortItems = computed<MenuItem[]>(() => [['name', 'packages.sortByName'], ['size', 'packages.sortBySize'], ['creator', 'packages.sortByCreator'], ['created', 'packages.sortByCreatedTime'], ['imported', 'packages.sortByImportTime']].map(([value, label]) => ({ value, label: t(label), checked: sortBy.value === value })))
function typeCount(type: string) { return type === 'all' ? packages.value.length : packages.value.filter(pkg => pkg.resource_types.includes(type)).length }
function openFolders(event: MouseEvent) { folderAnchor.value = event.currentTarget as HTMLElement; isFolderPanelOpen.value = !isFolderPanelOpen.value }
function openSort(event: MouseEvent) { sortAnchor.value = event.currentTarget as HTMLElement; sortMenuOpen.value = !sortMenuOpen.value }
function setView(value: string) { if (value === 'large-card' || value === 'small-card' || value === 'list') viewMode.value = value }
function focusSearch(event: KeyboardEvent) { if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === 'k') { event.preventDefault(); searchInput.value?.focus() } }
onMounted(() => window.addEventListener('keydown', focusSearch))
onActivated(() => window.addEventListener('keydown', focusSearch))
onDeactivated(() => window.removeEventListener('keydown', focusSearch))
onUnmounted(() => window.removeEventListener('keydown', focusSearch))
</script>
<template><div class="packages-view"><PageHeader :title="t('sidebar.packages')" :subtitle="packageSubtitle"><template #actions><SearchInput shortcut="Ctrl K" ref="searchInput" v-model="packageNameQuery" class="package-name-search" :placeholder="t('packages.searchNamePlaceholder')" /><UiButton icon="sort" :aria-expanded="sortMenuOpen" aria-haspopup="menu" @click="openSort">{{ t('ui.sort') }}</UiButton><UiSegmented :model-value="viewMode" :options="viewOptions" :label="t('ui.view')" icon-only @update:model-value="setView" /><UiButton variant="primary" size="lg" icon="refresh" :disabled="isScanning || !vamRootPath" :loading="isScanning" :loading-text="t('toolbar.scanning')" @click="handleScan">{{ t('packages.scan') }}</UiButton></template><template #filters><div class="type-filters"><UiChip v-for="filter in typeFilters" :key="filter.value" :selected="selectedType === filter.value" :count="typeCount(filter.value)" :color="filter.value === 'all' ? undefined : resourceColor(filter.value)" @click="selectType(filter.value)">{{ filter.label }}</UiChip></div><div class="extra-filters"><UiButton size="sm" variant="ghost" icon="folder" :aria-expanded="isFolderPanelOpen" aria-haspopup="menu" @click="openFolders">{{ t('ui.directory') }}: {{ selectedFolder || t('ui.all') }}<Icon name="chevron-down" :size="14" /></UiButton><UiChip :selected="issuesOnly" :count="problemIds.size" @click="issuesOnly = !issuesOnly">{{ t('ui.issues') }}</UiChip></div></template></PageHeader><div class="packages-content"><ResourceDisplay :packages="displayPackages" :view-mode="viewMode" :missing-counts="missingCounts" :result-count-label="t('packages.packagesCount', { count: displayPackages.length })" :empty-title="t('packages.noPackages')" :empty-description="t('packages.noPackagesDesc')" hide-controls show-quick-delete @update:view-mode="viewMode = $event" @package-deleted="handlePackageDeleted"><template #empty-action><UiButton variant="primary" size="lg" @click="goToSettings">{{ t('packages.selectDirectory') }}</UiButton></template></ResourceDisplay></div><UiMenu v-model:open="sortMenuOpen" :anchor="sortAnchor" :items="sortItems" :label="t('ui.sort')" @select="sortBy = $event.value" /><UiMenu v-model:open="isFolderPanelOpen" :anchor="folderAnchor" :items="[]" :label="t('ui.folders')"><div class="folder-tree"><div v-for="folder in flatFolderNodes" :key="folder.path || '__root__'" class="folder-tree-row" :class="{ active: selectedFolder === folder.path }" :style="{ paddingLeft: (folder.depth * 14) + 'px' }"><UiButton v-if="folder.hasChildren" variant="ghost" size="sm" :icon="folder.isExpanded ? 'chevron-down' : 'chevron-right'" icon-only :aria-label="folder.isExpanded ? t('packages.collapseFolder') : t('packages.expandFolder')" @click="toggleFolder(folder.path)" /><button type="button" role="menuitemradio" :aria-checked="selectedFolder === folder.path" class="folder-tree-item" @click="selectedFolder = folder.path; isFolderPanelOpen = false"><Icon name="folder" :size="16" /><span>{{ folder.label }}</span><small>{{ folder.count }}</small></button></div></div></UiMenu></div></template>
<style scoped>
.packages-view { display: flex; flex-direction: column; height: 100%; min-height: 0; overflow: hidden; }.packages-content { flex: 1; min-height: 0; padding: 16px 20px; }.package-name-search { width: 280px; max-width: 100%; }.type-filters,.extra-filters { display: flex; align-items: center; flex-wrap: wrap; gap: 6px; }.folder-tree { min-width: 260px; max-width: calc(100vw - 32px); padding: 4px; }.folder-tree-row { display: flex; align-items: center; border-radius: var(--radius-sm); }.folder-tree-row.active { background: var(--bg-active); }.folder-tree-item { display: flex; align-items: center; gap: 8px; flex: 1; min-width: 0; min-height: 36px; padding: 4px 8px; border-radius: var(--radius-sm); text-align: left; font-size: var(--text-base); }.folder-tree-item span { flex: 1; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }.folder-tree-item small { color: var(--text-tertiary); font-size: var(--text-xs); }@media(max-width: 800px) { .packages-content { padding: 12px; } }
</style>
