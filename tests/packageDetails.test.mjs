import { componentLoader } from './helpers/vue.mjs'
import assert from 'node:assert/strict'
import { readFile } from 'node:fs/promises'
import { after, test } from 'node:test'
import { setImmediate } from 'node:timers/promises'
import { JSDOM } from 'jsdom'
import { parse, compileScript } from '@vue/compiler-sfc'
import ts from 'typescript'
import { clearMocks, mockIPC, mockConvertFileSrc } from '@tauri-apps/api/mocks'
import { compileModule, moduleUrl } from './helpers/typescript.mjs'

const dom = new JSDOM('<div id="app"></div>', { url: 'http://localhost', pretendToBeVisual: true })
for (const key of ['window', 'document', 'Element', 'HTMLElement', 'SVGElement', 'Node']) {
  globalThis[key] = key === 'window' ? dom.window : dom.window[key]
}
globalThis.requestAnimationFrame = dom.window.requestAnimationFrame.bind(dom.window)
const { createApp, h, nextTick, ref } = await import('vue')
after(() => {
  dom.window.close()
  for (const key of ['window', 'document', 'Element', 'HTMLElement', 'SVGElement', 'Node', 'requestAnimationFrame']) delete globalThis[key]
})

const actionPanelStub = moduleUrl(`import { h } from ${JSON.stringify(import.meta.resolve('vue'))}
export default { setup(_, { slots }) { return () => h('div', slots.actions?.({ disabled: true, panel: 'test-extraction' })) } }`)
const emptyStub = moduleUrl('export default { render: () => null }')
const bytesUrl = await compileModule(new URL('../src/utils/bytes.ts', import.meta.url))
const replacements = {
  '@/stores/favorites': moduleUrl('export const useFavoritesStore = () => ({ countFor: () => 0, refresh: async () => {} })'),
  '@/utils/hubFiles': await compileModule(new URL('../src/utils/hubFiles.ts', import.meta.url), { './bytes': bytesUrl }),
  '@/utils/bytes': bytesUrl,
  '@/components/common/EmptyState.vue': emptyStub,
  '@/components/ShareModal.vue': emptyStub,
  '@/components/CleanupToolbar.vue': emptyStub,
  '@/stores/cleanup': moduleUrl('export const useCleanupStore = () => ({ selected: new Set(), protectedIds: new Set(), markMap: new Map() })'),
  '@/components/PackageSceneExtractor.vue': actionPanelStub,
  '@/composables/useNotification': moduleUrl('export const useNotification = () => ({ error(message) { globalThis.detailNotifications?.push(message) }, success() {}, warning() {}, info() {} })'),
  '@/stores/download': moduleUrl('export const useDownloadStore = () => ({ addItems: async () => [] })'),
  'vue-i18n': moduleUrl('export const useI18n = () => ({ t: key => key })'),
  'vue-router': moduleUrl('export const useRouter = () => ({ push() {} })'),
}
const loadComponent = componentLoader(replacements)
const { default: ResourceDisplay } = await import(loadComponent('src/components/ResourceDisplay.vue'))
const { default: SharedDetails } = await import(loadComponent('src/components/packages/PackageDetails.vue'))
const { invalidateResourceCache } = await import(loadComponent('src/utils/resourceCache.ts'))
const loadWithSceneActions = componentLoader({
  ...replacements,
  '@/components/PackageSceneExtractor.vue': undefined,
  '@/stores/favorites': moduleUrl('export const useFavoritesStore = () => ({ countFor: () => 0, refresh: async () => {} })'),
  '@/stores/app': moduleUrl('export const useAppStore = () => ({ vamRootPath: "C:/fixture" })'),
})
const { default: SceneResourceDisplay } = await import(loadWithSceneActions('src/components/ResourceDisplay.vue'))

