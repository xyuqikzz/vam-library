<template>
  <div class="empty-state">
    <div class="empty-icon-wrapper">
      <svg width="48" height="48" viewBox="0 0 24 24" fill="none">
        <path :d="icon" stroke="currentColor" stroke-width="1.2" stroke-linecap="round" stroke-linejoin="round" />
      </svg>
    </div>
    <h3 class="empty-title">{{ title }}</h3>
    <p class="empty-description">{{ description }}</p>
    <div v-if="$slots.action" class="empty-action">
      <slot name="action" />
    </div>
  </div>
</template>

<script setup lang="ts">
interface Props {
  icon: string
  title: string
  description: string
}

defineProps<Props>()
</script>

<style scoped>
.empty-state {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  padding: var(--space-12) var(--space-8);
  text-align: center;
}

.empty-icon-wrapper {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 80px;
  height: 80px;
  border-radius: var(--radius-xl);
  background: var(--bg-hover);
  color: var(--text-tertiary);
  margin-bottom: var(--space-6);
}

.empty-title {
  font-size: var(--text-lg);
  font-weight: var(--font-semibold);
  color: var(--text-primary);
  margin-bottom: var(--space-2);
}

.empty-description {
  font-size: var(--text-sm);
  color: var(--text-secondary);
  max-width: 320px;
  line-height: var(--leading-relaxed);
}

.empty-action {
  margin-top: var(--space-6);
}
</style>
