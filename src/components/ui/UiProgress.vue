<script setup lang="ts">
import { computed } from 'vue'
const props = defineProps<{
  value: number
  max?: number
  circular?: boolean
  complete?: boolean
  label: string
  indeterminate?: boolean
}>()
const percent = computed(() =>
  Math.round(
    Math.max(
      0,
      Math.min(
        100,
        ((Number.isFinite(props.value) ? props.value : 0) / Math.max(1, props.max ?? 100)) * 100,
      ),
    ),
  ),
)
</script>
<template>
  <div
    class="ui-progress"
    :class="{ circular, complete, indeterminate }"
    role="progressbar"
    :aria-label="label"
    :aria-valuenow="indeterminate ? undefined : percent"
    :aria-valuemin="0"
    :aria-valuemax="100"
    :style="{ '--progress': `${percent}%` }"
  >
    <span v-if="circular">{{ percent }}%</span>
    <span v-else class="fill" />
  </div>
</template>
<style scoped>
.ui-progress {
  --progress-color: var(--accent-primary);
  height: 4px;
  border-radius: var(--radius-progress);
  background: var(--bg-active);
  overflow: hidden;
}
.ui-progress.complete {
  --progress-color: var(--color-success);
}
.fill {
  display: block;
  width: var(--progress);
  height: 100%;
  border-radius: inherit;
  background: var(--progress-color);
  transition: width var(--duration-base);
}
.circular {
  display: grid;
  place-items: center;
  width: 36px;
  height: 36px;
  flex-shrink: 0;
  border-radius: var(--radius-full);
  background: conic-gradient(var(--progress-color) var(--progress), var(--bg-active) 0);
}
.circular span {
  display: grid;
  place-items: center;
  width: 28px;
  height: 28px;
  border-radius: var(--radius-full);
  background: var(--bg-elevated);
  color: var(--text-primary);
  font-size: var(--text-xs);
  font-weight: var(--font-heavy);
}
.indeterminate .fill {
  width: 35%;
  animation: progress-slide 1.2s infinite var(--ease);
}
@keyframes progress-slide {
  from {
    transform: translateX(-100%);
  }
  to {
    transform: translateX(300%);
  }
}
</style>
