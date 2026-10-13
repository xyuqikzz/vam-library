import assert from 'node:assert/strict'
import { test, after } from 'node:test'
import { setImmediate } from 'node:timers/promises'
import { JSDOM } from 'jsdom'
import { mockIPC, clearMocks } from '@tauri-apps/api/mocks'
import { componentLoader } from './helpers/vue.mjs'
import { moduleUrl } from './helpers/typescript.mjs'
const dom = new JSDOM('<div id="app"></div>', { url: 'http://localhost' })
for (const key of ['window','document','Element','HTMLElement','SVGElement','Node']) globalThis[key] = dom.window[key]
globalThis.getComputedStyle = dom.window.getComputedStyle.bind(dom.window)
globalThis.requestAnimationFrame = cb => setTimeout(cb, 0)
const { createApp, h, nextTick, reactive, ref } = await import('vue')
const { createPinia, disposePinia } = await import('pinia')
const appState = reactive({ vamRootPath: '/fixture' })
globalThis.__batchApp = appState
const appUrl = moduleUrl('export const useAppStore = () => globalThis.__batchApp')
const load = componentLoader({
  '@/stores/app': appUrl, './app': appUrl,
  '@/stores/localLibrary': moduleUrl('export const useLocalLibraryStore = () => ({ refreshAll: async () => {} })'),
  '@/composables/useConfirm': moduleUrl('export const useConfirm = () => async () => true'),
  'vue-i18n': moduleUrl('export const useI18n = () => ({ t: key => key, locale: { value: "zh-CN" } })'),
  '@tauri-apps/api/event': moduleUrl('export const listen = async (_, cb) => { globalThis.__batchProgress = cb; return () => { delete globalThis.__batchProgress } }'),
})
const { default: Panel } = await import(load('src/components/CleanupBatchPanel.vue'))
const { useCleanupStore } = await import(load('src/stores/cleanup.ts'))
const batch = id => ({ id, created_at: '2026-10-12T01:00:00Z', expires_at: '2026-11-12T01:00:00Z', status: 'completed', errors: [], files: [{ package_id: 'A.'+id+'.1', original: { path: '/fixture/'+id+'.var', size: 1024, modified: '' }, trash_path: '/trash/'+id, hash: 'hash', status: 'trashed' }] })
const settle = async () => { await nextTick(); await setImmediate() }
function deferred() { let resolve; const promise = new Promise(done => { resolve = done }); return { resolve, promise } }
async function mount(t, handler) {
  appState.vamRootPath = '/fixture'
  mockIPC(handler)
  const pinia = createPinia(), store = useCleanupStore(pinia), panel = ref()
  const app = createApp({ render: () => h(Panel, { ref: panel }) }); app.use(pinia); app.mount('#app')
  t.after(async () => { app.unmount(); disposePinia(pinia); await settle(); clearMocks() })
  await settle(); return { store, panel }
}

test('a completed batch is visible and expanded even when an older list request finishes later', async t => {
  const pending = deferred()
  const { store, panel } = await mount(t, () => pending.promise)
  store.recordBatch(batch('fresh')); store.focusBatch('fresh'); await settle()
  assert.equal(document.querySelector('details.batch').open, true)
  assert.match(document.querySelector('.batch-files').textContent, /A.fresh.1/)
  pending.resolve([]); await settle()
  assert.equal(document.querySelectorAll('details.batch').length, 1)
  mockIPC(() => [batch('fresh')]); await panel.value.load(); await settle()
  assert.equal(document.querySelectorAll('details.batch').length, 1)
})

test('instance changes discard old records and late list responses', async t => {
  const old = deferred()
  await mount(t, (_, args) => args.vamRoot === '/fixture' ? old.promise : [batch('new-root')])
  appState.vamRootPath = '/new'; await settle()
  old.resolve([batch('old-root')]); await settle()
  assert.deepEqual([...document.querySelectorAll('.batch')].map(el => el.dataset.batchId), ['new-root'])
})

test('permanent deletion reports progress and replaces the completed record with its result', async t => {
  const pending = deferred(), fresh = batch('purge')
  const { store } = await mount(t, cmd => cmd === 'list_resource_cleanup_batches' ? [fresh] : pending.promise)
  store.recordBatch(fresh); store.focusBatch(fresh.id); await settle()
  document.querySelector('.batch-actions .danger').click(); await settle()
  globalThis.__batchProgress({ payload: { request_id: fresh.id, stage: 'purging_bytes', current: 512, total: 1024 } }); await settle()
  assert.match(document.querySelector('[role="status"]').textContent, /cleanup.purging.*cleanup.verifiedBytes/)
  assert.equal(document.querySelector('[role="progressbar"]').getAttribute('aria-valuenow'), '50')
  assert.equal(document.querySelector('.batch-actions .danger').disabled, true)
  pending.resolve({ ...fresh, status: 'purged', files: fresh.files.map(f => ({ ...f, status: 'purged' })) }); await settle()
  assert.match(document.querySelector('.status-tag').textContent, /cleanup.status.purged/)
  assert.equal(store.latestBatch.status, 'purged')
  assert.equal(document.querySelector('.batch-progress'), null)
})

after(() => { dom.window.close(); for (const key of ['window','document','Element','HTMLElement','SVGElement','Node','getComputedStyle','requestAnimationFrame','__batchApp']) delete globalThis[key] })
