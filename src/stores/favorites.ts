import { computed, ref, watch } from 'vue'
import { defineStore } from 'pinia'
import { invoke } from '@tauri-apps/api/core'
import { useAppStore } from './app'
import { useLocalLibraryStore } from './localLibrary'

export interface FavoritePackage { packageId: string; count: number }

export const useFavoritesStore = defineStore('favorites', () => {
  const app = useAppStore()
  const library = useLocalLibraryStore()
  const entries = ref<FavoritePackage[]>([])
  const error = ref('')
  const started = ref(false)
  let epoch = 0
  const counts = computed(() => new Map(entries.value.map(entry => [entry.packageId.toLowerCase(), entry.count])))
  function countFor(id: string) { return counts.value.get(id.toLowerCase()) || 0 }
  async function refresh() {
    const request = ++epoch
    const root = app.vamRootPath
    error.value = ''
    if (!root) { entries.value = []; return }
    try {
      const result = await invoke<FavoritePackage[]>('list_favorite_package_ids', { vamRoot: root })
      if (request === epoch && root === app.vamRootPath) entries.value = result
    } catch (e) {
      if (request === epoch && root === app.vamRootPath) error.value = String(e)
    }
  }
  watch(() => [started.value, app.vamRootPath, library.revision] as const, (current, previous) => {
    if (current[1] !== previous?.[1]) entries.value = []
    if (current[0]) void refresh()
  })
  function start() { started.value = true }
  return { entries, error, countFor, refresh, start }
})
