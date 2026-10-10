<script setup lang="ts">
import { icons, type IconName } from './icons'
withDefaults(defineProps<{ name?: IconName; path?: string; size?: number | string }>(), { name: 'image', size: 18 })
</script>
<template>
  <svg :width="size" :height="size" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true" focusable="false">
    <slot><path :d="path || icons[name]" /></slot>
  </svg>
</template>
