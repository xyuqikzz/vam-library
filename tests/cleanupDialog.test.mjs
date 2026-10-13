import { componentLoader } from './helpers/vue.mjs'
import assert from 'node:assert/strict'
import { test, after } from 'node:test'
import { readFile } from 'node:fs/promises'
import { setImmediate, setTimeout as delay } from 'node:timers/promises'
import { JSDOM } from 'jsdom'
import { parse, compileScript } from '@vue/compiler-sfc'
import ts from 'typescript'
import { mockIPC, clearMocks } from '@tauri-apps/api/mocks'
import { compileModule, moduleUrl } from './helpers/typescript.mjs'

const dom = new JSDOM('<div id="app"></div>', { url: 'http://localhost' })
for (const key of ['window', 'document', 'Element', 'HTMLElement', 'SVGElement', 'Node', 'HTMLDialogElement']) globalThis[key] = dom.window[key]
dom.window.HTMLDialogElement.prototype.showModal = function () { this.open = true }
dom.window.HTMLDialogElement.prototype.close = function () { this.open = false }
globalThis.getComputedStyle = dom.window.getComputedStyle.bind(dom.window)
globalThis.requestAnimationFrame = callback => setTimeout(callback, 0)
const { createApp, h, nextTick, ref } = await import('vue')
const { createPinia, disposePinia } = await import('pinia')
const bytesUrl = await compileModule(new URL('../src/utils/bytes.ts', import.meta.url))
const appStoreUrl = moduleUrl('export const useAppStore = () => ({ vamRootPath: "/fixture" })')
const replacements = {
  '@/utils/bytes': bytesUrl,
  '@/stores/app': appStoreUrl,
  './app': appStoreUrl,
  'vue-i18n': moduleUrl('export const useI18n = () => ({ t: key => key })'),
  'vue-router': moduleUrl('export const useRouter = () => ({ push(path) { globalThis.__cleanupRoute = path } })'),
  '@tauri-apps/api/event': moduleUrl('export const listen = async (_, callback) => { globalThis.__cleanupProgressListener = callback; return () => { if (globalThis.__cleanupProgressListener === callback) delete globalThis.__cleanupProgressListener } }'),
}
const loadComponent = componentLoader(replacements)
const { default: CleanupPreviewDialog } = await import(loadComponent('src/components/CleanupPreviewDialog.vue'))
const { default: ConfirmHost } = await import(loadComponent('src/components/ui/ConfirmHost.vue'))
const { useCleanupStore } = await import(loadComponent('src/stores/cleanup.ts'))

after(async () => { await delay(100); dom.window.close(); for (const key of ['window', 'document', 'Element', 'HTMLElement', 'SVGElement', 'Node', 'HTMLDialogElement']) delete globalThis[key] })
const row = (id, attached = false) => ({ package_id: id, file_count: 1, size_bytes: 1024, content_count: 1, attached, reasons: [] })
async function mount(t, ids, preview, handlers = {}, props = {}) {
  const requests = []
  mockIPC((command, args) => {
    requests.push({ command, args })
    if (handlers[command]) return handlers[command](args)
    if (command === 'cancel_resource_cleanup') return
    if (command === 'get_package_thumbnails') return args.packageIds.map(packageId => ({ packageId, path: null, cached: true }))
    if (command === 'get_package_thumbnail') return null
    if (command === 'list_package_images') return []
    if (command === 'get_package_dependency_relations') return { direct: [], sub: [] }
    if (command === 'preview_resource_cleanup') return Promise.resolve(preview(args)).then(result => ({ plan_id: args.requestId, total_files: ids.length, total_bytes: ids.length * 1024, warnings: [], suggested: [], suggested_bytes: 0, requires_confirmation: false, retained: [], blocked: [], ...result }))
    throw new Error(`Unexpected command: ${command}`)
  }, { shouldMockEvents: true })
  const open = ref(true)
  const app = createApp({ render: () => [h(ConfirmHost), open.value ? h(CleanupPreviewDialog, { packageIds: ids, ...props, onClose: () => { open.value = false } }) : null] })
  const pinia = createPinia()
  app.component('RouterLink', { render: () => null })
  app.use(pinia)
  globalThis.__dialogCleanupStore = useCleanupStore(pinia)
  app.mount('#app')
  t.after(async () => { app.unmount(); disposePinia(pinia); await setImmediate(); clearMocks() })
  await nextTick(); await setImmediate()
  return requests
}
const previews = requests => requests.filter(request => request.command === 'preview_resource_cleanup').map(request => request.args)
const toggle = async input => { input.checked = !input.checked; input.dispatchEvent(new dom.window.Event('change', { bubbles: true })); await nextTick(); await setImmediate() }
const buttonIn = (root, key) => [...root.querySelectorAll('button')].find(button => button.textContent.trim().startsWith(key))
const primary = () => document.querySelector('.cleanup-preview-modal footer .primary')
const settle = async () => { await nextTick(); await setImmediate() }
const used = (id, by = 'Other.Consumer.1') => ({ ...row(id), reasons: [{ code: 'referenced', source: by }] })