test('an open package detail uses refreshed paths and metadata after a move or replacement', async t => {
  const original = {
    id: 'Author.Package.1', creator: 'Author', name: 'Package', version: 1,
    file_path: 'C:/fixture/old/Author.Package.1.var', size_bytes: 1024, resource_types: ['other'],
    dependency_count: 0, dependents_count: 0, content_count: 0, tags: [],
    created_time: '2026-10-07T01:00:00Z', scan_time: '2026-10-07T01:00:00Z',
  }
  const packages = ref([original])
  const revealedPaths = []
  const imageRequests = []
  mockConvertFileSrc('windows')
  mockIPC((command, args) => {
    if (command === 'open_package_in_explorer') { revealedPaths.push(args.filePath); return }
    if (command === 'list_package_images') { imageRequests.push(args.packageId); return [] }
    if (command === 'get_package_thumbnails') return args.packageIds.map(packageId => ({ packageId, path: null, cached: true }))
    if (command === 'get_package_thumbnail') return null
    if (command === 'get_package_dependency_relations') return { direct: [], sub: [] }
    if (command === 'get_reverse_dependencies' || command === 'find_missing_dependencies') return []
    throw new Error(`Unexpected command: ${command}`)
  })
  const app = createApp({
    setup: () => () => h(ResourceDisplay, {
      packages: packages.value, viewMode: 'large-card', resultCountLabel: '', emptyTitle: '', emptyDescription: '',
    }, { browser: ({ selectPackage, openDetails }) => [
      h('button', { id: 'select', onClick: () => selectPackage(packages.value[0]) }, 'select'),
      h('button', { id: 'open', onClick: () => openDetails(packages.value[0]) }, 'open'),
    ] }),
  })
  app.config.globalProperties.$t = key => key
  globalThis.getComputedStyle = dom.window.getComputedStyle.bind(dom.window)
  app.mount('#app')
  t.after(() => { app.unmount(); clearMocks(); delete globalThis.getComputedStyle })
  document.querySelector('#select').click()
  await nextTick()
  document.querySelector('.open-location').click()
  await setImmediate()
  assert.deepEqual(revealedPaths, [original.file_path])

  const moved = { ...original, file_path: 'C:/fixture/场景 with spaces/Author.Package.1.var', size_bytes: 2048 }
  packages.value = [moved]
  await nextTick()
  await setImmediate()
  document.querySelector('.open-location').click()
  await setImmediate()
  assert.equal(revealedPaths.at(-1), moved.file_path)
  assert.ok(document.querySelector('.detail-panel').textContent.includes('2.0 KB'))
  assert.equal(imageRequests.length, 0, 'collapsed details must defer the image list')

  packages.value = [{ ...moved, scan_time: '2026-10-07T02:00:00Z', size_bytes: 4096 }]
  await nextTick()
  await setImmediate()
  assert.ok(document.querySelector('.detail-panel').textContent.includes('4.0 KB'))
  assert.equal(imageRequests.length, 0, 'metadata refresh must not eagerly scan archive images')
  assert.equal(document.querySelector('.package-detail-modal'), null, 'selecting for card actions keeps the details hidden')
  assert.ok(document.querySelector('.package-detail-headless[hidden]'))
  document.querySelector('#open').click()
  await nextTick(); await setImmediate()
  assert.ok(document.querySelector('.package-detail-modal'), 'opening details uses the shared detail dialog')
  assert.equal(document.querySelector('.ui-inspector'), null, 'no side panel')
  document.querySelector('.gallery-close').click()
  await nextTick()
  assert.equal(document.querySelector('.package-detail-modal'), null)
  assert.equal(document.querySelector('.detail-panel'), null, 'closing details deselects the package')
  assert.ok(document.querySelector('#select'), 'closing details leaves the resource browser mounted')
})

