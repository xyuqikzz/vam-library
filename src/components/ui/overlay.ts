export const modalStack: symbol[] = []
let previousOverflow = ''
export function enterModal(id: symbol) {
  if (modalStack.includes(id)) return
  if (!modalStack.length) {
    previousOverflow = document.body.style.overflow
    document.body.style.overflow = 'hidden'
  }
  modalStack.push(id)
}
export function leaveModal(id: symbol) {
  const index = modalStack.indexOf(id)
  if (index >= 0) modalStack.splice(index, 1)
  if (!modalStack.length) document.body.style.overflow = previousOverflow
}
export function focusable(root: HTMLElement): HTMLElement[] {
  return Array.from(
    root.querySelectorAll<HTMLElement>(
      'button, [href], input, select, textarea, [tabindex], summary',
    ),
  ).filter((el) => {
    if (
      el.matches(':disabled, [tabindex="-1"], [type="hidden"]') ||
      el.closest('[hidden], [inert], [aria-hidden="true"]')
    )
      return false
    // Closed disclosure contents and CSS-hidden ancestors are excluded from the tab order.
    for (let ancestor: HTMLElement | null = el; ancestor; ancestor = ancestor.parentElement) {
      const style = getComputedStyle(ancestor)
      if (style.visibility === 'hidden' || style.display === 'none') return false
      if (
        ancestor.matches('details:not([open])') &&
        !ancestor.querySelector(':scope > summary')?.contains(el)
      )
        return false
      if (ancestor === root) break
    }
    return true
  })
}

// 最近一次交互是否来自键盘。弹窗/浮层打开时用它决定"初始焦点"要不要画焦点环：
// 鼠标点开或由外部事件弹出时不画（否则一打开就有一圈白框和 tooltip），键盘打开时照常显示。
let keyboardModality = false
if (typeof document !== 'undefined') {
  document.addEventListener(
    'keydown',
    (event) => {
      if (!event.metaKey && !event.ctrlKey && !event.altKey) keyboardModality = true
    },
    true,
  )
  document.addEventListener('pointerdown', () => (keyboardModality = false), true)
}
export function focusInitial(el: HTMLElement) {
  if (!keyboardModality) {
    el.setAttribute('data-initial-focus', '')
    el.addEventListener('blur', () => el.removeAttribute('data-initial-focus'), { once: true })
  }
  el.focus()
}
