import { shallowRef } from 'vue'

export interface ConfirmOptions { title: string; message: string; danger?: boolean; confirmText?: string; cancelText?: string }
interface Request { options: ConfirmOptions; resolve: (result: boolean) => void }
const queue: Request[] = []
export const confirmation = shallowRef<ConfirmOptions | null>(null)
let active: Request | null = null
let advancing = false
function advance() { active = queue.shift() || null; confirmation.value = active?.options || null }
export function resolveConfirmation(result: boolean) {
  active?.resolve(result)
  active = null
  confirmation.value = null
  // Separate consecutive dialogs so focus is restored before the next one opens.
  advancing = true
  queueMicrotask(() => { advancing = false; if (!active && !advancing) advance() })
}
export function dismissConfirmations() { active?.resolve(false); for (const request of queue.splice(0)) request.resolve(false); active = null; confirmation.value = null }
export function useConfirm() {
  return (options: ConfirmOptions): Promise<boolean> => new Promise(resolve => { queue.push({ options, resolve }); if (!active) advance() })
}
