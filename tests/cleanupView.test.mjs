import { componentLoader } from './helpers/vue.mjs'
import assert from 'node:assert/strict'
import { test, after } from 'node:test'
import { readFile } from 'node:fs/promises'
import { setImmediate, setTimeout } from 'node:timers/promises'
import { JSDOM } from 'jsdom'
import { parse, compileScript } from '@vue/compiler-sfc'
import ts from 'typescript'
import { mockIPC, clearMocks } from '@tauri-apps/api/mocks'
import { compileModule, moduleUrl } from './helpers/typescript.mjs'

const dom = new JSDOM('<div id="app"></div>', { url: 'http://localhost', pretendToBeVisual: true })
const globals = ['window', 'document', 'Element', 'HTMLElement', 'SVGElement', 'Node']
for (const key of globals) globalThis[key] = dom.window[key]
globalThis.requestAnimationFrame = dom.window.requestAnimationFrame.bind(dom.window)
globalThis.getComputedStyle = dom.window.getComputedStyle.bind(dom.window)
globalThis.IntersectionObserver = class { observe() {} disconnect() {} }
const { createApp, h, nextTick, reactive } = await import('vue')
const { createPinia, disposePinia } = await import('pinia')
after(() => {
  dom.window.close()
  for (const key of [...globals, 'requestAnimationFrame', 'IntersectionObserver', '__cleanupViewApp', '__cleanupViewLibrary', '__cleanupViewErrors', '__cleanupViewRoutes']) delete globalThis[key]
})

const appUrl = moduleUrl('export const useAppStore = () => globalThis.__cleanupViewApp')
const libraryUrl = moduleUrl('export const useLocalLibraryStore = () => globalThis.__cleanupViewLibrary')
const selectionUrl = await compileModule(new URL('../src/utils/cleanupSelection.ts', import.meta.url))
const cleanupUrl = await compileModule(new URL('../src/stores/cleanup.ts', import.meta.url), { './app': appUrl, '@/utils/cleanupSelection': selectionUrl })
const { useCleanupStore } = await import(cleanupUrl)
const bytesUrl = await compileModule(new URL('../src/utils/bytes.ts', import.meta.url))
const emptyStub = moduleUrl('export default { render: () => null }')
const replacements = {
  '@/stores/favorites': moduleUrl('export const useFavoritesStore = () => ({ countFor: () => 0, refresh: async () => {} })'),
  '@/utils/bytes': bytesUrl,
  '@/utils/hubFiles': await compileModule(new URL('../src/utils/hubFiles.ts', import.meta.url), { './bytes': bytesUrl }),
  '@/utils/cleanupSelection': selectionUrl,
  '@/stores/app': appUrl,
  '@/stores/localLibrary': libraryUrl,
  '@/stores/cleanup': cleanupUrl,
  '@/stores/download': moduleUrl('export const useDownloadStore = () => ({ addItems: async () => [] })'),
  '@/composables/useNotification': moduleUrl('export const useNotification = () => ({ error: value => globalThis.__cleanupViewErrors.push(value), success() {} })'),
  'vue-i18n': moduleUrl('export const useI18n = () => ({ t: (key, params) => key + (params ? JSON.stringify(params) : "") })'),
  'vue-router': moduleUrl('export const useRoute = () => ({ path: "/cleanup", query: globalThis.__cleanupViewQuery ?? {} }); export const useRouter = () => ({ push: path => globalThis.__cleanupViewRoutes.push(path) })'),
  '@/components/ShareModal.vue': emptyStub,
  '@/components/PackageSceneExtractor.vue': emptyStub,
  '@/components/CleanupBatchPanel.vue': emptyStub,
  './DeduplicationView.vue': emptyStub,
  // Reference analysis is covered by cleanupDialog.test.mjs; inspect the exact selection passed to it here.
  '@/components/CleanupPreviewDialog.vue': moduleUrl(`import { h } from ${JSON.stringify(import.meta.resolve('vue'))}; export default { props: ['packageIds'], setup(props) { return () => h('div', { id: 'preview-selection' }, props.packageIds.join('|')) } }`),
}
replacements['./CleanupPreviewDialog.vue'] = replacements['@/components/CleanupPreviewDialog.vue']
const loadComponent = componentLoader(replacements)
async function component(path) { return loadComponent(new URL(`../src/${path}`, import.meta.url)) }
replacements['@/components/common/EmptyState.vue'] = await component('components/common/EmptyState.vue', 'cleanup-empty-test')
replacements['@/components/common/SearchInput.vue'] = await component('components/common/SearchInput.vue', 'cleanup-search-test')
replacements['@/components/CleanupToolbar.vue'] = await component('components/CleanupToolbar.vue', 'cleanup-toolbar-test')
replacements['@/components/ResourceDisplay.vue'] = await component('components/ResourceDisplay.vue', 'cleanup-display-test')
const { default: CleanupView } = await import(await component('views/CleanupView.vue', 'cleanup-view-test'))

