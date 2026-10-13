import assert from 'node:assert/strict'
import { test } from 'node:test'
import { webcrypto } from 'node:crypto'
import { createPinia, disposePinia } from 'pinia'
import { ref, nextTick } from 'vue'
import { clearMocks, mockIPC } from '@tauri-apps/api/mocks'
import { compileModule, moduleUrl } from './helpers/typescript.mjs'
const selectionUrl = await compileModule(new URL('../src/utils/cleanupSelection.ts', import.meta.url))
const { selectSnapshot, toggleSelection, hiddenSelectionCount } = await import(selectionUrl)
test('select-all is a snapshot and excludes manually protected resources', () => {
  const source = ['A.1', 'B.1']
  const selected = selectSnapshot(new Set(['existing']), source, new Set(['b.1']))
  source.push('C.1')
  assert.deepEqual([...selected], ['existing', 'a.1'])
})
test('selection survives filters and a shift range is limited to the supplied page', () => {
  const selected = toggleSelection(new Set(['hidden']), ['A', 'B', 'C'], 'C', 'a', true)
  assert.deepEqual([...selected], ['hidden', 'a', 'b', 'c'])
  assert.equal(hiddenSelectionCount(selected, ['B', 'C']), 2)
  assert.deepEqual([...toggleSelection(selected, ['D', 'E'], 'E', 'a', true)], ['hidden', 'a', 'b', 'c', 'e'])
})
const app = ref({ vamRootPath: '/one' })
globalThis.__cleanupTestApp = app
const appUrl = moduleUrl('export const useAppStore = () => new Proxy({}, { get: (_, key) => globalThis.__cleanupTestApp.value[key] })')
const { useCleanupStore } = await import(await compileModule(new URL('../src/stores/cleanup.ts', import.meta.url), { './app': appUrl, '@/utils/cleanupSelection': selectionUrl }))
function deferred() { let resolve; const promise = new Promise(done => { resolve = done }); return { resolve, promise } }
function setup(t, handler) {
  globalThis.window = { crypto: webcrypto }
  app.value = { vamRootPath: '/one' }
  mockIPC(handler)
  const pinia = createPinia(), store = useCleanupStore(pinia)
  t.after(() => { disposePinia(pinia); clearMocks(); delete globalThis.window })
  return store
}
test('instance switch clears selection and discards a late mark query', async t => {
  const pending = deferred()
  const store = setup(t, () => pending.promise)
  store.select(['A.1'])
  store.recordBatch({ id: 'last-batch', files: [], errors: [] }); store.focusBatch('last-batch')
  const load = store.load()
  app.value = { vamRootPath: '/two' }; await nextTick()
  pending.resolve([{ package_id: 'A.1', pending: true, protected: false }]); await load
  assert.equal(store.selected.size, 0); assert.deepEqual(store.marks, [])
  assert.equal(store.latestBatch, null); assert.equal(store.focusedBatchId, '')
})
test('an old-instance mark mutation cannot overwrite the new instance state', async t => {
  const pending = deferred()
  const store = setup(t, (cmd, args) => { assert.equal(cmd, 'update_cleanup_marks'); assert.equal(args.vamRoot, '/one'); return pending.promise })
  store.select(['A.1']); const update = store.update('protect')
  app.value = { vamRootPath: '/two' }; await nextTick()
  pending.resolve([{ package_id: 'A.1', pending: false, protected: true }]); await update
  assert.deepEqual(store.marks, []); assert.equal(store.selected.size, 0)
})
test('a mark query started during a write cannot overwrite the committed result', async t => {
  const write = deferred(), read = deferred()
  const store = setup(t, cmd => cmd === 'update_cleanup_marks' ? write.promise : read.promise)
  store.select(['A.1'])
  const update = store.update('protect'), load = store.load()
  write.resolve([{ package_id: 'a.1', pending: false, protected: true }]); await update
  read.resolve([]); await load
  assert.equal(store.markMap.get('a.1').protected, true)
  assert.equal(store.loading, false)
  assert.equal(store.selected.size, 0)
})
test('background index status loads once, follows events and stops with the last watcher', async t => {
  globalThis.window = { crypto: webcrypto }
  const status = (state, current = 0) => ({ state, stage: 'building_index', current, total: 4, message: null, finished_at: null })
  let loads = 0
  mockIPC(command => { if (command === 'get_cleanup_index_status') { loads++; return status('running', 1) } }, { shouldMockEvents: true })
  const pinia = createPinia(), store = useCleanupStore(pinia)
  t.after(() => { disposePinia(pinia); clearMocks(); delete globalThis.window })
  const { emit } = await import('@tauri-apps/api/event')
  await Promise.all([store.watchIndexStatus(), store.watchIndexStatus()])
  assert.equal(loads, 1)
  assert.equal(store.indexStatus.current, 1)
  await emit('cleanup-index-status', status('running', 3))
  assert.equal(store.indexStatus.current, 3)
  store.unwatchIndexStatus()
  await emit('cleanup-index-status', status('ready'))
  assert.equal(store.indexStatus.state, 'ready')
  store.unwatchIndexStatus()
  await emit('cleanup-index-status', status('error'))
  assert.equal(store.indexStatus.state, 'ready')
})
test('package health loads per game folder, follows newer checks and is dropped on a folder switch', async t => {
  globalThis.window = { crypto: webcrypto }
  app.value = { vamRootPath: '/one' }
  const roots = []
  let report = { packages: [{ package_id: 'Bad.Page.1', issue: 'invalid_download', copies: 1, unused: true }], checked_at: '' }
  mockIPC((command, args) => {
    if (command === 'get_cleanup_health') { roots.push(args.vamRoot); return report }
    if (command === 'get_cleanup_index_status') return null
  }, { shouldMockEvents: true })
  const pinia = createPinia(), store = useCleanupStore(pinia)
  t.after(() => { disposePinia(pinia); clearMocks(); delete globalThis.window })
  const { emit } = await import('@tauri-apps/api/event')
  await store.loadHealth()
  assert.equal(store.health.get('bad.page.1').issue, 'invalid_download')
  await store.watchIndexStatus()
  report = { packages: [], checked_at: '' }
  await emit('cleanup-health-changed', 'key')
  await new Promise(done => setTimeout(done, 0))
  assert.equal(store.health.size, 0)
  store.unwatchIndexStatus()
  app.value = { vamRootPath: '/two' }; await nextTick()
  assert.equal(store.health, null)
  assert.deepEqual(roots, ['/one', '/one'])
})
const issuesUrl = await compileModule(new URL('../src/utils/cleanupIssues.ts', import.meta.url))
const { packageIssues, issueCounts } = await import(issuesUrl)
const { isStandalonePackage } = await import(issuesUrl)
test('standalone cleanup requires a confirmed full check and excludes broken, missing and protected packages', () => {
  const confirmed = { package_id: 'A.Complete.1', issue: null, copies: 1, unused: true, standalone: true }
  assert.equal(isStandalonePackage(confirmed, undefined, false), true)
  for (const health of [undefined, { ...confirmed, standalone: undefined }, { ...confirmed, standalone: false }, { ...confirmed, unused: false }, { ...confirmed, issue: 'damaged' }, { ...confirmed, invalid_name: true }]) {
    assert.equal(isStandalonePackage(health, undefined, false), false)
  }
  assert.equal(isStandalonePackage(confirmed, undefined, true), false)
  for (const issues of [{ invalid: true }, { invalidName: true }, { damaged: 'info' }, { missing: 1 }]) {
    assert.equal(isStandalonePackage(confirmed, issues, false), false)
  }
})
const pkg = (id, types = ['morph']) => { const [creator, name, version] = id.split('.'); return { id, creator, name, version: Number(version), resource_types: types } }
test('package issues combine the resource check, missing dependencies and installed versions', () => {
  const packages = [pkg('A.Look.1'), pkg('A.Look.2'), pkg('B.Page.1', ['other']), pkg('C.Scene.1', ['scene']), pkg('D.Morph.1')]
  const health = new Map([
    ['b.page.1', { package_id: 'B.Page.1', issue: 'wrong_format', copies: 1, unused: true }],
    ['c.scene.1', { package_id: 'C.Scene.1', issue: null, copies: 2, unused: true }],
    ['d.morph.1', { package_id: 'D.Morph.1', issue: null, copies: 1, unused: true }],
  ])
  const issues = packageIssues(packages, { health, missing: [{ package_id: 'C.Scene.1' }], corrupted: [{ package_id: 'D.Morph.1', error: "Parse error: Failed to parse meta.json: expected `,` or `}`" }], protectedIds: new Set() })
  assert.deepEqual(issues.get('a.look.1'), { newerVersion: 2 })
  assert.equal(issues.has('a.look.2'), false)
  assert.deepEqual(issues.get('b.page.1'), { invalid: true })
  assert.deepEqual(issues.get('c.scene.1'), { missing: 1, copies: 2 })
  // The resource check opens it fine, but the scan could not read its info.
  assert.deepEqual(issues.get('d.morph.1'), { damaged: 'info' })
  assert.deepEqual(issueCounts(issues), { invalid: 1, invalidName: 0, damaged: 1, missingDeps: 1, unused: 0, oldVersion: 1, copies: 1 })
})
const { brokenInfoIds } = await import(issuesUrl)
test('scan failures are split into unusable archives and broken package info', () => {
  const corrupted = [
    { package_id: 'A.Meta.1', error: "Failed to parse meta.json from 'A.Meta.1.var': EOF while parsing an object" },
    { package_id: 'B.Page.1', error: '此文件包含网页或 XML 文本，不是 VAR 压缩包。' },
    { package_id: 'C.Zip.1', error: '无法读取资源包: invalid Zip archive' },
  ]
  // Without a resource check, the scan error decides.
  assert.deepEqual([...brokenInfoIds(corrupted, null)], ['a.meta.1'])
  const before = packageIssues(['A.Meta.1', 'B.Page.1', 'C.Zip.1'].map(id => pkg(id, ['other'])), { health: null, missing: [], corrupted, protectedIds: new Set() })
  assert.deepEqual([before.get('a.meta.1'), before.get('b.page.1'), before.get('c.zip.1')], [{ damaged: 'info' }, { invalid: true }, { damaged: 'archive' }])
  // With one, an archive it could open only has broken info; one it could not open is damaged.
  const health = new Map([['c.zip.1', { package_id: 'C.Zip.1', issue: 'damaged', copies: 1, unused: true }]])
  assert.deepEqual([...brokenInfoIds(corrupted, health)], ['a.meta.1'])
})

