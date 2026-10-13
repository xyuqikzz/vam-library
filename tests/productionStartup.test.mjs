import assert from 'node:assert/strict'
import { test } from 'node:test'
import { readFile } from 'node:fs/promises'
import { setTimeout as delay } from 'node:timers/promises'
import { createContext, runInContext } from 'node:vm'
import { resolve } from 'node:path'
import { JSDOM } from 'jsdom'
import { build } from 'vite'

// Build the real entry, router, stores and locale resources in production mode.
// Inline chunks only so jsdom can execute them without a browser module loader.
const result = await build({
  logLevel: 'silent',
  build: {
    write: false,
    rollupOptions: {
      input: resolve('src/main.ts'),
      output: { format: 'iife', inlineDynamicImports: true },
    },
  },
})
const code = result.output.find(item => item.type === 'chunk' && item.isEntry).code
const conf = JSON.parse(await readFile(new URL('../src-tauri/tauri.conf.json', import.meta.url), 'utf8'))
assert.ok(!conf.app.security.csp.includes("'unsafe-eval'"))

function mockDesktop(window) {
  let id = 0
  const callbacks = new Map()
  window.__TAURI_INTERNALS__ = {
    metadata: { currentWindow: { label: 'main' }, currentWebview: { label: 'main' } },
    transformCallback(callback) { const key = ++id; callbacks.set(key, callback); return key },
    unregisterCallback(key) { callbacks.delete(key) },
    async invoke(command, args) {
      if (command === 'plugin:event|listen') return args.handler
      if (command === 'plugin:event|unlisten') return null
      if (command === 'get_settings') return { vam_root_path: null, auto_scan: false, theme: 'dark', blur_previews: true, vam_instances: [] }
      if (command === 'set_app_language' || command === 'set_download_settings') return null
      if (command === 'get_dependency_graph') return { nodes: [], edges: [] }
      if (command === 'get_dashboard_stats') return {
        total_packages: 0, total_size_bytes: 0, scene_count: 0, appearance_count: 0,
        morph_count: 0, plugin_count: 0, missing_dependencies: 0, duplicate_resources: 0,
        orphaned_packages: 0, corrupted_packages: 0,
      }
      if (['get_download_queue', 'take_external_import_requests', 'list_packages', 'list_all_tags',
        'find_missing_dependencies', 'find_corrupted_packages'].includes(command)) return []
      throw new Error(`Unexpected startup IPC command: ${command}`)
    },
  }
  window.__TAURI_EVENT_PLUGIN_INTERNALS__ = { unregisterListener(_event, key) { callbacks.delete(key) } }
}

for (const [locale, title] of [['zh-CN', '仪表盘'], ['en-US', 'Dashboard']]) {
  test(`the complete production app starts in ${locale} with dynamic code evaluation prohibited`, async t => {
    const dom = new JSDOM('<div id="app"></div>', { url: 'http://localhost/', pretendToBeVisual: true })
    t.after(() => dom.window.close())
    const window = dom.window
    window.localStorage.setItem('vamlibrary-locale', locale)
    mockDesktop(window)
    const errors = []
    const sandbox = {
      window, document: window.document, navigator: window.navigator,
      location: window.location, history: window.history, localStorage: window.localStorage,
      Element: window.Element, HTMLElement: window.HTMLElement, SVGElement: window.SVGElement,
      Node: window.Node, MutationObserver: window.MutationObserver, URL: window.URL,
      setTimeout: window.setTimeout.bind(window), clearTimeout: window.clearTimeout.bind(window),
      setInterval: window.setInterval.bind(window), clearInterval: window.clearInterval.bind(window),
      requestAnimationFrame: window.requestAnimationFrame.bind(window),
      cancelAnimationFrame: window.cancelAnimationFrame.bind(window),
      console: { error: (...args) => errors.push(args.map(String).join(' ')), warn() {}, log() {}, info() {} },
    }
    const context = createContext(sandbox, { codeGeneration: { strings: false, wasm: false } })
    assert.throws(() => runInContext('new Function("return 1")()', context), /Code generation from strings disallowed/)
    runInContext(code, context, { timeout: 5000 })
    const deadline = Date.now() + 2000
    while (!window.document.querySelector('.dashboard-page') && errors.length === 0 && Date.now() < deadline) await delay(20)
    // Let the async settings and initial library refresh finish as well.
    await delay(100)
    assert.deepEqual(errors, [])
    assert.equal(window.document.querySelector('.startup-fatal'), null)
    assert.equal(window.document.querySelector('.startup-error-overlay'), null)
    assert.ok(window.document.querySelector('.dashboard-page'))
    assert.ok(window.document.querySelector('.app-layout').textContent.includes(title))
  })
}
