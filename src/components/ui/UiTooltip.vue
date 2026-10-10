<script setup lang="ts">
import { nextTick, onBeforeUnmount, ref, useId, watch } from 'vue'
const props = withDefaults(defineProps<{ text: string; open?: boolean; anchor?: HTMLElement | null; id?: string }>(), { open: false })
const generatedId = useId()
const visible = ref(false)
const tip = ref<HTMLElement | null>(null)
const position = ref({ left: '0px', top: '0px' })
let timer: ReturnType<typeof setTimeout> | undefined
function place() {
  if (!props.anchor || !tip.value) return
  const box = props.anchor.getBoundingClientRect()
  const bounds = tip.value.getBoundingClientRect()
  position.value = { left: `${Math.max(8, Math.min(window.innerWidth - bounds.width - 8, box.left + (box.width - bounds.width) / 2))}px`, top: `${box.top > bounds.height + 12 ? box.top - bounds.height - 8 : box.bottom + 8}px` }
}
watch(() => props.open, open => {
  clearTimeout(timer)
  visible.value = false
  if (open) timer = setTimeout(async () => { visible.value = true; await nextTick(); place() }, 400)
}, { immediate: true })
onBeforeUnmount(() => clearTimeout(timer))
</script>
<template><Teleport to="body"><div v-if="visible && text" :id="id || generatedId" ref="tip" class="ui-tooltip" role="tooltip" :style="position">{{ text }}</div></Teleport></template>
<style scoped>
.ui-tooltip { position: fixed; z-index: calc(var(--z-toast) + 100); max-width: min(320px, calc(100vw - 16px)); min-height: 30px; display: flex; align-items: center; padding: 6px 12px; border-radius: var(--radius-sm); background: var(--text-primary); color: var(--text-inverse); font-size: var(--text-tooltip); font-weight: var(--font-bold); pointer-events: none; box-shadow: var(--shadow-sm); }
</style>
