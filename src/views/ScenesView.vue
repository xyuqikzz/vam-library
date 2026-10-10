<template>
<div class="scenes-view animate-fadeIn b2-page"><PageHeader :title="$t('sidebar.scenes')" ><template #filters><div class="content-tabs">
      <UiChip :class="{ active: contentTab === 'files' }" :aria-pressed="contentTab === 'files'" :disabled="managerBusy" @click="contentTab = 'files'" :selected="contentTab === 'files'">{{ t('gameContent.scenes') }}</UiChip>
      <UiChip :class="{ active: contentTab === 'packages' }" :aria-pressed="contentTab === 'packages'" :disabled="managerBusy" @click="contentTab = 'packages'" :selected="contentTab === 'packages'">{{ t('gameContent.packageView') }}</UiChip>
    </div></template></PageHeader><div class="b2-page-body">

    <GameContentManager v-show="contentTab === 'files'" kind="scene" @busy-change="managerBusy = $event" />
    <ResourceDisplay
      v-if="contentTab === 'packages'"
      :packages="sortedScenes"
      :view-mode="viewMode"
      :result-count-label="t('scenes.scenesCount', { count: filteredScenes.length })"
      :empty-title="t('scenes.noScenes')"
      :empty-description="t('scenes.noScenesDesc')"
      empty-icon="M2 4h20v16H2zM2 9h20M5 6.5h0M7.5 6.5h0M10 6.5h0"
      @update:view-mode="viewMode = $event"
    >
      <template #actions>
        <UiButton :disabled="isScanning || !vamRootPath" @click="handleScan" variant="primary" size="lg">
          <span>{{ isScanning ? t('toolbar.scanning') : t('packages.scan') }}</span>
        </UiButton>
        <div class="sort-dropdown">
          <select v-model="sortBy" class="sort-select">
            <option value="name">{{ t('packages.sortByName') }}</option>
            <option value="size">{{ t('packages.sortBySize') }}</option>
            <option value="creator">{{ t('packages.sortByCreator') }}</option>
          </select>
        </div>
      </template>
      <template #empty-action>
        <UiButton @click="goToSettings" variant="secondary">
          <Icon  :size="16" name="outline-15" />
          {{ t('packages.selectDirectory') }}
        </UiButton>
      </template>
    </ResourceDisplay>
  </div></div>
</template>

<script setup lang="ts">
import PageHeader from '@/components/layout/PageHeader.vue'
import Icon from '@/components/ui/Icon.vue'
import UiButton from '@/components/ui/UiButton.vue'
import UiChip from '@/components/ui/UiChip.vue'
import { computed, onMounted, ref } from 'vue'
import { useRouter } from 'vue-router'
import { useI18n } from 'vue-i18n'
import { storeToRefs } from 'pinia'
import { useAppStore } from '@/stores/app'
import { useLocalLibraryStore } from '@/stores/localLibrary'
import ResourceDisplay from '@/components/ResourceDisplay.vue'
import GameContentManager from '@/components/GameContentManager.vue'
const contentTab = ref<'files' | 'packages'>('files')
const managerBusy = ref(false)

const { t } = useI18n()
const router = useRouter()
const appStore = useAppStore()
const localLibraryStore = useLocalLibraryStore()
const { vamRootPath, isScanning, searchQuery } = storeToRefs(appStore)
const { packages } = storeToRefs(localLibraryStore)

const viewMode = ref<'large-card' | 'small-card' | 'list'>('large-card')
const sortBy = ref('name')

const filteredScenes = computed(() => {
  let list = packages.value.filter(p => p.resource_types.includes('scene'))

  const query = searchQuery.value.toLowerCase().trim()
  if (query) {
    list = list.filter(p =>
      p.creator.toLowerCase().includes(query) ||
      p.name.toLowerCase().includes(query) ||
      p.id.toLowerCase().includes(query) ||
      `${p.id.toLowerCase()}.var`.includes(query)
    )
  }

  return list
})

const sortedScenes = computed(() => {
  const sorted = [...filteredScenes.value]
  switch (sortBy.value) {
    case 'size':
      sorted.sort((a, b) => b.size_bytes - a.size_bytes)
      break
    case 'creator':
      sorted.sort((a, b) => a.creator.localeCompare(b.creator))
      break
    case 'name':
    default:
      sorted.sort((a, b) => a.creator.localeCompare(b.creator) || a.name.localeCompare(b.name))
      break
  }
  return sorted
})

onMounted(async () => {
  await localLibraryStore.ensureLoaded()
})

async function handleScan() {
  if (!vamRootPath.value || isScanning.value) return
  try {
    await appStore.startScan(vamRootPath.value)
  } catch {
    // Handle silently
  }
}

function goToSettings() {
  router.push('/settings')
}
</script>

<style scoped>
.scenes-view {
  display: flex;
  flex-direction: column;
  height: 100%;
  position: relative;
  overflow: hidden;
}
.scenes-view > .game-content { flex: 1; min-height: 0; overflow: hidden; }
.scenes-view > .resource-display { flex: 1; min-height: 0; height: auto; }


.sort-select {
  height: 30px;
  padding: 0 var(--space-3);
  background: var(--bg-base);
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-sm);
  color: var(--text-secondary);
  font-size: var(--text-sm);
  cursor: pointer;
  transition: border-color var(--duration-fast) var(--ease);
}

.sort-select:focus { border-color: var(--accent-primary); }

.b2-page { display: flex; flex-direction: column; height: 100%; min-height: 0; max-width: none; margin: 0; padding: 0; gap: 0; overflow: hidden; }
.b2-page-body { flex: 1; min-height: 0; min-width: 0; overflow: auto; padding: 20px 24px; display: flex; flex-direction: column; gap: 20px; }
.b2-page :deep(.page-header) { width: 100%; }
@media(max-width: 800px) { .b2-page-body { padding: 12px; } }
</style>
