import assert from 'node:assert/strict'
import { after, test } from 'node:test'
import { setImmediate } from 'node:timers/promises'
import { JSDOM } from 'jsdom'
import { mockIPC, clearMocks, mockConvertFileSrc } from '@tauri-apps/api/mocks'
import { componentLoader } from './helpers/vue.mjs'
import { moduleUrl } from './helpers/typescript.mjs'

const dom = new JSDOM('<div id="app"></div>', { url: 'http://localhost', pretendToBeVisual: true })
const globals = ['window', 'document', 'Element', 'HTMLElement', 'SVGElement', 'Node', 'localStorage']
for (const key of globals) globalThis[key] = dom.window[key]
globalThis.requestAnimationFrame = dom.window.requestAnimationFrame.bind(dom.window)
globalThis.getComputedStyle = dom.window.getComputedStyle.bind(dom.window)
const { createApp, h, nextTick, reactive, ref } = await import('vue')
const load = componentLoader({
  '@/stores/favorites': moduleUrl('export const useFavoritesStore = () => ({ countFor: () => 0, refresh: async () => {} })'),
  '@/stores/app': moduleUrl('export const useAppStore = () => globalThis.__extractionApp'),
  'vue-i18n': moduleUrl('export const useI18n = () => ({ t: key => key })'),
})
const { invalidateResourceCache } = await import(load('src/utils/resourceCache.ts'))
const { default: Extractor } = await import(load('src/components/PackageSceneExtractor.vue'))
after(() => {
  dom.window.close()
  for (const key of [...globals, '__extractionApp', 'requestAnimationFrame', 'getComputedStyle']) delete globalThis[key]
})
const scenes = ['First', 'Second'].map((name, i) => ({
  key: name, name, source: { packageId: 'Author.Package.1', path: `Saves/scene/${name}.json` }, favorite: i === 1, available: true,
}))
const detail = {
  revision: 'revision', image: null,
  characters: [{ index: 2, name: 'Person', character: 'Person', clothing: 2, hair: 1, morphs: 3, error: null }],
}
async function settle() { await nextTick(); await setImmediate(); await nextTick() }
async function mount(t, handler, props = { packageId: 'Author.Package.1' }) {
  invalidateResourceCache(); mockConvertFileSrc('windows')
  globalThis.__extractionApp = reactive({ vamRootPath: 'C:/fixture' })
  const component = ref(null), requests = [], busy = []
  mockIPC(async (command, args) => {
    requests.push({ command, args })
    if (handler) { const result = handler(command, args); if (result !== undefined) return await result }
    if (command === 'list_package_scene_contents') return { items: structuredClone(scenes), warnings: [] }
    if (command === 'get_game_content_detail') return structuredClone(detail)
    if (command === 'save_game_scene_appearance') return 'Custom/Atom/Person/Appearance/Preset_New.vap'
    if (command === 'set_game_scene_favorite') return null
    throw new Error(`Unexpected command: ${command}`)
  })
  const app = createApp({ render: () => h(Extractor, { ...props, ref: component, onBusyChange: value => busy.push(value) }, {
    actions: ({ extract, disabled }) => h('button', { id: 'extract', disabled, onClick: extract }, 'extract'),
  }) })
  app.component('RouterLink', { render() { return h('a', this.$slots.default?.()) } })
  app.mount('#app')
  t.after(() => { app.unmount(); clearMocks(); localStorage.clear() })
  await settle()
  return { requests, component, busy }
}

test('extract opens a separate modal, and cancel creates no preset', async t => {
  const { requests, busy } = await mount(t)
  assert.equal(document.querySelector('.extraction-form'), null)
  document.querySelector('#extract').click(); await settle()
  assert.ok(document.querySelector('[role="dialog"] .extraction-form'))
  assert.equal(document.querySelector('#app .extraction-form'), null, 'form is teleported outside the inspector')
  assert.equal(document.querySelector('.extraction-save').textContent.trim(), 'gameContent.addAppearance')
  document.querySelector('.extraction-cancel').click(); await settle()
  assert.equal(document.querySelector('[role="dialog"]'), null)
  assert.equal(requests.some(r => r.command === 'save_game_scene_appearance'), false)
  assert.deepEqual(busy, [true, false])
})

test('add uses the selected scene and character, with native form submission from the footer', async t => {
  const { requests } = await mount(t)
  document.querySelector('#extract').click(); await settle()
  const select = document.querySelector('.extraction-panel .extract-select')
  select.value = 'Second'; select.dispatchEvent(new dom.window.Event('change', { bubbles: true })); await settle()
  const name = document.querySelector('.extraction-form input')
  name.value = 'New'; name.dispatchEvent(new dom.window.Event('input', { bubbles: true })); await settle()
  const submit = document.querySelector('.extraction-save')
  assert.equal(submit.getAttribute('form'), document.querySelector('.extraction-form').id)
  submit.click(); await settle()
  const saved = requests.find(r => r.command === 'save_game_scene_appearance')
  assert.deepEqual(saved.args, { vamRoot: 'C:/fixture', source: scenes[1].source, revision: 'revision', atomIndex: 2, name: 'New', outputDir: 'C:/fixture\\Custom\\Atom\\Person\\Appearance' })
  assert.ok(document.querySelector('.saved'))
  assert.equal(document.querySelector('.extraction-save').disabled, true, 'a successful save cannot be accidentally repeated')
})

