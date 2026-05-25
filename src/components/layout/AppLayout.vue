<template>
  <div :class="['app-layout', { 'sidebar-collapsed': sidebarCollapsed }]">
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
import { onMounted, ref, nextTick } from 'vue'
import { useRouter } from 'vue-router'
import { useAppStore } from '@/stores/app'
import { useLocalLibraryStore } from '@/stores/localLibrary'
import { storeToRefs } from 'pinia'
import { toastRef } from '@/composables/useNotification'
import AppSidebar from './AppSidebar.vue'
import Toast from '@/components/common/Toast.vue'

const appStore = useAppStore()
const localLibraryStore = useLocalLibraryStore()
const { sidebarCollapsed } = storeToRefs(appStore)

const toastEl = ref<InstanceType<typeof Toast> | null>(null)

const router = useRouter()
const scrollPositions = ref(new Map<string, {
  mainContent?: number
  localScrollable?: number
  hubScrollable?: number
}>())

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
