<script setup lang="ts">
import { ref } from 'vue'
import { useI18n } from 'vue-i18n'
import Icon from '@/components/ui/Icon.vue'
defineProps<{ packageId: string; thumbnail?: string | null; disabled?: boolean }>()
const emit = defineEmits<{ details: [] }>()
const { t } = useI18n()
const failed = ref('')
</script>
<template>
  <div class="cleanup-resource-row">
    <button class="cleanup-resource-open" :disabled="disabled" :aria-label="t('cleanup.viewPackageDetails', { name: packageId })" @click="emit('details')">
      <span class="cleanup-resource-thumb">
        <img v-if="thumbnail && failed !== thumbnail" data-resource-preview :src="thumbnail" :alt="packageId" @error="failed = thumbnail" />
        <Icon v-else name="package" :size="20" />
      </span>
      <span class="cleanup-resource-copy"><strong>{{ packageId }}</strong><slot /></span>
    </button>
    <div v-if="$slots.actions" class="cleanup-resource-actions"><slot name="actions" /></div>
  </div>
</template>
<style scoped>
.cleanup-resource-row { display: flex; align-items: center; gap: var(--space-stack); min-width: 0; }
.cleanup-resource-open { display: flex; align-items: center; gap: var(--space-stack); flex: 1; min-width: 0; border: 0; background: transparent; color: var(--text-primary); padding: var(--space-item); border-radius: var(--radius-control-sm); text-align: left; cursor: pointer; }
.cleanup-resource-open:hover:not(:disabled) { background: var(--bg-ghost-hover); }
.cleanup-resource-open:disabled { cursor: default; }
.cleanup-resource-thumb { display: grid; place-items: center; flex-shrink: 0; width: var(--control-h-lg); height: var(--control-h-lg); border-radius: var(--radius-xs); background: var(--bg-control); color: var(--text-tertiary); overflow: hidden; }
.cleanup-resource-thumb img { width: 100%; height: 100%; object-fit: contain; background: var(--bg-base); }
.cleanup-resource-copy { display: flex; flex-direction: column; gap: var(--space-1); min-width: 0; }
.cleanup-resource-copy strong { font-size: var(--text-sm); overflow-wrap: anywhere; }
.cleanup-resource-actions { display: flex; align-items: center; flex-wrap: wrap; gap: var(--space-item); flex-shrink: 0; }
@media (max-width: 600px) { .cleanup-resource-row { flex-wrap: wrap; } .cleanup-resource-actions { padding-left: calc(var(--control-h-lg) + var(--space-stack) + var(--space-item)); } }
</style>
