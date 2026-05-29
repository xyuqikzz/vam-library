<template>
  <div :class="['app-layout', { 'sidebar-collapsed': sidebarCollapsed }]">
    <div v-if="showLibraryLoading" class="global-loading-overlay">
      <div class="global-loading-card">
        <div class="global-loading-head">
          <strong>{{ loadingTitle }}</strong>
          <span>{{ $t('common.loading') }}</span>
        </div>
        <p class="global-loading-text">{{ loadingDescription }}</p>
        <div class="global-loading-progress">
          <div class="global-loading-progress-bar" />
        </div>
      </div>
    </div>
    <AppSidebar :collapsed="sidebarCollapsed" @update:collapsed="onSidebarToggle" />
    <div class="main-area">
      <main class="main-content">
        <router-view v-slot="{ Component, route }">
          <Transition
            name="fade"
            mode="out-in"
            @before-enter="() => onTransitionBeforeEnter(route.fullPath)"
            @enter="() => onTransitionEnter(route.fullPath)"
            @after-enter="() => onTransitionAfterEnter(route.fullPath)"
          >
            <keep-alive>
              <component :is="Component" :key="route.fullPath" />
            </keep-alive>
          </Transition>
        </router-view>
      </main>
    </div>
    <!-- Background gradient overlay -->
    <div class="bg-gradient-overlay" aria-hidden="true" />
    <!-- Toast notifications -->
    <Toast ref="toastEl" />
  </div>
</template>

<script setup lang="ts">
import { computed, onMounted, ref, nextTick } from 'vue'
import { useRouter } from 'vue-router'
import { useAppStore } from '@/stores/app'
import { useLocalLibraryStore } from '@/stores/localLibrary'
import { storeToRefs } from 'pinia'
import { toastRef } from '@/composables/useNotification'
import { useI18n } from 'vue-i18n'
import AppSidebar from './AppSidebar.vue'
import Toast from '@/components/common/Toast.vue'

const appStore = useAppStore()
const localLibraryStore = useLocalLibraryStore()
const { sidebarCollapsed } = storeToRefs(appStore)
const { state: localLibraryState, loading: localLibraryLoading } = storeToRefs(localLibraryStore)
const { t } = useI18n()

const toastEl = ref<InstanceType<typeof Toast> | null>(null)

const router = useRouter()
const scrollPositions = ref(new Map<string, {
  mainContent?: number
  localScrollable?: number
  hubScrollable?: number
}>())

const showLibraryLoading = computed(() => localLibraryLoading.value)

const loadingTitle = computed(() => {
  if (localLibraryState.value === 'indexing') return t('common.updatingData')
  if (localLibraryState.value === 'scanning') return t('common.syncingData')
  return t('common.fetchingData')
})

const loadingDescription = computed(() => {
  if (localLibraryState.value === 'indexing') return t('common.updatingDataDesc')
  if (localLibraryState.value === 'scanning') return t('common.syncingDataDesc')
  return t('common.fetchingDataDesc')
})

function saveScrollPosition(path: string) {
  const mainContent = document.querySelector('.main-content')
  const localScrollable = document.querySelector('.resource-content-scrollable')
  const hubScrollable = document.querySelector('.hub-content-scroll')

  const positions: { mainContent?: number, localScrollable?: number, hubScrollable?: number } = {}

  if (mainContent) {
    positions.mainContent = mainContent.scrollTop
  }
  if (localScrollable) {
    positions.localScrollable = localScrollable.scrollTop
  }
  if (hubScrollable) {
    positions.hubScrollable = hubScrollable.scrollTop
  }

  scrollPositions.value.set(path, positions)
}

function restoreScroll(path: string) {
  const positions = scrollPositions.value.get(path)
  if (!positions) return

  const doRestore = () => {
    let restored = false
    if (positions.mainContent !== undefined) {
      const el = document.querySelector('.main-content')
      if (el) {
        el.scrollTop = positions.mainContent
        restored = true
      }
    }
    if (positions.localScrollable !== undefined) {
      const el = document.querySelector('.resource-content-scrollable')
      if (el) {
        el.scrollTop = positions.localScrollable
        restored = true
      }
    }
    if (positions.hubScrollable !== undefined) {
      const el = document.querySelector('.hub-content-scroll')
      if (el) {
        el.scrollTop = positions.hubScrollable
        restored = true
      }
    }
    return restored
  }

  // Multi-stage restoration to guarantee accuracy
  doRestore()
  nextTick(() => {
    doRestore()
  })
  setTimeout(() => {
    doRestore()
  }, 50)
  setTimeout(() => {
    doRestore()
  }, 150)
}

