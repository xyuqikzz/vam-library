<template>
  <div class="appearances-view animate-fadeIn b2-page">
    <PageHeader :title="$t('sidebar.appearances')">
      <template #filters>
        <nav class="preset-type-tabs" :aria-label="t('gameContent.presetType')">
          <UiChip
            v-for="kind in presetKinds"
            :key="kind"
            :class="{ active: presetKind === kind }"
            :aria-pressed="presetKind === kind"
            :disabled="managerBusy"
            @click="selectPresetKind(kind)"
            :selected="presetKind === kind"
          >
            {{ t(`gameContent.presetTypes.${kind}`) }}
          </UiChip>
        </nav>
        <div class="content-tabs">
          <UiChip
            :class="{ active: contentTab === 'files' }"
            :aria-pressed="contentTab === 'files'"
            :disabled="managerBusy"
            @click="contentTab = 'files'"
            :selected="contentTab === 'files'"
          >
            {{ t('gameContent.presetFiles') }}
          </UiChip>
          <UiChip
            :class="{ active: contentTab === 'packages' }"
            :aria-pressed="contentTab === 'packages'"
            :disabled="managerBusy"
            @click="contentTab = 'packages'"
            :selected="contentTab === 'packages'"
          >
            {{ t('gameContent.packageView') }}
          </UiChip>
        </div>
      </template>
    </PageHeader>
    <div class="b2-page-body">
      <GameContentManager
        v-show="contentTab === 'files'"
        :key="presetKind"
        :kind="presetKind"
        @loaded="presetPackageIds = new Set($event)"
        @busy-change="managerBusy = $event"
      />
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
          <UiButton
            :disabled="isScanning || !vamRootPath"
            @click="handleScan"
            variant="primary"
            size="lg"
          >
            <Icon :size="14" name="outline-7" />
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
            <Icon :size="16" name="outline-15" />
            {{ t('packages.selectDirectory') }}
          </UiButton>
        </template>
      </ResourceDisplay>
    </div>
  </div>
</template>

<script setup lang="ts">
import PageHeader from '@/components/layout/PageHeader.vue'
import Icon from '@/components/ui/Icon.vue'
import UiButton from '@/components/ui/UiButton.vue'
import UiChip from '@/components/ui/UiChip.vue'
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
.appearances-view {
  display: flex;
  flex-direction: column;
  height: 100%;
  position: relative;
  overflow: hidden;
}
.appearances-view > .game-content {
  flex: 1;
  min-height: 0;
  overflow: hidden;
}
.appearances-view > .resource-display {
  flex: 1;
  min-height: 0;
  height: auto;
}

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

.sort-select:focus {
  border-color: var(--accent-primary);
}

.b2-page {
  display: flex;
  flex-direction: column;
  height: 100%;
  min-height: 0;
  max-width: none;
  margin: 0;
  padding: 0;
  gap: 0;
  overflow: hidden;
}
.b2-page-body {
  flex: 1;
  min-height: 0;
  min-width: 0;
  overflow: auto;
  padding: 20px 24px;
  display: flex;
  flex-direction: column;
  gap: 20px;
}
.b2-page :deep(.page-header) {
  width: 100%;
}
@media (max-width: 800px) {
  .b2-page-body {
    padding: 12px;
  }
}
</style>
