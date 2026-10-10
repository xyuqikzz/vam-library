import { readFileSync, existsSync } from 'node:fs'
import { dirname, resolve, extname } from 'node:path'
import { fileURLToPath } from 'node:url'
import { parse, compileScript } from '@vue/compiler-sfc'
import ts from 'typescript'
import { moduleUrl } from './typescript.mjs'

// Compile actual presentation components for jsdom, with only external services mocked.
export function componentLoader(replacements = {}) {
  const cache = new Map()
  function load(file) {
    const filename = file instanceof URL ? fileURLToPath(file) : resolve(file)
    if (cache.has(filename)) return cache.get(filename)
    let source = readFileSync(filename, 'utf8')
    if (filename.endsWith('.vue')) {
      const { descriptor } = parse(source, { filename })
      source = compileScript(descriptor, { id: filename, inlineTemplate: true }).content
    }
    const { outputText } = ts.transpileModule(source, { compilerOptions: { target: ts.ScriptTarget.ES2020, module: ts.ModuleKind.ES2022 } })
    const compiled = outputText.replace(/from ['"]([^'"]+)['"]/g, (_, specifier) => {
      let url = replacements[specifier]
      if (!url && (specifier.startsWith('.') || specifier.startsWith('@/'))) {
        let dependency = specifier.startsWith('@/') ? resolve('src', specifier.slice(2)) : resolve(dirname(filename), specifier)
        if (!extname(dependency)) dependency += existsSync(dependency + '.ts') ? '.ts' : '.vue'
        url = load(dependency)
      }
      return `from ${JSON.stringify(url || import.meta.resolve(specifier))}`
    })
    const url = moduleUrl(compiled)
    cache.set(filename, url)
    return url
  }
  return load
}
