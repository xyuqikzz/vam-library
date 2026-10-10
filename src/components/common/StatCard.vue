<template>
  <div class="stat-card glass-card-component" :style="{ '--stat-color': color }">
    <div class="stat-header">
      <div class="stat-icon-wrapper">
        <Icon  :size="20" :path="icon" />
      </div>
      <div v-if="trend" :class="['stat-trend', `trend-${trend}`]">
        <Icon v-if="trend === 'up'" :size="12" name="outline-4" />
        <Icon v-else-if="trend === 'down'" :size="12" name="outline-5" />
        <Icon v-else :size="12" name="outline-6" />
      </div>
    </div>
    <div class="stat-value">{{ value }}</div>
    <div class="stat-title">{{ title }}</div>
    <div v-if="subtitle" class="stat-subtitle">{{ subtitle }}</div>
  </div>
</template>

<script setup lang="ts">
import Icon from '@/components/ui/Icon.vue'
interface Props {
  title: string
  value: string | number
  subtitle?: string
  icon: string
  trend?: 'up' | 'down' | 'neutral'
  color: string
}

withDefaults(defineProps<Props>(), {
  subtitle: undefined,
  trend: undefined,
})
</script>

<style scoped>
.stat-card {
  background: var(--bg-surface);
  border: none;
  border-radius: var(--radius-lg);
  box-shadow: none;
  padding: var(--space-5);
  transition:
    transform var(--duration-base) var(--ease),
    box-shadow var(--duration-base) var(--ease),
    border-color var(--duration-base) var(--ease);
}

.stat-card:hover {
  transform: translateY(-1px);
  box-shadow: var(--shadow-md);
  border-color: var(--border-strong);
}

.stat-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-bottom: var(--space-4);
}

.stat-icon-wrapper {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 40px;
  height: 40px;
  border-radius: var(--radius-md);
  background: color-mix(in srgb, var(--stat-color) 12%, transparent);
  color: var(--stat-color);
}

.stat-trend {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 24px;
  height: 24px;
  border-radius: var(--radius-full);
}

.trend-up {
  background: var(--color-success-bg);
  color: var(--color-success);
}

.trend-down {
  background: var(--color-error-bg);
  color: var(--color-error);
}

.trend-neutral {
  background: var(--bg-hover);
  color: var(--text-tertiary);
}

.stat-value {
  font-size: var(--text-2xl);
  font-weight: var(--font-bold);
  color: var(--text-primary);
  line-height: 1;
  margin-bottom: var(--space-1);
  font-variant-numeric: tabular-nums;
}

.stat-title {
  font-size: var(--text-sm);
  font-weight: var(--font-medium);
  color: var(--text-secondary);
}

.stat-subtitle {
  font-size: var(--text-xs);
  color: var(--text-tertiary);
  margin-top: var(--space-1);
}
</style>