const fixture = (name, size, type = 'scene', creator = 'Author') => ({
  id: `${creator}.${name}.1`, name, creator, version: 1, file_path: `/fixture/${creator}.${name}.1.var`,
  size_bytes: size, resource_types: [type], dependency_count: 0, dependents_count: 0, content_count: 1,
  created_time: '2026-10-01T00:00:00Z', scan_time: '2026-10-01T00:00:00Z', tags: [],
})
async function mount(t, packages = [fixture('Small', 10), fixture('Large', 100), fixture('Hair', 40, 'hair', 'Other')], marks = [], options = {}) {
  const requests = [], scans = [], refreshes = []
  globalThis.__cleanupViewErrors = []
  globalThis.__cleanupViewRoutes = []
  const appStore = globalThis.__cleanupViewApp = reactive({ vamRootPath: '/fixture', isScanning: false, searchQuery: 'unrelated package-browser search', async startScan(path) { scans.push(path) } })
  const library = globalThis.__cleanupViewLibrary = reactive({ packages, loading: false, missingDependencies: options.missing ?? [], corruptedPackages: options.corrupted ?? [], async ensureLoaded() {}, async refreshAll(state) { refreshes.push(state) } })
  mockIPC((command, args) => {
    requests.push({ command, args })
    if (command === 'list_cleanup_marks') return marks
    if (command === 'get_cleanup_health') { if (options.healthError) throw new Error('health failed'); return { packages: options.health ?? [], checked_at: '' } }
    if (command === 'get_cleanup_index_status') return globalThis.__cleanupIndexStatus ?? { state: 'idle', stage: '', current: 0, total: 0, message: null, finished_at: null }
    if (command === 'get_package_thumbnails') return args.packageIds.map(packageId => ({ packageId, path: null, cached: true }))
    if (command === 'get_package_thumbnail') return null
    if (command === 'update_cleanup_marks') return args.packageIds.map(package_id => ({ package_id, pending: args.action === 'pending', protected: args.action === 'protect' }))
    throw new Error(`Unexpected command: ${command}`)
  })
  const pinia = createPinia()
  const app = createApp({ render: () => h(CleanupView) })
  app.use(pinia)
  app.config.globalProperties.$t = key => key
  t.after(() => { app.unmount(); disposePinia(pinia); clearMocks() })
  app.mount('#app')
  await nextTick(); await setImmediate()
  return { requests, scans, refreshes, appStore, library, cleanup: useCleanupStore(pinia) }
}
const visibleIds = () => [...document.querySelectorAll('.large-card .cleanup-select input')].map(input => input.getAttribute('aria-label'))
const button = key => [...document.querySelectorAll('button')].find(element => element.textContent === key)
async function openMoreMenu() {
  button('cleanup.more').click(); await nextTick(); await nextTick()
  assert.ok(document.querySelector('.ui-menu'), 'more-actions menu should open')
}
async function choose(label, value) {
  const input = document.querySelector(`select[aria-label="${label}"]`)
  input.value = value
  input.dispatchEvent(new dom.window.Event('change', { bubbles: true }))
  await nextTick()
}
async function search(value) {
  const input = document.querySelector('.cleanup-search input')
  input.value = value
  input.dispatchEvent(new dom.window.Event('input', { bubbles: true }))
  await setTimeout(350); await nextTick()
}

