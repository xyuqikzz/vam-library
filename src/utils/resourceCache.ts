import { ref } from 'vue'
import { invoke, convertFileSrc } from '@tauri-apps/api/core'
import type { ResourceDetailItem } from '@/types/package'

const requests = new Map<string, Promise<unknown>>()
export const resourceCacheRevision = ref(0)

export function invalidateResourceCache(prefix = '') {
  if (!prefix) { requests.clear(); resourceCacheRevision.value++ }
  else for (const key of requests.keys()) if (key.startsWith(prefix)) requests.delete(key)
}
export function resourceKey(item: ResourceDetailItem, root = '', revision = 0) {
  return JSON.stringify([root, revision, resourceCacheRevision.value, item.id.toLowerCase(), item.file_path, item.scan_time, item.size_bytes])
}
export function cachedResource<T>(key: string, load: () => Promise<T>, force = false): Promise<T> {
  if (force) requests.delete(key)
  const cached = requests.get(key)
  if (cached) return cached as Promise<T>
  const pending = load().catch(error => {
    if (requests.get(key) === pending) requests.delete(key)
    throw error
  })
  requests.set(key, pending)
  return pending
}
export async function resourceInvoke<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  const start = performance.now()
  try { return await invoke<T>(command, args) }
  finally { if (import.meta.env?.DEV) console.debug(`[perf:ipc] ${command} ${(performance.now() - start).toFixed(2)}ms`) }
}
export function previewUrl(path: string | null): string | null {
  // Full preset JSON previews and remote Hub images retain their existing URL forms.
  return path && !/^(data:|https?:|asset:)/i.test(path) ? convertFileSrc(path) : path
}

interface ThumbnailResult { packageId: string; path: string | null; cached: boolean; error: string | null }
interface ThumbnailJob {
  item: ResourceDetailItem
  promise: Promise<string | null>
  resolve: (url: string | null) => void
  reject: (error: unknown) => void
  consumers: Set<() => boolean>
}
const thumbnailJobs = new Map<string, ThumbnailJob>()
const coldQueue: ThumbnailJob[] = []
let coldActive = 0

function pumpColdThumbnails() {
  while (coldActive < 4 && coldQueue.length) {
    const job = coldQueue.shift()!
    if (![...job.consumers].some(current => current())) {
      job.reject(new Error('Preview request cancelled'))
      continue
    }
    coldActive++
    void resourceInvoke<string | null>('get_package_thumbnail', { packageId: job.item.id })
      .then(previewUrl).then(job.resolve, job.reject)
      .finally(() => { coldActive--; pumpColdThumbnails() })
  }
}

async function loadThumbnailBatch(jobs: ThumbnailJob[]) {
  try {
    const results = await resourceInvoke<ThumbnailResult[]>('get_package_thumbnails', {
      packageIds: jobs.map(job => job.item.id), cachedOnly: true,
    })
    const byId = new Map(results.map(result => [result.packageId, result]))
    for (const job of jobs) {
      const result = byId.get(job.item.id)
      if (!result) job.reject(new Error('Missing thumbnail response'))
      else if (result.error) job.reject(new Error(result.error))
      else if (result.cached) job.resolve(previewUrl(result.path))
      else coldQueue.push(job)
    }
    // Let cached promises publish before starting any cold work.
    await Promise.resolve()
    pumpColdThumbnails()
  } catch (error) {
    for (const job of jobs) job.reject(error)
  }
}

export async function loadPackageThumbnails(items: ResourceDetailItem[], apply: (id: string, url: string | null, error?: string) => void, current: () => boolean = () => true) {
  const generation = resourceCacheRevision.value
  const isCurrent = () => generation === resourceCacheRevision.value && current()
  const unique = new Map(items.map(item => [resourceKey(item), item]))
  const fresh: ThumbnailJob[] = []
  const published: Promise<void>[] = []
  for (const [resource, item] of unique) {
    const key = `thumbnail:${resource}`
    let promise = requests.get(key) as Promise<string | null> | undefined
    let job = thumbnailJobs.get(key)
    if (!promise) {
      let resolve!: ThumbnailJob['resolve'], reject!: ThumbnailJob['reject']
      promise = new Promise<string | null>((done, fail) => { resolve = done; reject = fail })
      const pending = promise
      const complete = () => { if (thumbnailJobs.get(key)?.promise === pending) thumbnailJobs.delete(key) }
      job = {
        item, promise, consumers: new Set(),
        resolve: url => { complete(); resolve(url) },
        reject: error => {
          complete()
          if (requests.get(key) === pending) requests.delete(key)
          reject(error)
        },
      }
      // Reserve before the first await so overlapping pages share the warm query
      // as well as extraction. A cleared generation is never repopulated later.
      requests.set(key, promise)
      thumbnailJobs.set(key, job)
      fresh.push(job)
    }
    job?.consumers.add(isCurrent)
    const subscribed = job
    published.push(promise.then(
      url => { if (isCurrent()) apply(item.id, url) },
      error => { if (isCurrent()) apply(item.id, null, String(error)) },
    ).finally(() => { subscribed?.consumers.delete(isCurrent) }))
  }
  const batches: Promise<void>[] = []
  for (let i = 0; i < fresh.length; i += 400) batches.push(loadThumbnailBatch(fresh.slice(i, i + 400)))
  await Promise.all([...batches, ...published])
}
