<template>
  <div class="progress-bar-wrapper">
    <div class="progress-track">
      <div
        :class="['progress-fill', `progress-${variant}`, { animated }]"
        :style="{ width: `${clampedValue}%` }"
      >
        <div v-if="animated" class="progress-shimmer" />
      </div>
    </div>
    <span v-if="showLabel" class="progress-label">{{ Math.round(clampedValue) }}%</span>
  </div>
</template>

<script setup lang="ts">
import { computed } from 'vue'

interface Props {
  value: number
  variant?: 'default' | 'accent'
  showLabel?: boolean
  animated?: boolean
}

const props = withDefaults(defineProps<Props>(), {
  variant: 'default',
  showLabel: false,
  animated: false,
})

const clampedValue = computed(() => Math.min(100, Math.max(0, props.value)))
</script>

<style scoped>
.progress-bar-wrapper {
  display: flex;
  align-items: center;
  gap: var(--space-3);
  width: 100%;
}

.progress-track {
  flex: 1;
  height: 6px;
  background: var(--bg-hover);
  border-radius: var(--radius-full);
  overflow: hidden;
}

.progress-fill {
  position: relative;
  height: 100%;
  border-radius: var(--radius-full);
  transition: width var(--duration-slow) var(--ease);
  overflow: hidden;
}

.progress-default {
  background: linear-gradient(90deg, var(--accent-primary), var(--accent-secondary));
}

.progress-accent {
  background: var(--accent-gradient);
}

.progress-shimmer {
  position: absolute;
  top: 0;
  left: 0;
  right: 0;
  bottom: 0;
  background: linear-gradient(
    90deg,
    transparent 0%,
    rgba(255, 255, 255, 0.2) 50%,
    transparent 100%
  );
  background-size: 200% 100%;
  animation: shimmer 2s infinite linear;
}

.progress-label {
  font-size: var(--text-xs);
  font-weight: var(--font-medium);
  color: var(--text-secondary);
  min-width: 36px;
  text-align: right;
  font-variant-numeric: tabular-nums;
}
</style>
