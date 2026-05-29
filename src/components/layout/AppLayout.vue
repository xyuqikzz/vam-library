<template>
  <div class="app-layout">
    <div v-if="visibleStartupError" class="startup-error-overlay">
      <div class="startup-error-card">
        <strong>启动遇到问题</strong>
        <p>{{ visibleStartupError }}</p>
        <div class="startup-error-actions">
          <button class="startup-error-btn primary" @click="retryStartup">重试</button>
          <button class="startup-error-btn" @click="openSettings">打开设置</button>
        </div>
      </div>
    </div>
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
    <AppSidebar />
    <div class="main-area">
      <AppToolbar />
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
import AppToolbar from './AppToolbar.vue'
import Toast from '@/components/common/Toast.vue'

const appStore = useAppStore()
const localLibraryStore = useLocalLibraryStore()
const { state: localLibraryState, loading: localLibraryLoading, error: localLibraryError } = storeToRefs(localLibraryStore)
const { t } = useI18n()

const toastEl = ref<InstanceType<typeof Toast> | null>(null)
const startupError = ref<string | null>(null)

const router = useRouter()
const scrollPositions = ref(new Map<string, {
  mainContent?: number
  localScrollable?: number
  hubScrollable?: number
}>())

const showLibraryLoading = computed(() => localLibraryLoading.value)
const visibleStartupError = computed(() => startupError.value || localLibraryError.value)

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

async function bootstrapApp() {
  startupError.value = null
  toastRef.value = toastEl.value as any
  await appStore.setupScanListener()
  await localLibraryStore.startListeners()
  await appStore.loadSettings()

  window.requestAnimationFrame(() => {
    window.requestAnimationFrame(() => {
      void localLibraryStore.ensureLoaded().then(() => {
        if (appStore.autoScan && appStore.vamRootPath && !appStore.isScanning) {
          appStore.startScan(appStore.vamRootPath).catch((err) => {
            console.error('Failed auto-scan on startup:', err)
          })
        }
      }).catch((err) => {
        console.error('Failed to load local library:', err)
        startupError.value = String(err)
      })
    })
  })
}

async function retryStartup() {
  await bootstrapApp().catch((err) => {
    startupError.value = String(err)
  })
}

function openSettings() {
  startupError.value = null
  router.push('/settings')
}

onMounted(async () => {
  await bootstrapApp().catch((err) => {
    console.error('Failed to start app:', err)
    startupError.value = String(err)
  })
})
</script>

<style scoped>
.app-layout {
  position: relative;
  display: flex;
  width: 100%;
  height: 100%;
  min-width: 0;
  min-height: 0;
  background: var(--bg-base);
  overflow: hidden;
}

.startup-error-overlay {
  position: absolute;
  inset: 0;
  z-index: 800;
  display: flex;
  align-items: center;
  justify-content: center;
  padding: var(--space-6);
  background: rgba(22, 22, 24, 0.78);
}

.startup-error-card {
  width: min(460px, 100%);
  padding: var(--space-5);
  border: 1px solid rgba(255, 69, 58, 0.28);
  border-radius: var(--radius-lg);
  background: var(--bg-surface);
  box-shadow: var(--shadow-lg);
}

.startup-error-card strong {
  display: block;
  margin-bottom: var(--space-2);
  color: var(--color-error);
  font-size: var(--text-lg);
}

.startup-error-card p {
  margin-bottom: var(--space-4);
  color: var(--text-secondary);
  font-size: var(--text-sm);
  line-height: 1.6;
  user-select: text;
}

.startup-error-actions {
  display: flex;
  justify-content: flex-end;
  gap: var(--space-2);
}

.startup-error-btn {
  height: 32px;
  padding: 0 var(--space-4);
  border-radius: var(--radius-sm);
  background: var(--bg-hover);
  color: var(--text-primary);
  font-size: var(--text-sm);
  font-weight: var(--font-semibold);
}

.startup-error-btn.primary {
  background: var(--accent-gradient);
  color: #ffffff;
}

.global-loading-overlay {
  position: absolute;
  inset: 0;
  z-index: 650;
  display: flex;
  align-items: center;
  justify-content: center;
  padding: var(--space-6);
  background: rgba(22, 22, 24, 0.6);
}

.global-loading-card {
  width: min(420px, 100%);
  padding: var(--space-5);
  border: 1px solid var(--border-default);
  border-radius: var(--radius-xl);
  background: var(--bg-surface);
  box-shadow: var(--shadow-lg);
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
  height: 6px;
  border-radius: var(--radius-full);
  background: rgba(255, 255, 255, 0.06);
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
  min-height: 0;
}

.main-content {
  flex: 1;
  min-width: 0;
  min-height: 0;
  overflow-y: auto;
  overflow-x: hidden;
  padding: var(--space-5);
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
    radial-gradient(ellipse 80% 60% at 20% 10%, rgba(110, 107, 240, 0.04) 0%, transparent 60%),
    radial-gradient(ellipse 60% 50% at 80% 80%, rgba(91, 141, 239, 0.03) 0%, transparent 50%);
}
</style>
