<template>
  <header class="app-toolbar" data-tauri-drag-region>
    <div class="toolbar-left">
      <h1 class="toolbar-title">{{ pageTitle }}</h1>
    </div>
    <div class="toolbar-right">
      <button
        class="toolbar-btn"
        :class="{ 'is-refreshing': isRefreshing }"
        :title="isRefreshing ? t('toolbar.refreshing') : t('toolbar.refresh')"
        :disabled="isRefreshing || isLibraryLoading"
        @click="handleRefresh"
      >
        <svg
          width="14"
          height="14"
          viewBox="0 0 24 24"
          fill="none"
          :class="{ 'spin-icon': isRefreshing || isLibraryLoading }"
        >
          <path d="M21 12a9 9 0 1 1-6.219-8.56" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" />
        </svg>
        <span class="toolbar-btn-label">
          {{ isRefreshing || isLibraryLoading ? t('toolbar.refreshing') : t('toolbar.refresh') }}
        </span>
      </button>
    </div>
  </header>
</template>

<script setup lang="ts">
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
    '/on-demand': () => t('sidebar.onDemand'),
    '/migration': () => t('sidebar.migration'),
    '/unpack': () => t('sidebar.unpack'),
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

.toolbar-btn {
  display: flex;
  align-items: center;
  gap: 6px;
  height: 28px;
  padding: 0 10px;
  border-radius: var(--radius-sm);
  background: var(--bg-subtle);
  border: 1px solid var(--border-subtle);
  color: var(--text-tertiary);
  font-size: var(--text-xs);
  font-weight: var(--font-medium);
  cursor: pointer;
  transition:
    color var(--duration-fast) var(--ease),
    background var(--duration-fast) var(--ease),
    border-color var(--duration-fast) var(--ease);
}

.toolbar-btn:hover:not(:disabled) {
  color: var(--text-secondary);
  background: var(--bg-hover);
  border-color: var(--border-default);
}

.toolbar-btn:active:not(:disabled) {
  transform: scale(0.98);
}

.toolbar-btn:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}

.toolbar-btn.is-refreshing {
  color: var(--accent-primary);
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
