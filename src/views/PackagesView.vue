<template>
  <div class="packages-view animate-fadeIn">
    <div class="packages-browser">
      <div class="folder-shell" :class="{ collapsed: !isFolderPanelOpen }">
        <Transition name="folder-panel">
          <aside v-if="isFolderPanelOpen" class="folder-tree glass-panel">
            <div class="folder-tree-title">
              <span class="text-xs text-tertiary">{{ t('packages.folderTree') }}</span>
              <button
                class="folder-panel-toggle"
                :title="t('packages.collapseFolderPanel')"
                :aria-label="t('packages.collapseFolderPanel')"
                @click="isFolderPanelOpen = false"
              >
                <svg width="14" height="14" viewBox="0 0 24 24" fill="none">
                  <path d="M15 18l-6-6 6-6" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round" />
                </svg>
              </button>
            </div>
            <div
              v-for="folder in flatFolderNodes"
              :key="folder.path || '__root__'"
              :class="['folder-tree-row', { active: selectedFolder === folder.path }]"
              :style="{ paddingLeft: `${8 + folder.depth * 14}px` }"
              :title="folder.fullLabel"
            >
              <button
                class="folder-expand-btn"
                :class="{ hidden: !folder.hasChildren }"
                :title="folder.isExpanded ? t('packages.collapseFolder') : t('packages.expandFolder')"
                :aria-label="folder.isExpanded ? t('packages.collapseFolder') : t('packages.expandFolder')"
                @click="toggleFolder(folder.path)"
              >
                <svg :class="['folder-caret', { expanded: folder.isExpanded }]" width="12" height="12" viewBox="0 0 24 24" fill="none">
                  <path d="M9 18l6-6-6-6" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" />
                </svg>
              </button>
              <button class="folder-tree-item" @click="selectedFolder = folder.path">
                <svg width="14" height="14" viewBox="0 0 24 24" fill="none">
                  <path d="M3 6.5A2.5 2.5 0 0 1 5.5 4H9l2 2h7.5A2.5 2.5 0 0 1 21 8.5v9A2.5 2.5 0 0 1 18.5 20h-13A2.5 2.5 0 0 1 3 17.5v-11Z" stroke="currentColor" stroke-width="1.5" stroke-linejoin="round" />
                </svg>
                <span class="folder-name truncate">{{ folder.label }}</span>
                <span class="folder-count">{{ folder.count }}</span>
              </button>
            </div>
          </aside>
        </Transition>
        <button
          v-if="!isFolderPanelOpen"
          class="folder-panel-open glass-panel"
          :title="t('packages.expandFolderPanel')"
          :aria-label="t('packages.expandFolderPanel')"
          @click="isFolderPanelOpen = true"
        >
          <svg width="14" height="14" viewBox="0 0 24 24" fill="none">
            <path d="M9 18l6-6-6-6" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round" />
          </svg>
        </button>
      </div>

      <!-- Local Packages View -->
      <ResourceDisplay
        :packages="sortedPackages"
        :view-mode="viewMode"
        :result-count-label="t('packages.packagesCount', { count: filteredPackages.length })"
        :empty-title="t('packages.noPackages')"
        :empty-description="t('packages.noPackagesDesc')"
        :show-quick-delete="true"
        @update:view-mode="viewMode = $event"
        @package-deleted="handlePackageDeleted"
      >
        <template #actions>
          <SearchInput
            v-model="packageNameQuery"
            class="package-name-search"
            :placeholder="t('packages.searchNamePlaceholder')"
          />
          <button
            class="scan-btn-inline"
            :disabled="isScanning || !vamRootPath"
            @click="handleScan"
          >
            <span>{{ isScanning ? t('toolbar.scanning') : t('packages.scan') }}</span>
          </button>
          <div class="filter-dropdown">
            <button class="filter-btn" :class="{ active: selectedType !== 'all' }" @click="showFilterMenu = !showFilterMenu">
              <svg width="14" height="14" viewBox="0 0 24 24" fill="none">
                <path d="M22 3H2l8 9.46V19l4 2v-8.54L22 3Z" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" />
              </svg>
              <span>{{ activeTypeLabel }}</span>
            </button>
            <div v-if="showFilterMenu" class="filter-menu glass-panel">
              <button
                v-for="item in typeFilters"
                :key="item.value"
                :class="['filter-menu-item', { active: selectedType === item.value }]"
                @click="selectType(item.value)"
              >
                {{ item.label }}
              </button>
            </div>
          </div>
          <div class="sort-dropdown">
            <select v-model="sortBy" class="sort-select" @change="sortPackages">
              <option value="name">{{ t('packages.sortByName') }}</option>
              <option value="size">{{ t('packages.sortBySize') }}</option>
              <option value="creator">{{ t('packages.sortByCreator') }}</option>
              <option value="created">{{ t('packages.sortByCreatedTime') }}</option>
              <option value="imported">{{ t('packages.sortByImportTime') }}</option>
            </select>
          </div>
        </template>
        <template #empty-action>
          <button class="action-btn" @click="goToSettings">
            <svg width="16" height="16" viewBox="0 0 24 24" fill="none">
              <path d="M4 20h16M4 20V4h16v16M9 10h6M12 7v6" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" />
            </svg>
            {{ t('packages.selectDirectory') }}
          </button>
        </template>
      </ResourceDisplay>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, onMounted, ref, watch } from 'vue'
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
const showFilterMenu = ref(false)
const packageNameQuery = ref('')
const isFolderPanelOpen = ref(true)
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

