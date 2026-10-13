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
globalThis.getComputedStyle = dom.window.getComputedStyle.bind(dom.window)
globalThis.requestAnimationFrame = dom.window.requestAnimationFrame.bind(dom.window)
const { createApp, h, nextTick, reactive } = await import('vue')
const load = componentLoader({
  '@/stores/favorites': moduleUrl('export const useFavoritesStore = () => ({ countFor: () => 0, refresh: async () => {} })'),
  '@/stores/app': moduleUrl('export const useAppStore = () => globalThis.__sceneApp'),
  '@/stores/localLibrary': moduleUrl('export const useLocalLibraryStore = () => ({ revision: 0, packages: [{ id: "Author.Package.4", name: "Package title", creator: "Author", version: 4, size_bytes: 2048, file_path: "/fixture/Author.Package.4.var", resource_types: ["scene"], dependency_count: 1, scan_time: "2026-10-10" }] })'),
  '@/stores/download': moduleUrl('export const useDownloadStore = () => ({ addItems: async () => [] })'),
  '@/components/ShareModal.vue': moduleUrl('export default { render: () => null }'),
  '@/composables/useNotification': moduleUrl('export const useNotification = () => ({ error() {}, success() {}, warning() {}, info() {} })'),
  'vue-router': moduleUrl('export const useRoute = () => ({ path: "/scenes", query: {} }); export const useRouter = () => ({ push() {} })'),
  'vue-i18n': moduleUrl('export const useI18n = () => ({ t: key => key })'),
})
const { invalidateResourceCache } = await import(load('src/utils/resourceCache.ts'))
const { default: GameContentManager } = await import(load('src/components/GameContentManager.vue'))
const { default: PackageDetails } = await import(load('src/components/packages/PackageDetails.vue'))
after(() => {
  dom.window.close()
  for (const key of [...globals, '__sceneApp']) delete globalThis[key]
})
const scene = (key, favorite, available = true) => ({
  key, name: key, alias: null, favorite, available,
  source: { packageId: null, path: `Saves/scene/${key}.json` },
})
async function settle() { await nextTick(); await setImmediate(); await nextTick() }
async function mount(t, catalog = [scene('Favorite', true), scene('Other', false)], write, kind = 'scene') {
  invalidateResourceCache(); mockConvertFileSrc('windows')
  const state = catalog.map(item => ({ ...item, source: { ...item.source } }))
  const requests = []
  globalThis.__sceneApp = reactive({ vamRootPath: '/fixture', searchQuery: '', isScanning: false })
  mockIPC(async (command, args) => {
    requests.push({ command, args })
    if (command === 'list_game_contents') return {
      directory: '/fixture/Saves/scene', warnings: [],
      items: state.filter(item => item.available || item.favorite).map(item => ({ ...item, source: { ...item.source } })),
    }
    if (command === 'launch_game_scene') return null
    if (command === 'get_game_content_image') return '/fixture/preview.jpg'
    if (command === 'get_game_content_detail') return { image: '/fixture/preview.jpg', json: {} }
    if (command === 'get_package_summary') return {
      id: args.packageId, name: 'Package title', creator: 'Author', version: 4, size_bytes: 2048,
      file_path: '/fixture/Author.Package.4.var', resource_types: ['scene'], tags: [],
      dependency_count: 1, dependents_count: 0, content_count: 2, created_time: '', scan_time: '2026-10-10T01:00:00Z',
    }
    if (command === 'list_package_images') return []
    if (command === 'get_package_thumbnail') return '/fixture/preview.jpg'
    if (command === 'get_package_dependency_relations') return { direct: [{ id: 'Author.Dependency.1', tier: 1, status: 'satisfied' }], sub: [] }
    if (command === 'get_reverse_dependencies' || command === 'find_missing_dependencies') return []
    if (command === 'set_game_scene_favorite') {
      if (write) await write(args)
      state.find(item => item.source.path === args.source.path).favorite = args.favorite
      return null
    }
    throw new Error(`Unexpected command: ${command}`)
  })
  const app = createApp({ render: () => h(GameContentManager, { kind }) })
  app.component('RouterLink', { render() { return h('a', this.$slots.default?.()) } })
  app.mount('#app')
  t.after(() => { app.unmount(); clearMocks() })
  await settle()
  return { requests, state, app }
}
const target = name => [...document.querySelectorAll('.card-hit-target')].find(el => el.getAttribute('aria-label') === name)
const favoriteWrites = requests => requests.filter(request => request.command === 'set_game_scene_favorite')

