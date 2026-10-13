import { computed, ref, watch } from 'vue'
import { defineStore } from 'pinia'
import { invoke } from '@tauri-apps/api/core'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import { useAppStore } from './app'
import { packageKey, selectSnapshot, toggleSelection } from '@/utils/cleanupSelection'
import type { CleanupHealthReport, CleanupIndexStatus, CleanupMark, PackageHealth, CleanupBatch } from '@/types/cleanup'

export const useCleanupStore = defineStore('cleanup', () => {
  const app = useAppStore()
  const marks = ref<CleanupMark[]>([])
  const selected = ref(new Set<string>())
  const latestBatch = ref<CleanupBatch | null>(null)
  const focusedBatchId = ref('')
  function recordBatch(batch: CleanupBatch) { latestBatch.value = batch }
  function focusBatch(id: string) { focusedBatchId.value = id }

  const loading = ref(false)
  let epoch = 0
  let writeEpoch = 0
  let anchor: string | null = null
  const markMap = computed(() => new Map(marks.value.map(mark => [packageKey(mark.package_id), mark])))
  const protectedIds = computed(() => new Set(marks.value.filter(m => m.protected).map(m => packageKey(m.package_id))))
  watch(() => app.vamRootPath, () => {
    epoch++; writeEpoch++; marks.value = []; selected.value = new Set(); anchor = null; loading.value = false
    healthEpoch++; health.value = null; healthLoading.value = false; healthError.value = ''
    previewIds.value = null
    latestBatch.value = null
    focusedBatchId.value = ''
  })
  async function load() {
    const root = app.vamRootPath
    if (!root) return
    const token = ++epoch
    loading.value = true
    try {
      const result = await invoke<CleanupMark[]>('list_cleanup_marks', { vamRoot: root })
      if (token === epoch) marks.value = result
    } finally { if (token === epoch) loading.value = false }
  }
  async function update(action: 'pending' | 'protect' | 'unmark' | 'unprotect', ids = [...selected.value]) {
    if (!ids.length || !app.vamRootPath) return
    const root = app.vamRootPath
    const token = ++writeEpoch
    epoch++; loading.value = false
    const result = await invoke<CleanupMark[]>('update_cleanup_marks', { vamRoot: root, packageIds: ids, action })
    if (token === writeEpoch && root === app.vamRootPath) {
      epoch++; loading.value = false
      marks.value = result
      if (action === 'protect') selected.value = new Set([...selected.value].filter(id => !protectedIds.value.has(id)))
    }
  }
  function select(ids: string[]) { selected.value = selectSnapshot(selected.value, ids, protectedIds.value) }
  function toggle(id: string, pageIds: string[], shift = false) {
    if (protectedIds.value.has(packageKey(id))) return
    const eligible = pageIds.filter(id => !protectedIds.value.has(packageKey(id)))
    selected.value = toggleSelection(selected.value, eligible, id, anchor, shift)
    anchor = packageKey(id)
  }
  function clear() { selected.value = new Set(); anchor = null }

  // The cleanup preview is opened from the toolbar and from the problem filters.
  const previewIds = ref<string[] | null>(null)
  function openPreview(ids: string[]) { if (ids.length) previewIds.value = ids.map(packageKey) }
  function closePreview() { previewIds.value = null }

  // Problem packages and packages nothing uses, from the last full resource check.
  const health = ref<Map<string, PackageHealth> | null>(null)
  const healthLoading = ref(false)
  const healthError = ref('')
  let healthEpoch = 0
  async function loadHealth() {
    const root = app.vamRootPath
    if (!root) return
    const token = ++healthEpoch
    healthLoading.value = true
    healthError.value = ''
    try {
      const report = await invoke<CleanupHealthReport>('get_cleanup_health', { vamRoot: root })
      if (token === healthEpoch) health.value = new Map(report.packages.map(item => [packageKey(item.package_id), item]))
    } catch (e) {
      if (token === healthEpoch) healthError.value = String(e)
    } finally {
      if (token === healthEpoch) healthLoading.value = false
    }
  }

  // Background reference-index status, shared by the cleanup views.
  const indexStatus = ref<CleanupIndexStatus | null>(null)
  let indexWatchers = 0
  let unlistenIndex: UnlistenFn[] = []
  async function watchIndexStatus() {
    if (++indexWatchers > 1) return
    try {
      const offs = await Promise.all([
        listen<CleanupIndexStatus>('cleanup-index-status', event => { indexStatus.value = event.payload }),
        // A newer full check finished in the background or in a preview.
        listen('cleanup-health-changed', () => { if (indexWatchers > 0) void loadHealth() }),
      ])
      if (indexWatchers === 0) { offs.forEach(off => off()); return }
      unlistenIndex = offs
    } catch { /* without events, still show the current status */ }
    try {
      const current = await invoke<CleanupIndexStatus>('get_cleanup_index_status')
      if (indexWatchers > 0) indexStatus.value = current
    } catch { /* informational only */ }
  }
  function unwatchIndexStatus() {
    indexWatchers = Math.max(0, indexWatchers - 1)
    if (indexWatchers === 0) { unlistenIndex.forEach(off => off()); unlistenIndex = [] }
  }
  return {
    latestBatch, focusedBatchId, recordBatch, focusBatch,
    marks, markMap, selected, loading, protectedIds, load, update, select, toggle, clear,
    previewIds, openPreview, closePreview,
    health, healthLoading, healthError, loadHealth,
    indexStatus, watchIndexStatus, unwatchIndexStatus,
  }
})