test('large cleanup previews render one page instead of thousands of rows', async t => {
  const ids = Array.from({ length: 250 }, (_, index) => `Author.Package${index}.1`)
  await mount(t, ids, () => ({ items: ids.map(id => row(id)) }))
  const group = document.querySelector('.remove-group')
  assert.equal(group.querySelectorAll('li').length, 100)
  const next = buttonIn(group, 'cleanup.nextPage')
  next.click(); await nextTick()
  assert.equal(group.querySelectorAll('li').length, 100)
  assert.ok(group.querySelector('li').textContent.includes('Author.Package100.1'))
  next.click(); await nextTick()
  assert.equal(group.querySelectorAll('li').length, 50)
  assert.match(primary().textContent, /cleanup\.toTrashCount/)
  assert.match(document.querySelector('dialog footer').textContent, /cleanup\.restoreHint/)
})
test('reference progress distinguishes first indexing, changed files and cached analysis', async t => {
  let resolve
  const pending = new Promise(done => { resolve = done })
  const requests = await mount(t, ['Author.Root.1'], () => pending)
  const requestId = requests.find(request => request.command === 'preview_resource_cleanup').args.requestId
  const progress = async (stage, current, total) => {
    globalThis.__cleanupProgressListener({ payload: { request_id: requestId, stage, current, total } })
    await nextTick()
    return document.querySelector('p[role="status"]').textContent
  }
  assert.match(await progress('building_index', 1, 20), /cleanup\.buildingIndex.*1 \/ 20/)
  assert.match(await progress('updating_index', 1, 2), /cleanup\.updatingIndex.*1 \/ 2/)
  assert.equal((await progress('loading_index', 0, 0)).trim(), 'cleanup.loadingIndex')
  assert.equal((await progress('analyzing', 0, 0)).trim(), 'cleanup.analyzing')
  resolve({ items: [row('Author.Root.1')] }); await settle()
  assert.equal(document.querySelector('p[role="status"]'), null)
})
test('the result comes first and only speaks about the selected packages', async t => {
  const requests = await mount(t, ['Author.Selected.1'], () => ({
    items: [row('Author.Selected.1')],
    // Library-wide notes from older backends must not reach the dialog.
    diagnostics: ['Other.Package.1: format inspected'], invalid_files: ['AddonPackages/Other.Download.1.var'],
  }), { execute_resource_cleanup: () => ({ id: 'batch', status: 'completed', errors: [], files: [] }) })
  const body = document.querySelector('.cleanup-dialog-body')
  assert.equal(body.firstElementChild.className, 'preview-overview')
  assert.doesNotMatch(body.textContent, /Other\.Package\.1|Other\.Download\.1|compatibleDetails|invalidFiles|quickCheck/)
  assert.equal(document.querySelector('.unreadable-summary'), null)
  assert.equal(document.querySelector('.decide-group'), null)
  assert.equal(buttonIn(document.querySelector('dialog footer'), 'cleanup.refreshPreview'), undefined)
  assert.deepEqual([previews(requests)[0].quick, previews(requests)[0].forcePackageIds], [false, []])
  primary().click(); await settle()
  assert.equal(requests.filter(request => request.command === 'execute_resource_cleanup').length, 1)
})
test('a preview containing only protected packages cannot execute deletion', async t => {
  const requests = await mount(t, ['Author.Package.1'], () => ({ items: [], blocked: [{ ...row('Author.Package.1'), reasons: [{ code: 'manual', source: 'Author.Package.1' }] }] }))
  assert.equal(primary().disabled, true)
  const group = document.querySelector('.decide-group')
  assert.match(group.textContent, /Author\.Package\.1/)
  assert.match(group.textContent, /cleanup\.reasons\.manual/)
  assert.equal(group.querySelector('.force-action'), null)
  assert.match(document.querySelector('.cleanup-footer-note').textContent, /cleanup\.decideFirst/)
  primary().click(); await setImmediate()
  assert.equal(requests.some(request => request.command === 'execute_resource_cleanup'), false)
})
test('unreadable files keep the selection, explain why and can be checked again', async t => {
  const warnings = Array.from({ length: 45 }, (_, index) => `Custom/broken${index}.vap: unfinished`)
  const requests = await mount(t, ['Author.Selected.1'], () => ({ items: [], warnings, blocked: [{ ...row('Author.Selected.1'), reasons: [{ code: 'incomplete', source: '' }] }] }))
  const notice = document.querySelector('.unreadable-summary')
  assert.equal(notice.getAttribute('role'), 'alert')
  assert.match(notice.textContent, /cleanup\.unreadableTitle/)
  const details = notice.querySelector('details')
  assert.equal(details.open, false)
  assert.equal(details.querySelectorAll('li').length, 20)
  details.open = true
  buttonIn(details, 'cleanup.nextPage').click(); await nextTick()
  assert.match(details.querySelector('li').textContent, /broken20/)
  assert.ok(document.querySelector('.decide-group .force-action'), 'an unconfirmed use can be overridden')
  assert.equal(primary().disabled, true)
  buttonIn(notice, 'cleanup.checkAgain').click(); await settle()
  assert.equal(previews(requests).length, 2)
  assert.equal(previews(requests).at(-1).reuseIndex, false)
})
test('the dependency suggestion shows its effect and turning it off drops dependency exclusions', async t => {
  const requests = await mount(t, ['Author.Root.1'], args => ({
    items: [row('Author.Root.1'), ...(args.includeDependencies && !args.excludedPackageIds.includes('author.dependency.1') ? [row('Author.Dependency.1', true)] : [])],
    retained: args.excludedPackageIds.includes('author.dependency.1') ? [{ ...row('Author.Dependency.1', true), reasons: [{ code: 'excluded', source: '' }] }] : [],
    suggested: args.includeDependencies ? [] : [row('Author.Dependency.1', true)], suggested_bytes: args.includeDependencies ? 0 : 1024,
  }))
  const choice = document.querySelector('.dependency-choice')
  assert.match(choice.textContent, /cleanup\.suggestDependencies/)
  await toggle(choice.querySelector('input'))
  assert.deepEqual([previews(requests).at(-1).includeDependencies, previews(requests).at(-1).reuseIndex], [true, true])
  const dependency = [...document.querySelectorAll('.remove-group li')].find(li => li.textContent.includes('Author.Dependency.1'))
  assert.match(dependency.textContent, /cleanup\.attached/)
  buttonIn(dependency, 'cleanup.keep').click(); await settle()
  assert.deepEqual(previews(requests).at(-1).excludedPackageIds, ['author.dependency.1'])
  assert.match(document.querySelector('.kept-group').textContent, /Author\.Dependency\.1/)
  await toggle(document.querySelector('.dependency-choice input'))
  const last = previews(requests).at(-1)
  assert.equal(last.includeDependencies, false); assert.deepEqual(last.excludedPackageIds, [])
})
test('removing anyway applies to one package and the confirmation names only it', async t => {
  const requests = await mount(t, ['Author.A.1', 'Author.B.1'], args => {
    const forced = args.forcePackageIds.includes('author.a.1')
    return {
      items: forced ? [used('Author.A.1')] : [],
      blocked: [...(forced ? [] : [used('Author.A.1')]), used('Author.B.1')],
      requires_confirmation: forced,
    }
  }, { execute_resource_cleanup: () => ({ id: 'batch', status: 'completed', errors: [], files: [] }) })
  const rowOf = id => [...document.querySelectorAll('.cleanup-item')].find(li => li.textContent.includes(id))
  assert.match(rowOf('Author.A.1').textContent, /cleanup\.usedBy/)
  assert.match(rowOf('Author.A.1').textContent, /Other\.Consumer\.1/)
  rowOf('Author.A.1').querySelector('.force-action').click(); await settle()
  assert.deepEqual(previews(requests).at(-1).forcePackageIds, ['author.a.1'])
  assert.match(rowOf('Author.A.1').textContent, /cleanup\.forced/)
  assert.ok(document.querySelector('.decide-group').textContent.includes('Author.B.1'))
  const action = primary()
  action.click(); await settle()
  assert.match(document.querySelector('.confirm-message').textContent, /cleanup\.removalConfirm/)
  const cancel = [...document.querySelectorAll('dialog:not(.cleanup-preview-modal) footer button')].find(button => button.textContent.includes('common.cancel'))
  cancel.click(); await settle()
  assert.equal(requests.some(r => r.command === 'execute_resource_cleanup'), false)
  assert.equal(document.activeElement, action)
  action.click(); await settle()
  document.querySelector('dialog:not(.cleanup-preview-modal) footer .danger').click()
  await settle()
  const executions = requests.filter(r => r.command === 'execute_resource_cleanup')
  assert.equal(executions.length, 1)
  assert.deepEqual(executions[0].args, { planId: previews(requests).at(-1).requestId, confirmRisks: true })
})
test('undoing a removal choice re-plans without it', async t => {
  const requests = await mount(t, ['Author.A.1'], args => args.forcePackageIds.length
    ? { items: [used('Author.A.1')], requires_confirmation: true }
    : { items: [], blocked: [used('Author.A.1')] })
  document.querySelector('.force-action').click(); await settle()
  buttonIn(document.querySelector('.remove-group'), 'cleanup.undo').click(); await settle()
  assert.deepEqual(previews(requests).at(-1).forcePackageIds, [])
  assert.ok(document.querySelector('.decide-group'))
})
test('the packages using a blocked package can be added to the cleanup', async t => {
  const requests = await mount(t, ['Author.Look.1'], args => args.packageIds.includes('other.consumer.1')
    ? { items: [row('Author.Look.1'), row('Other.Consumer.1')] }
    : { items: [], blocked: [used('Author.Look.1')] })
  buttonIn(document.querySelector('.decide-group'), 'cleanup.addUsers').click(); await settle()
  assert.deepEqual(previews(requests).at(-1).packageIds, ['author.look.1', 'other.consumer.1'])
  assert.equal(document.querySelector('.decide-group'), null)
  assert.equal(document.querySelectorAll('.remove-group li').length, 2)
})
test('manual or directory protections do not offer removing anyway', async t => {
  await mount(t, ['Author.Selected.1'], () => ({ items: [], blocked: [{ ...row('Author.Selected.1'), reasons: [{ code: 'manual', source: '' }, { code: 'incomplete', source: '' }] }], warnings: ['fixture'] }))
  assert.equal(document.querySelector('.force-action'), null)
  assert.equal(primary().disabled, true)
})
test('unusable packages are tagged and removed without a usage confirmation', async t => {
  const requests = await mount(t, ['Bad.Download.1', 'Bad.Archive.1'], () => ({
    items: [{ ...row('Bad.Download.1'), issue: 'invalid_download' }, { ...row('Bad.Archive.1'), issue: 'damaged' }],
  }), { execute_resource_cleanup: () => ({ id: 'batch', status: 'completed', errors: [], files: [] }) })
  const tags = [...document.querySelectorAll('.remove-group .cleanup-tag')].map(tag => tag.textContent)
  assert.deepEqual(tags, ['cleanup.issues.tag.invalid', 'cleanup.issues.tag.damaged'])
  primary().click(); await settle()
  assert.equal(document.querySelector('.confirm-message'), null)
  assert.equal(requests.filter(r => r.command === 'execute_resource_cleanup').length, 1)
})
test('execution reports re-checking, verifying and moving separately', async t => {
  let finish
  const requests = await mount(t, ['Author.Root.1'], () => ({ items: [row('Author.Root.1')] }), {
    execute_resource_cleanup: () => new Promise(done => { finish = done }),
  })
  const requestId = previews(requests)[0].requestId
  primary().click(); await nextTick()
  const status = () => document.querySelector('p[role="status"]').textContent.trim()
  assert.equal(status(), 'cleanup.rechecking')
  const progress = async (stage, current, total) => {
    globalThis.__cleanupProgressListener({ payload: { request_id: requestId, stage, current, total } })
    await nextTick()
    return status()
  }
  assert.match(await progress('checking_files', 3, 10), /^cleanup\.rechecking 3 \/ 10$/)
  assert.match(await progress('verifying', 1, 2), /^cleanup\.verifying 1 \/ 2$/)
  assert.match(await progress('moving', 2, 2), /^cleanup\.executing 2 \/ 2$/)
  finish({ id: requestId, created_at: '', expires_at: '', status: 'completed', files: [], errors: [] })
  await settle()
  assert.match(document.querySelector('dialog').textContent, /cleanup\.status\.completed/)
  assert.equal(document.querySelector('.remove-group'), null)
})
test('analysis errors show a recovery message, keep the raw error collapsed and can be retried', async t => {
  let fail = true
  const requests = await mount(t, ['Author.Root.1'], () => { if (fail) throw new Error('fixture: invalid reference data'); return { items: [row('Author.Root.1')] } })
  const notice = document.querySelector('.cleanup-error[role="alert"]')
  assert.match(notice.querySelector('strong').textContent, /cleanup\.errorTitle/)
  const details = notice.querySelector('details')
  assert.equal(details.open, false)
  assert.match(details.textContent, /fixture: invalid reference data/)
  assert.equal(primary().disabled, true)
  fail = false
  buttonIn(notice, 'cleanup.checkAgain').click(); await settle()
  assert.equal(previews(requests).length, 2)
  assert.equal(document.querySelector('.cleanup-error'), null)
  assert.equal(primary().disabled, false)
  const cancel = [...document.querySelectorAll('dialog footer button')].find(button => button.textContent.includes('common.cancel'))
  cancel.click(); await nextTick()
  assert.equal(document.querySelector('dialog'), null)
})