test('cleanup starts with all unmarked resources and previews selection without a mark or package-page visit', async t => {
  const { requests } = await mount(t)
  assert.deepEqual(visibleIds(), ['Author.Large.1', 'Other.Hair.1', 'Author.Small.1'])
  assert.equal(document.querySelector('.ui-menu'), null)
  assert.equal(button('cleanup.preview').disabled, true)
  const toolbar = document.querySelector('.cleanup-toolbar')
  const content = document.querySelector('.resource-content-area')
  const pagination = document.querySelector('.pagination-bar')
  assert.ok(content.compareDocumentPosition(toolbar) & Node.DOCUMENT_POSITION_FOLLOWING)
  assert.ok(toolbar.compareDocumentPosition(pagination) & Node.DOCUMENT_POSITION_FOLLOWING)
  document.querySelector('.cleanup-select input').click(); await nextTick()
  assert.equal(document.querySelector('.cleanup-toolbar'), toolbar, 'selection should keep the toolbar in its layout slot')
  assert.equal(toolbar.classList.contains('floating'), false)
  button('cleanup.preview').click(); await nextTick()
  assert.equal(document.querySelector('#preview-selection').textContent, 'author.large.1')
  assert.equal(requests.some(request => request.command === 'update_cleanup_marks'), false)
  assert.deepEqual(globalThis.__cleanupViewRoutes, [])
})

test('the shared card and list views preserve the cleanup selection when switching', async t => {
  const { cleanup } = await mount(t)
  const radios = [...document.querySelectorAll('[role="radio"]')]
  assert.deepEqual(radios.map(radio => radio.getAttribute('aria-label')), ['packages.cardView', 'packages.listView'])
  document.querySelector('.cleanup-select input').click(); await nextTick()
  const selected = [...cleanup.selected]
  radios[1].click(); await nextTick()
  assert.equal(document.querySelector('.package-card'), null)
  assert.ok(document.querySelector('.resource-list[role="table"]'))
  assert.deepEqual([...cleanup.selected], selected)
  assert.equal(document.querySelector('.resource-list input[type="checkbox"]').checked, true)
  radios[0].click(); await nextTick()
  assert.ok(document.querySelector('.package-card'))
  assert.equal(document.querySelector('.cleanup-select input').checked, true)
})

test('type and creator search filter resources directly while keeping a prior selection visible in the count', async t => {
  const { cleanup } = await mount(t)
  document.querySelector('.cleanup-select input').click(); await nextTick()
  await choose('packages.filter', 'hair')
  assert.deepEqual(visibleIds(), ['Other.Hair.1'])
  assert.match(document.querySelector('.hidden-selection').textContent, /"count":1/)
  await search('OTHER')
  assert.deepEqual(visibleIds(), ['Other.Hair.1'])
  await search('not-found')
  assert.deepEqual(visibleIds(), [])
  button('cleanup.resetFilters').click(); await nextTick()
  assert.equal(visibleIds().length, 3)
  assert.deepEqual([...cleanup.selected], ['author.large.1'])
  assert.equal(document.querySelector('.hidden-selection'), null)
})

