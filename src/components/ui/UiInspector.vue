<script setup lang="ts">
import { nextTick, onActivated, onBeforeUnmount, onDeactivated, onMounted, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import UiButton from './UiButton.vue'
import { focusInitial, modalStack } from './overlay'

const props = defineProps<{
  label: string
  title?: string
  contentKey?: string | number
  closeDisabled?: boolean
  closeLabel?: string
}>()
const emit = defineEmits<{ close: [] }>()
const { t } = useI18n()
const panel = ref<HTMLElement | null>(null)
const body = ref<HTMLElement | null>(null)
let returnFocus: HTMLElement | null = null
let active = false

function visible() {
  if (!panel.value) return false
  for (let element: HTMLElement | null = panel.value; element; element = element.parentElement) {
    const style = window.getComputedStyle(element)
    if (element.matches('[hidden], [inert], [aria-hidden="true"]') || style.display === 'none' || style.visibility === 'hidden') return false
  }
  return true
}
function close() {
  if (!props.closeDisabled) emit('close')
}
function keydown(event: KeyboardEvent) {
  // Menus and modal dialogs own Escape while they are above the inspector.
  if (event.key !== 'Escape' || event.defaultPrevented || modalStack.length || !visible() ||
      (event.target instanceof Element && event.target.closest('.ui-menu'))) return
  if (props.closeDisabled) return
  event.preventDefault()
  event.stopPropagation()
  close()
}
function rememberOpener() {
  if (document.activeElement instanceof HTMLElement && !panel.value?.contains(document.activeElement)) {
    returnFocus = document.activeElement
  }
}
async function activate() {
  if (active) return
  active = true
  rememberOpener()
  document.addEventListener('keydown', keydown)
  await nextTick()
  if (!active || !panel.value || !visible()) return
  focusInitial(props.closeDisabled ? panel.value : panel.value.querySelector<HTMLElement>('.ui-inspector-close') || panel.value)
}
function deactivate(restoreFocus = false) {
  active = false
  document.removeEventListener('keydown', keydown)
  if (restoreFocus && panel.value?.contains(document.activeElement) && returnFocus?.isConnected) {
    focusInitial(returnFocus)
  }
}
watch(() => props.contentKey, () => {
  rememberOpener()
  if (body.value) body.value.scrollTop = 0
}, { flush: 'post' })
onMounted(activate)
onActivated(activate)
onDeactivated(() => deactivate())
onBeforeUnmount(() => deactivate(true))
</script>

<template>
  <aside ref="panel" class="ui-inspector" :aria-label="label" tabindex="-1">
    <header v-if="title" class="ui-inspector-header">
      <h2 class="ui-inspector-title">{{ title }}</h2>
    </header>
    <div v-if="$slots.navigation" class="ui-inspector-navigation"><slot name="navigation" /></div>
    <div class="ui-inspector-controls" :class="{ 'has-title': title }">
    <slot name="header-actions" />
    <UiButton
      class="ui-inspector-close"
      variant="ghost"
      size="sm"
      icon="close"
      icon-only
      :aria-label="closeLabel || t('ui.closeDetail')"
      :disabled="closeDisabled"
      @click="close"
    />
    </div>
    <div ref="body" class="ui-inspector-body"><slot /></div>
  </aside>
</template>

<style scoped>
.ui-inspector {
  position: absolute;
  inset: 0 0 0 auto;
  z-index: var(--z-inspector);
  width: min(100%, var(--resource-inspector-width));
  min-width: 0;
  min-height: 0;
  display: flex;
  flex-direction: column;
  overflow: hidden;
  border-radius: var(--radius-shell);
  background: var(--bg-elevated);
  box-shadow: var(--shadow-lg);
}
.ui-inspector-header {
  flex-shrink: 0;
  padding: var(--space-panel);
  padding-right: calc(var(--space-panel) + var(--control-h-sm) + var(--space-item));
}
.ui-inspector-title {
  font-size: var(--text-dialog);
  font-weight: var(--font-bold);
  overflow-wrap: anywhere;
}
.ui-inspector-navigation {
  position: absolute;
  top: var(--space-stack);
  left: var(--space-stack);
  z-index: 1;
}
.ui-inspector-controls {
  position: absolute;
  top: var(--space-stack);
  right: var(--space-stack);
  z-index: 1;
  display: flex;
  gap: var(--space-item);
}
.ui-inspector-controls .ui-inspector-close {
  background: var(--bg-on-image);
}
.ui-inspector-controls.has-title {
  top: var(--space-panel);
}
.ui-inspector-body {
  flex: 1;
  min-height: 0;
  overflow: auto;
  overscroll-behavior: contain;
  scrollbar-gutter: stable;
}
</style>