// A route unmount while a nested confirmation is open must never delete resources.
test('a confirmation dismissed during unmount never executes cleanup', async t => {
  const requests = await mount(t, ['Author.Selected.1'], () => ({ items: [used('Author.Selected.1')], requires_confirmation: true }))
  primary().click(); await settle()
  document.querySelector('#app').__vue_app__.unmount(); await settle()
  assert.equal(requests.some(r => r.command === 'execute_resource_cleanup'), false)
})

test('packages with broken info are labelled wherever they appear', async t => {
  await mount(t, ['A1X.Violet.2', 'Amaimon.Bhaviour.2'], () => ({
    items: [row('A1X.Violet.2')], blocked: [used('Amaimon.Bhaviour.2')],
  }), {}, { brokenInfoIds: ['a1x.violet.2', 'amaimon.bhaviour.2'] })
  assert.deepEqual([...document.querySelectorAll('.remove-group .cleanup-tag')].map(tag => tag.textContent), ['cleanup.issues.tag.brokenInfo'])
  assert.deepEqual([...document.querySelectorAll('.decide-group .cleanup-tag')].map(tag => tag.textContent), ['cleanup.issues.tag.brokenInfo'])
  // Broken info is not "unusable": a package still in use needs a decision as usual.
  assert.ok(document.querySelector('.decide-group .force-action'))
})