// Router guard to save scroll position before routing
router.beforeEach((_to, from) => {
  saveScrollPosition(from.fullPath)
})

function onTransitionBeforeEnter(path: string) {
  restoreScroll(path)
}

function onTransitionEnter(path: string) {
  restoreScroll(path)
}

function onTransitionAfterEnter(path: string) {
  restoreScroll(path)
}

onMounted(async () => {
  toastRef.value = toastEl.value as any
  appStore.setupScanListener()
  await localLibraryStore.startListeners()
  await appStore.loadSettings()
  await localLibraryStore.ensureLoaded()

  // Auto-scan on startup if enabled and VAM directory is set
  if (appStore.autoScan && appStore.vamRootPath && !appStore.isScanning) {
    appStore.startScan(appStore.vamRootPath).catch((err) => {
      console.error('Failed auto-scan on startup:', err)
    })
  }
})

function onSidebarToggle(collapsed: boolean) {
  appStore.sidebarCollapsed = collapsed
}
</script>

<style scoped>
.app-layout {
  position: relative;
  display: flex;
  width: 100vw;
  height: 100vh;
  background: var(--bg-base);
  overflow: hidden;
}

.global-loading-overlay {
  position: absolute;
  inset: 0;
  z-index: 650;
  display: flex;
  align-items: center;
  justify-content: center;
  padding: var(--space-6);
  background: rgba(9, 10, 18, 0.48);
  backdrop-filter: blur(8px);
  -webkit-backdrop-filter: blur(8px);
}

.global-loading-card {
  width: min(420px, 100%);
  padding: var(--space-5);
  border: 1px solid rgba(255, 255, 255, 0.08);
  border-radius: var(--radius-xl);
  background: rgba(19, 21, 34, 0.88);
  box-shadow: 0 20px 60px rgba(0, 0, 0, 0.35);
}

.global-loading-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--space-3);
  margin-bottom: var(--space-2);
}

.global-loading-head strong {
  color: var(--text-primary);
  font-size: var(--text-md);
}

.global-loading-head span,
.global-loading-text {
  color: var(--text-secondary);
  font-size: var(--text-sm);
}

.global-loading-text {
  margin-bottom: var(--space-4);
  line-height: 1.6;
}

.global-loading-progress {
  position: relative;
  overflow: hidden;
  height: 8px;
  border-radius: var(--radius-full);
  background: rgba(255, 255, 255, 0.08);
}

.global-loading-progress-bar {
  position: absolute;
  inset: 0 auto 0 -35%;
  width: 35%;
  border-radius: inherit;
  background: var(--accent-gradient);
  animation: global-progress-slide 1.15s ease-in-out infinite;
}

@keyframes global-progress-slide {
  0% {
    transform: translateX(0);
  }

  100% {
    transform: translateX(420%);
  }
}

.main-area {
  flex: 1;
  display: flex;
  flex-direction: column;
  min-width: 0;
  transition: margin-left var(--duration-base) var(--ease);
}

.main-content {
  flex: 1;
  overflow-y: auto;
  overflow-x: hidden;
  padding: var(--space-6);
}

/* ── Background gradient overlay ───────────────────────────── */
.bg-gradient-overlay {
  position: fixed;
  top: 0;
  left: 0;
  right: 0;
  bottom: 0;
  pointer-events: none;
  z-index: -1;
  background:
    radial-gradient(ellipse 80% 60% at 20% 10%, rgba(124, 92, 252, 0.06) 0%, transparent 60%),
    radial-gradient(ellipse 60% 50% at 80% 80%, rgba(91, 141, 239, 0.04) 0%, transparent 50%);
}
</style>
