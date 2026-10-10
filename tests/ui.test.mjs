import assert from 'node:assert/strict'
import { after, test } from 'node:test'
import { setTimeout as delay } from 'node:timers/promises'
import { JSDOM } from 'jsdom'
import { componentLoader } from './helpers/vue.mjs'
import { moduleUrl } from './helpers/typescript.mjs'

const dom = new JSDOM('<button id="opener">Open</button><div id="app"></div>', { url: 'http://localhost', pretendToBeVisual: true })
for (const key of ['window', 'document', 'Element', 'HTMLElement', 'SVGElement', 'Node', 'getComputedStyle']) globalThis[key] = key === 'window' ? dom.window : key === 'getComputedStyle' ? dom.window.getComputedStyle.bind(dom.window) : dom.window[key]
globalThis.requestAnimationFrame = callback => setTimeout(callback, 0)
globalThis.cancelAnimationFrame = clearTimeout
const { createApp, h, nextTick, ref } = await import('vue')
const load = componentLoader({ 'vue-i18n': moduleUrl('export const useI18n = () => ({ t: key => key })') })
const { default: UiModal } = await import(load('src/components/ui/UiModal.vue'))
const { default: UiMenu } = await import(load('src/components/ui/UiMenu.vue'))
const { default: Toast } = await import(load('src/components/common/Toast.vue'))
const { default: PackageCard } = await import(load('src/components/packages/PackageCard.vue'))
const { useConfirm, resolveConfirmation, confirmation } = await import(load('src/composables/useConfirm.ts'))
after(async () => { await delay(100); dom.window.close(); for (const key of ['window', 'document', 'Element', 'HTMLElement', 'SVGElement', 'Node', 'getComputedStyle', 'requestAnimationFrame', 'cancelAnimationFrame']) delete globalThis[key] })
const key = (target, value, shiftKey = false) => target.dispatchEvent(new dom.window.KeyboardEvent('keydown', { key: value, shiftKey, bubbles: true, cancelable: true }))

test('modal traps keyboard focus, closes on Escape and restores its opener', async () => {
  const open = ref(false)
  const app = createApp({ render: () => h(UiModal, { open: open.value, title: 'Confirm', showClose: false, onClose: () => { open.value = false } }, () => [h('button', { id: 'first' }, 'Cancel'), h('button', { id: 'last' }, 'Accept')]) })
  app.mount('#app')
  const opener = document.querySelector('#opener'); opener.focus(); open.value = true
  await nextTick(); await nextTick()
  assert.equal(document.activeElement.id, 'first')
  document.querySelector('#last').focus(); key(document.activeElement, 'Tab')
  assert.equal(document.activeElement.id, 'first')
  key(document.activeElement, 'Tab', true); assert.equal(document.activeElement.id, 'last')
  key(document.activeElement, 'Escape'); await nextTick()
  assert.equal(open.value, false); assert.equal(document.activeElement, opener)
  app.unmount()
})

test('menu skips disabled items, selects with Enter and restores focus on Escape', async () => {
  const open = ref(true), selected = []
  const opener = document.querySelector('#opener'); opener.focus()
  const app = createApp({ render: () => h(UiMenu, { open: open.value, label: 'Actions', anchor: opener, items: [{ value: 'a', label: 'A' }, { value: 'b', label: 'B', disabled: true }, { value: 'c', label: 'C' }], 'onUpdate:open': value => { open.value = value }, onSelect: item => selected.push(item.value) }) })
  app.mount('#app'); await nextTick(); await nextTick()
  assert.equal(document.activeElement.textContent, 'A')
  key(document.activeElement, 'ArrowDown'); assert.equal(document.activeElement.textContent, 'C')
  // Native buttons activate on Enter in browsers; jsdom requires the click explicitly.
  document.activeElement.click(); await nextTick(); assert.deepEqual(selected, ['c'])
  open.value = true; await nextTick(); await nextTick(); key(document.activeElement, 'Escape'); await nextTick()
  assert.equal(open.value, false); assert.equal(document.activeElement, opener)
  app.unmount()
})

test('error toasts stay until dismissed even when a duration is provided', async t => {
  const instance = ref(null)
  const app = createApp({ render: () => h(Toast, { ref: instance }) }); app.mount('#app'); t.after(() => app.unmount())
  instance.value.showToast({ type: 'error', message: 'Keep this error', duration: 1 })
  instance.value.showToast({ type: 'success', message: 'Short success', duration: 1 })
  await delay(100); await nextTick()
  assert.equal(document.querySelector('[role="alert"]').textContent.includes('Keep this error'), true)
  assert.equal(document.querySelector('[role="status"]'), null)
  document.querySelector('[role="alert"] button').click(); await nextTick(); await delay(100)
})

test('queued confirmation requests resolve independently', async () => {
  const ask = useConfirm()
  const first = ask({ title: 'First', message: 'One' })
  const second = ask({ title: 'Second', message: 'Two' })
  resolveConfirmation(true); assert.equal(await first, true)
  assert.equal(confirmation.value.title, 'Second')
  resolveConfirmation(false); assert.equal(await second, false)
  await Promise.resolve(); assert.equal(confirmation.value, null)
})

test('package cards expose independent keyboard and selection actions without nested buttons', async t => {
  const events = [], checked = ref(false)
  const app = createApp({ render: () => h(PackageCard, {
    item: { id: 'Author.Scene.1', name: 'Scene', creator: 'Author', version: 1, resource_types: ['scene'] },
    multiSelect: true, checked: checked.value, canLaunch: true,
    onDetails: () => events.push('details'), onLaunch: () => events.push('launch'),
    onToggle: () => { checked.value = !checked.value },
  }) })
  app.mount('#app'); t.after(() => app.unmount())
  assert.equal(document.querySelector('button button'), null)
  const target = document.querySelector('.card-hit-target')
  key(target, ' '); key(target, 'Enter')
  assert.deepEqual(events, ['details', 'launch'])
  document.querySelector('.package-check input').click(); await nextTick()
  assert.equal(checked.value, true)
  assert.equal(document.querySelector('.package-check input').checked, true)
})