const activeTypeLabel = computed(() => {
  const item = typeFilters.value.find(item => item.value === selectedType.value)
  return item?.label || t('packages.filter')
})

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
  showFilterMenu.value = false
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

function sortPackages() {
  // reactivity handled by computed
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
</script>

<style scoped>
.packages-view {
  height: 100%;
  position: relative;
  overflow: hidden;
}

.packages-browser {
  position: relative;
  display: grid;
  grid-template-columns: auto minmax(0, 1fr);
  gap: var(--space-4);
  height: 100%;
  min-height: 0;
}

.folder-shell {
  position: relative;
  width: 240px;
  min-height: 0;
  overflow: hidden;
  transition: width 220ms var(--ease);
}

.folder-shell.collapsed {
  width: 20px;
}

.folder-tree {
  position: absolute;
  inset: 0;
  width: 240px;
  min-height: 0;
  overflow-y: auto;
  padding: var(--space-3);
  box-sizing: border-box;
  transition:
    opacity 220ms var(--ease),
    transform 220ms var(--ease);
  will-change: opacity, transform;
}

.folder-tree-title {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--space-2);
  padding: 0 var(--space-1) var(--space-2) var(--space-2);
  font-weight: var(--font-semibold);
  text-transform: uppercase;
  letter-spacing: 0.04em;
}

.folder-panel-toggle,
.folder-panel-open,
.folder-expand-btn {
  display: flex;
  align-items: center;
  justify-content: center;
  flex-shrink: 0;
  color: var(--text-tertiary);
  transition: color var(--duration-fast) var(--ease), background var(--duration-fast) var(--ease);
}

.folder-panel-toggle,
.folder-expand-btn {
  width: 24px;
  height: 24px;
  border-radius: var(--radius-xs);
}

.folder-panel-toggle:hover,
.folder-expand-btn:hover {
  color: var(--text-primary);
  background: var(--bg-hover);
}

.folder-panel-open {
  position: absolute;
  top: 50%;
  right: 2px;
  width: 16px;
  height: 86px;
  min-height: 0;
  border-radius: 999px;
  transform: translateY(-50%);
  z-index: 2;
  overflow: hidden;
  background: rgba(30, 30, 55, 0.72);
  backdrop-filter: blur(24px);
}

.folder-panel-open:hover {
  color: var(--text-primary);
}

.folder-panel-open svg {
  margin-left: 4px;
}

.folder-shell.collapsed .folder-panel-open {
  animation: folder-handle-in 220ms var(--ease);
}

.folder-panel-enter-active,
.folder-panel-leave-active {
  transition:
    opacity 220ms var(--ease),
    transform 220ms var(--ease);
}

.folder-panel-enter-from,
.folder-panel-leave-to {
  opacity: 0;
  transform: translateX(-10px);
}

.folder-panel-enter-to,
.folder-panel-leave-from {
  opacity: 1;
  transform: translateX(0);
}

.folder-tree-row {
  display: flex;
  align-items: center;
  min-height: 30px;
  border-radius: var(--radius-sm);
  color: var(--text-secondary);
  transition: background var(--duration-fast) var(--ease), color var(--duration-fast) var(--ease);
}

.folder-tree-row:hover {
  color: var(--text-primary);
  background: var(--bg-hover);
}