test('scene package favorites sit left of close, non-scenes cannot favorite, and extraction uses its own modal', async t => {
  globalThis.localStorage = dom.window.localStorage
  globalThis.getComputedStyle = dom.window.getComputedStyle.bind(dom.window)
  globalThis.IntersectionObserver = class { observe() {} unobserve() {} disconnect() {} }
  const pkg = {
    id: 'Author.Scene.1', creator: 'Author', name: 'Scene', version: 1,
    file_path: 'C:/fixture/Author.Scene.1.var', size_bytes: 1024, resource_types: ['scene'],
    dependency_count: 0, dependents_count: 0, content_count: 1, tags: [],
    created_time: '', scan_time: '',
  }
  const other = { ...pkg, id: 'Author.Clothing.1', name: 'Clothing', resource_types: ['clothing'] }
  const source = { packageId: pkg.id, path: 'Saves/scene/Scene.json' }
  const writes = []
  mockConvertFileSrc('windows')
  mockIPC((command, args) => {
    if (command === 'list_package_scene_contents') return { items: args.packageId === pkg.id ? [{ source, key: 'scene', name: 'Scene', favorite: false, available: true }] : [], warnings: [] }
    if (command === 'get_game_content_detail') return { revision: 'revision', image: null, characters: [] }
    if (command === 'set_game_scene_favorite') { writes.push(args); return null }
    if (command === 'list_package_images' || command === 'get_reverse_dependencies' || command === 'find_missing_dependencies') return []
    if (command === 'get_package_thumbnails') return args.packageIds.map(packageId => ({ packageId, path: null, cached: true }))
    if (command === 'get_package_thumbnail') return null
    if (command === 'get_package_dependency_relations') return { direct: [], sub: [] }
    throw new Error(`Unexpected command: ${command}`)
  })
  const app = createApp({ render: () => h(SceneResourceDisplay, {
    packages: [pkg, other], viewMode: 'large-card', resultCountLabel: '', emptyTitle: '', emptyDescription: '',
  }) })
  app.component('RouterLink', { render() { return h('a', this.$slots.default?.()) } })
  app.mount('#app')
  t.after(() => { app.unmount(); clearMocks(); delete globalThis.localStorage; delete globalThis.getComputedStyle; delete globalThis.IntersectionObserver })
  const settle = async () => { await nextTick(); await setImmediate(); await nextTick() }
  const card = label => [...document.querySelectorAll('.card-hit-target')].find(el => el.getAttribute('aria-label') === label)
  card('Scene').click(); await settle()
  assert.ok(document.querySelector('.package-detail-modal .gallery-controls .scene-favorite-action + .gallery-close'))
  document.querySelector('.scene-favorite-action').click(); await settle()
  assert.deepEqual(writes, [{ vamRoot: 'C:/fixture', source, favorite: true }])
  assert.equal(document.querySelector('.scene-favorite-action').getAttribute('aria-pressed'), 'true')
  document.querySelector('.package-actions [aria-haspopup="menu"]').click(); await settle()
  Array.from(document.querySelectorAll('[role="menuitem"]')).find(el => el.textContent.includes('gameContent.extractPreset')).click(); await settle()
  assert.ok(document.querySelector('.scene-extraction-modal'), 'extraction opens its own dialog')
  assert.ok(document.querySelector('.package-detail-modal'), 'the details stay underneath')
  document.querySelector('.extraction-cancel').click(); await settle()
  card('Clothing').click(); await settle()
  assert.equal(document.querySelector('.scene-favorite-action'), null)
})

test('details paint before dependencies resolve, cache reopen, and defer gallery and reverse queries until requested', async t => {
  invalidateResourceCache()
  globalThis.getComputedStyle = dom.window.getComputedStyle.bind(dom.window)
  const shown = ref(true), expanded = ref(false)
  const pkg = ref({ id: 'Perf.Scene.1', name: 'Immediate title', creator: 'Author', resource_types: ['scene'], scan_time: 'first', dependency_count: 1, dependents_count: 1 })
  const calls = [], pending = []
  mockIPC((command, args) => {
    calls.push(command)
    if (command === 'get_package_dependency_relations') return new Promise(resolve => pending.push(resolve))
    if (command === 'list_package_images') return [{ path: 'Saves/scene/Photo.jpg', size_bytes: 100 }]
    if (command === 'get_package_image') return 'https://fixture/preview.jpg'
    if (command === 'get_reverse_dependencies') return [{ id: 'Other.Scene.1' }]
    if (command === 'get_package_thumbnails') return args.packageIds.map(packageId => ({ packageId, path: null, cached: true }))
    assert.fail(`Unexpected command ${command}`)
  })
  const app = createApp({ render: () => shown.value ? h(SharedDetails, {
    item: pkg.value, image: 'https://fixture/cover.jpg', expanded: expanded.value,
  }) : null })
  app.mount('#app')
  t.after(() => { app.unmount(); invalidateResourceCache(); clearMocks(); delete globalThis.getComputedStyle })
  const settle = async () => { await nextTick(); await setImmediate(); await nextTick() }
  assert.equal(document.querySelector('.detail-body h2').textContent, 'Immediate title')
  assert.ok(document.querySelector('.detail-preview img'))
  assert.ok(document.querySelector('.package-actions'))
  assert.deepEqual(calls, ['get_package_dependency_relations'])
  pending.shift()({ direct: [{ id: 'Missing.Asset.1', tier: 1, status: 'missing', required_version: '1', installed_version: null }], sub: [] })
  await settle()
  assert.ok(document.querySelector('.dependency-row.missing'))
  shown.value = false; await settle(); shown.value = true; await settle()
  assert.deepEqual(calls, ['get_package_dependency_relations'], 'reopening should make no IPC request')
  expanded.value = true; await settle(); await settle()
  assert.ok(document.querySelector('.package-detail-modal'))
  assert.deepEqual(calls, ['get_package_dependency_relations', 'list_package_images', 'get_package_image'])
  const reverseTab = [...document.querySelectorAll('[role="radio"]')].find(el => el.textContent.includes('ui.dependents'))
  reverseTab.click(); await settle()
  assert.deepEqual(calls.slice(-2), ['get_reverse_dependencies', 'get_package_thumbnails'])
  assert.ok(document.body.textContent.includes('Other.Scene.1'))
  pkg.value = { ...pkg.value, scan_time: 'replacement' }; await settle()
  assert.equal(calls.filter(command => command === 'get_package_dependency_relations').length, 2)
  assert.equal(calls.filter(command => command === 'list_package_images').length, 2)
  pending.shift()({ direct: [], sub: [] }); await settle()
  assert.equal(calls.includes('find_missing_dependencies'), false)
})