test('abnormal names stay intact through mixed cleanup and are not labelled broken info', async t => {
  const ids = ['Qing.Alet01.1_2', 'V2.Morphs.latest', 'Bad.Zip.1']
  const requests = await mount(t, ids, () => ({ items: [
    { ...row(ids[0]), invalid_name: true }, { ...row(ids[1]), invalid_name: true }, { ...row(ids[2]), issue: 'damaged' },
  ] }), { execute_resource_cleanup: () => ({ id: 'batch', status: 'completed', errors: [], files: [] }) }, { brokenInfoIds: [ids[0].toLowerCase()] })
  assert.deepEqual(previews(requests)[0].packageIds, ids.map(id => id.toLowerCase()))
  assert.deepEqual([...document.querySelectorAll('.remove-group .cleanup-tag')].map(tag => tag.textContent), ['cleanup.issues.tag.invalidName', 'cleanup.issues.tag.invalidName', 'cleanup.issues.tag.damaged'])
  assert.equal(primary().disabled, false)
  primary().click(); await settle()
  assert.equal(requests.filter(r => r.command === 'execute_resource_cleanup').length, 1)
})

test('all sixteen consumers show thumbnails and open the common package detail modal', async t => {
  const consumers = Array.from({ length: 16 }, (_, i) => `Full.Consumer${i}.1`)
  const cover = 'data:image/svg+xml,<svg xmlns="http://www.w3.org/2000/svg"/>'
  await mount(t, ['Full.Root.1'], () => ({ items: [], blocked: [{ ...row('Full.Root.1'), reasons: [...consumers,consumers[0]].map(source => ({ code: 'referenced', source })) }] }), {
    get_package_thumbnails: args => args.packageIds.map(packageId => ({ packageId, path: cover, cached: true })),
  }, { packages: consumers.map(id => ({ id, name: 'Consumer details ' + id, creator: 'Full', resource_types: ['clothing'], size_bytes: 123 })) })
  await settle()
  const rows = [...document.querySelectorAll('.cleanup-consumers .cleanup-resource-open')]
  assert.equal(rows.length, 16)
  assert.deepEqual(rows.map(row => row.querySelector('strong').textContent), consumers)
  assert.equal(document.querySelectorAll('.cleanup-consumers img').length, 16)
  rows[15].focus(); rows[15].click(); await settle(); await settle()
  const detail = document.querySelector('.package-detail-modal')
  assert.ok(detail?.closest('dialog')?.open)
  assert.match(detail.textContent, /Consumer details Full.Consumer15.1/)
  assert.ok(document.querySelector('.cleanup-preview-modal').open)
  document.dispatchEvent(new dom.window.KeyboardEvent('keydown', { key: 'Escape', bubbles: true, cancelable: true })); await settle()
  assert.equal(document.querySelector('.package-detail-modal'), null)
  assert.equal(document.activeElement, rows[15])
})

