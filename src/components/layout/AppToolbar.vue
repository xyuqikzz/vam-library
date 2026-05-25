<template>
  <header class="app-toolbar glass-toolbar" data-tauri-drag-region>
    <!-- Empty space acting as a window drag region -->
  </header>
</template>

<script setup lang="ts">
interface Props {
  title?: string
}

defineProps<Props>()
</script>

<style scoped>
.app-toolbar {
  height: var(--toolbar-height);
  z-index: var(--z-toolbar);
  -webkit-app-region: drag;
}
</style>
