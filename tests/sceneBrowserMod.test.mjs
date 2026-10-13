import assert from 'node:assert/strict'
import { after, test } from 'node:test'
import { setImmediate } from 'node:timers/promises'
import { JSDOM } from 'jsdom'
import { mockIPC, clearMocks } from '@tauri-apps/api/mocks'
import { componentLoader } from './helpers/vue.mjs'
import { moduleUrl } from './helpers/typescript.mjs'

const dom = new JSDOM('<div id="app"></div>', { url: 'http://localhost', pretendToBeVisual: true })
const globals = ['window', 'document', 'Element', 'HTMLElement', 'SVGElement', 'Node', 'localStorage']
for (const key of globals) globalThis[key] = dom.window[key]
globalThis.getComputedStyle = dom.window.getComputedStyle.bind(dom.window)
const { createApp, h, nextTick, reactive } = await import('vue')
const load = componentLoader({
  '@/stores/app': moduleUrl('export const useAppStore = () => globalThis.__modApp'),
  '@/composables/useNotification': moduleUrl('export const useNotification = () => ({ success() {} })'),
  '@/composables/useConfirm': moduleUrl('export const useConfirm = () => options => globalThis.__modConfirm(options)'),
  'vue-i18n': moduleUrl('export const useI18n = () => ({ t: key => key })'),
})
const { default: Panel } = await import(load('src/components/SceneBrowserModPanel.vue'))
after(() => {
  dom.window.close()
  for (const key of [...globals, 'getComputedStyle', '__modApp', '__modConfirm']) delete globalThis[key]
})
async function settle() { await nextTick(); await setImmediate(); await nextTick() }
async function mount(t, overrides = {}) {
  globalThis.__modApp = reactive({ vamRootPath: 'C:/Game' })
  const calls = [], prompts = []
  let answer
  globalThis.__modConfirm = options => { prompts.push(options); return new Promise(resolve => { answer = resolve }) }
  const status = { version: '1.2.0', installed: false, updateAvailable: false, compatible: true, verified: false, conflict: false, reason: null, runtime: null, targetPath: 'fixture', ...overrides }
  mockIPC((cmd, args) => { calls.push({ cmd, args }); return { ...status, installed: cmd === 'install_scene_browser_mod' || status.installed } })
  const app = createApp({ render: () => h(Panel) }); app.mount('#app')
  t.after(() => { app.unmount(); clearMocks() })
  await settle()
  return { calls, prompts, answer: value => answer(value), install: () => [...document.querySelectorAll('button')].find(el => el.textContent.trim() === 'sceneBrowserMod.install') }
}

test('untested installation waits for confirmation and passes consent only when accepted', async t => {
  const ctx = await mount(t)
  assert.ok(document.body.textContent.includes('sceneBrowserMod.unverified'))
  ctx.install().click(); await settle()
  assert.equal(ctx.prompts.length, 1)
  assert.equal(ctx.calls.some(c => c.cmd === 'install_scene_browser_mod'), false)
  ctx.answer(false); await settle()
  assert.equal(ctx.calls.some(c => c.cmd === 'install_scene_browser_mod'), false)
  ctx.install().click(); await settle(); ctx.answer(true); await settle()
  assert.deepEqual(ctx.calls.find(c => c.cmd === 'install_scene_browser_mod').args, { vamRoot: 'C:/Game', allowUnverified: true })
})

test('changing game folder while confirmation is open cancels the old installation', async t => {
  const ctx = await mount(t)
  ctx.install().click(); await settle()
  globalThis.__modApp.vamRootPath = 'D:/AnotherGame'; await settle()
  ctx.answer(true); await settle()
  assert.equal(ctx.calls.some(c => c.cmd === 'install_scene_browser_mod'), false)
})

test('verified installation needs no warning and runtime status is visible', async t => {
  const ctx = await mount(t, { verified: true, runtime: { browser: 'ok', launch: 'unavailable' } })
  assert.ok(document.body.textContent.includes('sceneBrowserMod.runtimeStatus'))
  ctx.install().click(); await settle()
  assert.equal(ctx.prompts.length, 0)
  assert.deepEqual(ctx.calls.find(c => c.cmd === 'install_scene_browser_mod').args, { vamRoot: 'C:/Game', allowUnverified: false })
})
