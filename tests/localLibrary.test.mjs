import assert from 'node:assert/strict'
import { webcrypto } from 'node:crypto'
import { test } from 'node:test'
import { setImmediate } from 'node:timers/promises'
import { createPinia, disposePinia } from 'pinia'
import { clearMocks, mockIPC } from '@tauri-apps/api/mocks'
import { emit } from '@tauri-apps/api/event'
import { compileModule, moduleUrl } from './helpers/typescript.mjs'

const appStoreUrl = moduleUrl('export const useAppStore = () => ({ vamRootPath: "/fixture" })')
const eventsUrl = await compileModule(new URL('../src/utils/eventListeners.ts', import.meta.url))
const { useLocalLibraryStore } = await import(await compileModule(
  new URL('../src/stores/localLibrary.ts', import.meta.url),
  { './app': appStoreUrl, '@/utils/eventListeners': eventsUrl, '@/utils/resourceCache': await compileModule(new URL('../src/utils/resourceCache.ts', import.meta.url)) },
))

function deferred() {
  let resolve
  const promise = new Promise(done => { resolve = done })
  return { promise, resolve }
}

const original = { id: 'Author.Package.1', name: 'Package', tags: [] }
const updated = { ...original, tags: ['updated'] }

function setup(t, overrides = {}) {
  globalThis.window = { crypto: webcrypto }
  t.mock.timers.enable({ apis: ['setTimeout'] })
  const responses = {
    list_packages: () => [original],
    get_package_summary: () => updated,
    get_dashboard_stats: () => ({ total_packages: 1 }),
    get_dependency_graph: () => ({ nodes: [{ id: original.id }], edges: [] }),
    find_missing_dependencies: () => [{ package_id: original.id }],
    find_corrupted_packages: () => [{ package_id: 'broken' }],
    list_all_tags: () => ['updated'],
    list_package_folders: () => [{ path: 'folder' }],
    ...overrides,
  }
  mockIPC((command, args) => {
    assert.ok(command in responses, `Unexpected IPC command: ${command}`)
    return responses[command](args)
  }, { shouldMockEvents: true })
  const pinia = createPinia()
  const store = useLocalLibraryStore(pinia)
  t.after(() => {
    store.stopListeners()
    disposePinia(pinia)
    clearMocks()
    delete globalThis.window
  })
  return { store, responses }
}

async function change(t, reason = 'package_indexed', ids = [original.id]) {
  await emit('library-index-changed', {
    revision: 1, reason, changed_package_ids: ids, changed_paths: [], invalidates: [],
  })
  t.mock.timers.tick(200)
  await setImmediate()
}

test('recent packages show the twelve latest imports without changing the library order', t => {
  const { store } = setup(t)
  const packages = Array.from({ length: 15 }, (_, index) => ({
    ...original,
    id: `Author.Package${index}.1`,
    scan_time: new Date(Date.UTC(2026, 9, 6, 0, index)).toISOString(),
  }))
  store.packages = packages

  assert.deepEqual(store.recentPackages.map(pkg => pkg.id), packages.slice(3).reverse().map(pkg => pkg.id))
  assert.deepEqual(store.packages.map(pkg => pkg.id), packages.map(pkg => pkg.id))

  store.packages = packages.slice(0, 2)
  assert.equal(store.recentPackages.length, 2)
  store.packages = []
  assert.deepEqual(store.recentPackages, [])
})

for (const refresh of ['refreshStartup', 'refreshAll']) {
  test(`reset discards an in-flight ${refresh} result`, async t => {
    const pending = deferred()
    const { store } = setup(t, { list_packages: () => pending.promise })
    const loading = store[refresh]()
    store.resetState()
    pending.resolve([original])
    await loading
    await setImmediate()
    assert.deepEqual(store.packages, [])
    assert.deepEqual(store.allTags, [])
    assert.equal(store.dashboardStats.total_packages, 0)
    assert.equal(store.state, 'idle')
  })
}

test('reset discards secondary startup data after the package list is ready', async t => {
  const pending = deferred()
  const { store } = setup(t, { get_dashboard_stats: () => pending.promise })
  await store.refreshStartup()
  assert.equal(store.state, 'ready')
  store.resetState()
  pending.resolve({ total_packages: 99 })
  await setImmediate()
  assert.equal(store.dashboardStats.total_packages, 0)
  assert.deepEqual(store.corruptedPackages, [])
  assert.deepEqual(store.dependencyGraph.nodes, [])
})

