export const packageKey = (id: string) => id.toLowerCase()

/** The result IDs are copied now: future imports never join an existing selection. */
export function selectSnapshot(current: Set<string>, ids: string[], protectedIds = new Set<string>()) {
  const next = new Set(current)
  for (const id of ids) if (!protectedIds.has(packageKey(id))) next.add(packageKey(id))
  return next
}
export function toggleSelection(current: Set<string>, ids: string[], id: string, anchor: string | null, shift: boolean) {
  const next = new Set(current)
  const key = packageKey(id)
  const keys = ids.map(packageKey)
  const index = keys.indexOf(key)
  const start = anchor ? keys.indexOf(anchor) : -1
  const range = shift && start >= 0 && index >= 0 ? keys.slice(Math.min(start, index), Math.max(start, index) + 1) : [key]
  const select = !next.has(key)
  for (const key of range) { if (select) next.add(key); else next.delete(key) }
  return next
}
export function hiddenSelectionCount(selected: Set<string>, visibleIds: string[]) {
  const visible = new Set(visibleIds.map(packageKey))
  return [...selected].filter(id => !visible.has(id)).length
}
