<script setup lang="ts">
import { useI18n } from 'vue-i18n'
import Icon from './Icon.vue'
withDefaults(defineProps<{ selected?: boolean; count?: number; color?: string; removable?: boolean; disabled?: boolean; label?: string }>(), { selected: false })
defineEmits<{ click: [event: MouseEvent]; remove: [] }>()
const { t } = useI18n()
</script>
<template><span class="ui-chip-group" :class="{ selected }"><button type="button" class="ui-chip" :aria-pressed="selected" :disabled="disabled" @click="$emit('click', $event)"><span v-if="color" class="ui-chip-dot" :style="{ background: color }" /><slot>{{ label }}</slot><span v-if="count !== undefined" class="ui-chip-count">{{ count }}</span></button><button v-if="removable && selected" type="button" class="ui-chip-remove" :aria-label="t('ui.removeFilter')" :title="t('ui.removeFilter')" :disabled="disabled" @click="$emit('remove')"><Icon name="close" :size="12" /></button></span></template>
<style scoped>
.ui-chip-group { display: inline-flex; align-items: center; flex-shrink: 0; background: var(--bg-input); color: var(--text-muted-strong); border-radius: var(--radius-full); }.ui-chip-group:hover { background: var(--bg-control); }.ui-chip-group.selected { background: var(--text-primary); color: var(--text-inverse); }
.ui-chip { height: var(--control-h-sm); padding: 0 12px; display: inline-flex; align-items: center; gap: 6px; border-radius: var(--radius-full); font-size: var(--text-base); font-weight: var(--font-bold); white-space: nowrap; }.ui-chip-dot { width: 7px; height: 7px; border-radius: var(--radius-full); }.ui-chip-count { color: var(--text-tertiary); font-weight: var(--font-medium); }.selected .ui-chip-count { color: var(--text-disabled); }.ui-chip-remove { display: grid; place-items: center; margin-right: 6px; width: 20px; height: 20px; border-radius: var(--radius-full); }.ui-chip:disabled { opacity: .5; cursor: not-allowed; }
</style>
