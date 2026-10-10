<script setup lang="ts">
import { computed } from 'vue'
import { icons, type IconName } from './icons'
const props = withDefaults(
  defineProps<{ name?: IconName; path?: string; size?: number | string }>(),
  { name: 'image', size: 18 },
)
const definition = computed(() => icons[props.name])
const iconPath = computed(
  () =>
    props.path || (typeof definition.value === 'string' ? definition.value : definition.value.path),
)
const scale = computed(() =>
  typeof definition.value === 'string'
    ? undefined
    : 'scale' in definition.value
      ? definition.value.scale
      : undefined,
)
</script>
<template>
  <svg
    :width="size"
    :height="size"
    viewBox="0 0 24 24"
    fill="none"
    stroke="currentColor"
    stroke-width="1.7"
    stroke-linecap="round"
    stroke-linejoin="round"
    aria-hidden="true"
    focusable="false"
  >
    <g :transform="scale ? `scale(${scale})` : undefined">
      <slot><path :d="iconPath" /></slot>
    </g>
  </svg>
</template>
