import { readFile } from 'node:fs/promises'
import ts from 'typescript'

export const moduleUrl = source => `data:text/javascript;base64,${Buffer.from(source).toString('base64')}`

export async function compileModule(file, replacements = {}) {
  const source = await readFile(file, 'utf8')
  const { outputText } = ts.transpileModule(source, {
    compilerOptions: { target: ts.ScriptTarget.ES2020, module: ts.ModuleKind.ES2022 },
  })
  return moduleUrl(outputText.replace(/from ['"]([^'"]+)['"]/g,
    (_, specifier) => `from ${JSON.stringify(replacements[specifier] || import.meta.resolve(specifier))}`))
}
