import { computed, nextTick, ref, watch, type Ref } from 'vue'
import { useI18n } from 'vue-i18n'
import type { MenuItem } from '@/components/ui/UiMenu.vue'

export interface ResourceActions {
  resetNavigation?(): void
  ready: boolean
  canComplete: boolean
  completing: boolean
  deleting: boolean
  launch(): unknown
  extract(): unknown
  openLocation(): unknown
  copyName(): unknown
  share(): unknown
  complete(): unknown
  delete(): unknown
}

// Both package cards and individual scenes dispatch through the mounted common details.
// A scene key includes its path: two favorites in the same VAR must never share a launch.
export function useResourceActions<T>(options: {
  selected: Ref<T | null>
  details: Ref<ResourceActions | null>
  busy: Readonly<Ref<boolean>>
  expanded: Ref<boolean>
  key(item: T): string
  select(item: T): unknown
  isScene(item: T): boolean
  isPackage(item: T): boolean
  canDelete(item: T): boolean
}) {
  const { t } = useI18n()
  const cardMenuOpen = ref(false)
  const cardMenuAnchor = ref<HTMLElement | null>(null)
  const cardMenuPoint = ref<{ x?: number; y?: number }>({})
  const queuedAction = ref<{ key: string; action: 'launch' | 'extract' } | null>(null)
  const cardMenuItems = computed<MenuItem[]>(() => {
    const selected = options.selected.value
    const packaged = !!selected && options.isPackage(selected)
    const scene = !!selected && options.isScene(selected)
    const details = options.details.value
    return [
      { value: 'launch', label: t('gameContent.launchScene'), icon: 'play', shortcut: 'Enter', disabled: !scene || options.busy.value },
      { value: 'open', label: t('packages.openLocation'), icon: 'folder', shortcut: 'Ctrl O' },
      { value: 'copy', label: t('ui.copyId'), icon: 'copy', shortcut: 'Ctrl C' },
      ...(packaged ? [{ value: 'share', label: t('share.btn'), icon: 'share' as const }] : []),
      { value: 'extract', label: t('gameContent.extractPreset'), icon: 'user', disabled: !scene || options.busy.value },
      ...(packaged ? [{ value: 'complete', label: t('ui.allDependencies'), icon: 'link' as const, disabled: !details?.canComplete || details.completing }] : []),
      ...(selected && options.canDelete(selected) ? [
        { value: 'separator', label: '', separator: true },
        { value: 'delete', label: t('ui.toTrash'), icon: 'trash' as const, shortcut: 'Del', danger: true, disabled: details?.deleting || options.busy.value },
      ] : []),
    ]
  })

  function select(item: T) {
    options.details.value?.resetNavigation?.()
    if (!options.selected.value || options.key(options.selected.value) !== options.key(item)) void options.select(item)
  }
  function openDetails(item: T) {
    if (options.busy.value) return
    select(item)
    options.expanded.value = true
  }
  async function openCardMenu(item: T, event: MouseEvent) {
    if (options.busy.value) return
    select(item)
    cardMenuAnchor.value = event.currentTarget instanceof HTMLElement ? event.currentTarget : null
    cardMenuPoint.value = event.type === 'contextmenu' ? { x: event.clientX, y: event.clientY } : {}
    // Let the details mount before the menu claims focus.
    await nextTick()
    if (!options.selected.value || options.key(options.selected.value) !== options.key(item)) return
    cardMenuOpen.value = true
  }
  async function queueSceneAction(item: T, action: 'launch' | 'extract') {
    if (options.busy.value || !options.isScene(item)) return
    select(item)
    // 启动场景的进度和结果显示在详情里，所以启动时把详情弹窗打开；提取外观有自己的弹窗
    if (action === 'launch') options.expanded.value = true
    queuedAction.value = { key: options.key(item), action }
    // Wait for the keyed details instance to mount before using its readiness.
    await nextTick()
    runQueuedAction()
  }
  function runQueuedAction() {
    const pending = queuedAction.value
    const selected = options.selected.value
    if (!pending || !selected || options.key(selected) !== pending.key || !options.details.value?.ready) return
    queuedAction.value = null
    if (pending.action === 'launch') void options.details.value.launch()
    else options.details.value.extract()
  }
  watch(() => options.details.value?.ready, runQueuedAction, { flush: 'post' })
  watch(() => options.selected.value && options.key(options.selected.value), key => {
    if (!key) options.expanded.value = false
    if (queuedAction.value?.key !== key) queuedAction.value = null
  })
  function handleCardAction(item: MenuItem) {
    const selected = options.selected.value
    if (!selected || options.busy.value || !cardMenuItems.value.some(entry => entry.value === item.value && !entry.disabled)) return
    if (item.value === 'launch' || item.value === 'extract') void queueSceneAction(selected, item.value)
    else {
      const actions = { open: 'openLocation', copy: 'copyName', share: 'share', complete: 'complete', delete: 'delete' } as const
      const action = actions[item.value as keyof typeof actions]
      if (action) void options.details.value?.[action]()
    }
  }
  async function packageShortcut(event: KeyboardEvent, item: T) {
    if (event.defaultPrevented || options.busy.value || (event.target as HTMLElement).closest('input, textarea, select, [contenteditable="true"]')) return
    const action = event.ctrlKey || event.metaKey ? ({ c: 'copy', o: 'open' } as Record<string, string>)[event.key.toLowerCase()] : event.key === 'Delete' ? 'delete' : undefined
    if (!action || (action === 'delete' && !options.canDelete(item))) return
    event.preventDefault()
    event.stopPropagation()
    select(item)
    await nextTick()
    const entry = cardMenuItems.value.find(entry => entry.value === action)
    if (entry) handleCardAction(entry)
  }
  return { cardMenuOpen, cardMenuAnchor, cardMenuPoint, cardMenuItems, openDetails, openCardMenu, queueSceneAction, handleCardAction, packageShortcut }
}
