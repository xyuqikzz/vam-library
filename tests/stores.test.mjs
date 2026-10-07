import assert from 'node:assert/strict'
import { webcrypto } from 'node:crypto'
import { test } from 'node:test'
import { setImmediate } from 'node:timers/promises'
import { createPinia, disposePinia } from 'pinia'
import { mockIPC, clearMocks } from '@tauri-apps/api/mocks'
import { emit } from '@tauri-apps/api/event'
import { compileModule, moduleUrl } from './helpers/typescript.mjs'

const eventsUrl = await compileModule(new URL('../src/utils/eventListeners.ts', import.meta.url))
const imports = { '@/utils/eventListeners': eventsUrl }
const { useDownloadStore } = await import(await compileModule(new URL('../src/stores/download.ts', import.meta.url), imports))
const { useAppStore } = await import(await compileModule(new URL('../src/stores/app.ts', import.meta.url), {
  ...imports, '../i18n': moduleUrl('export default { global: { locale: { value: "zh-CN" } } }'),
}))

function deferred() { let resolve; const promise = new Promise(r => { resolve = r }); return { promise, resolve } }

function setup(t, handler) {
  globalThis.window = { crypto: webcrypto }
  globalThis.localStorage = { getItem: () => null }
  globalThis.document = { documentElement: { dataset: {} } }
  t.mock.timers.enable({ apis: ['setTimeout'] })
  mockIPC(handler, { shouldMockEvents: true })
  const pinia = createPinia()
  t.after(() => {
    disposePinia(pinia)
    clearMocks()
    delete globalThis.window
    delete globalThis.localStorage
    delete globalThis.document
  })
  return pinia
}

test('overlapping queue refreshes are coalesced and only publish the latest snapshot', async t => {
  const first = deferred()
  const second = deferred()
  let calls = 0
  const store = useDownloadStore(setup(t, command => {
    assert.equal(command, 'get_download_queue')
    return ++calls === 1 ? first.promise : second.promise
  }))
  const requests = [store.fetchQueue(), store.fetchQueue(), store.fetchQueue()]
  assert.equal(calls, 1)
  first.resolve([{ id: 'old' }])
  await setImmediate()
  assert.equal(calls, 2)
  assert.deepEqual(store.queue, [])
  assert.equal(store.loading, true)
  second.resolve([{ id: 'new' }])
  await Promise.all(requests)
  assert.deepEqual(store.queue, [{ id: 'new' }])
  assert.equal(store.loading, false)
})

test('stopping downloads prevents an old queue response overwriting a restarted store', async t => {
  const first = deferred()
  let calls = 0
  const store = useDownloadStore(setup(t, () => ++calls === 1 ? first.promise : [{ id: 'new' }]))
  const request = store.fetchQueue()
  store.stopListeners()
  await store.fetchQueue()
  first.resolve([{ id: 'old' }])
  await request
  assert.deepEqual(store.queue, [{ id: 'new' }])
  assert.equal(store.loading, false)
})

test('batch pause targets active tasks, continues after failure and refreshes once', async t => {
  const calls = []
  const errors = t.mock.method(console, 'error', () => {})
  const store = useDownloadStore(setup(t, (command, args) => {
    calls.push([command, args?.id])
    if (command === 'pause_download' && args.id === 'active') throw new Error('failed')
    return []
  }))
  store.queue = [{ id: 'active', status: 'Downloading' }, { id: 'pending', status: 'Pending' }, { id: 'done', status: 'Completed' }]
  await store.pauseAll()
  assert.deepEqual(calls, [['pause_download', 'active'], ['pause_download', 'pending'], ['get_download_queue', undefined]])
  assert.equal(errors.mock.callCount(), 1)
})

test('completion events preserve saved paths, warnings and indexing status', async t => {
  const store = useDownloadStore(setup(t, () => []))
  store.queue = [{ id: 'one', status: 'Downloading', speed_bytes_per_sec: 42 }]
  await Promise.all([store.startListeners(), store.startListeners()])
  await emit('download-progress', { id: 'one', downloaded: 50, total: 50, progress: 100, speed: 42, status: 'completed', final_path: 'file.var', install_mode: 'real_addon', indexed: false, warning_msg: 'retry indexing' })
  assert.equal(store.queue[0].status, 'Completed')
  assert.equal(store.queue[0].speed_bytes_per_sec, 0)
  assert.equal(store.queue[0].indexed, false)
  assert.equal(store.queue[0].final_path, 'file.var')
  assert.equal(store.queue[0].warning_msg, 'retry indexing')
})

test('scan stays busy until database completion even when the parser emits done', async t => {
  const pending = deferred()
  let calls = 0
  const store = useAppStore(setup(t, command => {
    assert.equal(command, 'scan_vam_directory')
    calls++
    return pending.promise
  }))
  const scan = store.startScan('/fixture')
  await setImmediate()
  await emit('scan-progress', { phase: 'done', total_files: 1, processed_files: 1, current_file: '' })
  assert.equal(store.isScanning, true)
  await store.startScan('/fixture')
  assert.equal(calls, 1)
  pending.resolve({ packages_found: 1, errors: [] })
  await scan
  assert.equal(store.isScanning, false)
  assert.equal(store.lastScanResult.packages_found, 1)
})

test('scan clears its busy flag after command failure', async t => {
  const store = useAppStore(setup(t, () => { throw new Error('scan failed') }))
  await assert.rejects(store.startScan('/fixture'), /scan failed/)
  assert.equal(store.isScanning, false)
})

test('scan clears its busy flag after event-registration failure', async t => {
  const pinia = setup(t, () => {})
  mockIPC(() => { throw new Error('listen failed') })
  const other = useAppStore(pinia)
  await assert.rejects(other.startScan('/fixture'), /listen failed/)
  assert.equal(other.isScanning, false)
})

test('settings failures propagate and do not update downloader settings after a failed save', async t => {
  const commands = []
  const store = useAppStore(setup(t, command => {
    commands.push(command)
    throw new Error('write denied')
  }))
  await assert.rejects(store.saveSettings(), /write denied/)
  assert.deepEqual(commands, ['save_settings'])
})

test('a previous scan timer cannot clear progress of the next scan', async t => {
  const pending = deferred()
  let calls = 0
  const store = useAppStore(setup(t, () => ++calls === 1 ? {} : pending.promise))
  await store.startScan('/fixture')
  await emit('scan-progress', { phase: 'done' })
  const scan = store.startScan('/fixture')
  await setImmediate()
  await emit('scan-progress', { phase: 'parsing', current_file: 'current.var' })
  t.mock.timers.tick(2000)
  assert.equal(store.scanProgress.current_file, 'current.var')
  pending.resolve({})
  await scan
})
