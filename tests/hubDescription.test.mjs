import assert from 'node:assert/strict'
import { after, test } from 'node:test'
import { JSDOM } from 'jsdom'
import { compileModule } from './helpers/typescript.mjs'

const dom = new JSDOM('')
globalThis.window = dom.window
const { formatDescription } = await import(await compileModule(new URL('../src/utils/hubDescription.ts', import.meta.url)))
after(() => { dom.window.close(); delete globalThis.window })

function render(source) {
  const root = dom.window.document.createElement('div')
  root.innerHTML = formatDescription(source)
  return root
}

test('Hub descriptions retain ordinary HTML, BBCode, images and line breaks', () => {
  const root = render('<p>中文 <strong>HTML</strong></p>\n[b]bold[/b] [i]italic[/i]<img src="https://example.com/preview.jpg">')
  assert.equal(root.querySelector('p strong').textContent, 'HTML')
  assert.equal(root.querySelectorAll('strong').length, 2)
  assert.equal(root.querySelector('em').textContent, 'italic')
  assert.equal(root.querySelectorAll('br').length, 1)
  assert.equal(root.querySelector('img').getAttribute('src'), 'https://example.com/preview.jpg')
})

test('links open separately with opener protection and preserve query parameters', () => {
  for (const source of ['[url=https://example.com/?a=1&b=2]link[/url]', '<a href="https://example.com/?a=1&amp;b=2" target="_self">link</a>']) {
    const a = render(source).querySelector('a')
    assert.equal(a.getAttribute('href'), 'https://example.com/?a=1&b=2')
    assert.equal(a.getAttribute('target'), '_blank')
    assert.equal(a.getAttribute('rel'), 'noopener noreferrer')
  }
})

test('remote scripts, event attributes, embedded documents and overlay styles are removed', () => {
  const root = render('<script>attack()</script><img src=x onerror="attack()"><iframe srcdoc="<script>attack()</script>"></iframe><svg onload="attack()"></svg><div style="position:fixed;inset:0" onclick="attack()">text</div>')
  assert.equal(root.querySelector('script, iframe, svg, [onerror], [onclick], [onload], [style]'), null)
  assert.equal(root.querySelector('div').textContent, 'text')
})

test('raw and BBCode links cannot carry script, local file or data URLs', () => {
  for (const url of ['javascript:alert(1)', 'java&#x09;script:alert(1)', 'file:///C:/secret', 'data:text/html,attack']) {
    for (const source of [`[url=${url}]link[/url]`, `<a href="${url}">link</a>`]) {
      const a = render(source).querySelector('a')
      assert.ok(!a.hasAttribute('href') || !/^(javascript|file|data):/i.test(a.href))
    }
  }
  const root = render('[url=https://example.com/" onmouseover="attack()]link[/url]')
  assert.equal(root.querySelector('[onmouseover]'), null)
})
