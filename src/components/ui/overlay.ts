export const modalStack: symbol[] = []
let previousOverflow = ''
export function enterModal(id: symbol) {
  if (modalStack.includes(id)) return
  if (!modalStack.length) { previousOverflow = document.body.style.overflow; document.body.style.overflow = 'hidden' }
  modalStack.push(id)
}
export function leaveModal(id: symbol) {
  const index = modalStack.indexOf(id)
  if (index >= 0) modalStack.splice(index, 1)
  if (!modalStack.length) document.body.style.overflow = previousOverflow
}
export function focusable(root: HTMLElement): HTMLElement[] {
  return Array.from(root.querySelectorAll<HTMLElement>('button, [href], input, select, textarea, [tabindex], summary')).filter(el =>
    !el.matches(':disabled, [tabindex="-1"], [type="hidden"]') && !el.closest('[hidden], [inert], [aria-hidden="true"]') && getComputedStyle(el).visibility !== 'hidden' && getComputedStyle(el).display !== 'none')
}
