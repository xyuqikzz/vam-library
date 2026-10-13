import { uiEn, uiZh } from '../i18n/ui'

export function stringifyError(error: unknown): string {
  if (error instanceof Error) return error.stack || error.message
  if (typeof error === 'string') return error
  try {
    return JSON.stringify(error) ?? String(error)
  } catch {
    return String(error)
  }
}

export function showFatalError(error: unknown, locale = 'zh-CN') {
  // Keep the original failure and leave Vue's mounted nodes and anchors intact.
  if (document.getElementById('startup-fatal')) return
  const root = document.getElementById('app')
  if (!root || !document.body) return

  // Read plain locale strings: reporting an error must not invoke the compiler
  // that may have caused that error in the first place.
  const text = locale === 'en-US' ? uiEn : uiZh
  const host = document.createElement('div')
  host.id = 'startup-fatal'
  host.className = 'startup-fatal'
  host.setAttribute('role', 'alert')
  host.setAttribute('aria-labelledby', 'startup-fatal-title')
  const card = document.createElement('div')
  card.className = 'startup-fatal-card'
  const title = document.createElement('strong')
  title.id = 'startup-fatal-title'
  title.className = 'startup-fatal-title'
  title.textContent = text.startupFailed
  const description = document.createElement('p')
  description.className = 'startup-fatal-description'
  description.textContent = text.startupFailedHelp
  const details = document.createElement('pre')
  details.className = 'startup-fatal-details'
  details.tabIndex = 0
  details.textContent = stringifyError(error)
  card.append(title, description, details)
  host.append(card)
  root.inert = true
  document.body.append(host)
}
