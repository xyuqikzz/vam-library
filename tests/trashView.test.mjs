import assert from 'node:assert/strict'
import { test, after } from 'node:test'
import { setImmediate } from 'node:timers/promises'
import { JSDOM } from 'jsdom'
import { mockIPC, clearMocks } from '@tauri-apps/api/mocks'
import { componentLoader } from './helpers/vue.mjs'
import { moduleUrl } from './helpers/typescript.mjs'
const dom = new JSDOM('<div id="app"></div>', { url: 'http://localhost' })
for (const key of ['window', 'document', 'Element', 'HTMLElement', 'SVGElement', 'Node']) globalThis[key] = dom.window[key]
globalThis.getComputedStyle = dom.window.getComputedStyle.bind(dom.window)
globalThis.requestAnimationFrame = cb => setTimeout(cb, 0)
const { createApp, h, ref, KeepAlive, nextTick } = await import('vue')
const load = componentLoader({
  'vue-router': moduleUrl('export const useRouter = () => ({ push() {} }); export const useRoute = () => ({ path: "/trash" })'),
  '@/components/CleanupBatchPanel.vue': moduleUrl('export default { setup(_, { expose }) { expose({ load: () => { globalThis.__batchReloads++ } }); return () => null } }'),
  '@/composables/useConfirm': moduleUrl('export const useConfirm = () => async () => true'),
  '@/composables/useNotification': moduleUrl('export const useNotification = () => ({ error: e => globalThis.__trashErrors.push(e), success() {} })'),
  'vue-i18n': moduleUrl('export const useI18n = () => ({ t: key => key, locale: { value: "zh-CN" } })'),
})
const { default: View } = await import(load('src/views/TrashView.vue'))
const row = id => ({ id, package_id: 'A.Item'+id+'.1', original_path: '/fixture/A.Item'+id+'.1.var', trash_path: '/fixture/.trash/'+id+'.var', size_bytes: 1024, created_at: '2026-10-11T00:00:00Z' })
const settle = async () => { await nextTick(); await setImmediate() }
const button = key => [...document.querySelectorAll('button')].find(b => b.textContent.trim() === key)
async function mount(t, fail) {
  let items = [row(1), row(2)], reads = 0, deletions = 0
  globalThis.__trashErrors = []
  mockIPC(command => {
    if (command === 'list_cleanup_trash') { reads++; return items }
    if (command === 'empty_cleanup_trash') {
      deletions++
      items = fail ? [row(2)] : []
      if (fail) throw new Error('One file could not be removed')
      return 2
    }
    throw new Error('Unexpected command: '+command)
  })
  const app = createApp(View); app.config.globalProperties.$t = key => key; app.mount('#app')
  t.after(() => { app.unmount(); clearMocks() })
  await settle()
  return () => ({ reads, deletions })
}
test('partial trash deletion refreshes successful removals and retains the failed row', async t => {
  const stats = await mount(t, true)
  button('trash.emptyBtn').click(); await settle()
  assert.deepEqual([...document.querySelectorAll('.package-name')].map(n => n.textContent.trim()), ['A.Item2.1.var'])
  assert.equal(globalThis.__trashErrors.length, 1)
  assert.deepEqual(stats(), { reads: 2, deletions: 1 })
  assert.equal(button('trash.emptyBtn').disabled, false)
})
test('successful trash emptying refreshes once and shows the empty state', async t => {
  const stats = await mount(t, false)
  button('trash.emptyBtn').click(); await settle()
  assert.equal(document.querySelectorAll('.trash-row').length, 0)
  assert.match(document.body.textContent, /trash.emptyTitle/)
  assert.deepEqual(stats(), { reads: 2, deletions: 1 })
})
after(() => { dom.window.close(); for(const key of ['window','document','Element','HTMLElement','SVGElement','Node','getComputedStyle','requestAnimationFrame','__trashErrors']) delete globalThis[key] })

test('returning to a cached recycle bin refreshes both legacy files and cleanup records', async t => {
  let reads = 0
  globalThis.__batchReloads = 0
  mockIPC(command => { assert.equal(command, 'list_cleanup_trash'); reads++; return reads === 1 ? [] : [row(3)] })
  const shown = ref(true)
  const other = { render: () => h('div', 'other route') }
  const app = createApp({ render: () => h(KeepAlive, null, { default: () => h(shown.value ? View : other) }) })
  app.config.globalProperties.$t = key => key; app.mount('#app')
  t.after(() => { app.unmount(); clearMocks() })
  await settle(); assert.equal(reads, 1)
  shown.value = false; await settle()
  shown.value = true; await settle()
  assert.equal(reads, 2); assert.equal(globalThis.__batchReloads, 1)
  assert.match(document.querySelector('.package-name').textContent, /A.Item3.1.var/)
})
