const naturalCollator = new Intl.Collator('en', { numeric: true, sensitivity: 'base', ignorePunctuation: true })
const chineseCollator = new Intl.Collator('zh-CN', { numeric: true, sensitivity: 'base', ignorePunctuation: true })

function nameGroup(name: string): number {
  // Pure numbers must precede names such as "1a", even when the number is larger.
  if (/^[0-9]+$/.test(name)) return 0
  // Decorative prefixes do not change the group; Chinese names remain last.
  const first = name.match(/[\p{L}\p{N}]/u)?.[0] || ''
  if (/^[0-9]$/.test(first)) return 1
  if (/^[a-z]$/i.test(first)) return 2
  if (/^\p{Script=Han}$/u.test(first)) return 4
  return 3
}

export function compareDisplayNames(a: string, b: string): number {
  const left = a.trim().normalize('NFKC')
  const right = b.trim().normalize('NFKC')
  const group = nameGroup(left)
  const groupDifference = group - nameGroup(right)
  if (groupDifference) return groupDifference
  return (group === 4 ? chineseCollator : naturalCollator).compare(left, right)
}