test('pending marks are an optional filter and manual protection prevents selection', async t => {
  const { cleanup } = await mount(t, undefined, [
    { package_id: 'Author.Large.1', protected: true, pending: false },
    { package_id: 'Author.Small.1', protected: false, pending: true },
  ])
  assert.equal(document.querySelector('input[aria-label="Author.Large.1"]').disabled, true)
  button('cleanup.selectPage').click(); await nextTick()
  assert.deepEqual([...cleanup.selected], ['other.hair.1', 'author.small.1'])
  await choose('cleanup.stateFilter', 'pending')
  assert.deepEqual(visibleIds(), ['Author.Small.1'])
  await choose('cleanup.stateFilter', 'protected')
  assert.deepEqual(visibleIds(), ['Author.Large.1'])
  await choose('cleanup.stateFilter', 'normal')
  assert.deepEqual(visibleIds(), ['Other.Hair.1'])
})

test('select-page and select-all-filtered remain distinct across pagination', async t => {
  const { cleanup, library } = await mount(t, Array.from({ length: 105 }, (_, index) => fixture(`Item${index}`, index + 1)))
  button('cleanup.selectPage').click(); await nextTick()
  assert.equal(cleanup.selected.size, 100)
  await openMoreMenu()
  button('cleanup.selectAll').click(); await nextTick()
  assert.equal(cleanup.selected.size, 105)
  assert.equal(document.querySelector('.ui-menu'), null)
  library.packages.push(fixture('ImportedLater', 1000)); await nextTick()
  assert.equal(cleanup.selected.size, 105)
  assert.equal(cleanup.selected.has('author.importedlater.1'), false)
})

test('optional protection is applied from the same page and clears protected selections', async t => {
  const { requests, cleanup } = await mount(t)
  document.querySelector('.cleanup-select input').click(); await nextTick()
  await openMoreMenu()
  button('cleanup.protect').click(); await nextTick(); await setImmediate()
  assert.equal(document.querySelector('.ui-menu'), null)
  assert.equal(cleanup.selected.size, 0)
  assert.equal(document.querySelector('input[aria-label="Author.Large.1"]').disabled, true)
  assert.deepEqual(requests.find(request => request.command === 'update_cleanup_marks').args.packageIds, ['author.large.1'])
})

test('empty resources can be scanned from cleanup and scan errors are reported', async t => {
  const { scans, appStore } = await mount(t, [])
  button('cleanup.rescan').click(); await nextTick(); await setImmediate()
  assert.deepEqual(scans, ['/fixture'])
  appStore.startScan = async () => { throw new Error('scan failed') }
  button('cleanup.rescan').click(); await nextTick(); await setImmediate()
  assert.deepEqual(globalThis.__cleanupViewErrors, ['Error: scan failed'])
  appStore.isScanning = true; await nextTick()
  assert.equal(button('toolbar.scanning').disabled, true)
})

test('switching instances clears filters and selection and refreshes the library', async t => {
  const { appStore, refreshes, cleanup } = await mount(t)
  document.querySelector('.cleanup-select input').click(); await nextTick()
  await choose('packages.filter', 'hair')
  appStore.vamRootPath = '/other-fixture'; await nextTick(); await setImmediate()
  assert.equal(document.querySelector('select[aria-label="packages.filter"]').value, 'all')
  assert.equal(cleanup.selected.size, 0)
  assert.deepEqual(refreshes, ['loading'])
})
const indexStatus = (state, extra = {}) => ({ state, stage: 'building_index', current: 0, total: 0, message: null, finished_at: null, ...extra })
for (const [label, value, expected] of [
  ['background progress', indexStatus('running', { current: 3, total: 10 }), 'cleanup.index.running{"current":3,"total":10}'],
  ['a pause while the game runs', indexStatus('paused_game'), 'cleanup.index.pausedGame'],
  ['a failure', indexStatus('error', { message: 'disk' }), 'cleanup.index.error{"message":"disk"}'],
  ['nothing when idle', indexStatus('idle'), ''],
]) {
  test(`the reference index status shows ${label}`, async t => {
    globalThis.__cleanupIndexStatus = value
    t.after(() => { delete globalThis.__cleanupIndexStatus })
    await mount(t)
    await setImmediate(); await nextTick()
    assert.equal(document.querySelector('.index-status')?.textContent ?? '', expected)
  })
}

