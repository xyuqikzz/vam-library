import assert from 'node:assert/strict'
import { test } from 'node:test'
import { compileModule } from './helpers/typescript.mjs'

const bytesUrl = await compileModule(new URL('../src/utils/bytes.ts', import.meta.url))
const { formatSize, parseSizeValue } = await import(bytesUrl)
const { parsePackageId, parseHubFiles, selectHubFile } = await import(await compileModule(
  new URL('../src/utils/hubFiles.ts', import.meta.url), { './bytes': bytesUrl },
))

test('byte formatting preserves normal labels and bounds invalid or very large inputs', () => {
  for (const [input, expected] of [[0, '0 B'], [1, '1 B'], [1023, '1023 B'], [1024, '1.0 KB'], [1536, '1.5 KB'], [1024 ** 5, '1024.0 TB'], [NaN, '0 B'], [-1, '0 B'], [Infinity, '0 B']]) {
    assert.equal(formatSize(input), expected)
  }
})

test('Hub sizes produce integers and reject malformed numbers', () => {
  for (const [input, expected] of [['1.5 mb', 1572864], [' 2 KB ', 2048], ['.5 B', 1], ['12', 12], [123, 123], ['1..2 MB', 0], ['. MB', 0], ['-2 MB', 0], ['1e3 KB', 0], [null, 0], [undefined, 0], ['999999999999999999 TB', 0]]) {
    assert.equal(parseSizeValue(input), expected)
  }
})

test('Hub files keep dotted names, skip unavailable entries and preserve input order', () => {
  assert.deepEqual(parsePackageId('Author.Sub.Name.12.VAR'), { creator: 'Author', name: 'Sub.Name', version: 12 })
  assert.deepEqual(parsePackageId('Author.Name.latest'), { creator: 'Author', name: 'Name', version: null })
  const files = parseHubFiles([
    { filename: 'Author.Name.10.var', urlHosted: 'ten', file_size: '1 KB' },
    { filename: 'Author.Name.latest', urlHosted: 'latest' },
    { filename: 'Author.Name.1.var' },
    { filename: 'Author.Name.2.var', urlHosted: 'two', file_size: 'bad' },
  ])
  assert.deepEqual(files.map(f => [f.version, f.sizeBytes]), [[10, 1024], [2, 0]])
  assert.equal(selectHubFile(files, null), files[0])
  assert.equal(selectHubFile(files, 1), files[1])
  assert.equal(selectHubFile(files, 3), files[0])
  assert.equal(selectHubFile(files, 11), null)
  assert.deepEqual(files.map(f => f.version), [10, 2])
})

test('Hub selection preserves first minimum and last maximum on version ties', () => {
  const a = { filename: 'a', version: 2, sizeBytes: 1, url: 'a' }
  const b = { ...a, url: 'b' }
  assert.equal(selectHubFile([a, b], 1), a)
  assert.equal(selectHubFile([a, b], null), b)
  assert.equal(selectHubFile([], null), null)
})

test('filename versions are complete numbers, never truncated copy suffixes', () => {
  for (const suffix of ['1_2', '1 (2)', 'v3', '3abc', 'latest', '-1', '+1', '2147483648']) {
    assert.deepEqual(parsePackageId(`Author.Sub.Name.${suffix}.var`), { creator: 'Author', name: 'Sub.Name', version: null })
    assert.deepEqual(parseHubFiles([{ filename: `Author.Name.${suffix}.var`, urlHosted: 'file' }]), [])
  }
  for (const id of ['.Name.1', 'Author..1', 'Author.Name.', 'Author. .1']) assert.equal(parsePackageId(id), null)
  assert.deepEqual(parsePackageId('Author.Name.001'), { creator: 'Author', name: 'Name', version: 1 })
})

const { resourceVersion } = await import(await compileModule(new URL('../src/utils/resourcePresentation.ts', import.meta.url)))
test('unknown versions display their raw suffix instead of the numeric placeholder', () => {
  for (const [id, expected] of [['A.Name.1_2', '1_2'], ['A.Name.latest', 'latest'], ['A.Name.1 (2)', '1 (2)'], ['A.Name', '—'], ['A.Name.0', 'v0']]) {
    assert.equal(resourceVersion({ id, version: 0 }), expected)
  }
  assert.equal(resourceVersion({ id: 'A.Name.12', version: 12 }), 'v12')
})
