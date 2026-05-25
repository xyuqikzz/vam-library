import { computed, ref } from 'vue'
import { defineStore } from 'pinia'
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { useAppStore } from './app'
import type { CorruptedPackage, DashboardStats, PackageDisplayItem, PackageFolderEntry } from '@/types/package'
import type { DependencyGraphData } from '@/types/dependency'

export type LocalLibraryState = 'idle' | 'loading' | 'ready' | 'scanning' | 'indexing' | 'refreshing' | 'stale' | 'error'

export interface MissingDependency {
  package_id: string
  depends_on_id: string
  required_version: string
  dependent_package: string
  status: 'missing' | 'lower_version' | string
  installed_version: number | null
}

export interface DependencyAnalysis {
  total_packages: number
  total_dependencies: number
  missing_count: number
  orphaned_count: number
}

export interface LibraryIndexChangedPayload {
  revision: number
  reason: string
  changed_package_ids: string[]
  changed_paths: string[]
  invalidates: string[]
}

function emptyDashboardStats(): DashboardStats {
  return {
    total_packages: 0,
    total_size_bytes: 0,
    scene_count: 0,
    appearance_count: 0,
    morph_count: 0,
    plugin_count: 0,
    missing_dependencies: 0,
    duplicate_resources: 0,
    orphaned_packages: 0,
    corrupted_packages: 0,
  }
}

function emptyDependencyGraph(): DependencyGraphData {
  return { nodes: [], edges: [] }
}

function packageTime(value?: string): number {
  const time = value ? new Date(value).getTime() : 0
  return Number.isFinite(time) ? time : 0
}