test('saving blocks dismissal and duplicate requests; errors leave the form available for retry', async t => {
  let rejectSave, attempts = 0
  const { requests } = await mount(t, command => {
    if (command === 'save_game_scene_appearance' && ++attempts === 1) return new Promise((_, reject) => { rejectSave = reject })
  })
  document.querySelector('#extract').click(); await settle()
  document.querySelector('.extraction-save').click(); await settle()
  assert.equal(document.querySelector('.extraction-cancel').disabled, true)
  document.dispatchEvent(new dom.window.KeyboardEvent('keydown', { key: 'Escape', bubbles: true })); await settle()
  assert.ok(document.querySelector('[role="dialog"]'))
  document.querySelector('.extraction-save').click()
  assert.equal(requests.filter(r => r.command === 'save_game_scene_appearance').length, 1)
  rejectSave(new Error('Save failed')); await settle()
  assert.ok(document.querySelector('[role="alert"]').textContent.includes('Save failed'))
  assert.equal(document.querySelector('.extraction-save').disabled, false)
  document.querySelector('.extraction-save').click(); await settle()
  assert.equal(requests.filter(r => r.command === 'save_game_scene_appearance').length, 2)
})

test('no characters cannot create a preset, and cancelling during loading keeps the modal closed', async t => {
  let resolveDetail
  await mount(t, command => command === 'get_game_content_detail' ? new Promise(resolve => { resolveDetail = resolve }) : undefined)
  document.querySelector('#extract').click(); await settle()
  assert.equal(document.querySelector('.extraction-save').disabled, true)
  document.querySelector('.extraction-cancel').click(); await settle()
  resolveDetail({ ...detail, characters: [] }); await settle()
  assert.equal(document.querySelector('[role="dialog"]'), null)
  document.querySelector('#extract').click(); await settle()
  assert.ok(document.querySelector('.extraction-panel').textContent.includes('gameContent.noCharacters'))
  assert.equal(document.querySelector('.extraction-save').disabled, true)
})

test('favorites apply to the selected scene in a multi-scene package', async t => {
  const { component, requests } = await mount(t)
  await component.value.toggleFavorite(); await settle()
  assert.equal(component.value.selectedScene.favorite, true)
  const select = document.querySelector('.scene-extractor .extract-select')
  select.value = 'Second'; select.dispatchEvent(new dom.window.Event('change', { bubbles: true })); await settle()
  assert.equal(component.value.selectedScene.favorite, true)
  await component.value.toggleFavorite(); await settle()
  assert.deepEqual(requests.filter(r => r.command === 'set_game_scene_favorite').map(r => r.args), [
    { vamRoot: 'C:/fixture', source: scenes[0].source, favorite: true },
    { vamRoot: 'C:/fixture', source: scenes[1].source, favorite: false },
  ])
})

test('local favorite scenes can extract without requiring a package catalog', async t => {
  const source = { packageId: null, path: 'Saves/scene/Local.json' }
  const { requests } = await mount(t, undefined, { source, sceneName: 'Local' })
  document.querySelector('#extract').click(); await settle()
  assert.equal(requests.some(r => r.command === 'list_package_scene_contents'), false)
  assert.deepEqual(requests.find(r => r.command === 'get_game_content_detail').args.source, source)
  assert.ok(document.querySelector('.extraction-form'))
})

test('launch progress separates delivery from loading, and long loading is informational', async t => {
  let finish, progress
  const { component } = await mount(t, (command, args) => {
    if (command === 'launch_game_scene') {
      progress = args.onProgress
      return new Promise(resolve => { finish = resolve })
    }
  })
  const launch = component.value.launch(); await settle()
  assert.ok(document.body.textContent.includes('sceneLaunch.opening'))
  assert.equal(document.querySelector('.scene-extractor').open, true, 'launch progress opens the options disclosure')
  progress.onmessage('sending'); await settle()
  assert.ok(document.body.textContent.includes('sceneLaunch.sending'))
  progress.onmessage('loading'); await settle()
  assert.ok(document.body.textContent.includes('sceneLaunch.loading'))
  finish('still_loading'); await launch; await settle()
  assert.ok(document.body.textContent.includes('sceneLaunch.stillLoading'))
  assert.equal(document.querySelector('[role="alert"]'), null)
  assert.equal(document.body.textContent.includes('gameContent.sceneLoaded'), false)
})

test('launch errors translate codes and keep raw details collapsed', async t => {
  const { component } = await mount(t, command => {
    if (command === 'launch_game_scene') return Promise.reject({ code: 'package_disabled', detail: 'Diagnostic detail' })
  })
  await component.value.launch(); await settle()
  assert.equal(document.querySelector('[role="alert"]').textContent, 'sceneLaunch.errors.package_disabled')
  const details = [...document.querySelectorAll('details')].find(el => el.textContent.includes('Diagnostic detail') && el.querySelector('summary')?.textContent === 'sceneLaunch.details')
  assert.ok(details)
  assert.equal(details.open, false)
})
