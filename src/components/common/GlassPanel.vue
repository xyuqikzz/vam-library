<template>
  <div class="glass-panel-component">
    <div v-if="title || $slots.header || collapsible" class="panel-header" @click="toggleCollapse">
      <slot name="header">
        <h3 class="panel-title">{{ title }}</h3>
      </slot>
      <UiButton
        v-if="collapsible"
        class="collapse-toggle"
        :aria-label="isCollapsed ? t('ui.expand') : t('ui.collapse')"
        :aria-expanded="!isCollapsed"
        variant="secondary"
        icon-only
      >
        <Icon :class="['collapse-icon', { rotated: isCollapsed }]" :size="16" name="outline-1" />
      </UiButton>
    </div>
    <Transition name="collapse">
      <div v-show="!isCollapsed" class="panel-content">
        <slot />
      </div>
    </Transition>
  </div>
</template>

<script setup lang="ts">
import Icon from '@/components/ui/Icon.vue'
import UiButton from '@/components/ui/UiButton.vue'
import { ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
const { t } = useI18n()

interface Props {
  title?: string
  collapsible?: boolean
  collapsed?: boolean
}

const props = withDefaults(defineProps<Props>(), {
  title: undefined,
  collapsible: false,
  collapsed: false,
})

const isCollapsed = ref(props.collapsed)

watch(() => props.collapsed, (val) => {
  isCollapsed.value = val
})

function toggleCollapse() {
  if (props.collapsible) {
    isCollapsed.value = !isCollapsed.value
  }
}
</script>

<style scoped>
.glass-panel-component {
  flex-shrink: 0;
  background: var(--bg-elevated);
  border: none;
  border-radius: var(--radius-lg);
  box-shadow: none;
  overflow: hidden;
}

.panel-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  min-height: 52px;
  box-sizing: border-box;
  padding: var(--space-3) var(--space-panel);
  border-bottom: 1px solid var(--border-subtle);
  cursor: default;
}

.glass-panel-component:has(.collapse-toggle) .panel-header {
  cursor: pointer;
}

.panel-title {
  margin: 0;
  font-size: var(--text-md);
  font-weight: var(--font-heavy);
  color: var(--text-primary);
}

.collapse-toggle {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 24px;
  height: 24px;
  border-radius: var(--radius-sm);
  color: var(--text-secondary);
  transition:
    color var(--duration-fast) var(--ease),
    background var(--duration-fast) var(--ease);
}

.collapse-toggle:hover {
  color: var(--text-primary);
  background: var(--bg-hover);
}

.collapse-icon {
  transition: transform var(--duration-base) var(--ease);
}

.collapse-icon.rotated {
  transform: rotate(-90deg);
}

.panel-content {
  padding: var(--space-panel);
}

/* Collapse transition */
.collapse-enter-active,
.collapse-leave-active {
  transition:
    opacity var(--duration-base) var(--ease),
    max-height var(--duration-base) var(--ease);
  overflow: hidden;
}

.collapse-enter-from,
.collapse-leave-to {
  opacity: 0;
}
</style>
