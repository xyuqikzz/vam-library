import assert from 'node:assert/strict'
import { test } from 'node:test'
import { mockIPC, mockConvertFileSrc, clearMocks } from '@tauri-apps/api/mocks'
import { compileModule } from './helpers/typescript.mjs'
const cache = await import(await compileModule(new URL('../src/utils/resourceCache.ts', import.meta.url)))
function setup(t, handler) {
  globalThis.window = {}
  cache.invalidateResourceCache()
  mockConvertFileSrc('windows'); mockIPC(handler)
  t.after(() => { cache.invalidateResourceCache(); clearMocks(); delete globalThis.window })
}
const items = Array.from({ length: 100 }, (_, i) => ({ id: `Author.Package${i}.1`, name: String(i), file_path: `C:/game/${i}.var`, scan_time: 'first', resource_types: ['scene'] }))

test('one batch serves a 100-card page, including negative hits, and revisiting does not evict cards', async t => {
  const calls = []
  setup(t, (command, args) => {
    calls.push(command)
    assert.equal(command, 'get_package_thumbnails')
    assert.equal(args.packageIds.length, 100)
    return args.packageIds.map((packageId, i) => ({ packageId, cached: true, path: i ? `C:/cache/${i}.jpg` : null }))
  })
  const shown = new Map()
  await cache.loadPackageThumbnails(items, (id, url, error) => { assert.equal(error, undefined); shown.set(id, url) })
  assert.equal(shown.size, 100)
  assert.equal(shown.get(items[0].id), null)
  assert.match(shown.get(items[1].id), /^http:\/\/asset.localhost\//)
  shown.clear()
  await cache.loadPackageThumbnails(items, (id, url) => shown.set(id, url))
  assert.equal(shown.size, 100)
  assert.deepEqual(calls, ['get_package_thumbnails'])
})
test('cold images do not delay warm hits and decompression requests are bounded to four', async t => {
  let inFlight = 0, max = 0
  const published = []
  setup(t, async (command, args) => {
    if (command === 'get_package_thumbnails') return args.packageIds.map((packageId, i) => ({ packageId, cached: i === 0, path: i === 0 ? 'C:/cache/first.jpg' : null }))
    assert.equal(command, 'get_package_thumbnail')
    assert.deepEqual(published[0], items[0].id)
    max = Math.max(max, ++inFlight)
    await new Promise(resolve => setTimeout(resolve, 1)); inFlight--
    return 'C:/cache/cold.jpg'
  })
  await cache.loadPackageThumbnails(items.slice(0, 12), id => published.push(id))
  assert.equal(published.length, 12)
  assert.equal(max, 4)
})
test('scan changes and explicit cache clears invalidate URLs; failures can be retried', async t => {
  let calls = 0, fail = true
  setup(t, (_, args) => {
    calls++
    if (fail) { fail = false; throw new Error('temporary read failure') }
    return args.packageIds.map(packageId => ({ packageId, cached: true, path: null }))
  })
  const errors = []
  await cache.loadPackageThumbnails(items.slice(0, 1), (_, __, error) => errors.push(error))
  assert.match(errors[0], /temporary read failure/)
  await cache.loadPackageThumbnails(items.slice(0, 1), () => {})
  await cache.loadPackageThumbnails([{ ...items[0], scan_time: 'second' }], () => {})
  cache.invalidateResourceCache()
  await cache.loadPackageThumbnails(items.slice(0, 1), () => {})
  assert.equal(calls, 4)
})
test('late results cannot repopulate a cleared generation and cancelled pages stop cold work', async t => {
  let resolve, calls = 0, current = true
  setup(t, () => { calls++; return new Promise(done => { resolve = done }) })
  const pending = cache.loadPackageThumbnails(items.slice(0, 1), () => assert.fail('cancelled page published'), () => current)
  current = false; cache.invalidateResourceCache()
  resolve([{ packageId: items[0].id, cached: true, path: 'C:/cache/old.jpg' }]); await pending
  const next = cache.loadPackageThumbnails(items.slice(0, 1), () => {})
  assert.equal(calls, 2, 'old request must not populate the new generation')
  resolve([{ packageId: items[0].id, cached: true, path: null }]); await next
})

test('overlapping pages share pending warm queries and survive cancellation of one consumer', async t => {
  let finishBatch, firstCurrent = true, batches = 0, extractions = 0
  setup(t, command => {
    if (command === 'get_package_thumbnails') {
      batches++
      return new Promise(resolve => { finishBatch = resolve })
    }
    extractions++
    return 'C:/cache/shared.jpg'
  })
  const first = cache.loadPackageThumbnails([items[0], items[0]], () => assert.fail('cancelled consumer'), () => firstCurrent)
  const shown = []
  const second = cache.loadPackageThumbnails([items[0]], (_, url) => shown.push(url))
  firstCurrent = false
  finishBatch([{ packageId: items[0].id, cached: false, path: null }])
  await Promise.all([first, second])
  assert.equal(batches, 1)
  assert.equal(extractions, 1)
  assert.equal(shown.length, 1)
})

test('cold request limit is shared across pages and generation clears suppress late publications', async t => {
  let active = 0, maximum = 0
  setup(t, async (command, args) => {
    if (command === 'get_package_thumbnails') return args.packageIds.map(packageId => ({ packageId, cached: false }))
    maximum = Math.max(maximum, ++active)
    await new Promise(resolve => setTimeout(resolve, 2))
    active--
    return 'C:/cache/image.jpg'
  })
  await Promise.all([
    cache.loadPackageThumbnails(items.slice(0, 8), () => {}),
    cache.loadPackageThumbnails(items.slice(8, 16), () => {}),
  ])
  assert.equal(maximum, 4)
  const late = cache.loadPackageThumbnails(items.slice(20, 22), () => assert.fail('old generation published'))
  cache.invalidateResourceCache()
  await late
})

test('large batches are chunked and per-image failures remain retryable', async t => {
  const many = Array.from({ length: 405 }, (_, i) => ({ ...items[0], id: `A.P${i}.1`, file_path: `${i}.var` }))
  const batches = []
  let fail = true
  setup(t, (command, args) => {
    assert.equal(command, 'get_package_thumbnails')
    batches.push(args.packageIds.length)
    return args.packageIds.map(packageId => ({ packageId, cached: true, path: null,
      error: fail && packageId === many[0].id ? 'cannot read' : null }))
  })
  let errors = 0
  await cache.loadPackageThumbnails(many, (_, __, error) => { if (error) errors++ })
  assert.deepEqual(batches, [400, 5])
  assert.equal(errors, 1)
  fail = false
  await cache.loadPackageThumbnails(many, (_, __, error) => assert.equal(error, undefined))
  assert.deepEqual(batches, [400, 5, 1])
})
