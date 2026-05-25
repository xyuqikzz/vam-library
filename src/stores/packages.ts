import { defineStore } from 'pinia'
import { ref, reactive } from 'vue'

export interface PackageInfo {
  id: string
  filename: string
  displayName: string
  creator: string
  type: string
  size: number
  path: string
  dependencies: string[]
  lastModified: string
}

export interface PackageFilters {
  search: string
  type: string | null
  creator: string | null
  sortBy: 'name' | 'size' | 'date' | 'creator' | 'created' | 'imported'
  sortOrder: 'asc' | 'desc'
}

export const usePackagesStore = defineStore('packages', () => {
  // ── State ──────────────────────────────────────────────────
  const packages = ref<PackageInfo[]>([])
  const totalCount = ref(0)
  const loading = ref(false)
  const selectedPackageId = ref<string | null>(null)
  const viewMode = ref<'grid' | 'list'>('grid')

  const filters = reactive<PackageFilters>({
    search: '',
    type: null,
    creator: null,
    sortBy: 'name',
    sortOrder: 'asc',
  })

  // ── Actions ────────────────────────────────────────────────
  async function fetchPackages() {
    loading.value = true
    try {
      // TODO: Invoke Tauri command to fetch packages
      // const result = await invoke('get_packages', { filters })
      // packages.value = result.packages
      // totalCount.value = result.total
    } finally {
      loading.value = false
    }
  }

  function setFilter<K extends keyof PackageFilters>(key: K, value: PackageFilters[K]) {
    filters[key] = value
  }

  function setViewMode(mode: 'grid' | 'list') {
    viewMode.value = mode
  }

  function selectPackage(id: string | null) {
    selectedPackageId.value = id
  }

  return {
    packages,
    totalCount,
    loading,
    selectedPackageId,
    viewMode,
    filters,
    fetchPackages,
    setFilter,
    setViewMode,
    selectPackage,
  }
})