const issueChip = filter => document.querySelector(`.issue-${filter} .ui-chip`)
const tagsOf = id => [...(document.querySelector(`input[aria-label="${id}"]`)?.closest('.package-cell')?.querySelectorAll('.status-tag') ?? [])].map(tag => tag.textContent)
const versioned = (name, version, type = 'morph', creator = 'Author') => ({ ...fixture(name, 10, type, creator), id: `${creator}.${name}.${version}`, version, file_path: `/fixture/${creator}.${name}.${version}.var` })

test('problem filters list unusable downloads, tag them and open one preview for all of them', async t => {
  const packages = [fixture('Scene', 10), fixture('Page', 1, 'other', 'Bad'), fixture('Zip', 2, 'other', 'Broken'), fixture('Needs', 5)]
  await mount(t, packages, [], {
    health: [
      { package_id: 'Bad.Page.1', issue: 'invalid_download', copies: 1, unused: true },
      { package_id: 'Broken.Zip.1', issue: 'damaged', copies: 1, unused: true },
    ],
    missing: [{ package_id: 'Author.Needs.1' }, { package_id: 'Author.Needs.1' }],
  })
  await setImmediate(); await nextTick()
  assert.match(issueChip('invalid').textContent, /cleanup\.issues\.invalid\s*1/)
  assert.match(issueChip('damaged').textContent, /cleanup\.issues\.damaged\s*1/)
  assert.match(issueChip('missingDeps').textContent, /cleanup\.issues\.missingDeps\s*1/)
  assert.equal(issueChip('unused'), null, 'broken files are not cleanup suggestions')
  assert.deepEqual(tagsOf('Bad.Page.1'), ['cleanup.issues.tag.invalid'])
  assert.deepEqual(tagsOf('Author.Needs.1'), ['cleanup.issues.tag.missing{"count":2}'])
  issueChip('invalid').click(); await nextTick()
  assert.deepEqual(visibleIds(), ['Bad.Page.1'])
  assert.match(document.querySelector('.issue-hint').textContent, /cleanup\.issues\.help\.invalid/)
  button('cleanup.issues.cleanAll{"count":1}').click(); await nextTick()
  assert.equal(document.querySelector('#preview-selection').textContent, 'bad.page.1')
  issueChip('invalid').click(); await nextTick()
  assert.equal(visibleIds().length, 4)
  assert.equal(document.querySelector('.issue-hint'), null)
})

test('unused suggestions only cover dependency packages that are not protected', async t => {
  const packages = [fixture('Scene', 10), fixture('Morph', 3, 'morph'), fixture('Texture', 4, 'texture'), fixture('Hair', 6, 'hair'), fixture('Kept', 2, 'asset')]
  await mount(t, packages, [{ package_id: 'Author.Kept.1', protected: true, pending: false }], {
    health: ['Author.Scene.1', 'Author.Morph.1', 'Author.Texture.1', 'Author.Hair.1', 'Author.Kept.1'].map(package_id => ({ package_id, issue: null, copies: 1, unused: true })),
  })
  await setImmediate(); await nextTick()
  issueChip('unused').click(); await nextTick()
  assert.deepEqual(visibleIds(), ['Author.Texture.1', 'Author.Morph.1'])
  assert.match(document.querySelector('.issue-hint').textContent, /cleanup\.issues\.help\.unused/)
})

