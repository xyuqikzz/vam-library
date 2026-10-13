import { parseSizeValue } from './bytes'

export interface HubFile {
  filename?: string | null
  file_size?: string | null
  creatorName?: string | null
  licenseType?: string | null
  urlHosted?: string | null
}

export interface HubFileCandidate {
  filename: string
  version: number
  sizeBytes: number
  url: string
}

export function parsePackageId(packageId: string) {
  const parts = packageId.replace(/\.var$/i, '').split('.')
  if (parts.length < 2) return null
  if (parts.some(part => !part.trim() || part.trim() !== part)) return null
  const text = parts.length > 2 ? parts[parts.length - 1] : ''
  const version = /^\d+$/.test(text) ? Number(text) : NaN
  return {
    creator: parts[0],
    name: parts.length === 2 ? parts[1] : parts.slice(1, -1).join('.'),
    version: Number.isInteger(version) && version <= 2147483647 ? version : null,
  }
}

export function parseHubFiles(files: HubFile[]): HubFileCandidate[] {
  return files.flatMap(file => {
    if (!file.filename || !file.urlHosted) return []
    const parsed = parsePackageId(file.filename)
    if (!parsed || parsed.version === null) return []
    return [{ filename: file.filename, version: parsed.version, sizeBytes: parseSizeValue(file.file_size), url: file.urlHosted }]
  })
}

export function selectHubFile(files: HubFileCandidate[], requiredVersion: number | null): HubFileCandidate | null {
  let selected: HubFileCandidate | null = null
  for (const file of files) {
    if (requiredVersion === null) {
      if (!selected || file.version >= selected.version) selected = file
    } else if (file.version >= requiredVersion && (!selected || file.version < selected.version)) {
      selected = file
    }
  }
  return selected
}
