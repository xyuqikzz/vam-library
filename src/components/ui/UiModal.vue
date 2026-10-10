<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, ref, useAttrs, useId, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import UiButton from './UiButton.vue'
import { enterModal, leaveModal, modalStack, focusable } from './overlay'
defineOptions({ inheritAttrs: false })
const props = withDefaults(
  defineProps<{
    open?: boolean
    modelValue?: boolean
    title?: string
    description?: string
    ariaLabel?: string
    wide?: boolean
    closeDisabled?: boolean
    dismissible?: boolean
    showClose?: boolean
    imperative?: boolean
  }>(),
  { open: true, modelValue: undefined, dismissible: true, showClose: true },
)
const emit = defineEmits<{
  'update:open': [open: boolean]
  'update:modelValue': [open: boolean]
  close: []
  cancel: [event: Event]
}>()
const { t } = useI18n()
const imperativeOpen = ref(false)
const attrs = useAttrs()
const visible = computed(() =>
  props.imperative ? imperativeOpen.value : (props.modelValue ?? props.open),
)
const dialog = ref<HTMLElement | null>(null)
const titleId = useId()
const descriptionId = useId()
const stackId = Symbol('modal')
let returnFocus: HTMLElement | null = null
let active = false
function close() {
  if (props.closeDisabled || !visible.value) return
  imperativeOpen.value = false
  emit('update:open', false)
  emit('update:modelValue', false)
  emit('close')
}
function requestClose() {
  if (!props.dismissible || props.closeDisabled) return
  const event = new Event('cancel', { cancelable: true })
  emit('cancel', event)
  if (!event.defaultPrevented) close()
}
function focusFirst() {
  if (dialog.value)
    (
      dialog.value.querySelector<HTMLElement>('[autofocus]') ||
      focusable(dialog.value)[0] ||
      dialog.value
    ).focus()
}
function keydown(event: KeyboardEvent) {
  if (
    modalStack[modalStack.length - 1] !== stackId ||
    !dialog.value ||
    (event.target instanceof Element && event.target.closest('.ui-menu'))
  )
    return
  if (event.key === 'Escape' && !event.defaultPrevented) {
    event.preventDefault()
    event.stopPropagation()
    requestClose()
  }
  if (event.key === 'Tab') {
    const elements = focusable(dialog.value)
    const first = elements[0],
      last = elements[elements.length - 1]
    if (!first) {
      event.preventDefault()
      dialog.value.focus()
    } else if (
      event.shiftKey &&
      (document.activeElement === first || !dialog.value.contains(document.activeElement))
    ) {
      event.preventDefault()
      last?.focus()
    } else if (
      !event.shiftKey &&
      (document.activeElement === last || !dialog.value.contains(document.activeElement))
    ) {
      event.preventDefault()
      first.focus()
    }
  }
}
function keepFocus(event: FocusEvent) {
  if (event.target instanceof Element && event.target.closest('.ui-menu')) return
  if (
    modalStack[modalStack.length - 1] === stackId &&
    dialog.value &&
    !dialog.value.contains(event.target as Node)
  )
    focusFirst()
}
function deactivate() {
  if (!active) return
  active = false
  document.removeEventListener('keydown', keydown)
  document.removeEventListener('focusin', keepFocus)
  leaveModal(stackId)
  if (returnFocus?.isConnected) returnFocus.focus()
}
watch(
  visible,
  async (open) => {
    if (!open) {
      deactivate()
      return
    }
    if (active) return
    active = true
    returnFocus = document.activeElement instanceof HTMLElement ? document.activeElement : null
    enterModal(stackId)
    await nextTick()
    if (!active) return
    focusFirst()
    document.addEventListener('keydown', keydown)
    document.addEventListener('focusin', keepFocus)
  },
  { immediate: true },
)
onBeforeUnmount(deactivate)
defineExpose({
  focus: focusFirst,
  showModal: () => {
    imperativeOpen.value = true
  },
  close,
  open: visible,
})
</script>
<template>
  <Teleport to="body">
    <Transition name="fade">
      <div v-if="visible" class="ui-modal-overlay" @click.self="requestClose()">
        <dialog
          open
          ref="dialog"
          v-bind="$attrs"
          class="ui-modal"
          :class="{ wide }"
          role="dialog"
          aria-modal="true"
          :aria-labelledby="title ? titleId : (attrs['aria-labelledby'] as string | undefined)"
          :aria-label="title ? undefined : ariaLabel || t('ui.dialog')"
          :aria-describedby="
            description ? descriptionId : (attrs['aria-describedby'] as string | undefined)
          "
          tabindex="-1"
        >
          <header v-if="title || $slots.header || showClose" class="ui-modal-header">
            <slot name="header">
              <h2 v-if="title" :id="titleId">{{ title }}</h2>
            </slot>
            <UiButton
              v-if="showClose"
              class="ui-modal-close"
              variant="ghost"
              size="sm"
              icon="close"
              icon-only
              :aria-label="t('common.close')"
              :disabled="closeDisabled"
              @click="requestClose"
            />
          </header>
          <p v-if="description" :id="descriptionId" class="ui-modal-description">
            {{ description }}
          </p>
          <div class="ui-modal-body"><slot /></div>
          <footer v-if="$slots.footer" class="ui-modal-footer"><slot name="footer" /></footer>
        </dialog>
      </div>
    </Transition>
  </Teleport>
</template>
<style scoped>
.ui-modal-overlay {
  position: fixed;
  inset: 0;
  z-index: var(--z-modal);
  display: flex;
  align-items: center;
  justify-content: center;
  padding: 24px;
  background: var(--bg-overlay);
}
.ui-modal {
  position: relative;
  margin: 0;
  display: block;
  width: min(560px, 100%);
  max-height: calc(100dvh - 48px);
  overflow: auto;
  padding: 22px;
  border: none;
  border-radius: var(--radius-xl);
  background: var(--bg-elevated);
  box-shadow:
    var(--shadow-lg),
    inset 0 0 0 1px var(--border-subtle);
  color: var(--text-primary);
}
.ui-modal.wide {
  width: min(1200px, 100%);
}
.ui-modal-header {
  display: flex;
  align-items: center;
  gap: var(--space-3);
  margin-bottom: var(--space-4);
}
.ui-modal-header h2 {
  flex: 1;
  font-size: var(--text-dialog);
  font-weight: var(--font-heavy);
}
.ui-modal-close {
  margin-left: auto;
}
.ui-modal-description {
  margin-bottom: var(--space-4);
  font-size: var(--text-description);
  color: var(--text-secondary);
}
.ui-modal-body {
  min-width: 0;
}
.ui-modal-footer {
  display: flex;
  justify-content: flex-end;
  flex-wrap: wrap;
  gap: var(--space-2);
  margin-top: var(--space-5);
}
@media (max-width: 600px) {
  .ui-modal-overlay {
    padding: 12px;
  }
  .ui-modal {
    max-height: calc(100dvh - 24px);
    padding: 18px;
  }
}
</style>