test('older versions and duplicate copies show what to compare and where to go next', async t => {
  const packages = [versioned('Pack', 1), versioned('Pack', 3), fixture('Twice', 20, 'texture')]
  await mount(t, packages, [], { health: [{ package_id: 'Author.Twice.1', issue: null, copies: 2, unused: false }] })
  await setImmediate(); await nextTick()
  assert.deepEqual(tagsOf('Author.Pack.1'), [], 'version hints only appear with their filter')
  issueChip('oldVersion').click(); await nextTick()
  assert.deepEqual(visibleIds(), ['Author.Pack.1'])
  assert.deepEqual(tagsOf('Author.Pack.1'), ['cleanup.issues.tag.newer{"version":3}'])
  issueChip('copies').click(); await nextTick()
  assert.deepEqual(visibleIds(), ['Author.Twice.1'])
  assert.deepEqual(tagsOf('Author.Twice.1'), ['cleanup.issues.tag.copies{"count":2}'])
  button('cleanup.issues.openDedup').click(); await nextTick()
  assert.equal(document.querySelector('.resource-surface'), null)
  assert.ok(document.querySelector('.dedup-surface'))
})

test('until the resource check is ready, scan failures still show as problems', async t => {
  await mount(t, [fixture('Page', 1, 'other', 'Bad'), fixture('Zip', 2, 'other', 'Broken')], [], {
    healthError: true,
    corrupted: [
      { package_id: 'Bad.Page.1', error: '此文件包含网页或 XML 文本，不是 VAR 压缩包。' },
      { package_id: 'Broken.Zip.1', error: '无法读取资源包: invalid Zip archive' },
    ],
  })
  await setImmediate(); await nextTick()
  assert.match(issueChip('invalid').textContent, /1/)
  assert.match(issueChip('damaged').textContent, /1/)
  assert.match(document.querySelector('.issue-status.error').textContent, /cleanup\.issues\.checkFailed/)
})

test('packages with broken info join the damaged filter, also when linked from the dashboard', async t => {
  globalThis.__cleanupViewQuery = { issue: 'damaged' }
  t.after(() => { delete globalThis.__cleanupViewQuery })
  const { cleanup } = await mount(t, [fixture('Scene', 10), fixture('Violet', 30, 'other', 'A1X'), fixture('Zip', 2, 'other', 'Broken')], [], {
    health: [{ package_id: 'Broken.Zip.1', issue: 'damaged', copies: 1, unused: true }],
    corrupted: [
      { package_id: 'A1X.Violet.1', error: "Parse error: Failed to parse meta.json from 'A1X.Violet.1.var': expected `,` or `}` at line 105 column 4" },
      { package_id: 'Broken.Zip.1', error: '无法读取资源包: invalid Zip archive' },
    ],
  })
  await setImmediate(); await nextTick()
  assert.equal(issueChip('damaged').getAttribute('aria-pressed'), 'true')
  assert.deepEqual(visibleIds(), ['A1X.Violet.1', 'Broken.Zip.1'])
  assert.deepEqual(tagsOf('A1X.Violet.1'), ['cleanup.issues.tag.brokenInfo'])
  assert.deepEqual(tagsOf('Broken.Zip.1'), ['cleanup.issues.tag.damaged'])
  assert.equal(issueChip('unused'), null, 'damaged packages are not cleanup suggestions')
  button('cleanup.issues.cleanAll{"count":2}').click(); await nextTick()
  assert.equal(document.querySelector('#preview-selection').textContent, 'a1x.violet.1|broken.zip.1')
  assert.deepEqual(cleanup.selected.size, 0)
})

test('invalid filename filter labels scan failures and previews their original names', async t => {
  const renamed = { ...fixture('Alet01', 10, 'other', 'Qing'), id: 'Qing.Alet01.1_2', version: 0 }
  await mount(t, [renamed, fixture('Zip', 2, 'other', 'Broken')], [], {
    health: [{ package_id: 'Broken.Zip.1', issue: 'damaged', copies: 1, unused: true }],
    corrupted: [{ package_id: renamed.id, error: "Parse error: Invalid version '1_2' in filename" }],
  })
  await setImmediate(); await nextTick()
  assert.deepEqual(tagsOf(renamed.id), ['cleanup.issues.tag.invalidName'])
  issueChip('invalidName').click(); await nextTick()
  assert.deepEqual(visibleIds(), [renamed.id])
  assert.match(document.querySelector('.issue-hint').textContent, /cleanup\.issues\.help\.invalidName/)
  button('cleanup.issues.cleanAll{"count":1}').click(); await nextTick()
  assert.equal(document.querySelector('#preview-selection').textContent, 'qing.alet01.1_2')
})

