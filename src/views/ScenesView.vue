<template>
  <div class="scenes-view animate-fadeIn">
    <div class="content-tabs">
      <button :class="{ active: contentTab === 'files' }" :aria-pressed="contentTab === 'files'" :disabled="managerBusy" @click="contentTab = 'files'">{{ t('gameContent.scenes') }}</button>
      <button :class="{ active: contentTab === 'packages' }" :aria-pressed="contentTab === 'packages'" :disabled="managerBusy" @click="contentTab = 'packages'">{{ t('gameContent.packageView') }}</button>
    </div>
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
        <button
          class="scan-btn-inline"
          :disabled="isScanning || !vamRootPath"
          @click="handleScan"
        >
          <span>{{ isScanning ? t('toolbar.scanning') : t('packages.scan') }}</span>
        </button>
        <div class="sort-dropdown">
          <select v-model="sortBy" class="sort-select">
            <option value="name">{{ t('packages.sortByName') }}</option>
            <option value="size">{{ t('packages.sortBySize') }}</option>
            <option value="creator">{{ t('packages.sortByCreator') }}</option>
          </select>
        </div>
      </template>
      <template #empty-action>
        <button class="action-btn" @click="goToSettings">
          <svg width="16" height="16" viewBox="0 0 24 24" fill="none">
            <path d="M4 20h16M4 20V4h16v16M9 10h6M12 7v6" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" />
          </svg>
          {{ t('packages.selectDirectory') }}
        </button>
      </template>
    </ResourceDisplay>
  </div>
</template>

<script setup lang="ts">
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
.content-tabs { display: flex; flex-shrink: 0; gap: 8px; margin-bottom: 12px; }
.content-tabs button { padding: 7px 12px; border-radius: 7px; font-size: 13px; color: var(--text-secondary); }
.content-tabs button.active { background: var(--bg-elevated); color: var(--text-primary); }
.content-tabs button:focus-visible { outline: 2px solid var(--accent-primary); }
.content-tabs button:disabled { opacity: .5; cursor: not-allowed; }
.scenes-view {
  display: flex;
  flex-direction: column;
  height: 100%;
  position: relative;
  overflow: hidden;
}
.scenes-view > .game-content { flex: 1; min-height: 0; overflow: hidden; }
.scenes-view > .resource-display { flex: 1; min-height: 0; height: auto; }

.scan-btn-inline {
  display: inline-flex;
  align-items: center;
  gap: var(--space-1);
  padding: var(--space-1) var(--space-3);
  height: 30px;
  border-radius: var(--radius-sm);
  color: var(--accent-primary);
  font-size: var(--text-sm);
  font-weight: var(--font-medium);
  transition: color var(--duration-fast) var(--ease), background var(--duration-fast) var(--ease);
}

.scan-btn-inline:hover:not(:disabled) {
  background: rgba(110, 107, 240, 0.1);
}

.scan-btn-inline:disabled { opacity: 0.5; cursor: not-allowed; }


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

.action-btn {
  display: inline-flex;
  align-items: center;
  gap: var(--space-2);
  padding: var(--space-2) var(--space-5);
  height: 36px;
  background: var(--accent-gradient);
  color: white;
  font-size: var(--text-sm);
  font-weight: var(--font-semibold);
  border-radius: var(--radius-md);
  transition: opacity var(--duration-fast) var(--ease), transform var(--duration-fast) var(--ease);
}

.action-btn:hover { opacity: 0.9; transform: translateY(-1px); }
.action-btn:active { transform: translateY(0); }
</style>
