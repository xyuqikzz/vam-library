<script setup lang="ts">
defineProps<{ modelValue: boolean; disabled?: boolean; label?: string }>()
defineEmits<{ 'update:modelValue': [value: boolean] }>()
</script>
<template><label class="ui-switch"><input type="checkbox" role="switch" :checked="modelValue" :disabled="disabled" :aria-label="label" @change="$emit('update:modelValue', ($event.target as HTMLInputElement).checked)" /><span v-if="$slots.default"><slot /></span></label></template>
<style scoped>
.ui-switch { display: inline-flex; align-items: center; gap: var(--space-2); font-size: var(--text-base); cursor: pointer; }.ui-switch input { width: 40px; height: 24px; border: none; border-radius: var(--radius-full); background: var(--bg-active); animation: none; }.ui-switch input::after { position: absolute; top: 4px; left: 4px; width: 16px; height: 16px; border: none; border-radius: var(--radius-full); background: var(--text-tertiary); opacity: 1; transform: none; transition: transform var(--duration-fast); }.ui-switch input:checked { background: var(--accent-primary); }.ui-switch input:checked::after { transform: translateX(16px); background: var(--accent-ink); }.ui-switch:has(input:disabled) { color: var(--text-disabled); cursor: not-allowed; }
</style>
