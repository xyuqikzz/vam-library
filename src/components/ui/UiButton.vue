<script setup lang="ts">
import { computed, ref, useAttrs, useId } from 'vue'
import { useI18n } from 'vue-i18n'
import Icon from './Icon.vue'
import UiTooltip from './UiTooltip.vue'
import type { IconName } from './icons'
defineOptions({ inheritAttrs: false })
const props = withDefaults(
  defineProps<{
    variant?: 'primary' | 'secondary' | 'ghost' | 'danger'
    size?: 'sm' | 'md' | 'lg'
    icon?: IconName
    iconOnly?: boolean
    loading?: boolean
    loadingText?: string
    disabled?: boolean
    ariaLabel?: string
    tooltip?: string
    type?: 'button' | 'submit' | 'reset'
  }>(),
  { variant: 'secondary', size: 'md', type: 'button' },
)
const { t } = useI18n()
const attrs = useAttrs()
const element = ref<HTMLButtonElement | null>(null)
const hovered = ref(false)
const focused = ref(false)
const tooltipId = useId()
const label = computed(
  () => props.ariaLabel || String(attrs['aria-label'] || attrs.title || props.tooltip || ''),
)
// 只在键盘聚焦时显示提示：弹窗/菜单用脚本把焦点移到按钮上时，不应凭空冒出 tooltip。
function onFocus(event: FocusEvent) {
  const target = event.target as HTMLElement
  focused.value = target.matches(':focus-visible') && !target.hasAttribute('data-initial-focus')
}
defineExpose({ focus: () => element.value?.focus(), element })
</script>
<template>
  <button
    ref="element"
    v-bind="attrs"
    :type="type"
    class="ui-button"
    :class="[variant, size, { 'icon-only': iconOnly, loading }]"
    :disabled="disabled || loading"
    :aria-busy="loading || undefined"
    :aria-label="label || undefined"
    :aria-describedby="iconOnly && (hovered || focused) ? tooltipId : undefined"
    @mouseenter="hovered = true"
    @mouseleave="hovered = false"
    @focus="onFocus"
    @blur="focused = false"
  >
    <span v-if="loading" class="ui-spinner" aria-hidden="true" />
    <Icon v-else-if="icon" :name="icon" />
    <span v-if="loading && !iconOnly">{{ loadingText || t('common.processing') }}</span>
    <slot v-else />
    <UiTooltip
      v-if="iconOnly || tooltip"
      :id="tooltipId"
      :text="tooltip || label"
      :open="hovered || focused"
      :anchor="element"
    />
  </button>
</template>
<style scoped>
.ui-button {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  gap: var(--space-2);
  flex-shrink: 0;
  height: var(--control-h-md);
  padding: 0 var(--space-4);
  border: none;
  border-radius: var(--radius-control);
  background: var(--bg-control);
  color: var(--text-primary);
  font-size: var(--text-base);
  font-weight: var(--font-bold);
  line-height: 1.2;
  white-space: nowrap;
  transition:
    background var(--duration-fast),
    transform var(--duration-fast);
}
.ui-button.sm {
  height: var(--control-h-sm);
  border-radius: var(--radius-control-sm);
  padding: 0 var(--space-3);
}
.ui-button.lg {
  height: var(--control-h-lg);
  padding: 0 var(--space-5);
}
.ui-button:hover {
  background: var(--bg-control-hover);
}
.ui-button:active:not(:disabled) {
  background: var(--bg-active);
  transform: scale(0.98);
}
.ui-button.primary {
  background: var(--accent-primary);
  color: var(--accent-ink);
  font-weight: var(--font-heavy);
}
.ui-button.primary:hover {
  background: var(--accent-primary-hover);
}
.ui-button.primary:active:not(:disabled) {
  background: var(--accent-primary-active);
}
.ui-button.ghost {
  background: transparent;
  color: var(--text-muted-strong);
}
.ui-button.ghost:hover {
  background: var(--bg-ghost-hover);
}
.ui-button.ghost:active:not(:disabled) {
  background: var(--bg-ghost-active);
}
.ui-button.danger {
  background: var(--color-error-surface);
  color: var(--color-error);
}
.ui-button.danger:hover {
  background: var(--color-error-surface-hover);
}
.ui-button.danger:active:not(:disabled) {
  background: var(--color-error-surface-active);
}
.ui-button:disabled {
  background: var(--bg-input);
  color: var(--text-disabled);
  cursor: not-allowed;
}
.ui-button.ghost:disabled {
  background: transparent;
}
.ui-button.loading {
  opacity: 0.8;
}
.ui-button.icon-only {
  width: var(--control-h-md);
  padding: 0;
}
.ui-button.icon-only.sm {
  width: var(--control-h-sm);
}
.ui-button.icon-only.lg {
  width: var(--control-h-lg);
}
.ui-spinner {
  width: 12px;
  height: 12px;
  border: 2px solid currentColor;
  border-right-color: transparent;
  border-radius: var(--radius-full);
  animation: spin 0.8s linear infinite;
}
</style>