test('reset cancels pending debounced changes', async t => {
  const { store } = setup(t)
  await store.startListeners()
  await emit('library-index-changed', {
    revision: 1, reason: 'scan_completed', changed_package_ids: [], changed_paths: [], invalidates: [],
  })
  store.resetState()
  t.mock.timers.tick(200)
  await setImmediate()
  assert.deepEqual(store.packages, [])
  assert.equal(store.state, 'idle')
})

test('a late incremental summary cannot repopulate a reset library', async t => {
  const pending = deferred()
  const { store } = setup(t, { get_package_summary: () => pending.promise })
  store.packages = [original]
  await store.startListeners()
  await change(t)
  store.resetState()
  pending.resolve(updated)
  await setImmediate()
  assert.deepEqual(store.packages, [])
  assert.deepEqual(store.allTags, [])
  assert.equal(store.state, 'idle')
})

test('a late incremental summary cannot overwrite a newer full refresh', async t => {
  const pending = deferred()
  const newer = { ...original, tags: ['newer'] }
  const { store, responses } = setup(t, { get_package_summary: () => pending.promise })
  store.packages = [original]
  await store.startListeners()
  await change(t)
  responses.list_packages = () => [newer]
  await store.refreshAll()
  pending.resolve(updated)
  await setImmediate()
  assert.deepEqual(store.packages, [newer])
  assert.equal(store.state, 'ready')
})

test('reset discards in-flight incremental statistics, tags and folders', async t => {
  const pending = deferred()
  const { store } = setup(t, {
    get_dashboard_stats: () => pending.promise.then(() => ({ total_packages: 99 })),
    get_dependency_graph: () => pending.promise.then(() => ({ nodes: [{ id: 'old' }], edges: [] })),
    list_all_tags: () => pending.promise.then(() => ['old']),
    list_package_folders: () => pending.promise.then(() => [{ path: 'old' }]),
  })
  store.packages = [original]
  await store.startListeners()
  await change(t)
  store.resetState()
  pending.resolve()
  await setImmediate()
  assert.equal(store.dashboardStats.total_packages, 0)
  assert.deepEqual(store.dependencyGraph.nodes, [])
  assert.deepEqual(store.missingDependencies, [])
  assert.deepEqual(store.corruptedPackages, [])
  assert.deepEqual(store.allTags, [])
  assert.deepEqual(store.packageFolders, [])
  assert.equal(store.state, 'idle')
})

for (const reason of ['package_indexed', 'tags_changed']) {
  test(`${reason} query failure preserves the row until a full refresh recovers`, async t => {
    const pending = deferred()
    let fullRefreshes = 0
    const { store } = setup(t, {
      get_package_summary: () => { throw new Error('database temporarily unavailable') },
      list_packages: () => { fullRefreshes++; return pending.promise },
    })
    store.packages = [original]
    await store.startListeners()
    await change(t, reason)
    assert.deepEqual(store.packages, [original])
    assert.equal(fullRefreshes, 1)
    pending.resolve([updated])
    await setImmediate()
    assert.deepEqual(store.packages, [updated])
    assert.equal(store.state, 'ready')
  })
}

test('a successful null summary removes the missing package', async t => {
  const { store } = setup(t, { get_package_summary: () => null })
  store.packages = [original]
  await store.startListeners()
  await change(t)
  assert.deepEqual(store.packages, [])
  assert.equal(store.state, 'ready')
})

test('batched changes update, append and remove packages while preserving list order', async t => {
  const second = { ...original, id: 'Second.Package.1' }
  const removed = { ...original, id: 'Removed.Package.1' }
  const added = { ...original, id: 'Added.Package.1' }
  const calls = []
  const { store } = setup(t, {
    get_package_summary: ({ packageId }) => {
      calls.push(packageId)
      return packageId === original.id ? updated : packageId === added.id ? added : null
    },
  })
  store.packages = [original, second, removed]
  await store.startListeners()
  await change(t, 'package_indexed', [added.id, original.id, removed.id, original.id])
  assert.deepEqual(store.packages, [updated, second, added])
  assert.deepEqual(calls, [added.id, original.id, removed.id])
  assert.equal(store.state, 'ready')
})

