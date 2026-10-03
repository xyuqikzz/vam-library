<template>
  <div class="appearances-view animate-fadeIn">
    <nav class="preset-type-tabs" :aria-label="t('gameContent.presetType')">
      <button v-for="kind in presetKinds" :key="kind" :class="{ active: presetKind === kind }"
        :aria-pressed="presetKind === kind" :disabled="managerBusy" @click="selectPresetKind(kind)">
        {{ t(`gameContent.presetTypes.${kind}`) }}
      </button>
    </nav>
    <div class="content-tabs">
      <button :class="{ active: contentTab === 'files' }" :aria-pressed="contentTab === 'files'" :disabled="managerBusy" @click="contentTab = 'files'">{{ t('gameContent.presetFiles') }}</button>
      <button :class="{ active: contentTab === 'packages' }" :aria-pressed="contentTab === 'packages'" :disabled="managerBusy" @click="contentTab = 'packages'">{{ t('gameContent.packageView') }}</button>
    </div>
    <GameContentManager v-show="contentTab === 'files'" :key="presetKind" :kind="presetKind"
      @loaded="presetPackageIds = new Set($event)" @busy-change="managerBusy = $event" />
    <ResourceDisplay
      v-if="contentTab === 'packages'"
      :packages="sortedPresets"
      :view-mode="viewMode"
      :result-count-label="t('gameContent.packageCount', { count: filteredPresets.length })"
      :empty-title="t('gameContent.noPresetPackages', { type: t(`gameContent.presetTypes.${presetKind}`) })"
      :empty-description="t('gameContent.empty')"
      empty-icon="M12 8a4 4 0 1 0 0-8 4 4 0 0 0 0 8ZM5 20c0-3.866 3.134-7 7-7s7 3.134 7 7"
      @update:view-mode="viewMode = $event"
    >
      <template #actions>
        <button
          class="scan-btn-inline"
          :disabled="isScanning || !vamRootPath"
          @click="handleScan"
        >
          <svg width="14" height="14" viewBox="0 0 24 24" fill="none">
            <path d="M21 12a9 9 0 1 1-6.219-8.56" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" />
          </svg>
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
import { computed, onMounted, ref, watch } from 'vue'
import { useRouter, useRoute } from 'vue-router'
import { useI18n } from 'vue-i18n'
import { storeToRefs } from 'pinia'
import { useAppStore } from '@/stores/app'
import { useLocalLibraryStore } from '@/stores/localLibrary'
import ResourceDisplay from '@/components/ResourceDisplay.vue'
import GameContentManager from '@/components/GameContentManager.vue'
import { presetKinds, type PresetKind } from '@/types/presets'
import { compareDisplayNames } from '@/utils/nameSort'
const contentTab = ref<'files' | 'packages'>('files')
const presetKind = ref<PresetKind>('appearance')
const presetPackageIds = ref(new Set<string>())
const managerBusy = ref(false)

function selectPresetKind(kind: PresetKind) {
  if (managerBusy.value) return
  if (presetKind.value !== kind) presetPackageIds.value = new Set()
  presetKind.value = kind
  contentTab.value = 'files'
}

const { t } = useI18n()
const router = useRouter()
const route = useRoute()
watch(() => [route.path, route.query.appearance], () => {
  if (route.path === '/appearances' && typeof route.query.appearance === 'string') selectPresetKind('appearance')
}, { immediate: true })
const appStore = useAppStore()
const localLibraryStore = useLocalLibraryStore()
const { vamRootPath, isScanning, searchQuery } = storeToRefs(appStore)
const { packages } = storeToRefs(localLibraryStore)

const viewMode = ref<'large-card' | 'small-card' | 'list'>('large-card')
const sortBy = ref('name')

const filteredPresets = computed(() => {
  let list = packages.value.filter(p => presetPackageIds.value.has(p.id))

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

const sortedPresets = computed(() => {
  const sorted = [...filteredPresets.value]
  switch (sortBy.value) {
    case 'size':
      sorted.sort((a, b) => b.size_bytes - a.size_bytes)
      break
    case 'creator':
      sorted.sort((a, b) => a.creator.localeCompare(b.creator))
      break
    case 'name':
    default:
      sorted.sort((a, b) => compareDisplayNames(a.name, b.name) || a.creator.localeCompare(b.creator))
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
.preset-type-tabs { display: flex; flex-shrink: 0; flex-wrap: wrap; gap: 6px; margin-bottom: 8px; }
.preset-type-tabs button { padding: 7px 10px; border-radius: 7px; font-size: 13px; color: var(--text-secondary); border: 1px solid transparent; }
.preset-type-tabs button.active { background: var(--accent-subtle); border-color: var(--accent-primary); color: var(--text-primary); }
.preset-type-tabs button:focus-visible { outline: 2px solid var(--accent-primary); outline-offset: 2px; }
.preset-type-tabs button:disabled, .content-tabs button:disabled { opacity: .5; cursor: not-allowed; }
.content-tabs { display: flex; flex-shrink: 0; gap: 8px; margin-bottom: 12px; }
.content-tabs button { padding: 7px 12px; border-radius: 7px; font-size: 13px; color: var(--text-secondary); }
.content-tabs button.active { background: var(--bg-elevated); color: var(--text-primary); }
.content-tabs button:focus-visible { outline: 2px solid var(--accent-primary); }
.appearances-view {
  display: flex;
  flex-direction: column;
  height: 100%;
  position: relative;
  overflow: hidden;
}
.appearances-view > .game-content { flex: 1; min-height: 0; overflow: hidden; }
.appearances-view > .resource-display { flex: 1; min-height: 0; height: auto; }

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
