<template>
  <div class="glass-panel-component">
    <div v-if="title || $slots.header || collapsible" class="panel-header" @click="toggleCollapse">
      <slot name="header">
        <h3 class="panel-title">{{ title }}</h3>
      </slot>
      <UiButton v-if="collapsible" class="collapse-toggle" :aria-label="isCollapsed ? 'Expand' : 'Collapse'" variant="secondary" icon-only>
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
  background: var(--bg-surface);
  border: none;
  border-radius: var(--radius-lg);
  box-shadow: none;
  overflow: hidden;
}

.panel-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: var(--space-4) var(--space-5);
  border-bottom: 1px solid var(--border-subtle);
  cursor: default;
}

.glass-panel-component:has(.collapsible) .panel-header {
  cursor: pointer;
}

.panel-title {
  font-size: var(--text-sm);
  font-weight: var(--font-semibold);
  text-transform: uppercase;
  letter-spacing: 0.06em;
  color: var(--text-secondary);
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
  padding: var(--space-5);
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
