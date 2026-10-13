<script setup lang="ts">
import { computed, ref } from 'vue'
import { useI18n } from 'vue-i18n'
import UiButton from '@/components/ui/UiButton.vue'
import UiMenu, { type MenuItem } from '@/components/ui/UiMenu.vue'
const props = withDefaults(defineProps<{
  disabled?: boolean
  launching?: boolean
  canDelete?: boolean
  deleting?: boolean
  completing?: boolean
  missing?: boolean
  canOpen?: boolean
  canShare?: boolean
}>(), { canOpen: true, canShare: true })
const emit = defineEmits<{
  launch: []
  extract: []
  share: []
  open: []
  copy: []
  delete: []
  complete: []
}>()
const { t } = useI18n()
const menuOpen = ref(false),
  anchor = ref<HTMLElement | null>(null)
const items = computed<MenuItem[]>(() => [
  {
    value: 'extract',
    label: t('gameContent.extractPreset'),
    icon: 'user',
    disabled: props.disabled,
  },
  { value: 'copy', label: t('ui.copyId'), icon: 'copy', shortcut: 'Ctrl C' },
  ...(props.canDelete
    ? [
        { value: 'separator', label: '', separator: true },
        {
          value: 'delete',
          label: t('ui.toTrash'),
          icon: 'trash' as const,
          danger: true,
          disabled: props.deleting || props.disabled,
          shortcut: 'Del',
        },
      ]
    : []),
])
function toggle(event: MouseEvent) {
  anchor.value = event.currentTarget as HTMLElement
  menuOpen.value = !menuOpen.value
}
function choose(item: MenuItem) {
  if (item.value === 'extract') emit('extract')
  if (item.value === 'copy') emit('copy')
  if (item.value === 'delete') emit('delete')
}
</script>
<template>
  <div class="package-actions">
    <UiButton
      class="launch-scene"
      variant="primary"
      size="lg"
      icon="play"
      :disabled="disabled || deleting"
      :loading="launching"
      :loading-text="t('gameContent.launchingScene')"
      @click="emit('launch')"
    >
      {{ t('gameContent.launchScene') }}
    </UiButton>
    <UiButton
      v-if="canOpen !== false"
      class="open-location"
      size="lg"
      icon="folder"
      icon-only
      :aria-label="t('packages.openLocation')"
      @click="emit('open')"
    />
    <UiButton
      v-if="canShare !== false"
      size="lg"
      icon="share"
      icon-only
      :aria-label="t('share.btn')"
      @click="emit('share')"
    />
    <UiButton
      size="lg"
      icon="more"
      icon-only
      :aria-label="t('ui.more')"
      :aria-expanded="menuOpen"
      aria-haspopup="menu"
      @click="toggle"
    />
    <UiButton
      v-if="missing"
      class="complete-dependencies"
      size="sm"
      :loading="completing"
      @click="emit('complete')"
    >
      {{ t('ui.allDependencies') }}
    </UiButton>
    <UiMenu
      v-model:open="menuOpen"
      :anchor="anchor"
      :items="items"
      :label="t('ui.packageActions')"
      @select="choose"
    />
  </div>
</template>
<style scoped>
.package-actions {
  display: flex;
  align-items: center;
  flex-wrap: wrap;
  gap: var(--space-2);
  min-width: 0;
}
.launch-scene {
  flex: 1;
  min-width: 0;
  padding: 0 var(--space-2);
}
.complete-dependencies {
  width: 100%;
  background: var(--color-warning);
  color: var(--color-warning-ink);
}
</style>
