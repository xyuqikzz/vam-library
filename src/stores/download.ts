import { defineStore } from 'pinia'
import { ref, computed, onScopeDispose } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { createEventListeners } from '@/utils/eventListeners'

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
  
  let queueRequest: Promise<void> | null = null
  let refreshAgain = false
  let queueGeneration = 0
  
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
  function fetchQueue(): Promise<void> {
    refreshAgain = true
    if (queueRequest) return queueRequest
    const generation = queueGeneration
    loading.value = true
    queueRequest = (async () => {
      do {
        refreshAgain = false
        try {
          const result = await invoke<DownloadItem[]>('get_download_queue')
          if (generation === queueGeneration && !refreshAgain) queue.value = result || []
        } catch (error) {
          console.error('Failed to fetch download queue:', error)
        }
      } while (generation === queueGeneration && refreshAgain)
    })().finally(() => {
      if (generation === queueGeneration) {
        loading.value = false
        queueRequest = null
      }
    })
    return queueRequest
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
  
  async function runQueueCommand(command: string, ids: (string | undefined)[] = [undefined]) {
    for (const id of ids) {
      try {
        await invoke(command, id === undefined ? undefined : { id })
      } catch (error) {
        console.error(`Failed to ${command}:`, error)
      }
    }
    if (ids.length) await fetchQueue()
  }

  function pauseTask(id: string) { return runQueueCommand('pause_download', [id]) }
  function resumeTask(id: string) { return runQueueCommand('resume_download', [id]) }
  function cancelTask(id: string) { return runQueueCommand('cancel_download', [id]) }
  function retryTask(id: string) { return runQueueCommand('retry_download', [id]) }
  function clearCompleted() { return runQueueCommand('clear_completed_downloads') }

  function pauseAll() {
    return runQueueCommand('pause_download', queue.value
      .filter(item => item.status === 'Downloading' || item.status === 'Pending').map(item => item.id))
  }

  function resumeAll() {
    return runQueueCommand('resume_download', queue.value
      .filter(item => item.status === 'Paused' || item.status === 'Failed').map(item => item.id))
  }

  // ── Setup Listeners ────────────────────────────────────────
  const listeners = createEventListeners([
    () => listen('download-queue-updated', () => { void fetchQueue() }),
    
    // 2. Progress event (extremely fast updates, we update in-place for performance)
    () => listen<ProgressPayload>('download-progress', (event) => {
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
    }),
  ])

  function startListeners() { return listeners.start() }

  function stopListeners() {
    listeners.stop()
    queueGeneration += 1
    queueRequest = null
    refreshAgain = false
    loading.value = false
  }

  onScopeDispose(stopListeners)
  
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
