import { defineStore } from 'pinia'
import { ref, computed } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'

export interface DownloadItem {
  id: string
  url: string
  filename: string
  creator: string
  name: string
  version: number
  total_bytes: number
  downloaded_bytes: number
  progress: number
  speed_bytes_per_sec: number
  status: 'Pending' | 'Downloading' | 'Paused' | 'Completed' | 'Failed'
  error_msg: string | null
  warning_msg?: string | null
  added_at: string
  final_path: string | null
  temp_path: string | null
  install_mode: string | null
  indexed: boolean
}

export interface ProgressPayload {
  id: string
  downloaded: number
  total: number
  progress: number
  speed: number
  status: string
  final_path?: string
  install_mode?: string
  indexed?: boolean
  warning_msg?: string | null
}

export const useDownloadStore = defineStore('download', () => {
  // ── State ──────────────────────────────────────────────────
  const queue = ref<DownloadItem[]>([])
  const loading = ref(false)
  
  // Tauri event unlisteners
  let unlistenProgress: (() => void) | null = null
  let unlistenQueueUpdated: (() => void) | null = null
  
  // ── Computed ──────────────────────────────────────────────
  const activeDownloads = computed(() => {
    return queue.value.filter(item => item.status === 'Downloading')
  })
  
  const pendingDownloads = computed(() => {
    return queue.value.filter(item => item.status === 'Pending')
  })
  
  const activeAndPendingCount = computed(() => {
    return queue.value.filter(item => item.status === 'Downloading' || item.status === 'Pending').length
  })
  
  const completedCount = computed(() => {
    return queue.value.filter(item => item.status === 'Completed').length
  })
  
  const failedCount = computed(() => {
    return queue.value.filter(item => item.status === 'Failed').length
  })
  
  const totalSpeed = computed(() => {
    return queue.value
      .filter(item => item.status === 'Downloading')
      .reduce((sum, item) => sum + item.speed_bytes_per_sec, 0)
  })
  
  // Calculate total space saved by deduplication (approximate based on skipped versions)
  const totalWastedBytesSaved = ref(0)
  
  // ── Actions ────────────────────────────────────────────────
  async function fetchQueue() {
    loading.value = true
    try {
      const q = await invoke<DownloadItem[]>('get_download_queue')
      queue.value = q || []
    } catch (e) {
      console.error('Failed to fetch download queue:', e)
    } finally {
      loading.value = false
    }
  }
  
  async function addItems(items: Omit<DownloadItem, 'downloaded_bytes' | 'progress' | 'speed_bytes_per_sec' | 'status' | 'error_msg' | 'added_at' | 'final_path' | 'temp_path' | 'install_mode' | 'indexed'>[]) {
    const preparedItems = items.map(item => ({
      ...item,
      downloaded_bytes: 0,
      progress: 0.0,
      speed_bytes_per_sec: 0,
      status: 'Pending' as const,
      error_msg: null,
      added_at: new Date().toISOString(),
      final_path: null,
      temp_path: null,
      install_mode: null,
      indexed: false
    }))
    
    try {
      const results = await invoke<any[]>('add_to_download_queue', { items: preparedItems })

      // Calculate saved space
      results.forEach((res, index) => {
        if (res && res.Skipped) {
          totalWastedBytesSaved.value += preparedItems[index].total_bytes || 0
        }
      })

      await fetchQueue()
      return results
    } catch (e) {
      console.error('Failed to add items to queue:', e)
      throw e // 重新抛出错误，让调用方可以感知失败
    }
  }
  
  async function pauseTask(id: string) {
    try {
      await invoke('pause_download', { id })
      await fetchQueue()
    } catch (e) {
      console.error('Failed to pause download:', e)
    }
  }
  
  async function resumeTask(id: string) {
    try {
      await invoke('resume_download', { id })
      await fetchQueue()
    } catch (e) {
      console.error('Failed to resume download:', e)
    }
  }
  
  async function cancelTask(id: string) {
    try {
      await invoke('cancel_download', { id })
      await fetchQueue()
    } catch (e) {
      console.error('Failed to cancel download:', e)
    }
  }
  
  async function retryTask(id: string) {
    try {
      await invoke('retry_download', { id })
      await fetchQueue()
    } catch (e) {
      console.error('Failed to retry download:', e)
    }
  }
  
  async function clearCompleted() {
    try {
      await invoke('clear_completed_downloads')
      await fetchQueue()
    } catch (e) {
      console.error('Failed to clear completed:', e)
    }
  }
  
  async function pauseAll() {
    const active = queue.value.filter(i => i.status === 'Downloading' || i.status === 'Pending')
    for (const item of active) {
      await pauseTask(item.id)
    }
  }
  
  async function resumeAll() {
    const paused = queue.value.filter(i => i.status === 'Paused' || i.status === 'Failed')
    for (const item of paused) {
      await resumeTask(item.id)
    }
  }
  
  // ── Setup Listeners ────────────────────────────────────────
  async function startListeners() {
    // 1. Queue updated event
    unlistenQueueUpdated = await listen('download-queue-updated', () => {
      fetchQueue()
    })
    
    // 2. Progress event (extremely fast updates, we update in-place for performance)
    unlistenProgress = await listen<ProgressPayload>('download-progress', (event) => {
      const payload = event.payload
      if (!payload) return
      
      const item = queue.value.find(i => i.id === payload.id)
      if (item) {
        item.downloaded_bytes = payload.downloaded
        item.total_bytes = payload.total
        item.progress = payload.progress
        item.speed_bytes_per_sec = payload.speed
        
        // Map status string
        if (payload.status === 'completed') {
          item.status = 'Completed'
          item.speed_bytes_per_sec = 0
          item.final_path = payload.final_path || item.final_path
          item.install_mode = payload.install_mode || item.install_mode
          item.indexed = Boolean(payload.indexed)
          item.warning_msg = payload.warning_msg || null
        } else if (payload.status === 'downloading') {
          item.status = 'Downloading'
        }
      }
    })
  }
  
  function stopListeners() {
    if (unlistenQueueUpdated) {
      unlistenQueueUpdated()
      unlistenQueueUpdated = null
    }
    if (unlistenProgress) {
      unlistenProgress()
      unlistenProgress = null
    }
  }
  
  return {
    queue,
    loading,
    activeDownloads,
    pendingDownloads,
    activeAndPendingCount,
    completedCount,
    failedCount,
    totalSpeed,
    totalWastedBytesSaved,
    fetchQueue,
    addItems,
    pauseTask,
    resumeTask,
    cancelTask,
    retryTask,
    clearCompleted,
    pauseAll,
    resumeAll,
    startListeners,
    stopListeners
  }
})
