const units = ['B', 'KB', 'MB', 'GB', 'TB']

export function formatSize(bytes: number): string {
  if (!Number.isFinite(bytes) || bytes <= 0) return '0 B'
  const index = Math.max(0, Math.min(Math.floor(Math.log(bytes) / Math.log(1024)), units.length - 1))
  return `${(bytes / 1024 ** index).toFixed(index > 0 ? 1 : 0)} ${units[index]}`
}

export function parseSizeValue(value: string | number | null | undefined): number {
  if (value === null || value === undefined) return 0
  const match = String(value).trim().toUpperCase().match(/^(\d+(?:\.\d*)?|\.\d+)\s*(B|KB|MB|GB|TB)?$/)
  if (!match) return 0
  const bytes = Math.round(Number(match[1]) * 1024 ** units.indexOf(match[2] || 'B'))
  // Tauri's u64 argument requires an integer, and invalid Hub sizes must not
  // become NaN/Infinity (serialized as null) in a download request.
  return Number.isSafeInteger(bytes) ? bytes : 0
}
