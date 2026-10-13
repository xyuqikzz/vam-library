<template>
  <div class="json-node" :class="{ nested: level > 0 }">
    <template v-if="isObjectNode">
      <div
        v-for="[key, value] in objectEntries"
        :key="key"
        class="field-row"
        :class="{ group: isObjectValue(value) }"
      >
        <div class="field-label">
          <span class="field-name">{{ fieldLabel(key) }}</span>
          <span class="field-meta">
            <span v-if="fieldLabel(key) !== key" class="field-key">{{ key }}</span>
            <span class="field-type">{{ typeLabel(value) }}</span>
          </span>
        </div>

        <JsonVisualEditor
          v-if="isObjectValue(value)"
          :model-value="value"
          :level="level + 1"
          @update:model-value="updateObjectField(key, $event)"
          @invalid="$emit('invalid', $event)"
        />

        <UiSwitch
          v-else-if="typeof value === 'boolean'"
          type="button"
          :model-value="value"
          :label="$t('ui.toggleSetting')"
          @update:model-value="updateObjectField(key, !value)"
        />

        <UiInput
          v-else-if="typeof value === 'number'"
          class="field-input number-input"
          size="md"
          type="number"
          :model-value="value"
          @input="updateObjectField(key, parseNumber(($event.target as HTMLInputElement).value, value))"
        />

        <UiInput
          v-else-if="Array.isArray(value)"
          class="field-textarea"
          :model-value="formatArray(value)"
          spellcheck="false"
          @change="updateArrayField(key, ($event.target as HTMLTextAreaElement).value)"
          multiline
        />

        <UiInput
          v-else
          class="field-input"
          size="md"
          type="text"
          :model-value="stringValue(value)"
          @input="updateObjectField(key, ($event.target as HTMLInputElement).value)"
        />
      </div>
    </template>

    <UiInput
      v-else-if="Array.isArray(modelValue)"
      class="field-textarea root-array"
      :model-value="formatArray(modelValue)"
      spellcheck="false"
      @change="updateRootArray(($event.target as HTMLTextAreaElement).value)"
      multiline
    />

    <UiInput
      v-else
      class="field-input"
      type="text"
      :model-value="stringValue(modelValue)"
      @input="$emit('update:modelValue', ($event.target as HTMLInputElement).value)"
    />
  </div>
</template>

<script setup lang="ts">
import UiInput from '@/components/ui/UiInput.vue'
import UiSwitch from '@/components/ui/UiSwitch.vue'
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'

defineOptions({ name: 'JsonVisualEditor' })

type JsonValue = null | boolean | number | string | JsonValue[] | { [key: string]: JsonValue }

const props = withDefaults(defineProps<{
  modelValue: JsonValue
  level?: number
}>(), {
  level: 0,
})

const emit = defineEmits<{
  'update:modelValue': [value: JsonValue]
  invalid: [message: string]
}>()

const { t, te } = useI18n()

const isObjectNode = computed(() => isPlainObject(props.modelValue))
const objectEntries = computed(() => {
  if (!isPlainObject(props.modelValue)) return []
  return Object.entries(props.modelValue)
})

function updateObjectField(key: string, value: JsonValue) {
  if (!isPlainObject(props.modelValue)) return
  emit('update:modelValue', {
    ...props.modelValue,
    [key]: value,
  })
}

function updateArrayField(key: string, value: string) {
  try {
    const parsed = JSON.parse(value)
    if (!Array.isArray(parsed)) {
      emit('invalid', t('prefsEditor.listRequired'))
      return
    }
    updateObjectField(key, parsed)
  } catch {
    emit('invalid', t('prefsEditor.listUnreadable'))
  }
}

function updateRootArray(value: string) {
  try {
    const parsed = JSON.parse(value)
    if (!Array.isArray(parsed)) {
      emit('invalid', t('prefsEditor.rootListRequired'))
      return
    }
    emit('update:modelValue', parsed)
  } catch {
    emit('invalid', t('prefsEditor.listUnreadable'))
  }
}