test('a failed detail request shows the error and reopening retries instead of caching failure', async t => {
  invalidateResourceCache()
  const shown = ref(true), calls = []
  mockIPC(command => {
    calls.push(command)
    assert.equal(command, 'get_package_dependency_relations')
    if (calls.length === 1) throw new Error('database unavailable')
    return { direct: [], sub: [] }
  })
  const app = createApp({ render: () => shown.value ? h(SharedDetails, {
    item: { id: 'Retry.Scene.1', name: 'Retry scene', resource_types: ['scene'], scan_time: 'first' },
    image: 'https://fixture/image.jpg',
  }) : null })
  app.mount('#app'); t.after(() => { app.unmount(); invalidateResourceCache(); clearMocks() })
  await nextTick(); await setImmediate(); await nextTick()
  assert.ok(document.querySelector('[role="alert"]'))
  shown.value = false; await nextTick(); shown.value = true; await nextTick(); await setImmediate(); await nextTick()
  assert.equal(calls.length, 2)
  assert.equal(document.querySelector('[role="alert"]'), null)
})

test('dependency names copy without versions, previews resolve installed versions, and navigation returns one level at a time', async t => {
  invalidateResourceCache()
  globalThis.getComputedStyle = dom.window.getComputedStyle.bind(dom.window)
  const navigatorDescriptor = Object.getOwnPropertyDescriptor(globalThis, 'navigator')
  const copied = []
  Object.defineProperty(globalThis, 'navigator', { configurable: true, value: { clipboard: { writeText: async text => copied.push(text) } } })
  const packageItem = (id, name) => ({ id, name, creator: id.split('.')[0], resource_types: ['clothing'], file_path: `C:/fixture/${id}.var`, dependency_count: 1 })
  const root = packageItem('Author.Root.Scene.1', 'Root scene')
  const child = packageItem('Author.Child.2', 'Child')
  const grandchild = packageItem('Author.Grandchild.4', 'Grandchild')
  const old = packageItem('Author.Old.3', 'Older installed package')
  const relation = (id, version, status = 'satisfied') => ({ id, installed_version: version, status, tier: 1, required_version: id.split('.').at(-1) })
  const relations = {
    [root.id]: [relation('Author.Child.latest', 2), relation('Missing.Asset.With.Dots.9', null, 'missing'), relation('Author.Old.10', 3, 'lower_version')],
    [child.id]: [relation(grandchild.id, 4)],
    [grandchild.id]: [relation(root.id, 1)],
  }
  const summaries = { [child.id]: child, [grandchild.id]: grandchild, [root.id]: root, [old.id]: old }
  const calls = []
  mockIPC((command, args) => {
    calls.push({ command, args })
    if (command === 'get_package_dependency_relations') return { direct: relations[args.packageId] || [], sub: [] }
    if (command === 'get_package_summary') return summaries[args.packageId]
    if (command === 'get_package_thumbnail') return `https://fixture/${args.packageId}.jpg`
    if (command === 'get_package_thumbnails') return args.packageIds.map(packageId => ({ packageId, path: `https://fixture/${packageId}.jpg`, cached: true }))
    if (command === 'list_package_images' || command === 'get_reverse_dependencies') return []
    throw new Error(`Unexpected command: ${command}`)
  })
  const expanded = ref(true), item = ref(root)
  const app = createApp({ render: () => h(SharedDetails, {
    item: item.value, image: 'https://fixture/root.jpg', expanded: expanded.value,
    'onUpdate:expanded': value => { expanded.value = value },
  }) })
  app.mount('#app')
  t.after(() => {
    app.unmount(); invalidateResourceCache(); clearMocks(); delete globalThis.getComputedStyle
    if (navigatorDescriptor) Object.defineProperty(globalThis, 'navigator', navigatorDescriptor)
    else delete globalThis.navigator
  })
  const settle = async () => { await nextTick(); await setImmediate(); await nextTick() }
  const row = id => [...document.querySelectorAll('.dependency-row')].find(el => el.querySelector('strong').textContent === id)
  const open = async id => { row(id).querySelector('.dependency-details').click(); await settle(); await settle() }
  const title = () => document.querySelector('.detail-title h2').textContent
  const back = async () => { document.querySelector('.dependency-back').click(); await settle(); await settle() }
  const directTab = async () => { [...document.querySelectorAll('[role="radio"]')].find(el => el.textContent.startsWith('ui.dependencies')).click(); await settle() }
  await settle()
  await directTab()
  document.querySelector('.copy-package-name').click()
  row('Author.Child.latest').querySelector('.dependency-copy').click()
  row('Missing.Asset.With.Dots.9').querySelector('.dependency-copy').click()
  await settle()
  assert.deepEqual(copied, ['Author.Root.Scene', 'Author.Child', 'Missing.Asset.With.Dots'])
  assert.equal(row('Author.Child.latest').querySelector('img').src, 'https://fixture/Author.Child.2.jpg')
  assert.equal(row('Author.Old.10').querySelector('img').src, 'https://fixture/Author.Old.3.jpg')
  assert.equal(row('Missing.Asset.With.Dots.9').querySelector('img'), null)
  row('Author.Old.10').querySelector('img').dispatchEvent(new dom.window.Event('error'))
  await settle()
  assert.equal(row('Author.Old.10').querySelector('img'), null, 'broken previews fall back to an icon')
  await open('Author.Child.latest')
  assert.equal(title(), 'Child')
  assert.ok(document.activeElement.classList.contains('dependency-back'))
  await directTab(); await open(grandchild.id)
  assert.equal(title(), 'Grandchild')
  await directTab(); await open(root.id)
  assert.equal(title(), 'Root scene', 'cycles are navigable without losing their history')
  await back(); assert.equal(title(), 'Grandchild')
  await back(); assert.equal(title(), 'Child')
  await back(); assert.equal(title(), 'Root scene')
  assert.equal(document.querySelector('.dependency-back'), null)
  assert.ok(document.activeElement.classList.contains('copy-package-name'))
  assert.equal(document.querySelector('.gallery-image img').src, 'https://fixture/root.jpg')
  await open('Author.Old.10')
  assert.equal(title(), 'Older installed package')
  assert.equal(calls.filter(call => call.command === 'get_package_summary').at(-1).args.packageId, old.id)
  await back()
  assert.equal(row('Missing.Asset.With.Dots.9').querySelector('.dependency-details'), null, 'missing dependencies only offer completion')
  assert.ok(row('Missing.Asset.With.Dots.9').querySelector('.resolve-button'))
  assert.ok(row('Author.Old.10').querySelector('.dependency-details'), 'outdated dependencies are installed and keep their details')
  assert.equal(calls.some(call => call.args?.packageId === 'Missing.Asset.With.Dots.9'), false, 'missing resources must not invoke local archive operations')
  await directTab()
  await open('Author.Child.latest')
  assert.equal(title(), 'Child')
  assert.ok(document.querySelector('.package-detail-modal .dependency-back'))
  await back()
  assert.ok(row('Author.Child.latest'), 'returning restores the dependency tab in the full details view')
  assert.ok(document.querySelector('.package-detail-modal'))
  await open('Author.Child.latest')
  item.value = packageItem('Other.New.1', 'New selection'); await settle()
  assert.equal(title(), 'New selection')
  assert.equal(document.querySelector('.dependency-back'), null, 'a new external selection starts a new history')
})

