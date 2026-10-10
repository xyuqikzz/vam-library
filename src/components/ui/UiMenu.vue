<script setup lang="ts">
import { nextTick, onBeforeUnmount, ref, watch } from 'vue'
import Icon from './Icon.vue'
import type { IconName } from './icons'
export interface MenuItem {
  value: string
  label: string
  icon?: IconName
  shortcut?: string
  danger?: boolean
  disabled?: boolean
  checked?: boolean
  separator?: boolean
}
const props = defineProps<{
  open: boolean
  items: MenuItem[]
  anchor?: HTMLElement | null
  x?: number
  y?: number
  label: string
}>()
const emit = defineEmits<{ 'update:open': [open: boolean]; select: [item: MenuItem] }>()
const menu = ref<HTMLElement | null>(null)
const position = ref({ left: '0px', top: '0px' })
let previous: HTMLElement | null = null
function close(restore = true) {
  emit('update:open', false)
  if (restore && previous?.isConnected) previous.focus()
}
function choose(item: MenuItem) {
  if (item.disabled || item.separator) return
  emit('select', item)
  close()
}
function outside(event: PointerEvent) {
  if (!menu.value?.contains(event.target as Node) && !props.anchor?.contains(event.target as Node))
    close(false)
}
function keydown(event: KeyboardEvent) {
  if (event.key === 'Escape') {
    event.preventDefault()
    event.stopPropagation()
    close()
    return
  }
  if (event.key === 'Tab') {
    close(false)
    return
  }
  if (!['ArrowDown', 'ArrowUp', 'Home', 'End'].includes(event.key)) return
  event.preventDefault()
  const buttons = Array.from(
    menu.value?.querySelectorAll<HTMLButtonElement>('button:not(:disabled)') || [],
  )
  if (!buttons.length) return
  let index = buttons.indexOf(document.activeElement as HTMLButtonElement)
  index =
    event.key === 'Home'
      ? 0
      : event.key === 'End'
        ? buttons.length - 1
        : (index + (event.key === 'ArrowDown' ? 1 : -1) + buttons.length) % buttons.length
  buttons[index].focus()
}
watch(
  () => props.open,
  async (open) => {
    document.removeEventListener('pointerdown', outside)
    if (!open) return
    previous =
      props.anchor ||
      (document.activeElement instanceof HTMLElement ? document.activeElement : null)
    await nextTick()
    if (!props.open || !menu.value) return
    const bounds = menu.value.getBoundingClientRect(),
      anchor = props.anchor?.getBoundingClientRect()
    position.value = {
      left: `${Math.max(8, Math.min(window.innerWidth - bounds.width - 8, props.x ?? anchor?.left ?? 8))}px`,
      top: `${Math.max(8, Math.min(window.innerHeight - bounds.height - 8, props.y ?? (anchor ? anchor.bottom + 6 : 8)))}px`,
    }
    menu.value.querySelector<HTMLButtonElement>('button:not(:disabled)')?.focus()
    document.addEventListener('pointerdown', outside)
  },
  { immediate: true },
)
onBeforeUnmount(() => document.removeEventListener('pointerdown', outside))
</script>
<template>
  <Teleport to="body">
    <div
      v-if="open"
      ref="menu"
      class="ui-menu"
      role="menu"
      :aria-label="label"
      :style="position"
      @keydown="keydown"
    >
      <template v-for="item in items" :key="item.value">
        <div v-if="item.separator" class="ui-menu-divider" role="separator" />
        <button
          v-else
          type="button"
          :role="item.checked === undefined ? 'menuitem' : 'menuitemradio'"
          :aria-checked="item.checked"
          :disabled="item.disabled"
          :class="{ danger: item.danger }"
          @click="choose(item)"
        >
          <Icon v-if="item.icon" :name="item.icon" :size="16" />
          <span>{{ item.label }}</span>
          <kbd v-if="item.shortcut">{{ item.shortcut }}</kbd>
          <Icon v-if="item.checked" class="ui-menu-check" name="check" :size="16" />
        </button>
      </template>
      <slot />
    </div>
  </Teleport>
</template>
<style scoped>
.ui-menu {
  position: fixed;
  z-index: calc(var(--z-overlay) + 50);
  min-width: 200px;
  max-width: calc(100vw - 16px);
  max-height: calc(100dvh - 16px);
  overflow: auto;
  padding: 6px;
  border-radius: var(--radius-card);
  background: var(--bg-menu);
  box-shadow:
    var(--shadow-lg),
    inset 0 0 0 1px var(--border-menu);
}
.ui-menu button {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  width: 100%;
  min-height: 36px;
  padding: 6px 10px;
  border-radius: var(--radius-sm);
  color: var(--text-primary);
  font-size: var(--text-base);
  font-weight: var(--font-semibold);
  text-align: left;
}
.ui-menu button:hover,
.ui-menu button:focus-visible {
  background: var(--bg-active);
}
.ui-menu button.danger {
  color: var(--color-error);
}
.ui-menu button:disabled {
  color: var(--text-disabled);
  cursor: not-allowed;
}
.ui-menu button span {
  flex: 1;
}
.ui-menu kbd {
  color: var(--text-tertiary);
  font-size: var(--text-xs);
}
.ui-menu-check {
  color: var(--accent-primary);
}
.ui-menu-divider {
  height: 1px;
  margin: 6px 8px;
  background: var(--bg-active);
}
</style>
