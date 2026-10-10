<template>
  <header class="app-toolbar" data-tauri-drag-region>
    <div class="toolbar-left">
      <h1 class="toolbar-title">{{ pageTitle }}</h1>
    </div>
    <div class="toolbar-right">
      <UiButton :class="{ 'is-refreshing': isRefreshing }" :title="isRefreshing ? t('toolbar.refreshing') : t('toolbar.refresh')" :disabled="isRefreshing || isLibraryLoading" @click="handleRefresh" variant="secondary">
        <Icon :class="{ 'spin-icon': isRefreshing || isLibraryLoading }" :size="14" name="outline-7" />
        <span class="toolbar-btn-label">
          {{ isRefreshing || isLibraryLoading ? t('toolbar.refreshing') : t('toolbar.refresh') }}
        </span>
      </UiButton>
    </div>
  </header>
</template>

<script setup lang="ts">
import Icon from '@/components/ui/Icon.vue'
import UiButton from '@/components/ui/UiButton.vue'
import { computed, ref } from 'vue'
import { useRoute } from 'vue-router'
import { useI18n } from 'vue-i18n'
import { useLocalLibraryStore } from '@/stores/localLibrary'
import { useAppStore } from '@/stores/app'
import { useNotification } from '@/composables/useNotification'
import { storeToRefs } from 'pinia'

const route = useRoute()
const { t } = useI18n()
const localLibraryStore = useLocalLibraryStore()
const appStore = useAppStore()
const notify = useNotification()
const { loading: isLibraryLoading } = storeToRefs(localLibraryStore)
const { vamRootPath } = storeToRefs(appStore)

const isRefreshing = ref(false)

const pageTitle = computed(() => {
  const titles: Record<string, () => string> = {
    '/dashboard': () => t('sidebar.dashboard'),
    '/packages': () => t('sidebar.packages'),
    '/statistics': () => t('sidebar.statistics'),
    '/online': () => t('sidebar.onlineHub'),
    '/download': () => t('sidebar.downloadCenter'),
    '/scenes': () => t('sidebar.scenes'),
    '/appearances': () => t('sidebar.appearances'),
    '/dependency-completion': () => t('sidebar.dependencyCompletion'),
    '/deduplication': () => t('sidebar.deduplication'),
    '/migration': () => t('sidebar.migration'),
    '/ingestion': () => t('sidebar.ingestion'),
    '/trash': () => t('sidebar.trash'),
    '/settings': () => t('sidebar.settings'),
  }
  const fn = titles[route.path]
  return fn ? fn() : (route.meta?.title as string) || ''
})

async function handleRefresh() {
  if (isRefreshing.value || isLibraryLoading.value) return
  isRefreshing.value = true
  try {
    if (vamRootPath.value) {
      await appStore.startScan(vamRootPath.value)
    }
    await localLibraryStore.refreshAll('refreshing')
  } catch (err) {
    notify.error(String(err), t('dashboard.scanFailed'))
  } finally {
    isRefreshing.value = false
  }
}
</script>

<style scoped>
.app-toolbar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--space-4);
  height: var(--toolbar-height);
  padding: 0 var(--space-5);
  border-bottom: 1px solid var(--border-subtle);
  flex-shrink: 0;
  z-index: var(--z-toolbar);
  -webkit-app-region: drag;
}

.toolbar-left {
  display: flex;
  align-items: center;
  min-width: 0;
  -webkit-app-region: drag;
}

.toolbar-title {
  font-size: var(--text-md);
  font-weight: var(--font-semibold);
  color: var(--text-primary);
  margin: 0;
  letter-spacing: -0.01em;
}

.toolbar-right {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  min-width: 0;
  -webkit-app-region: no-drag;
}

.toolbar-btn-label {
  white-space: nowrap;
}

@keyframes toolbar-spin {
  from { transform: rotate(0deg); }
  to { transform: rotate(360deg); }
}

.spin-icon {
  animation: toolbar-spin 1s linear infinite;
}

@media (max-width: 900px) {
}

@media (max-width: 720px) {
  .toolbar-btn-label {
    display: none;
  }
}
</style>