test('dependency detail errors can retry and delayed results cannot navigate a newer selection', async t => {
  invalidateResourceCache()
  const root = { id: 'Race.Root.1', name: 'Root', creator: 'Race', resource_types: [] }
  const child = { ...root, id: 'Race.Child.1', name: 'Child' }
  const item = ref(root)
  let requests = 0, resolvePending
  globalThis.detailNotifications = []
  mockIPC((command, args) => {
    if (command === 'get_package_dependency_relations') return { direct: args.packageId === root.id ? [{ id: child.id, installed_version: 1, status: 'satisfied', tier: 1 }] : [], sub: [] }
    if (command === 'get_package_thumbnails') return args.packageIds.map(packageId => ({ packageId, path: null, cached: true }))
    if (command === 'get_package_summary') {
      if (++requests === 1) throw new Error('temporary database failure')
      return new Promise(resolve => { resolvePending = resolve })
    }
    throw new Error(`Unexpected command: ${command}`)
  })
  const app = createApp({ render: () => h(SharedDetails, { item: item.value, image: 'https://fixture/root.jpg' }) })
  app.mount('#app')
  t.after(() => { app.unmount(); clearMocks(); invalidateResourceCache(); delete globalThis.detailNotifications })
  const settle = async () => { await nextTick(); await setImmediate(); await nextTick() }
  await settle()
  document.querySelector('.dependency-details').click(); await settle()
  assert.equal(document.querySelector('.detail-title h2').textContent, 'Root')
  assert.deepEqual(globalThis.detailNotifications, ['ui.dependencyDetailsFailed'])
  document.querySelector('.dependency-details').click(); await settle()
  assert.equal(requests, 2)
  assert.equal(document.querySelector('.dependency-details').getAttribute('aria-busy'), 'true')
  item.value = { ...root, id: 'Race.New.1', name: 'New selection' }; await settle()
  resolvePending(child); await settle()
  assert.equal(document.querySelector('.detail-title h2').textContent, 'New selection')
  assert.equal(document.querySelector('.dependency-back'), null)
})

