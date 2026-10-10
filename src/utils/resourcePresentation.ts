const knownTypes = new Set(['scene', 'appearance', 'morph', 'clothing', 'plugin', 'texture', 'asset', 'hair'])
export function resourceColor(type?: string): string { return knownTypes.has(type || '') ? `var(--color-${type})` : 'var(--text-tertiary)' }
export function resourceThumbnail(type?: string): string { return knownTypes.has(type || '') ? `var(--thumb-${type})` : 'var(--thumb-other)' }