const standaloneHealth = (package_id, standalone = true) => ({ package_id, issue: null, copies: 1, unused: true, standalone })

const exclusionChip = type => document.querySelector(`.standalone-exclusions .exclude-${type} .ui-chip`)
async function toggleExclusion(type) { exclusionChip(type).click(); await nextTick() }

test('standalone cleanup excludes multiple types and previews only the remaining resources', async t => {
  const packages = [fixture('Complete', 100), fixture('Dependent', 90, 'morph'), fixture('Used', 80, 'texture'), fixture('Kept', 70, 'asset'), fixture('Hair', 60, 'hair'), { ...fixture('Mixed', 50), resource_types: ['scene', 'appearance'] }, fixture('Appearance', 40, 'appearance')]
  await mount(t, packages, [{ package_id: 'Author.Kept.1', protected: true, pending: false }], {
    health: [standaloneHealth('Author.Complete.1'), standaloneHealth('Author.Dependent.1', false), { ...standaloneHealth('Author.Used.1'), unused: false }, standaloneHealth('Author.Kept.1'), standaloneHealth('Author.Hair.1'), standaloneHealth('Author.Mixed.1'), standaloneHealth('Author.Appearance.1')],
  })
  await choose('packages.filter', 'hair')
  button('cleanup.standalone.title').click(); await nextTick()
  assert.equal(document.querySelector('select[aria-label="packages.filter"]'), null, 'standalone cleanup has exclusion toggles instead of a targeted type filter')
  assert.equal(exclusionChip('scene').getAttribute('aria-pressed'), 'true')
  assert.equal(exclusionChip('hair').getAttribute('aria-pressed'), 'false')
  assert.deepEqual(visibleIds(), ['Author.Hair.1', 'Author.Appearance.1'])
  assert.equal(document.querySelector('.standalone-hint'), null)
  assert.equal(issueChip('unused'), null)
  button('cleanup.standalone.previewAll{"count":2}').click(); await nextTick()
  assert.equal(document.querySelector('#preview-selection').textContent, 'author.hair.1|author.appearance.1')
  await toggleExclusion('hair')
  assert.equal(exclusionChip('scene').getAttribute('aria-pressed'), 'true')
  assert.equal(exclusionChip('hair').getAttribute('aria-pressed'), 'true')
  assert.deepEqual(visibleIds(), ['Author.Appearance.1'])
  button('cleanup.standalone.previewAll{"count":1}').click(); await nextTick()
  assert.equal(document.querySelector('#preview-selection').textContent, 'author.appearance.1')
  await toggleExclusion('appearance')
  assert.deepEqual(visibleIds(), [])
  assert.equal(button('cleanup.standalone.previewAll{"count":0}'), undefined)
  await toggleExclusion('hair')
  await toggleExclusion('scene')
  assert.deepEqual(visibleIds(), ['Author.Complete.1', 'Author.Hair.1'], 'a mixed package is excluded if any of its types is excluded')
  await toggleExclusion('appearance')
  assert.deepEqual(visibleIds(), ['Author.Complete.1', 'Author.Hair.1', 'Author.Mixed.1', 'Author.Appearance.1'])
  button('cleanup.resources').click(); await nextTick()
  assert.equal(document.querySelector('select[aria-label="packages.filter"]').value, 'hair')
  button('cleanup.standalone.title').click(); await nextTick()
  assert.deepEqual(visibleIds(), ['Author.Complete.1', 'Author.Hair.1', 'Author.Mixed.1', 'Author.Appearance.1'])
  await search('not-found')
  button('cleanup.resetFilters').click(); await nextTick()
  assert.equal(exclusionChip('scene').getAttribute('aria-pressed'), 'true')
  assert.equal(exclusionChip('hair').getAttribute('aria-pressed'), 'false')
  assert.equal(exclusionChip('appearance').getAttribute('aria-pressed'), 'false')
  assert.deepEqual(visibleIds(), ['Author.Hair.1', 'Author.Appearance.1'])
})

