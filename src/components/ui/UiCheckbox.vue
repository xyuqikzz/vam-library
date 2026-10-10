<script setup lang="ts">
withDefaults(defineProps<{ modelValue: boolean; disabled?: boolean; label?: string; indeterminate?: boolean }>(), { disabled: false })
defineEmits<{ 'update:modelValue': [value: boolean] }>()
</script>
<template><label class="ui-checkbox"><input type="checkbox" :checked="modelValue" :indeterminate="indeterminate" :disabled="disabled" :aria-label="label" @change="$emit('update:modelValue', ($event.target as HTMLInputElement).checked)" /><span v-if="$slots.default"><slot /></span></label></template>
<style scoped>
.ui-checkbox { display: inline-flex; align-items: center; gap: var(--space-2); cursor: pointer; font-size: var(--text-base); }.ui-checkbox input { width: 20px; height: 20px; border: 2px solid var(--border-control); border-radius: var(--radius-xs); background: var(--bg-input); animation: none; }.ui-checkbox input:checked, .ui-checkbox input:indeterminate { background: var(--accent-primary); border-color: var(--accent-primary); }.ui-checkbox input::after { border-color: var(--accent-ink); left: 5px; top: 2px; }.ui-checkbox input:indeterminate::after { width: 8px; height: 0; left: 4px; top: 7px; border-width: 0 0 2px; transform: none; opacity: 1; }.ui-checkbox:has(input:disabled) { color: var(--text-disabled); cursor: not-allowed; }
</style>
