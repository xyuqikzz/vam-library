<template>
  <div class="quick-actions">
    <button
      class="quick-btn"
      :disabled="!vamRoot"
      @click="vamRoot && $emit('open', vamRoot)"
    >
      <svg width="14" height="14" viewBox="0 0 24 24" fill="none">
        <path d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"/>
      </svg>
      <span>{{ $t('dashboard.gameRoot') }}</span>
    </button>
    <button
      class="quick-btn"
      :disabled="!screenshotPath"
      @click="screenshotPath && $emit('open', screenshotPath)"
    >
      <svg width="14" height="14" viewBox="0 0 24 24" fill="none">
        <path d="M23 19a2 2 0 0 1-2 2H3a2 2 0 0 1-2-2V8a2 2 0 0 1 2-2h4l2-3h6l2 3h4a2 2 0 0 1 2 2z" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"/>
        <circle cx="12" cy="13" r="4" stroke="currentColor" stroke-width="1.5"/>
      </svg>
      <span>{{ $t('dashboard.screenshotDirectory') }}</span>
    </button>
    <button
      class="quick-btn"
      :disabled="!appearancePresetPath"
      @click="appearancePresetPath && $emit('open', appearancePresetPath)"
    >
      <span>{{ $t('dashboard.appearancePresetDirectory') }}</span>
    </button>
    <button
      class="quick-btn"
      :disabled="!clothingPresetPath"
      @click="clothingPresetPath && $emit('open', clothingPresetPath)"
    >
      <span>{{ $t('dashboard.clothingPresetDirectory') }}</span>
    </button>
    <button
      v-if="vamRoot"
      class="quick-btn"
      @click="$emit('gameConfig')"
    >
      <span>{{ $t('dashboard.gameConfig') }}</span>
    </button>
  </div>
</template>

<script setup lang="ts">
import { useI18n } from 'vue-i18n'

defineProps<{
  vamRoot: string | null
  screenshotPath: string | null
  appearancePresetPath: string | null
  clothingPresetPath: string | null
}>()

defineEmits<{
  open: [path: string]
  gameConfig: []
}>()

useI18n()
</script>

<style scoped>
.quick-actions {
  display: flex;
  flex-wrap: wrap;
  gap: var(--space-2);
}

.quick-btn {
  display: flex;
  align-items: center;
  gap: 6px;
  height: 32px;
  padding: 0 var(--space-3);
  border-radius: var(--radius-md);
  background: var(--bg-subtle);
  border: 1px solid var(--border-subtle);
  color: var(--text-secondary);
  font-size: var(--text-sm);
  font-weight: var(--font-medium);
  cursor: pointer;
  transition:
    color var(--duration-fast) var(--ease),
    background var(--duration-fast) var(--ease),
    border-color var(--duration-fast) var(--ease);
}

.quick-btn:not(:disabled):hover {
  color: var(--text-primary);
  background: var(--bg-hover);
  border-color: var(--border-default);
}

.quick-btn:not(:disabled):active {
  transform: scale(0.98);
}

.quick-btn:disabled {
  opacity: 0.4;
  cursor: not-allowed;
}
</style>