export const useLocalLibraryStore = defineStore('localLibrary', () => {
  const state = ref<LocalLibraryState>('idle')
  const packages = ref<PackageDisplayItem[]>([])
  const dashboardStats = ref<DashboardStats>(emptyDashboardStats())
  const dependencyGraph = ref<DependencyGraphData>(emptyDependencyGraph())
  const missingDependencies = ref<MissingDependency[]>([])
  const corruptedPackages = ref<CorruptedPackage[]>([])
  const packageFolders = ref<string[]>([])
  const allTags = ref<string[]>([])
  const revision = ref(0)
  const error = ref<string | null>(null)

  let unlistenIndexChanged: (() => void) | null = null
  let refreshTimer: ReturnType<typeof setTimeout> | null = null
  let refreshToken = 0
  let pendingChange: LibraryIndexChangedPayload | null = null

  const loading = computed(() => ['loading', 'scanning', 'indexing', 'refreshing'].includes(state.value))
  const ready = computed(() => state.value === 'ready')
  const recentPackages = computed(() => (
    [...packages.value]
      .sort((a, b) => packageTime(b.scan_time) - packageTime(a.scan_time))
      .slice(0, 5)
  ))

  async function startListeners() {
    if (unlistenIndexChanged) return
    unlistenIndexChanged = await listen<LibraryIndexChangedPayload>('library-index-changed', (event) => {
      const payload = event.payload
      revision.value = payload.revision
      scheduleRefresh(payload)
    })
  }

  function stopListeners() {
    if (unlistenIndexChanged) {
      unlistenIndexChanged()
      unlistenIndexChanged = null
    }
    if (refreshTimer) {
      clearTimeout(refreshTimer)
      refreshTimer = null
    }
  }

  function scheduleRefresh(payload: LibraryIndexChangedPayload) {
    pendingChange = mergePendingChange(pendingChange, payload)
    state.value = payload.reason === 'scan_completed'
      ? 'scanning'
      : payload.reason === 'package_indexed'
        ? 'indexing'
        : 'stale'

    if (refreshTimer) clearTimeout(refreshTimer)
    refreshTimer = setTimeout(() => {
      const change = pendingChange
      pendingChange = null
      refreshTimer = null
      void refreshForChange(change)
    }, 200)
  }

  function mergePendingChange(
    current: LibraryIndexChangedPayload | null,
    next: LibraryIndexChangedPayload,
  ): LibraryIndexChangedPayload {
    if (!current) return next
    return {
      revision: Math.max(current.revision, next.revision),
      reason: current.reason === next.reason ? next.reason : 'multiple_changes',
      changed_package_ids: [...new Set([...current.changed_package_ids, ...next.changed_package_ids])],
      changed_paths: [...new Set([...current.changed_paths, ...next.changed_paths])],
      invalidates: [...new Set([...current.invalidates, ...next.invalidates])],
    }
  }

  async function ensureLoaded() {
    if (state.value === 'idle' || state.value === 'error') {
      await refreshAll('loading')
    }
  }

  async function refreshForChange(change: LibraryIndexChangedPayload | null) {
    if (!change) return
    if (packages.value.length === 0 || change.reason === 'scan_completed' || change.reason === 'multiple_changes') {
      await refreshAll(change.reason === 'scan_completed' ? 'scanning' : 'refreshing')
      return
    }

    if (change.reason === 'package_indexed' && change.changed_package_ids.length > 0) {
      await refreshIndexedPackages(change.changed_package_ids)
      return
    }

    if (change.reason === 'tags_changed' && change.changed_package_ids.length > 0) {
      await refreshChangedPackages(change.changed_package_ids)
      await refreshTags()
      markReady()
      return
    }

    await refreshAll('refreshing')
  }

  async function refreshAll(nextState: LocalLibraryState = 'refreshing') {
    const token = ++refreshToken
    const appStore = useAppStore()
    state.value = nextState
    error.value = null

    try {
      const [
        packageList,
        stats,
        graph,
        missing,
        corrupted,
        tags,
        folders,
      ] = await Promise.all([
        invoke<PackageDisplayItem[]>('list_packages'),
        invoke<DashboardStats>('get_dashboard_stats'),
        invoke<DependencyGraphData>('get_dependency_graph').catch(() => emptyDependencyGraph()),
        invoke<MissingDependency[]>('find_missing_dependencies').catch(() => []),
        invoke<CorruptedPackage[]>('find_corrupted_packages').catch(() => []),
        invoke<string[]>('list_all_tags').catch(() => []),
        loadPackageFolders(appStore.vamRootPath),
      ])

      if (token !== refreshToken) return
      packages.value = packageList || []
      dashboardStats.value = stats || emptyDashboardStats()
      dependencyGraph.value = graph || emptyDependencyGraph()
      missingDependencies.value = missing || []
      corruptedPackages.value = corrupted || []
      allTags.value = tags || []
      packageFolders.value = folders
      markReady()
    } catch (err) {
      if (token !== refreshToken) return
      packages.value = []
      dashboardStats.value = emptyDashboardStats()
      dependencyGraph.value = emptyDependencyGraph()
      missingDependencies.value = []
      allTags.value = []
      packageFolders.value = []
      error.value = String(err)
      state.value = 'error'
    }
  }

  async function refreshIndexedPackages(packageIds: string[]) {
    state.value = 'indexing'
    error.value = null
    try {
      await refreshChangedPackages(packageIds)
      await Promise.all([
        refreshDashboard(),
        refreshDependencies(),
        refreshTags(),
        refreshFolders(),
      ])
      markReady()
    } catch (err) {
      error.value = String(err)
      await refreshAll('refreshing')
    }
  }

  async function refreshChangedPackages(packageIds: string[]) {
    const uniqueIds = [...new Set(packageIds)]
    const summaries = await Promise.all(uniqueIds.map(packageId => (
      invoke<PackageDisplayItem | null>('get_package_summary', { packageId }).catch(() => null)
    )))

    const nextPackages = [...packages.value]
    for (let index = 0; index < uniqueIds.length; index += 1) {
      const packageId = uniqueIds[index]
      const summary = summaries[index]
      const existingIndex = nextPackages.findIndex(pkg => pkg.id === packageId)
      if (summary) {
        if (existingIndex >= 0) {
          nextPackages[existingIndex] = summary
        } else {
          nextPackages.push(summary)
        }
      } else if (existingIndex >= 0) {
        nextPackages.splice(existingIndex, 1)
      }
    }
    packages.value = nextPackages
  }

  async function refreshDashboard() {
    dashboardStats.value = await invoke<DashboardStats>('get_dashboard_stats')
    missingDependencies.value = await invoke<MissingDependency[]>('find_missing_dependencies').catch(() => [])
    corruptedPackages.value = await invoke<CorruptedPackage[]>('find_corrupted_packages').catch(() => [])
  }

  async function refreshDependencies() {
    const graph = await invoke<DependencyGraphData>('get_dependency_graph').catch(() => emptyDependencyGraph())
    dependencyGraph.value = graph
  }

  async function refreshTags() {
    allTags.value = await invoke<string[]>('list_all_tags').catch(() => [])
  }

  async function refreshFolders() {
    const appStore = useAppStore()
    packageFolders.value = await loadPackageFolders(appStore.vamRootPath)
  }

  async function loadPackageFolders(vamRootPath: string | null): Promise<string[]> {
    if (!vamRootPath) return []
    const folders = await invoke<PackageFolderEntry[]>('list_package_folders', {
      vamRoot: vamRootPath,
    }).catch(() => [])
    return folders.map(folder => folder.path).filter(Boolean)
  }

  function markReady() {
    state.value = 'ready'
    error.value = null
  }

  function updatePackageTags(packageId: string, tags: string[]) {
    packages.value = packages.value.map(pkg => (
      pkg.id === packageId ? { ...pkg, tags } : pkg
    ))
  }

  function isPackageInstalled(creator: string, name: string, version = 1): boolean {
    const creatorLower = creator.toLowerCase()
    const nameLower = name.toLowerCase()
    return packages.value.some(pkg => (
      pkg.creator.toLowerCase() === creatorLower &&
      pkg.name.toLowerCase() === nameLower &&
      pkg.version >= version
    ))
  }

  return {
    state,
    loading,
    ready,
    packages,
    dashboardStats,
    dependencyGraph,
    missingDependencies,
    corruptedPackages,
    packageFolders,
    allTags,
    recentPackages,
    revision,
    error,
    startListeners,
    stopListeners,
    ensureLoaded,
    refreshAll,
    updatePackageTags,
    isPackageInstalled,
  }
})