test('type exclusions preserve a prior selection and reset to scenes when the game directory changes', async t => {
  const {cleanup,appStore} = await mount(t, [fixture('Scene', 50), fixture('Hair', 30, 'hair'), fixture('Asset', 10, 'asset')], [], {
    health: ['Author.Scene.1', 'Author.Hair.1', 'Author.Asset.1'].map(id => standaloneHealth(id)),
  })
  button('cleanup.standalone.title').click(); await nextTick()
  document.querySelector('input[aria-label="Author.Hair.1"]').click(); await nextTick()
  await toggleExclusion('hair')
  assert.deepEqual(visibleIds(), ['Author.Asset.1'])
  assert.deepEqual([...cleanup.selected], ['author.hair.1'])
  assert.match(document.querySelector('.hidden-selection').textContent, /"count":1/)
  appStore.vamRootPath = '/another-fixture'; await nextTick(); await setImmediate()
  assert.equal(exclusionChip('scene').getAttribute('aria-pressed'), 'true')
  assert.equal(exclusionChip('hair').getAttribute('aria-pressed'), 'false')
  assert.equal(cleanup.selected.size, 0)
  assert.deepEqual(visibleIds(), ['Author.Hair.1', 'Author.Asset.1'])
})

test('standalone cleanup excludes packages with scan failures or missing dependencies', async t => {
  await mount(t, [fixture('Missing', 10, 'morph'), fixture('BrokenInfo', 20, 'texture'), fixture('Unknown', 30, 'hair')], [], {
    health: [standaloneHealth('Author.Missing.1'), standaloneHealth('Author.BrokenInfo.1'), { ...standaloneHealth('Author.Unknown.1'), standalone: undefined }],
    missing: [{ package_id: 'Author.Missing.1' }],
    corrupted: [{ package_id: 'Author.BrokenInfo.1', error: 'Failed to parse meta.json' }],
  })
  button('cleanup.standalone.title').click(); await nextTick()
  assert.deepEqual(visibleIds(), [])
  assert.match(document.querySelector('.empty-description').textContent, /cleanup\.standalone\.none/)
  assert.equal(button('cleanup.standalone.previewAll{"count":0}'), undefined)
})

test('standalone cleanup shows check failures and retries without guessing from dependency counts', async t => {
  const { cleanup, requests } = await mount(t, [fixture('Unknown', 10, 'hair')], [], { healthError: true })
  button('cleanup.standalone.title').click(); await nextTick()
  assert.deepEqual(visibleIds(), [])
  assert.match(document.querySelector('.issue-status.error').textContent, /cleanup\.issues\.checkFailed/)
  button('cleanup.issues.retry').click(); await nextTick(); await setImmediate()
  assert.equal(requests.filter(r => r.command === 'get_cleanup_health').length, 2)
  cleanup.healthError = ''
  cleanup.healthLoading = true
  await nextTick()
  assert.match(document.querySelector('.issue-status').textContent, /cleanup\.issues\.checking/)
  assert.equal(document.querySelector('.spinner-ring') !== null, true)
  cleanup.health = new Map([['author.unknown.1', standaloneHealth('Author.Unknown.1')]])
  cleanup.healthLoading = false
  await nextTick()
  assert.deepEqual(visibleIds(), ['Author.Unknown.1'])
})
