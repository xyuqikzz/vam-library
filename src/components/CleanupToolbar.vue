<template>
  <div class="cleanup-toolbar" aria-live="polite">
    <div class="selection-actions">
      <UiButton
        :disabled="!pageIds.length || unavailable"
        @click="store.select(pageIds)"
        variant="secondary"
      >
        {{ t('cleanup.selectPage') }}
      </UiButton>
      <span>
        {{
          t(store.selected.size ? 'cleanup.selectionCount' : 'cleanup.selectHint', {
            count: store.selected.size,
          })
        }}
      </span>
      <span v-if="hidden" class="hidden-selection">
        {{ t('cleanup.hiddenSelection', { count: hidden }) }}
      </span>
      <UiButton
        v-if="store.selected.size"
        :disabled="busy"
        @click="store.clear"
        variant="ghost"
      >
        {{ t('cleanup.clear') }}
      </UiButton>
    </div>
    <div class="selection-actions main-actions">
      <UiButton
        ref="moreButton"
        icon="more"
        aria-haspopup="menu"
        :aria-expanded="moreOpen"
        @click="moreOpen = !moreOpen"
        variant="secondary"
      >
        {{ t('cleanup.more') }}
      </UiButton>
      <UiMenu
        v-model:open="moreOpen"
        :anchor="moreButton?.element"
        :items="moreItems"
        :label="t('cleanup.more')"
        placement="bottom-end"
        @select="runMore($event.value)"
      />
      <UiButton
        :disabled="unavailable || !store.selected.size"
        @click="store.openPreview([...store.selected])"
        variant="primary"
      >
        {{ t('cleanup.preview') }}
      </UiButton>
    </div>
    <CleanupPreviewDialog
      v-if="store.previewIds"
      :package-ids="store.previewIds"
      :packages="library.packages"
      :broken-info-ids="brokenInfo"
      @close="store.closePreview()"
      @completed="completed"
    />
  </div>
</template>
<script setup lang="ts">
import UiButton from '@/components/ui/UiButton.vue'
import UiMenu, { type MenuItem } from '@/components/ui/UiMenu.vue'
import { computed, ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { useCleanupStore } from '@/stores/cleanup'
import { useLocalLibraryStore } from '@/stores/localLibrary'
import { useAppStore } from '@/stores/app'
import { useNotification } from '@/composables/useNotification'
import { hiddenSelectionCount } from '@/utils/cleanupSelection'
import { brokenInfoIds } from '@/utils/cleanupIssues'
import CleanupPreviewDialog from './CleanupPreviewDialog.vue'
import type { PackageDisplayItem } from '@/types/package'
import type { CleanupBatch } from '@/types/cleanup'
const props = defineProps<{ packages: PackageDisplayItem[]; pageIds: string[] }>()
const store = useCleanupStore()
const library = useLocalLibraryStore()
const app = useAppStore()
const notify = useNotification()
const { t } = useI18n()
const busy = ref(false)
const moreOpen = ref(false)
// Packages whose info could not be read, labelled in the preview as well.
const brokenInfo = computed(() => [...brokenInfoIds(library.corruptedPackages ?? [], store.health)])
const moreButton = ref<InstanceType<typeof UiButton> | null>(null)
const unavailable = computed(
  () => busy.value || store.loading || app.isScanning || !app.vamRootPath,
)
const hidden = computed(() =>
  hiddenSelectionCount(
    store.selected,
    props.packages.map((p) => p.id),
  ),
)
const moreItems = computed<MenuItem[]>(() => {
  const noSelection = unavailable.value || !store.selected.size
  return [
    { value: 'selectAll', label: t('cleanup.selectAll'), disabled: !props.packages.length || unavailable.value },
    { value: 'sep', label: '', separator: true },
    { value: 'pending', label: t('cleanup.mark'), disabled: noSelection },
    { value: 'protect', label: t('cleanup.protect'), disabled: noSelection },
    { value: 'unmark', label: t('cleanup.unmark'), disabled: noSelection },
  ]
})
function runMore(action: string) {
  if (action === 'selectAll') selectAll()
  else if (action === 'pending' || action === 'protect' || action === 'unmark') void update(action)
}
function selectAll() {
  store.select(props.packages.map((p) => p.id))
}
async function update(action: 'pending' | 'protect' | 'unmark') {
  if (unavailable.value) return
  busy.value = true
  try {
    await store.update(action)
    notify.success(t('cleanup.updated'))
  } catch (e) {
    notify.error(String(e))
  } finally {
    busy.value = false
  }
}
async function completed(batch: CleanupBatch) {
  if (batch.status === 'completed') store.clear()
  try {
    await Promise.all([store.load(), library.refreshAll('refreshing'), store.loadHealth()])
  } catch (e) {
    notify.error(String(e))
  }
}
</script>
<style scoped>
.cleanup-toolbar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  flex-wrap: wrap;
  gap: var(--space-item);
  padding: var(--space-stack);
  background: var(--bg-elevated);
  border-radius: var(--radius-lg);
  flex-shrink: 0;
}
.selection-actions {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  flex-wrap: wrap;
  min-width: 0;
}
.selection-actions span {
  font-size: var(--text-sm);
  color: var(--text-secondary);
}
.selection-actions .hidden-selection {
  color: var(--text-tertiary);
}
.main-actions {
  margin-left: auto;
}
</style>
