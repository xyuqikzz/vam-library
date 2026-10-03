import { ref } from 'vue'
import { defineStore } from 'pinia'

export type ExternalImportMode = 'flatten' | 'by_type'
export interface ExternalImportSelection {
  files: string[]
  mode: ExternalImportMode
}

const commonPathsKey = 'vamlibrary-ingestion-common-paths'

function pathKey(path: string) {
  const normalized = path.trim().replace(/\\/g, '/').replace(/\/+$/, '')
  return /^[a-z]:|^\/\//i.test(normalized) ? normalized.toLowerCase() : normalized
}

function loadCommonPaths(): string[] {
  try {
    const stored: unknown = JSON.parse(localStorage.getItem(commonPathsKey) || '[]')
    if (!Array.isArray(stored)) return []
    const seen = new Set<string>()
    return stored.filter((path): path is string => typeof path === 'string' && !!path.trim())
      .map(path => path.trim())
      .filter(path => {
        const key = pathKey(path)
        if (seen.has(key)) return false
        seen.add(key)
        return true
      })
  } catch {
    // An unavailable or malformed preference must not block ordinary imports.
    return []
  }
}

export const useIngestionStore = defineStore('ingestion', () => {
  const busy = ref(false)
  const selection = ref<ExternalImportSelection | null>(null)
  const commonPaths = ref<string[]>(loadCommonPaths())

  function addCommonPath(path: string) {
    const trimmed = path.trim()
    if (!trimmed || commonPaths.value.some(existing => pathKey(existing) === pathKey(trimmed))) return
    const next = [...commonPaths.value, trimmed]
    localStorage.setItem(commonPathsKey, JSON.stringify(next))
    commonPaths.value = next
  }

  return { busy, selection, commonPaths, addCommonPath }
})
