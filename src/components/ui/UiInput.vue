<script setup lang="ts">
import { ref, useId } from 'vue'
import Icon from './Icon.vue'
defineOptions({ inheritAttrs: false })
const props = withDefaults(defineProps<{ modelValue?: string | number; type?: string; search?: boolean; error?: string; disabled?: boolean; shortcut?: string; modelModifiers?: { number?: boolean; trim?: boolean; lazy?: boolean } }>(), { type: 'text' })
const emit = defineEmits<{ 'update:modelValue': [value: string | number] }>()
const input = ref<HTMLInputElement | null>(null)
const errorId = useId()
function update(event: Event) {
  let value: string | number = (event.target as HTMLInputElement).value
  if (props.modelModifiers?.trim) value = value.trim()
  if (props.modelModifiers?.number || props.type === 'number') { const numeric = Number.parseFloat(value); if (!Number.isNaN(numeric)) value = numeric }
  emit('update:modelValue', value)
}
defineExpose({ focus: () => input.value?.focus(), select: () => input.value?.select(), input })
</script>
<template><div class="ui-input-field" :class="{ 'is-search': search, 'has-error': !!error, disabled }"><div class="ui-input-shell"><Icon v-if="search" name="search" class="search-icon" /><input ref="input" v-bind="$attrs" :type="search ? 'search' : type" :value="modelValue" :disabled="disabled" :aria-invalid="error ? 'true' : undefined" :aria-describedby="error ? errorId : $attrs['aria-describedby'] as string | undefined" @input="!modelModifiers?.lazy && update($event)" @change="modelModifiers?.lazy && update($event)" /><kbd v-if="shortcut">{{ shortcut }}</kbd></div><span v-if="error" :id="errorId" class="ui-input-error">{{ error }}</span></div></template>
<style scoped>
.ui-input-field { min-width: 0; }.ui-input-shell { display: flex; align-items: center; gap: var(--space-2); height: var(--control-h-lg); border-radius: var(--radius-md); background: var(--bg-input); padding: 0 14px; }.ui-input-shell:focus-within { box-shadow: inset 0 0 0 2px var(--accent-primary); }.ui-input-shell input { width: 100%; min-width: 0; height: 100%; padding: 0; background: none; border: none; box-shadow: none; border-radius: inherit; color: var(--text-primary); font-size: var(--text-base); }.ui-input-shell input::placeholder { color: var(--text-tertiary); }.is-search .ui-input-shell { height: var(--control-h-md); border-radius: var(--radius-full); }.search-icon { color: var(--text-tertiary); flex-shrink: 0; }kbd { padding: 0 8px; display: grid; place-items: center; height: 26px; border-radius: var(--radius-full); background: var(--bg-control); color: var(--text-tertiary); font-size: var(--text-xs); white-space: nowrap; }.has-error .ui-input-shell { background: var(--color-error-subtle); box-shadow: inset 0 0 0 2px var(--color-error); }.ui-input-error { display: block; margin-top: 6px; color: var(--color-error); font-size: var(--text-sm); font-weight: var(--font-semibold); }.disabled .ui-input-shell { background: var(--bg-input-disabled); }.disabled input { color: var(--text-disabled); cursor: not-allowed; }
</style>
