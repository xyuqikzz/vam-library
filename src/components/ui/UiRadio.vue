<script setup lang="ts">
defineProps<{ modelValue: string | number; value: string | number; name?: string; label?: string; disabled?: boolean }>()
defineEmits<{ 'update:modelValue': [value: string | number] }>()
</script>
<template><label class="ui-radio"><input type="radio" :checked="modelValue === value" :value="value" :name="name" :aria-label="label" :disabled="disabled" @change="$emit('update:modelValue', value)" /><span v-if="$slots.default"><slot /></span></label></template>
<style scoped>
.ui-radio { display: inline-flex; align-items: center; gap: var(--space-2); font-size: var(--text-base); cursor: pointer; }.ui-radio input { appearance: none; flex-shrink: 0; width: 20px; height: 20px; border: 2px solid var(--border-control); border-radius: var(--radius-full); background: var(--bg-input); cursor: inherit; }.ui-radio input:checked { border: 6px solid var(--accent-primary); }.ui-radio:has(input:disabled) { color: var(--text-disabled); cursor: not-allowed; opacity: .5; }
</style>
