<script setup lang="ts">
import { ref } from 'vue'
import UiButton from './UiButton.vue'
import type { IconName } from './icons'
export interface SegmentOption {
  value: string
  label: string
  icon?: IconName
  disabled?: boolean
}
const props = defineProps<{
  modelValue: string
  options: SegmentOption[]
  label: string
  iconOnly?: boolean
}>()
const emit = defineEmits<{ 'update:modelValue': [value: string] }>()
const root = ref<HTMLElement | null>(null)
function navigate(event: KeyboardEvent) {
  if (!['ArrowLeft', 'ArrowRight', 'Home', 'End'].includes(event.key)) return
  event.preventDefault()
  event.stopPropagation()
  const enabled = props.options.filter((option) => !option.disabled)
  if (!enabled.length) return
  let index = enabled.findIndex((option) => option.value === props.modelValue)
  index =
    event.key === 'Home'
      ? 0
      : event.key === 'End'
        ? enabled.length - 1
        : (index + (event.key === 'ArrowRight' ? 1 : -1) + enabled.length) % enabled.length
  emit('update:modelValue', enabled[index].value)
  root.value?.querySelectorAll<HTMLButtonElement>('button:not(:disabled)')[index]?.focus()
}
</script>
<template>
  <div ref="root" class="ui-segmented" role="radiogroup" :aria-label="label" @keydown="navigate">
    <UiButton
      v-for="option in options"
      :key="option.value"
      type="button"
      role="radio"
      :aria-checked="modelValue === option.value"
      :aria-label="option.label"
      :icon="option.icon"
      :icon-only="iconOnly"
      variant="ghost"
      :tabindex="modelValue === option.value ? 0 : -1"
      :disabled="option.disabled"
      :class="{ selected: modelValue === option.value, 'icon-only': iconOnly }"
      @click="emit('update:modelValue', option.value)"
    >
      <span v-if="!iconOnly">{{ option.label }}</span>
    </UiButton>
  </div>
</template>
<style scoped>
.ui-segmented {
  display: inline-flex;
  align-items: center;
  padding: 3px;
  border-radius: var(--radius-full);
  background: var(--bg-input);
  flex-shrink: 0;
}
.ui-segmented :deep(button) {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  gap: var(--space-2);
  height: 34px;
  padding: 0 12px;
  border-radius: var(--radius-full);
  color: var(--text-muted-strong);
  font-size: var(--text-base);
  font-weight: var(--font-bold);
}
.ui-segmented :deep(button.selected) {
  background: var(--text-primary);
  color: var(--text-inverse);
}
.ui-segmented :deep(button.icon-only) {
  width: 40px;
  padding: 0;
}
.ui-segmented :deep(button:disabled) {
  color: var(--text-disabled);
  cursor: not-allowed;
}
</style>
