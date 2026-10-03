import assert from 'node:assert/strict'
import { readFile } from 'node:fs/promises'
import { test } from 'node:test'
import ts from 'typescript'

const source = await readFile(new URL('../src/utils/nameSort.ts', import.meta.url), 'utf8')
const { outputText } = ts.transpileModule(source, {
  compilerOptions: { target: ts.ScriptTarget.ES2020, module: ts.ModuleKind.ES2022 },
})
const { compareDisplayNames } = await import(`data:text/javascript;base64,${Buffer.from(outputText).toString('base64')}`)

test('pure numeric names precede all mixed names, regardless of numeric value', () => {
  const names = ['1a', '10', '2b', '2', '100', '1', 'A', '中文']
  assert.deepEqual(names.sort(compareDisplayNames), ['1', '2', '10', '100', '1a', '2b', 'A', '中文'])
})

test('numbers sort naturally without losing precision', () => {
  const names = ['9007199254740993', '1a', '10', '9007199254740992', '2']
  assert.deepEqual(names.sort(compareDisplayNames), ['2', '10', '9007199254740992', '9007199254740993', '1a'])
})

test('full-width digits and surrounding whitespace remain numeric', () => {
  const names = ['1a', ' １０ ', '100', '２', '[1]', '#2']
  assert.deepEqual(names.sort(compareDisplayNames), ['２', ' １０ ', '100', '[1]', '1a', '#2'])
})

test('Latin and Chinese names retain their natural order after numbers', () => {
  const names = ['中文10', 'Preset10', '10', '苹果10', 'Zebra', 'Preset2', '2', '中文2', 'apple', '苹果2']
  assert.deepEqual(names.sort(compareDisplayNames), ['2', '10', 'apple', 'Preset2', 'Preset10', 'Zebra', '苹果2', '苹果10', '中文2', '中文10'])
})

test('equivalent names and leading zeros compare equally', () => {
  assert.equal(compareDisplayNames('02', '2'), 0)
  assert.equal(compareDisplayNames('Alpha', 'alpha'), 0)
  assert.equal(compareDisplayNames('', ''), 0)
})
