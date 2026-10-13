import type { PackageHealth } from '@/types/cleanup'

/** Filters on the cleanup page, in display order. */
export const ISSUE_FILTERS = ['invalid', 'invalidName', 'damaged', 'missingDeps', 'unused', 'oldVersion', 'copies'] as const
export type IssueFilter = (typeof ISSUE_FILTERS)[number]
/** The first four are problems; the rest are cleanup suggestions. */
export const PROBLEM_FILTERS: readonly IssueFilter[] = ['invalid', 'invalidName', 'damaged', 'missingDeps']
export const SUGGESTION_FILTERS: readonly IssueFilter[] = ['unused', 'oldVersion', 'copies']

/**
 * Packages the game uses directly (scenes, looks, clothing, hair, plugins) are
 * rarely referenced by anything, so "nothing uses it" only means something for
 * dependency packages.
 */
const DEPENDENCY_TYPES = new Set(['morph', 'texture', 'asset', 'sound', 'other'])

export interface IssuePackage {
  id: string
  creator: string
  name: string
  version: number
  resource_types: string[]
}
export interface PackageIssues {
  invalid?: boolean
  invalidName?: boolean
  /**
   * `archive`: the file cannot be opened at all. `info`: it opens, but its
   * package info (meta.json) is broken, so the scan could not read it.
   */
  damaged?: 'archive' | 'info'
  /** Number of missing dependencies. */
  missing?: number
  unused?: boolean
  /** Newest installed version of the same package. */
  newerVersion?: number
  copies?: number
}
export interface IssueSources {
  /** From the last full resource check; null until one is available. */
  health: Map<string, PackageHealth> | null
  missing: { package_id: string }[]
  /** Packages the scan could not read, with its error message. */
  corrupted: { package_id: string; error: string }[]
  protectedIds: Set<string>
}

/** A full check must confirm both directions; missing counts alone cannot do so. */
export function isStandalonePackage(health: PackageHealth | undefined, issues: PackageIssues | undefined, protectedPackage: boolean): boolean {
  return Boolean(
    health?.standalone && health.unused && !health.issue && !health.invalid_name
    && !protectedPackage && !issues?.invalid && !issues?.invalidName && !issues?.damaged && !issues?.missing,
  )
}

const key = (id: string) => id.toLowerCase()
const looksLikeWebPage = (error: string) => /网页|XML|7z|html/i.test(error)
const looksLikeInvalidName = (error: string) => /Invalid (?:version|\.var filename format)/i.test(error)
const looksLikeBrokenInfo = (error: string) => /meta\.json|parse/i.test(error)

/**
 * Scan failures whose archive still opens: only the package info is broken.
 * The resource check reads such packages in a tolerant way, so it reports no
 * problem for them; without that check, the scan error decides.
 */
export function brokenInfoIds(
  corrupted: IssueSources['corrupted'],
  health: IssueSources['health'],
): Set<string> {
  const result = new Set<string>()
  for (const item of corrupted) {
    const id = key(item.package_id)
    if (health?.get(id)?.issue || health?.get(id)?.invalid_name || looksLikeInvalidName(item.error) || looksLikeWebPage(item.error)) continue
    if (health || looksLikeBrokenInfo(item.error)) result.add(id)
  }
  return result
}

export function packageIssues(packages: IssuePackage[], sources: IssueSources): Map<string, PackageIssues> {
  const missing = new Map<string, number>()
  for (const item of sources.missing) missing.set(key(item.package_id), (missing.get(key(item.package_id)) ?? 0) + 1)
  const corrupted = new Map(sources.corrupted.map(item => [key(item.package_id), item.error]))
  const brokenInfo = brokenInfoIds(sources.corrupted, sources.health)
  const newest = new Map<string, number>()
  for (const pkg of packages) {
    const family = key(`${pkg.creator}.${pkg.name}`)
    newest.set(family, Math.max(newest.get(family) ?? 0, pkg.version))
  }
  const result = new Map<string, PackageIssues>()
  for (const pkg of packages) {
    const id = key(pkg.id)
    const issues: PackageIssues = {}
    const health = sources.health?.get(id)
    if (health?.invalid_name || !/^[^.]+(?:\.[^.]+)*\.[^.]+\.\d+$/.test(pkg.id) || looksLikeInvalidName(corrupted.get(id) ?? '')) issues.invalidName = true
    if (health?.issue === 'damaged') issues.damaged = 'archive'
    else if (health?.issue) issues.invalid = true
    else if (brokenInfo.has(id)) issues.damaged = 'info'
    else if (corrupted.has(id) && !sources.health && !issues.invalidName) {
      if (looksLikeWebPage(corrupted.get(id) ?? '')) issues.invalid = true
      else issues.damaged = 'archive'
    }
    const broken = issues.invalid || issues.invalidName || issues.damaged
    if (!broken && missing.has(id)) issues.missing = missing.get(id)
    if (
      !broken && health?.unused && !sources.protectedIds.has(id)
      && pkg.resource_types.length > 0 && pkg.resource_types.every(type => DEPENDENCY_TYPES.has(type))
    ) issues.unused = true
    const latest = newest.get(key(`${pkg.creator}.${pkg.name}`)) ?? 0
    if (!broken && pkg.version > 0 && latest > pkg.version) issues.newerVersion = latest
    if ((health?.copies ?? 0) > 1) issues.copies = health!.copies
    if (Object.keys(issues).length) result.set(id, issues)
  }
  return result
}

export function matchesIssue(issues: PackageIssues | undefined, filter: IssueFilter): boolean {
  if (!issues) return false
  switch (filter) {
    case 'invalid': return Boolean(issues.invalid)
    case 'invalidName': return Boolean(issues.invalidName)
    case 'damaged': return Boolean(issues.damaged)
    case 'missingDeps': return Boolean(issues.missing)
    case 'unused': return Boolean(issues.unused)
    case 'oldVersion': return Boolean(issues.newerVersion)
    case 'copies': return Boolean(issues.copies)
  }
}

export function issueCounts(issues: Map<string, PackageIssues>): Record<IssueFilter, number> {
  const counts = Object.fromEntries(ISSUE_FILTERS.map(filter => [filter, 0])) as Record<IssueFilter, number>
  for (const value of issues.values()) for (const filter of ISSUE_FILTERS) if (matchesIssue(value, filter)) counts[filter] += 1
  return counts
}