.folder-tree-row.active {
  color: var(--accent-primary);
  background: rgba(124, 92, 252, 0.14);
}

.folder-caret {
  transition: transform var(--duration-fast) var(--ease);
}

.folder-caret.expanded {
  transform: rotate(90deg);
}

.folder-expand-btn.hidden {
  visibility: hidden;
}

.folder-tree-item {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  width: 100%;
  min-height: 30px;
  min-width: 0;
  padding-right: var(--space-2);
  border-radius: var(--radius-sm);
  color: inherit;
  font-size: var(--text-sm);
  text-align: left;
}

.folder-name {
  flex: 1;
}

.folder-count {
  flex-shrink: 0;
  color: var(--text-tertiary);
  font-size: var(--text-xs);
  font-variant-numeric: tabular-nums;
}

@media (max-width: 900px) {
  .packages-browser {
    grid-template-columns: 1fr;
  }

  .folder-shell {
    width: 100%;
  }

  .folder-shell.collapsed {
    width: 20px;
    height: 64px;
  }

  .folder-panel-open {
    right: 0;
  }

  .folder-tree {
    width: 100%;
    max-height: 180px;
  }
}

@keyframes folder-handle-in {
  from {
    opacity: 0;
    transform: translateY(-50%) translateX(-6px);
  }
  to {
    opacity: 1;
    transform: translateY(-50%) translateX(0);
  }
}

/* ── Action buttons (passed into ResourceDisplay slot) ───── */
.package-name-search {
  width: 220px;
}

.scan-btn-inline {
  display: inline-flex;
  align-items: center;
  gap: var(--space-1);
  padding: var(--space-1) var(--space-3);
  height: 30px;
  border-radius: var(--radius-sm);
  color: var(--accent-primary);
  font-size: var(--text-sm);
  font-weight: var(--font-medium);
  transition: color var(--duration-fast) var(--ease), background var(--duration-fast) var(--ease);
}

.scan-btn-inline:hover:not(:disabled) {
  background: rgba(124, 92, 252, 0.1);
}

.scan-btn-inline:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}

.filter-btn {
  display: inline-flex;
  align-items: center;
  gap: var(--space-2);
  padding: var(--space-1) var(--space-3);
  height: 30px;
  border-radius: var(--radius-sm);
  color: var(--text-secondary);
  font-size: var(--text-sm);
  font-weight: var(--font-medium);
  transition: color var(--duration-fast) var(--ease), background var(--duration-fast) var(--ease);
}

.filter-btn:hover {
  color: var(--text-primary);
  background: var(--bg-hover);
}

.filter-btn.active {
  color: var(--text-primary);
  background: rgba(124, 92, 252, 0.15);
}

.filter-dropdown {
  position: relative;
}

.filter-menu {
  position: absolute;
  top: calc(100% + var(--space-2));
  right: 0;
  z-index: var(--z-dropdown);
  display: grid;
  grid-template-columns: repeat(2, minmax(72px, 1fr));
  gap: var(--space-1);
  width: 180px;
  padding: var(--space-2);
}

.filter-menu-item {
  height: 28px;
  padding: 0 var(--space-2);
  border-radius: var(--radius-sm);
  color: var(--text-secondary);
  font-size: var(--text-xs);
  text-align: left;
  transition: color var(--duration-fast) var(--ease), background var(--duration-fast) var(--ease);
}

.filter-menu-item:hover,
.filter-menu-item.active {
  color: var(--text-primary);
  background: var(--bg-hover);
}

.sort-select {
  height: 30px;
  padding: 0 var(--space-3);
  background: var(--bg-base);
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-sm);
  color: var(--text-secondary);
  font-size: var(--text-sm);
  cursor: pointer;
  transition: border-color var(--duration-fast) var(--ease);
}

.sort-select:focus {
  border-color: var(--accent-primary);
}

.action-btn {
  display: inline-flex;
  align-items: center;
  gap: var(--space-2);
  padding: var(--space-2) var(--space-5);
  height: 36px;
  background: var(--accent-gradient);
  color: white;
  font-size: var(--text-sm);
  font-weight: var(--font-semibold);
  border-radius: var(--radius-md);
  transition: opacity var(--duration-fast) var(--ease), transform var(--duration-fast) var(--ease);
}

.action-btn:hover {
  opacity: 0.9;
  transform: translateY(-1px);
}

.action-btn:active {
  transform: translateY(0);
}
</style>