test('consumer paging retains every package and requests the next thumbnails on demand', async t => {
  const consumers = Array.from({ length: 47 }, (_, i) => `Paged.Consumer${i}.1`)
  const requests = await mount(t, ['Paged.Root.1'], () => ({ items: [], blocked: [{ ...row('Paged.Root.1'), reasons: consumers.map(source => ({ code: 'referenced', source })) }] }))
  const list = () => [...document.querySelectorAll('.cleanup-consumers strong')].map(row => row.textContent)
  assert.deepEqual(list(), consumers.slice(0,20))
  const next = () => buttonIn(document.querySelector('.cleanup-consumers'), 'cleanup.nextPage')
  next().click(); await settle()
  assert.deepEqual(list(), consumers.slice(20,40))
  next().click(); await settle()
  assert.deepEqual(list(), consumers.slice(40))
  assert.equal(next().disabled, true)
  assert.ok(requests.filter(r => r.command === 'get_package_thumbnails').some(r => r.args.packageIds.includes(consumers[46])))
})

test('large-file byte progress stays visible and completed cleanup focuses the new trash record', async t => {
  let finish
  const batch = { id: 'new-batch', created_at: '', expires_at: '', status: 'completed', files: [], errors: [] }
  const requests = await mount(t, ['Bytes.Root.1'], () => ({ items: [row('Bytes.Root.1')] }), {
    execute_resource_cleanup: () => new Promise(done => { finish = done }),
  })
  primary().click(); await settle()
  globalThis.__cleanupProgressListener({ payload: { request_id: previews(requests)[0].requestId, stage: 'verifying_bytes', current: 512 * 1024 * 1024, total: 1024 * 1024 * 1024 } })
  await settle()
  assert.match(document.querySelector('p[role="status"]').textContent, /cleanup.verifying.*cleanup.verifiedBytes/)
  assert.equal(document.querySelector('[role="progressbar"]').getAttribute('aria-valuenow'), '50')
  finish(batch); await settle()
  assert.equal(globalThis.__dialogCleanupStore.latestBatch.id, batch.id)
  buttonIn(document.querySelector('.cleanup-result'), 'cleanup.viewBatch').click(); await settle()
  assert.equal(globalThis.__dialogCleanupStore.focusedBatchId, batch.id)
  assert.equal(globalThis.__cleanupRoute, '/trash')
  assert.equal(document.querySelector('.cleanup-preview-modal'), null)
})
