import assert from 'node:assert/strict'
import { readFile } from 'node:fs/promises'
import { after, test } from 'node:test'
import { JSDOM } from 'jsdom'
import ts from 'typescript'
import { moduleUrl } from './helpers/typescript.mjs'

// Exercise the actual startup error handlers without mounting the whole app.
const source = await readFile(new URL('../src/main.ts', import.meta.url), 'utf8')
const ast = ts.createSourceFile('main.ts', source, ts.ScriptTarget.Latest, true)
const handlers = ast.statements.filter(node => ts.isFunctionDeclaration(node) && ['stringifyError', 'showFatalError'].includes(node.name?.text))
const { outputText } = ts.transpileModule(handlers.map(node => `export ${node.getText(ast)}`).join('\n'), {
  compilerOptions: { target: ts.ScriptTarget.ES2020, module: ts.ModuleKind.ES2022 },
})
const { stringifyError, showFatalError } = await import(moduleUrl(outputText))
const dom = new JSDOM('<div id="app"></div>')
globalThis.document = dom.window.document
after(() => { dom.window.close(); delete globalThis.document })

test('startup errors without a JSON representation still render instead of causing a second failure', () => {
  for (const error of [undefined, Symbol('error'), () => {}, 1n]) {
    assert.equal(typeof stringifyError(error), 'string')
    assert.doesNotThrow(() => showFatalError(error))
    assert.ok(document.querySelector('#app pre').textContent)
  }
})

test('startup errors are shown as text and circular values still have a fallback', () => {
  const circular = {}; circular.self = circular
  assert.doesNotThrow(() => showFatalError(circular))
  showFatalError('<img src=x onerror=attack()>')
  assert.equal(document.querySelector('#app img'), null)
  assert.equal(document.querySelector('#app pre').textContent, '<img src=x onerror=attack()>')
})