function isPlainObject(value: JsonValue): value is { [key: string]: JsonValue } {
  return typeof value === 'object' && value !== null && !Array.isArray(value)
}

function isObjectValue(value: JsonValue) {
  return isPlainObject(value)
}

function valueType(value: JsonValue) {
  if (Array.isArray(value)) return 'array'
  if (value === null) return 'null'
  return typeof value
}

function typeLabel(value: JsonValue) {
  const type = valueType(value)
  const path = `prefsEditor.types.${type}`
  return te(path) ? t(path) : type
}

function fieldLabel(key: string) {
  // Inherited names such as "constructor" would resolve to Object.prototype members.
  const path = `prefsEditor.fields.${key}`
  if (!(key in Object.prototype) && te(path)) {
    return t(path)
  }
  // Unknown prefs keys still render, as readable words.
  return humanizeFieldKey(key)
}

function stringValue(value: JsonValue) {
  if (value === null) return ''
  if (typeof value === 'string') return value
  return String(value)
}

function parseNumber(value: string, fallback: number) {
  const parsed = Number(value)
  return Number.isFinite(parsed) ? parsed : fallback
}

function formatArray(value: JsonValue[]) {
  return JSON.stringify(value, null, 2)
}

function humanizeFieldKey(key: string) {
  const text = key
    .replace(/([a-z0-9])([A-Z])/g, '$1 $2')
    .replace(/[_-]+/g, ' ')
    .trim()
    .split(/\s+/)
    // Sentence case: lower-case capitalised words, keep acronyms such as VR or UID.
    .map((word, index) => (index > 0 && /^[A-Z][a-z]/.test(word) ? word.toLowerCase() : word))
    .join(' ')
  return text.charAt(0).toUpperCase() + text.slice(1)
}
</script>

<style scoped>
/* 和设置页一样：一行一个设置项，用分隔线区分，不再一层套一层底色。
   左边是名称，右边是控件；控件统一最宽 320px，数字和文字一样宽。 */
.json-node {
  display: flex;
  flex-direction: column;
}

.json-node.nested {
  width: 100%;
  min-width: 0;
}

.field-row {
  display: grid;
  grid-template-columns: minmax(180px, 260px) minmax(0, 1fr);
  align-items: center;
  gap: var(--space-4);
  padding: var(--space-3) 0;
  border-bottom: 1px solid var(--border-subtle);
}

.field-row:last-child {
  border-bottom: none;
}

.json-node:not(.nested) > .field-row.group {
  align-items: flex-start;
  padding: var(--space-4) 0;
}

.json-node:not(.nested) > .field-row.group > .field-label .field-name {
  font-size: var(--text-base);
  font-weight: var(--font-bold);
}

.field-row.group {
  align-items: flex-start;
}

.field-row.group > .json-node.nested {
  margin-top: calc(-1 * var(--space-3));
}

.field-label {
  display: flex;
  flex-direction: column;
  gap: 2px;
  min-width: 0;
}

.field-name {
  overflow: hidden;
  color: var(--text-primary);
  font-size: var(--text-sm);
  font-weight: var(--font-semibold);
  text-overflow: ellipsis;
  white-space: nowrap;
}

.field-meta {
  display: flex;
  flex-wrap: wrap;
  gap: var(--space-2);
  align-items: center;
}

.field-key,
.field-type {
  color: var(--text-tertiary);
  font-family: var(--font-mono);
  font-size: var(--text-xs);
}

.field-input {
  width: 100%;
  max-width: 320px;
}

.field-textarea {
  width: 100%;
}

/* 尺寸要落到 UiInput 内部的 textarea 上 */
.field-textarea :deep(textarea) {
  min-height: 92px;
  font-family: var(--font-mono);
  font-size: var(--text-sm);
  line-height: 1.5;
}

.root-array :deep(textarea) {
  min-height: 280px;
}

@media (max-width: 760px) {
  .field-row {
    grid-template-columns: 1fr;
  }
}
</style>
