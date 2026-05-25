<template>
  <div
    :class="['skeleton-loader', variant]"
    :style="{ width: width, height: height, borderRadius: radius }"
  >
    <div class="skeleton-shimmer" />
  </div>
</template>

<script setup lang="ts">
interface Props {
  variant?: 'text' | 'circle' | 'card'
  width?: string
  height?: string
  radius?: string
}

withDefaults(defineProps<Props>(), {
  variant: 'text',
  width: '100%',
  height: '14px',
  radius: 'var(--radius-sm)',
})
</script>

<style scoped>
.skeleton-loader {
  position: relative;
  overflow: hidden;
  background: var(--bg-hover);
}

.skeleton-loader.circle {
  border-radius: 50%;
}

.skeleton-loader.card {
  border-radius: var(--radius-lg);
}

.skeleton-shimmer {
  position: absolute;
  inset: 0;
  background: linear-gradient(
    90deg,
    transparent 0%,
    rgba(255, 255, 255, 0.04) 40%,
    rgba(255, 255, 255, 0.06) 50%,
    rgba(255, 255, 255, 0.04) 60%,
    transparent 100%
  );
  animation: shimmer 2s ease-in-out infinite;
}

@keyframes shimmer {
  0% { transform: translateX(-100%); }
  100% { transform: translateX(100%); }
}
</style>