test('scene card click and more share one preview, with favorite changes only inside it', async t => {
  const { requests } = await mount(t)
  const card = target('Favorite')
  card.click(); await settle()
  card.closest('.package-card').querySelector('.image-action').click(); await settle()
  assert.equal(requests.filter(request => request.command === 'get_game_content_detail').length, 0, 'scene details must not load or expose raw JSON')
  assert.equal(document.querySelectorAll('.ui-detail-dialog').length, 1)
  assert.equal(document.querySelector('.detail-body h2').textContent, 'Favorite')
  assert.ok(document.querySelector('.gallery-image img'))
  assert.equal(document.querySelector('.package-detail-modal .path, .package-detail-modal .rename-row, .package-detail-modal .sections, .package-detail-modal pre'), null)
  assert.equal(document.querySelector('.package-hover [aria-label="ui.favorite"]'), null)
  assert.ok(document.querySelector('.package-favorite'), 'card retains its non-interactive favorite status')
  assert.deepEqual(favoriteWrites(requests), [])

  const remove = document.querySelector('.scene-favorite-action')
  assert.equal(remove.getAttribute('aria-label'), 'gameContent.unfavorite')
  remove.click(); await settle()
  assert.equal(target('Favorite'), undefined, 'removed scene disappears from the favorites grid')
  assert.ok(document.querySelector('.package-detail-modal'), 'available scene details remain open after removal')
  assert.ok(document.querySelector('.gallery-controls .scene-favorite-action + .gallery-close'), 'favorite is directly left of close')
  const add = document.querySelector('.scene-favorite-action')
  assert.equal(add.getAttribute('aria-label'), 'gameContent.favorite')
  assert.equal(add.getAttribute('aria-pressed'), 'false')
  add.click(); await settle()
  assert.ok(target('Favorite'))
  assert.equal(document.querySelector('.scene-favorite-action').getAttribute('aria-pressed'), 'true')
  assert.deepEqual(favoriteWrites(requests).map(request => request.args.favorite), [false, true])
})

test('packaged favorites mount the actual common business component, with metadata and dependencies instead of legacy scene tools', async t => {
  const item = scene('Scene alias', true)
  item.source.packageId = 'Author.Package.4'
  const { requests, app } = await mount(t, [item])
  target('Scene alias').click(); await settle(); await settle()
  const instances = []
  function visit(vnode) {
    if (!vnode) return
    if (vnode.component) { instances.push(vnode.component); visit(vnode.component.subTree) }
    if (Array.isArray(vnode.children)) vnode.children.forEach(visit)
  }
  visit(app._instance.subTree)
  assert.ok(instances.some(instance => instance.type === PackageDetails), 'favorites must use the same business component, not a new presentation wrapper')
  const pane = document.querySelector('.package-detail-modal')
  assert.equal(pane.querySelector('.detail-body h2').textContent, 'Package title')
  assert.ok(pane.querySelector('.detail-subtitle').textContent.includes('Author · v4 · 2.0 KB'))
  ;[...pane.querySelectorAll('[role="radio"]')].find(el => el.textContent.startsWith('ui.dependencies')).click(); await settle()
  assert.ok(pane.textContent.includes('Author.Dependency.1'))
  assert.ok(pane.querySelector('.open-location'))
  assert.ok(pane.querySelector('[aria-label="share.btn"]'))
  assert.equal(pane.querySelector('.path, .rename-row, .sections, pre'), null)
  assert.equal(pane.textContent.includes('gameContent.aliasHelp'), false)
  assert.equal(pane.textContent.includes(item.source.path), false)
  assert.equal(requests.some(request => request.command === 'get_game_content_detail'), false)
  document.querySelector('.scene-favorite-action').click(); await settle()
  assert.deepEqual(favoriteWrites(requests)[0].args.source, item.source)
})

test('an unfavorited scene can be added from the all-scenes view through its floating details', async t => {
  const { requests } = await mount(t)
  const filter = [...document.querySelectorAll('.content-toolbar button')].find(el => el.textContent.trim() === 'gameContent.favorites')
  filter.click(); await settle()
  target('Other').click(); await settle()
  assert.equal(document.querySelector('.scene-favorite-action').getAttribute('aria-label'), 'gameContent.favorite')
  document.querySelector('.scene-favorite-action').click(); await settle()
  assert.equal(document.querySelector('.scene-favorite-action').getAttribute('aria-label'), 'gameContent.unfavorite')
  assert.deepEqual(favoriteWrites(requests)[0].args.source, { packageId: null, path: 'Saves/scene/Other.json' })
  assert.equal(favoriteWrites(requests)[0].args.favorite, true)
})

