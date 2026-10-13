import assert from 'node:assert/strict'
import { test } from 'node:test'
import { setImmediate } from 'node:timers/promises'
import { reactive, nextTick } from 'vue'
import { createPinia, disposePinia } from 'pinia'
import { mockIPC, clearMocks } from '@tauri-apps/api/mocks'
import { compileModule, moduleUrl } from './helpers/typescript.mjs'

const { useFavoritesStore } = await import(await compileModule(new URL('../src/stores/favorites.ts', import.meta.url), {
  './app': moduleUrl('export const useAppStore = () => globalThis.__favoriteApp'),
  './localLibrary': moduleUrl('export const useLocalLibraryStore = () => globalThis.__favoriteLibrary'),
}))
async function settle() { await nextTick(); await setImmediate(); await nextTick() }
function setup(t, handler) {
  globalThis.window = {}
  globalThis.__favoriteApp = reactive({ vamRootPath: '/first' })
  globalThis.__favoriteLibrary = reactive({ revision: 0 })
  const calls = []
  mockIPC((command, args) => {
    assert.equal(command, 'list_favorite_package_ids', 'favorites must never scan archives via list_game_contents')
    calls.push(args)
    return handler?.(args) ?? [{ packageId: 'author.scene.1', count: 2 }]
  })
  const pinia = createPinia(), store = useFavoritesStore(pinia)
  t.after(() => { disposePinia(pinia); clearMocks(); delete globalThis.window; delete globalThis.__favoriteApp; delete globalThis.__favoriteLibrary })
  return { store, calls }
}
test('favorite package counts load once, ignore case and refresh after library changes and favorite writes', async t => {
  const { store, calls } = setup(t)
  store.start(); store.start(); await settle()
  assert.equal(calls.length, 1)
  assert.equal(store.countFor('Author.Scene.1'), 2)
  assert.equal(store.countFor('Author.Scene.10'), 0)
  globalThis.__favoriteLibrary.revision++; await settle()
  await store.refresh()
  assert.equal(calls.length, 3)
})
test('changing game directory clears counts and discards a late response from the old directory', async t => {
  let resolve
  const pending = new Promise(done => { resolve = done })
  const { store } = setup(t, args => args.vamRoot === '/first' ? pending : [])
  store.start(); await settle()
  globalThis.__favoriteApp.vamRootPath = '/second'; await settle()
  resolve([{ packageId: 'old.package.1', count: 10 }]); await settle()
  assert.deepEqual(store.entries, [])
})
test('a failed count refresh preserves the last known snapshot and exposes the error', async t => {
  let failed = false
  const { store } = setup(t, () => { if (failed) throw new Error('read failed') })
  store.start(); await settle(); failed = true
  await store.refresh()
  assert.equal(store.countFor('AUTHOR.SCENE.1'), 2)
  assert.match(store.error, /read failed/)
})