test('reselecting a resource and invoking its card menu resets dependency navigation before acting', async t => {
  invalidateResourceCache()
  globalThis.getComputedStyle = dom.window.getComputedStyle.bind(dom.window)
  const root = { id: 'Cards.Root.1', name: 'Card root', creator: 'Cards', resource_types: ['clothing'], file_path: 'C:/root.var', dependency_count: 1 }
  const child = { ...root, id: 'Cards.Child.1', name: 'Card child', file_path: 'C:/child.var' }
  const paths = []
  mockIPC((command, args) => {
    if (command === 'get_package_dependency_relations') return { direct: args.packageId === root.id ? [{ id: child.id, status: 'satisfied', installed_version: 1 }] : [], sub: [] }
    if (command === 'get_package_summary') return child
    if (command === 'get_package_thumbnails') return args.packageIds.map(packageId => ({ packageId, path: null, cached: true }))
    if (command === 'get_package_thumbnail') return null
    if (command === 'open_package_in_explorer') { paths.push(args.filePath); return }
    throw new Error(`Unexpected command: ${command}`)
  })
  const app = createApp({ render: () => h(ResourceDisplay, { packages: [root], viewMode: 'large-card', resultCountLabel: '', emptyTitle: '', emptyDescription: '' }) })
  app.mount('#app')
  t.after(() => { app.unmount(); clearMocks(); invalidateResourceCache(); delete globalThis.getComputedStyle })
  const settle = async () => { await nextTick(); await setImmediate(); await nextTick() }
  await settle()
  const directTab = async () => { [...document.querySelectorAll('[role="radio"]')].find(el => el.textContent.startsWith('ui.dependencies')).click(); await settle() }
  document.querySelector('.card-hit-target').click(); await settle()
  assert.ok(document.querySelector('.package-detail-modal'), 'clicking a card opens the detail dialog')
  await directTab()
  document.querySelector('.dependency-details').click(); await settle()
  assert.equal(document.querySelector('.detail-title h2').textContent, 'Card child')
  document.querySelector('.card-hit-target').click(); await settle()
  assert.equal(document.querySelector('.detail-title h2').textContent, 'Card root')
  assert.equal(document.querySelector('.dependency-back'), null)
  await directTab()
  document.querySelector('.dependency-details').click(); await settle()
  document.querySelector('.package-card').dispatchEvent(new dom.window.MouseEvent('contextmenu', { bubbles: true, cancelable: true, clientX: 30, clientY: 30 })); await settle()
  assert.equal(document.querySelector('.detail-title h2').textContent, 'Card root')
  const open = [...document.querySelectorAll('[role="menuitem"]')].find(el => el.textContent.includes('packages.openLocation'))
  open.click(); await settle()
  assert.deepEqual(paths, ['C:/root.var'], 'card actions must act on the card, not the last dependency')
})

