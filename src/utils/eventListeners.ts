import type { UnlistenFn } from '@tauri-apps/api/event'

// Tauri registers listeners asynchronously. Stop may run before registration
// finishes, and retrying a partial registration must not leave listeners behind.
export function createEventListeners(registrations: (() => Promise<UnlistenFn>)[]) {
  let generation = 0
  let pending: Promise<void> | null = null
  let active: UnlistenFn[] = []

  function stop() {
    generation += 1
    pending = null
    for (const unlisten of active) unlisten()
    active = []
  }

  function start(): Promise<void> {
    if (pending) return pending
    if (active.length) return Promise.resolve()
    const token = ++generation
    pending = (async () => {
      try {
        for (const register of registrations) {
          const unlisten = await Promise.resolve().then(register)
          if (token !== generation) { unlisten(); return }
          active.push(unlisten)
        }
      } catch (error) {
        if (token === generation) stop()
        throw error
      }
    })().finally(() => {
      if (token === generation) pending = null
    })
    return pending
  }

  return { start, stop }
}