test('filename failures are separate from broken package info before and after health checks', () => {
  const packages = ['Qing.Alet01.1_2', 'V2.Morphs.latest', 'Bad.Archive.2_2'].map(id => pkg(id, ['other']))
  const corrupted = packages.map(p => ({ package_id: p.id, error: "Parse error: Invalid version '1_2' in filename" }))
  for (const health of [null, new Map(), new Map(packages.map(p => [p.id.toLowerCase(), { package_id: p.id, issue: null, invalid_name: true, copies: 1, unused: true }]))]) {
    const issues = packageIssues(packages, { health, missing: [], corrupted, protectedIds: new Set() })
    assert.deepEqual([...brokenInfoIds(corrupted, health)], [])
    assert.deepEqual([...issues.values()], [{ invalidName: true }, { invalidName: true }, { invalidName: true }])
    assert.equal(issueCounts(issues).invalidName, 3)
    assert.equal(issueCounts(issues).damaged, 0)
  }
  const health = new Map([['bad.archive.2_2', { issue: 'damaged', invalid_name: true, copies: 1, unused: true }]])
  assert.deepEqual(packageIssues(packages, { health, missing: [], corrupted, protectedIds: new Set() }).get('bad.archive.2_2'), { invalidName: true, damaged: 'archive' })
})

test('successfully scanned nonstandard names retain the filename warning without a health check', () => {
  const packages = ['Qing.Alet01.1_2', 'V2.Morphs.latest', 'Author.Name'].map(id => pkg(id, ['scene']))
  const issues = packageIssues(packages, { health: null, missing: [], corrupted: [], protectedIds: new Set() })
  for (const value of issues.values()) assert.deepEqual(value, { invalidName: true })
  assert.equal(issues.size, packages.length)
})