test('the contents tab lists package files on demand, filters by type and search, and fullscreen Esc only leaves fullscreen', async t => {
  invalidateResourceCache()
  globalThis.getComputedStyle = dom.window.getComputedStyle.bind(dom.window)
  const pkg = { id: 'Files.Scene.1', name: 'Files scene', creator: 'Files', resource_types: ['scene'], file_path: 'C:/fixture/Files.Scene.1.var', content_count: 3, dependency_count: 0, dependents_count: 0 }
  const calls = []
  mockIPC((command, args) => {
    calls.push(command)
    if (command === 'get_package_dependency_relations') return { direct: [], sub: [] }
    if (command === 'list_package_images') return [{ path: 'Saves/scene/Files.jpg', size_bytes: 10 }, { path: 'Saves/scene/Other.jpg', size_bytes: 10 }]
    if (command === 'get_package_image') return `https://fixture/${args.imagePath}`
    if (command === 'get_package_thumbnails') return args.packageIds.map(packageId => ({ packageId, path: null, cached: true }))
    if (command === 'list_package_contents') {
      assert.equal(args.packageId, pkg.id)
      return [
        { path: 'Saves/scene/Files.json', resource_type: 'scene', size_bytes: 2048 },
        { path: 'Custom/Atom/Person/Textures/Skin.png', resource_type: 'texture', size_bytes: 1024 },
        { path: 'meta.json', resource_type: 'other', size_bytes: 10 },
      ]
    }
    throw new Error(`Unexpected command: ${command}`)
  })
  const closed = []
  const app = createApp({ render: () => h(SharedDetails, { item: pkg, image: 'https://fixture/cover.jpg', expanded: true, 'onUpdate:expanded': value => closed.push(value) }) })
  app.mount('#app')
  t.after(() => { app.unmount(); invalidateResourceCache(); clearMocks(); delete globalThis.getComputedStyle })
  const settle = async () => { await nextTick(); await setImmediate(); await nextTick() }
  await settle(); await settle()
  assert.equal(calls.includes('list_package_contents'), false, 'contents load only when the tab is opened')
  const tab = [...document.querySelectorAll('[role="radio"]')].find(el => el.textContent.startsWith('ui.contents'))
  assert.equal(tab.querySelector('.ui-segmented-count').textContent, '3', 'counts render apart from the label')
  tab.click(); await settle()
  const names = () => [...document.querySelectorAll('.content-row strong')].map(el => el.textContent)
  assert.deepEqual(names(), ['Files.json', 'Skin.png', 'meta.json'])
  assert.equal(document.querySelector('.content-row small').textContent, 'Saves/scene/')
  ;[...document.querySelectorAll('.contents-filters .ui-chip')].find(el => el.textContent.startsWith('texture')).click(); await settle()
  assert.deepEqual(names(), ['Skin.png'])
  ;[...document.querySelectorAll('.contents-filters .ui-chip')].find(el => el.textContent.includes('ui.all')).click(); await settle()
  const search = document.querySelector('.contents-search input')
  search.value = 'META'; search.dispatchEvent(new dom.window.Event('input', { bubbles: true })); await settle()
  assert.deepEqual(names(), ['meta.json'])
  assert.equal(calls.filter(command => command === 'list_package_contents').length, 1)

  document.querySelector('.gallery-fullscreen').click(); await settle()
  assert.ok(document.querySelector('.gallery-lightbox'))
  assert.ok(document.querySelector('.gallery-details').hasAttribute('inert'), 'the page behind fullscreen is inert')
  document.activeElement.dispatchEvent(new dom.window.KeyboardEvent('keydown', { key: 'ArrowRight', bubbles: true })); await settle()
  assert.equal(document.querySelector('.lightbox-counter').textContent, '2 / 2')
  document.activeElement.dispatchEvent(new dom.window.KeyboardEvent('keydown', { key: 'Escape', bubbles: true, cancelable: true })); await settle()
  assert.equal(document.querySelector('.gallery-details').hasAttribute('inert'), false, 'Esc leaves fullscreen')
  assert.ok(document.activeElement.classList.contains('gallery-fullscreen'), 'focus returns to the fullscreen button')
  assert.ok(document.querySelector('.package-detail-modal'), 'Esc in fullscreen keeps the details open')
  assert.deepEqual(closed, [])
})