test('a missing favorite can still be removed without trying to load or delete its scene file', async t => {
  const { requests } = await mount(t, [scene('Missing', true, false)])
  target('Missing').click(); await settle()
  assert.equal(document.querySelector('.scene-favorite-action').disabled, false)
  assert.equal(requests.some(request => request.command === 'get_game_content_detail'), false)
  document.querySelector('.scene-favorite-action').click(); await settle()
  assert.equal(document.querySelector('.package-detail-modal'), null)
  assert.deepEqual(favoriteWrites(requests).map(request => request.args.favorite), [false])
})

test('a previously missing scene can use common scene actions after it becomes available again', async t => {
  const { requests, state } = await mount(t, [scene('Returned', true, false)])
  target('Returned').click(); await settle()
  assert.equal(requests.some(request => request.command === 'get_game_content_detail'), false)
  state[0].available = true
  const refresh = [...document.querySelectorAll('.content-toolbar button')].find(el => el.textContent.trim() === 'gameContent.refresh')
  refresh.click(); await settle()
  target('Returned').click(); await settle()
  assert.equal(requests.filter(request => request.command === 'get_game_content_detail').length, 0)
  assert.equal(document.querySelector('.launch-scene').disabled, false)
  assert.ok(document.querySelector('.gallery-image img'))
})

test('a favorite write disables repeated actions, preserves the current state on failure and can be retried', async t => {
  let rejectWrite, attempts = 0
  const { requests } = await mount(t, [scene('Favorite', true)], () => {
    if (++attempts === 1) return new Promise((_, reject) => { rejectWrite = reject })
  })
  target('Favorite').click(); await settle()
  document.querySelector('.scene-favorite-action').click(); await settle()
  assert.equal(document.querySelector('.scene-favorite-action').disabled, true)
  assert.equal(document.querySelector('.gallery-close').disabled, true)
  document.querySelector('.scene-favorite-action').click()
  assert.equal(favoriteWrites(requests).length, 1)
  rejectWrite(new Error('Could not save favorite')); await settle()
  assert.ok(target('Favorite'))
  assert.equal(document.querySelector('.scene-favorite-action').getAttribute('aria-pressed'), 'true')
  assert.equal(document.querySelector('.scene-favorite-action').disabled, false)
  assert.ok(document.querySelector('[role="alert"]').textContent.includes('Could not save favorite'))
  document.querySelector('.scene-favorite-action').click(); await settle()
  assert.deepEqual(favoriteWrites(requests).map(request => request.args.favorite), [false, false])
  assert.equal(document.querySelector('.scene-favorite-action').getAttribute('aria-pressed'), 'false')
})

test('non-scene presets do not offer a favorite action', async t => {
  await mount(t, [scene('Appearance', false)], undefined, 'appearance')
  target('Appearance').click(); await settle()
  assert.ok(document.querySelector('.preset-detail-modal'), 'presets open in the same detail dialog')
  assert.equal(document.querySelector('.scene-favorite-action'), null)
})

test('favorite scene cards use package metadata and launch the exact selected scene for hover and Enter', async t => {
  const first = scene('First', true), second = scene('Second', true)
  first.source.packageId = second.source.packageId = 'Author.Package.4'
  const { requests } = await mount(t, [first, second])
  const card = target('Second').closest('.package-card')
  assert.match(card.querySelector('.package-meta').textContent, /Author\s*· v4/)
  assert.match(card.querySelector('.package-meta').textContent, /2\.0\s*KB/)
  card.querySelector('.package-play').click(); await settle(); await settle()
  assert.equal(requests.find(request => request.command === 'launch_game_scene')?.args.source.path, second.source.path)
  target('First').dispatchEvent(new dom.window.KeyboardEvent('keydown', { key: 'Enter', bubbles: true })); await settle(); await settle()
  assert.deepEqual(requests.filter(request => request.command === 'launch_game_scene').map(request => request.args.source.path), [second.source.path, first.source.path])
})

test('local scenes use the shared menu without package-only actions and Space opens expanded details', async t => {
  await mount(t)
  const card = target('Favorite')
  card.closest('.package-card').dispatchEvent(new dom.window.MouseEvent('contextmenu', { bubbles: true })); await settle()
  const menu = document.querySelector('.ui-menu')
  assert.ok(menu)
  assert.match(menu.textContent, /gameContent.launchScene/)
  assert.doesNotMatch(menu.textContent, /share.btn|ui.allDependencies|ui.toTrash/)
  menu.dispatchEvent(new dom.window.KeyboardEvent('keydown', { key: 'Escape', bubbles: true })); await settle()
  card.dispatchEvent(new dom.window.KeyboardEvent('keydown', { key: ' ', bubbles: true })); await settle()
  assert.ok(document.querySelector('.package-detail-modal'))
  assert.equal(document.querySelector('.ui-inspector'), null, 'details never use the side panel')
})