test('stopping listeners discards pending responses', async t => {
  const pending = deferred()
  const { store } = setup(t, { get_package_summary: () => pending.promise })
  store.packages = [original]
  await store.startListeners()
  await change(t)
  store.stopListeners()
  pending.resolve(updated)
  await setImmediate()
  assert.deepEqual(store.packages, [original])
})

test('failed full refresh clears all stale secondary data', async t => {
  const { store, responses } = setup(t)
  await store.refreshAll()
  assert.equal(store.corruptedPackages.length, 1)
  responses.list_packages = () => { throw new Error('database unavailable') }
  await store.refreshAll()
  assert.deepEqual(store.packages, [])
  assert.deepEqual(store.corruptedPackages, [])
  assert.equal(store.state, 'error')
  assert.match(store.error, /database unavailable/)
})

test('a change invalidates old secondary data before the debounce fires', async t => {
  const pending = deferred()
  const { store } = setup(t, { find_missing_dependencies: () => pending.promise })
  await store.startListeners()
  await store.refreshStartup()
  store.missingDependencies = [{ package_id: 'keep' }]
  await emit('library-index-changed', {
    revision: 2, reason: 'package_indexed', changed_package_ids: [original.id], changed_paths: [], invalidates: [],
  })
  pending.resolve([{ package_id: 'obsolete' }])
  await setImmediate()
  assert.deepEqual(store.missingDependencies, [{ package_id: 'keep' }])
  assert.equal(store.state, 'indexing')
})

test('failed dependency analysis preserves previous missing rows instead of reporting zero', async t => {
  const { store } = setup(t, { find_missing_dependencies: () => { throw new Error('analysis failed') } })
  store.missingDependencies = [{ package_id: 'keep' }]
  store.dashboardStats = { total_packages: 1, missing_dependencies: 1 }
  await store.refreshStartup()
  await setImmediate()
  assert.deepEqual(store.missingDependencies, [{ package_id: 'keep' }])
  assert.equal(store.dashboardStats.missing_dependencies, 1)
  assert.equal(store.state, 'stale')
  assert.match(store.error, /analysis failed/)
})

test('dashboard missing count is taken from the same successful missing-list snapshot', async t => {
  const { store } = setup(t, {
    get_dashboard_stats: () => ({ total_packages: 1, missing_dependencies: 999 }),
    find_missing_dependencies: () => [{ package_id: 'one' }, { package_id: 'two' }],
  })
  await store.refreshAll()
  assert.equal(store.dashboardStats.missing_dependencies, 2)
})

test('a tag change interrupting startup also refreshes unfinished secondary data', async t => {
  const pending = deferred()
  let calls = 0
  const { store } = setup(t, {
    find_missing_dependencies: () => ++calls === 1 ? pending.promise : [{ package_id: 'current' }],
  })
  await store.startListeners()
  await store.refreshStartup()
  await change(t, 'tags_changed')
  pending.resolve([{ package_id: 'obsolete' }])
  await setImmediate()
  assert.deepEqual(store.missingDependencies, [{ package_id: 'current' }])
  assert.equal(store.dashboardStats.missing_dependencies, 1)
  assert.equal(store.state, 'ready')
})

test('an explicit deletion refresh consumes the queued event without reading the library twice', async t => {
  let lists = 0
  const { store } = setup(t, { list_packages: () => { lists++; return [] } })
  await store.startListeners()
  await emit('library-index-changed', { revision: 2, reason: 'cleanup_changed', changed_package_ids: [original.id], changed_paths: [], invalidates: ['packages'] })
  await store.refreshAll()
  t.mock.timers.tick(200); await setImmediate()
  assert.equal(lists, 1)
  assert.equal(store.state, 'ready')
})

test('a new mutation during deletion refresh still invalidates the old snapshot', async t => {
  const first = deferred(); let lists = 0
  const { store } = setup(t, { list_packages: () => ++lists === 1 ? first.promise : [] })
  await store.startListeners()
  const refresh = store.refreshAll()
  await emit('library-index-changed', { revision: 3, reason: 'cleanup_changed', changed_package_ids: [original.id], changed_paths: [], invalidates: ['packages'] })
  first.resolve([original]); await refresh
  assert.deepEqual(store.packages, [])
  t.mock.timers.tick(200); await setImmediate()
  assert.equal(lists, 2)
  assert.deepEqual(store.packages, [])
})
