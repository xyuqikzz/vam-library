import assert from 'node:assert/strict'
import { after, beforeEach, test } from 'node:test'
import { JSDOM } from 'jsdom'
import { compileModule } from './helpers/typescript.mjs'

const uiUrl = await compileModule(new URL('../src/i18n/ui.ts', import.meta.url))
const { stringifyError, showFatalError } = await import(await compileModule(
  new URL('../src/utils/startupError.ts', import.meta.url), { '../i18n/ui': uiUrl },
))
const dom = new JSDOM('<div id="app"></div>', { url: 'http://localhost' })
const globals = ['window', 'document', 'Element', 'HTMLElement', 'SVGElement', 'Node']
for (const key of globals) globalThis[key] = key === 'window' ? dom.window : dom.window[key]
const { createApp, h, ref, nextTick } = await import('vue')
function reset() { document.body.innerHTML = '<div id="app"></div>' }
beforeEach(reset)
after(() => { dom.window.close(); for (const key of globals) delete globalThis[key] })

test('startup errors without a JSON representation still render instead of causing a second failure', () => {
  for (const error of [undefined, Symbol('error'), () => {}, 1n]) {
    reset()
    assert.equal(typeof stringifyError(error), 'string')
    assert.doesNotThrow(() => showFatalError(error))
    assert.ok(document.querySelector('.startup-fatal pre').textContent)
  }
})

test('startup errors are shown as text and circular values still have a fallback', () => {
  const circular = {}; circular.self = circular
  assert.equal(stringifyError(circular), '[object Object]')
  assert.doesNotThrow(() => showFatalError(circular))
  assert.equal(document.querySelector('.startup-fatal pre').textContent, '[object Object]')
  reset()
  showFatalError('<img src=x onerror=attack()>')
  assert.equal(document.querySelector('.startup-fatal img'), null)
  assert.equal(document.querySelector('.startup-fatal pre').textContent, '<img src=x onerror=attack()>')
})

test('secondary render errors cannot replace the original startup failure', () => {
  showFatalError('original failure')
  showFatalError(new TypeError('insertBefore'))
  assert.equal(document.querySelector('.startup-fatal pre').textContent, 'original failure')
  assert.equal(document.querySelectorAll('.startup-fatal').length, 1)
})

test('fatal startup reporting supports English without invoking the message compiler', () => {
  showFatalError('original failure', 'en-US')
  assert.equal(document.querySelector('.startup-fatal-title').textContent, 'The app could not start')
  assert.equal(document.querySelector('.startup-fatal').getAttribute('role'), 'alert')
})

test('fatal reporting preserves mounted Vue fragment anchors during a pending update', async t => {
  const shown = ref(false)
  const errors = []
  const app = createApp({ render: () => [
    shown.value ? h('span', { class: 'updated' }, 'Updated') : null,
    h('span', { class: 'anchor' }, 'Anchor'),
  ] })
  app.config.errorHandler = error => errors.push(error)
  app.mount('#app')
  t.after(() => app.unmount())
  const anchor = document.querySelector('.anchor')
  showFatalError('first startup failure')
  shown.value = true
  await nextTick()
  assert.deepEqual(errors, [])
  assert.equal(document.querySelector('#app .anchor'), anchor)
  assert.equal(document.querySelector('#app .updated').textContent, 'Updated')
  assert.equal(document.querySelector('.startup-fatal').parentElement, document.body)
  assert.equal(document.querySelector('.startup-fatal pre').textContent, 'first startup failure')
  assert.equal(document.querySelector('#app').inert, true)
})
