<template>
  <div class="search-input-wrapper">
    <svg class="search-icon" width="16" height="16" viewBox="0 0 16 16" fill="none">
      <circle cx="7" cy="7" r="4.5" stroke="currentColor" stroke-width="1.5" />
      <path d="M10.5 10.5L14 14" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" />
    </svg>
    <input
      ref="inputRef"
      type="text"
      class="search-input"
      :value="modelValue"
      :placeholder="placeholder"
      @input="onInput"
    />
    <Transition name="fade">
      <button
        v-if="modelValue"
        class="clear-button"
        :aria-label="clearButtonLabel"
        @click="onClear"
      >
        <svg width="14" height="14" viewBox="0 0 14 14" fill="none">
          <path d="M4 4L10 10M10 4L4 10" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" />
        </svg>
      </button>
    </Transition>
  </div>
</template>

<script setup lang="ts">
import { computed, ref } from 'vue'
import { useI18n } from 'vue-i18n'

interface Props {
  modelValue: string
  placeholder?: string
  clearAriaLabel?: string
}

const props = withDefaults(defineProps<Props>(), {
  placeholder: 'Search...',
})

const emit = defineEmits<{
  'update:modelValue': [value: string]
}>()

const { t } = useI18n()
const inputRef = ref<HTMLInputElement | null>(null)
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
  display: flex;
  align-items: center;
  width: 100%;
  max-width: 380px;
}

.search-icon {
  position: absolute;
  left: 12px;
  color: var(--text-tertiary);
  pointer-events: none;
  transition: color var(--duration-fast) var(--ease);
  flex-shrink: 0;
}

.search-input {
  width: 100%;
  height: 34px;
  padding: 0 36px 0 36px;
  background: var(--bg-input);
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-full);
  color: var(--text-primary);
  font-size: var(--text-sm);
  transition:
    border-color var(--duration-fast) var(--ease),
    background var(--duration-fast) var(--ease),
    box-shadow var(--duration-fast) var(--ease);
}

.search-input::placeholder {
  color: var(--text-tertiary);
}

.search-input:focus {
  background: rgba(28, 28, 30, 0.8);
  border-color: var(--accent-primary);
  box-shadow: 0 0 0 3px rgba(110, 107, 240, 0.15);
}

.search-input-wrapper:focus-within .search-icon {
  color: var(--accent-primary);
}

.clear-button {
  position: absolute;
  right: 8px;
  display: flex;
  align-items: center;
  justify-content: center;
  width: 22px;
  height: 22px;
  border-radius: var(--radius-full);
  color: var(--text-tertiary);
  transition:
    color var(--duration-fast) var(--ease),
    background var(--duration-fast) var(--ease);
}

.clear-button:hover {
  color: var(--text-primary);
  background: var(--bg-hover);
}
</style>
