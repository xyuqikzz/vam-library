<template>
  <span :class="['badge', `badge-${variant}`, `badge-${size}`]">
    <slot />
  </span>
</template>

<script setup lang="ts">
interface Props {
  variant?: 'default' | 'success' | 'warning' | 'error' | 'info' | 'scene' | 'appearance' | 'morph' | 'clothing' | 'plugin' | 'texture' | 'asset' | 'hair'
  size?: 'sm' | 'md'
}

withDefaults(defineProps<Props>(), {
  variant: 'default',
  size: 'md',
})
</script>

<style scoped>
.badge {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  border-radius: var(--radius-full);
  font-weight: var(--font-medium);
  white-space: nowrap;
  line-height: 1;
  letter-spacing: 0.02em;
}

/* ── Sizes ─────────────────────────────────────────────────── */
.badge-sm {
  padding: 2px 8px;
  font-size: 10px;
  height: 20px;
}

.badge-md {
  padding: 3px 10px;
  font-size: var(--text-xs);
  height: 24px;
}

/* ── Variants ──────────────────────────────────────────────── */
.badge-default {
  background: var(--bg-hover);
  color: var(--text-secondary);
}

.badge-success {
  background: var(--color-success-bg);
  color: var(--color-success);
}

.badge-warning {
  background: var(--color-warning-bg);
  color: var(--color-warning);
}

.badge-error {
  background: var(--color-error-bg);
  color: var(--color-error);
}

.badge-info {
  background: var(--color-info-bg);
  color: var(--color-info);
}

.badge-scene {
  background: var(--color-scene-bg);
  color: var(--color-scene);
}

.badge-appearance {
  background: var(--color-appearance-bg);
  color: var(--color-appearance);
}

.badge-morph {
  background: var(--color-morph-bg);
  color: var(--color-morph);
}

.badge-clothing {
  background: var(--color-clothing-bg);
  color: var(--color-clothing);
}

.badge-plugin {
  background: var(--color-plugin-bg);
  color: var(--color-plugin);
}

.badge-texture {
  background: var(--color-texture-bg);
  color: var(--color-texture);
}

.badge-asset {
  background: var(--color-asset-bg);
  color: var(--color-asset);
}

.badge-hair {
  background: var(--color-hair-bg);
  color: var(--color-hair);
}
</style>
