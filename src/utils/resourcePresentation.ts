const knownTypes = new Set(['scene', 'appearance', 'morph', 'clothing', 'plugin', 'texture', 'asset', 'hair'])
export function resourceColor(type?: string): string { return knownTypes.has(type || '') ? `var(--color-${type})` : 'var(--text-tertiary)' }
export function resourceThumbnail(type?: string): string { return knownTypes.has(type || '') ? `var(--thumb-${type})` : 'var(--thumb-other)' }

/** Unknown numeric versions use 0 in the database; display the original suffix. */
export function resourceVersion(item: { id: string; version?: number }): string {
  if (item.version === undefined) return ''
  if (item.version !== 0) return `v${item.version}`
  const parts = item.id.split('.')
  if (parts.length < 3) return '—'
  const suffix = parts[parts.length - 1]
  return /^0+$/.test(suffix) ? 'v0' : suffix || '—'
}
