<template>
  <div class="search-input-wrapper">
    <UiInput
      ref="inputRef"
      :model-value="modelValue"
      :placeholder="placeholder || t('ui.search')"
      :aria-label="placeholder || t('ui.search')"
      :shortcut="modelValue ? undefined : shortcut"
      search
      @input="onInput"
    />
    <UiButton
      v-if="modelValue"
      class="search-clear"
      icon="close"
      icon-only
      size="sm"
      variant="ghost"
      :aria-label="clearButtonLabel"
      @click="onClear"
    />
  </div>
</template>

<script setup lang="ts">
import UiButton from '@/components/ui/UiButton.vue'
import UiInput from '@/components/ui/UiInput.vue'

import { computed, ref, onBeforeUnmount } from 'vue'
import { useI18n } from 'vue-i18n'

interface Props {
  modelValue: string
  placeholder?: string
  clearAriaLabel?: string
  shortcut?: string
}

const props = withDefaults(defineProps<Props>(), {
  placeholder: undefined,
})

const emit = defineEmits<{
  'update:modelValue': [value: string]
}>()

const { t } = useI18n()
const inputRef = ref<{ focus: () => void; select: () => void } | null>(null)
let debounceTimer: ReturnType<typeof setTimeout> | null = null
const clearButtonLabel = computed(() => props.clearAriaLabel || t('common.clear'))

function onInput(event: Event) {
  const value = (event.target as HTMLInputElement).value
  if (debounceTimer) clearTimeout(debounceTimer)
  debounceTimer = setTimeout(() => {
    emit('update:modelValue', value)
  }, 300)
}

function onClear() {
  emit('update:modelValue', '')
  if (debounceTimer) clearTimeout(debounceTimer)
  inputRef.value?.focus()
}

onBeforeUnmount(() => { if (debounceTimer) clearTimeout(debounceTimer) })

function focus() {
  inputRef.value?.focus()
}

defineExpose({
  focus,
})
</script>

<style scoped>
.search-input-wrapper {
  position: relative;
  min-width: 0;
  width: 100%;
  max-width: 380px;
}
.search-input-wrapper :deep(.ui-input-field) {
  width: 100%;
}
.search-input-wrapper:has(.search-clear) :deep(input) {
  padding-right: var(--space-8);
}
.search-clear {
  position: absolute;
  right: 4px;
  top: 4px;
}
</style>
