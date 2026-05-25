<template>
  <div
    :class="[
      'glass-card-component',
      {
        hoverable,
        clickable,
        [`padding-${padding}`]: true,
      },
    ]"
    @click="handleClick"
  >
    <slot />
  </div>
</template>

<script setup lang="ts">
interface Props {
  hoverable?: boolean
  padding?: 'sm' | 'md' | 'lg'
  clickable?: boolean
}

const props = withDefaults(defineProps<Props>(), {
  hoverable: false,
  padding: 'md',
  clickable: false,
})

const emit = defineEmits<{
  click: [event: MouseEvent]
}>()

function handleClick(event: MouseEvent) {
  if (props.clickable) {
    emit('click', event)
  }
}
</script>

<style scoped>
.glass-card-component {
  background: var(--glass-bg);
  backdrop-filter: var(--glass-blur);
  -webkit-backdrop-filter: var(--glass-blur);
  border: 1px solid var(--border-default);
  border-radius: var(--radius-lg);
  box-shadow: var(--glass-shadow);
  transition:
    transform var(--duration-base) var(--ease),
    box-shadow var(--duration-base) var(--ease),
    border-color var(--duration-base) var(--ease);
}

.glass-card-component.padding-sm {
  padding: var(--space-3);
}

.glass-card-component.padding-md {
  padding: var(--space-5);
}

.glass-card-component.padding-lg {
  padding: var(--space-8);
}

.glass-card-component.hoverable:hover {
  transform: translateY(-2px);
  box-shadow: var(--glass-shadow-lg);
  border-color: var(--border-strong);
}

.glass-card-component.clickable {
  cursor: pointer;
}

.glass-card-component.clickable:active {
  transform: translateY(0px);
  box-shadow: var(--glass-shadow);
  transition-duration: var(--duration-fast);
}
</style>
