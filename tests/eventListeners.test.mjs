import assert from 'node:assert/strict'
import { test } from 'node:test'
import { compileModule } from './helpers/typescript.mjs'

const { createEventListeners } = await import(await compileModule(new URL('../src/utils/eventListeners.ts', import.meta.url)))
const deferred = () => { let resolve; const promise = new Promise(r => { resolve = r }); return { promise, resolve } }

test('concurrent starts share a registration and repeated starts are idempotent', async () => {
  let registrations = 0
  let stops = 0
  const listeners = createEventListeners([async () => { registrations++; return () => { stops++ } }])
  await Promise.all([listeners.start(), listeners.start()])
  await listeners.start()
  assert.equal(registrations, 1)
  listeners.stop()
  listeners.stop()
  assert.equal(stops, 1)
})

test('stop disposes a registration that completes late', async () => {
  const late = deferred()
  let stops = 0
  const listeners = createEventListeners([() => late.promise])
  const starting = listeners.start()
  listeners.stop()
  late.resolve(() => { stops++ })
  await starting
  assert.equal(stops, 1)
})

test('partial failure cleans up and permits retry', async () => {
  let stops = 0
  let fail = true
  const listeners = createEventListeners([
    async () => () => { stops++ },
    async () => { if (fail) throw new Error('registration failed'); return () => { stops++ } },
  ])
  await assert.rejects(listeners.start(), /registration failed/)
  assert.equal(stops, 1)
  fail = false
  await listeners.start()
  listeners.stop()
  assert.equal(stops, 3)
})

test('a stale registration cannot replace a restarted subscription', async () => {
  const old = deferred()
  let attempt = 0
  let oldStops = 0
  let newStops = 0
  const listeners = createEventListeners([() => ++attempt === 1 ? old.promise : Promise.resolve(() => { newStops++ })])
  const first = listeners.start()
  listeners.stop()
  await listeners.start()
  old.resolve(() => { oldStops++ })
  await first
  assert.equal(oldStops, 1)
  assert.equal(newStops, 0)
  listeners.stop()
  assert.equal(newStops, 1)
})

test('a synchronously thrown registration error does not prevent retry', async () => {
  let fail = true
  const listeners = createEventListeners([() => {
    if (fail) throw new Error('synchronous failure')
    return Promise.resolve(() => {})
  }])
  await assert.rejects(listeners.start(), /synchronous failure/)
  fail = false
  await listeners.start()
  listeners.stop()
})
