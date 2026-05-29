<template>
  <aside class="app-sidebar">
    <!-- Sidebar Header (Brand) -->
    <div class="sidebar-header" data-tauri-drag-region>
      <div class="sidebar-brand">
        <div class="brand-logo">
          <img src="/logo-square.png" alt="VAM Library" width="22" height="22" style="border-radius: 5px; object-fit: contain;" />
        </div>
        <span class="brand-name">{{ $t('app.name') }}</span>
      </div>
    </div>

    <!-- Navigation -->
    <nav class="sidebar-nav">
      <!-- Browse Group -->
      <div class="nav-group">
        <span class="nav-group-label">{{ $t('sidebar.browse') }}</span>
        <button
          v-for="item in browseItems"
          :key="item.route"
          :class="['nav-item', { active: isActive(item.route) }]"
          @click="navigate(item.route)"
        >
          <span class="nav-icon" v-html="item.icon" />
          <span class="nav-label">{{ item.label }}</span>
        </button>
      </div>

      <!-- Resources Group -->
      <div class="nav-group">
        <span class="nav-group-label">{{ $t('sidebar.resources') }}</span>
        <button
          v-for="item in resourcesItems"
          :key="item.route"
          :class="['nav-item', { active: isActive(item.route) }]"
          @click="navigate(item.route)"
        >
          <span class="nav-icon" v-html="item.icon" />
          <span class="nav-label">{{ item.label }}</span>
          <span
            v-if="item.route === '/download' && activeAndPendingCount > 0"
            class="nav-badge"
          >
            {{ activeAndPendingCount }}
          </span>
        </button>
      </div>

      <!-- Tools Group -->
      <div class="nav-group">
        <span class="nav-group-label">{{ $t('sidebar.tools') }}</span>
        <button
          v-for="item in toolsItems"
          :key="item.route"
          :class="['nav-item', { active: isActive(item.route) }]"
          @click="navigate(item.route)"
        >
          <span class="nav-icon" v-html="item.icon" />
          <span class="nav-label">{{ item.label }}</span>
        </button>
      </div>
    </nav>

    <!-- Bottom -->
    <div class="sidebar-bottom">
      <button
        class="nav-item start-game-btn"
        :class="{ 'is-launching': isLaunching }"
        @click="startVamGame"
      >
        <span class="nav-icon">
          <svg width="16" height="16" viewBox="0 0 24 24" fill="none" class="play-icon">
            <path d="M8 5v14l11-7L8 5Z" fill="currentColor" stroke="currentColor" stroke-width="1.5" stroke-linejoin="round" />
          </svg>
        </span>
        <span class="nav-label">{{ isLaunching ? $t('onDemand.running') : $t('sidebar.startGame') }}</span>
      </button>
      <button
        :class="['nav-item', { active: isActive('/settings') }]"
        @click="navigate('/settings')"
      >
        <span class="nav-icon">
          <svg width="16" height="16" viewBox="0 0 24 24" fill="none">
            <path d="M12.22 2h-.44a2 2 0 0 0-2 2v.18a2 2 0 0 1-1 1.73l-.43.25a2 2 0 0 1-2 0l-.15-.08a2 2 0 0 0-2.73.73l-.22.38a2 2 0 0 0 .73 2.73l.15.1a2 2 0 0 1 1 1.72v.51a2 2 0 0 1-1 1.74l-.15.09a2 2 0 0 0-.73 2.73l.22.38a2 2 0 0 0 2.73.73l.15-.08a2 2 0 0 1 2 0l.43.25a2 2 0 0 1 1 1.73V20a2 2 0 0 0 2 2h.44a2 2 0 0 0 2-2v-.18a2 2 0 0 1 1-1.73l.43-.25a2 2 0 0 1 2 0l.15.08a2 2 0 0 0 2.73-.73l.22-.39a2 2 0 0 0-.73-2.73l-.15-.08a2 2 0 0 1-1-1.74v-.5a2 2 0 0 1 1-1.74l.15-.1a2 2 0 0 0 .73-2.73l-.22-.38a2 2 0 0 0-2.73-.73l-.15.08a2 2 0 0 1-2 0l-.43-.25a2 2 0 0 1-1-1.73V4a2 2 0 0 0-2-2z" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" />
            <circle cx="12" cy="12" r="3" stroke="currentColor" stroke-width="1.5" />
          </svg>
        </span>
        <span class="nav-label">{{ $t('sidebar.settings') }}</span>
      </button>
    </div>
  </aside>
</template>

<script setup lang="ts">
import { computed, ref, onMounted, onUnmounted } from 'vue'
import { useRouter, useRoute } from 'vue-router'
import { useI18n } from 'vue-i18n'
import { invoke } from '@tauri-apps/api/core'
import { useDownloadStore } from '@/stores/download'
import { useAppStore } from '@/stores/app'

const router = useRouter()
const route = useRoute()
const { t } = useI18n()

const downloadStore = useDownloadStore()
const appStore = useAppStore()
const activeAndPendingCount = computed(() => downloadStore.activeAndPendingCount)

const isLaunching = ref(false)

async function startVamGame() {
  if (isLaunching.value) return
  const rootPath = appStore.vamRootPath
  if (!rootPath) {
    alert(t('onDemand.vamNotSet'))
    return
  }
  isLaunching.value = true
  try {
    await invoke('launch_vam_direct', { vamRoot: rootPath })
  } catch (err) {
    console.error('Failed to launch VAM:', err)
    alert(String(err))
  } finally {
    isLaunching.value = false
  }
}

onMounted(() => {
  downloadStore.startListeners()
  downloadStore.fetchQueue()
})

onUnmounted(() => {
  downloadStore.stopListeners()
})

const browseItems = computed(() => [
  {
    route: '/dashboard',
    label: t('sidebar.dashboard'),
    icon: '<svg width="16" height="16" viewBox="0 0 24 24" fill="none"><rect x="3" y="3" width="7" height="7" rx="1.5" stroke="currentColor" stroke-width="1.5"/><rect x="14" y="3" width="7" height="7" rx="1.5" stroke="currentColor" stroke-width="1.5"/><rect x="3" y="14" width="7" height="7" rx="1.5" stroke="currentColor" stroke-width="1.5"/><rect x="14" y="14" width="7" height="7" rx="1.5" stroke="currentColor" stroke-width="1.5"/></svg>',
  },
  {
    route: '/packages',
    label: t('sidebar.packages'),
    icon: '<svg width="16" height="16" viewBox="0 0 24 24" fill="none"><path d="M12 2L3 7V17L12 22L21 17V7L12 2Z" stroke="currentColor" stroke-width="1.5" stroke-linejoin="round"/><path d="M12 12L21 7" stroke="currentColor" stroke-width="1.5"/><path d="M12 12V22" stroke="currentColor" stroke-width="1.5"/><path d="M12 12L3 7" stroke="currentColor" stroke-width="1.5"/></svg>',
  },
  {
    route: '/statistics',
    label: t('sidebar.statistics'),
    icon: '<svg width="16" height="16" viewBox="0 0 24 24" fill="none"><path d="M5 20V10" stroke="currentColor" stroke-width="1.5" stroke-linecap="round"/><path d="M12 20V4" stroke="currentColor" stroke-width="1.5" stroke-linecap="round"/><path d="M19 20v-7" stroke="currentColor" stroke-width="1.5" stroke-linecap="round"/><path d="M3 20h18" stroke="currentColor" stroke-width="1.5" stroke-linecap="round"/></svg>',
  },
])

const resourcesItems = computed(() => [
  {
    route: '/online',
    label: t('sidebar.onlineHub'),
    icon: '<svg width="16" height="16" viewBox="0 0 24 24" fill="none"><circle cx="12" cy="12" r="10" stroke="currentColor" stroke-width="1.5"/><path d="M2 12h20M12 2a15.3 15.3 0 0 1 4 10 15.3 15.3 0 0 1-4 10 15.3 15.3 0 0 1-4-10 15.3 15.3 0 0 1 4-10z" stroke="currentColor" stroke-width="1.5"/></svg>',
  },
  {
    route: '/download',
    label: t('sidebar.downloadCenter'),
    icon: '<svg width="16" height="16" viewBox="0 0 24 24" fill="none"><path d="M12 14V3m0 11l-4-4m4 4l4-4M4 16v2a2 2 0 002 2h12a2 2 0 002-2v-2" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"/></svg>',
  },
])

const toolsItems = computed(() => [
  {
    route: '/dependency-completion',
    label: t('sidebar.dependencyCompletion'),
    icon: '<svg width="16" height="16" viewBox="0 0 24 24" fill="none"><path d="M4 12h7m0 0-3-3m3 3-3 3M13 7h4a2 2 0 0 1 2 2v8a2 2 0 0 1-2 2h-4" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"/><path d="M17 12h3" stroke="currentColor" stroke-width="1.5" stroke-linecap="round"/></svg>',
  },
  {
    route: '/deduplication',
    label: t('sidebar.deduplication'),
    icon: '<svg width="16" height="16" viewBox="0 0 24 24" fill="none"><rect x="3" y="3" width="12" height="12" rx="2" stroke="currentColor" stroke-width="1.5"/><path d="M9 9H21V21H9" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"/></svg>',
  },
  {
    route: '/on-demand',
    label: t('sidebar.onDemand'),
    icon: '<svg width="16" height="16" viewBox="0 0 24 24" fill="none"><path d="M8 5v14l11-7L8 5Z" stroke="currentColor" stroke-width="1.5" stroke-linejoin="round"/><path d="M4 6v12" stroke="currentColor" stroke-width="1.5" stroke-linecap="round"/></svg>',
  },
  {
    route: '/migration',
    label: t('sidebar.migration'),
    icon: '<svg width="16" height="16" viewBox="0 0 24 24" fill="none"><path d="M4 12H20M20 12L16 8M20 12L16 16" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"/><path d="M20 6H4M4 6L8 2M4 6L8 10" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" opacity="0.4"/></svg>',
  },
  {
    route: '/unpack',
    label: t('sidebar.unpack'),
    icon: '<svg width="16" height="16" viewBox="0 0 24 24" fill="none"><path d="M12 2L3 7v10l9 5 9-5V7l-9-5z" stroke="currentColor" stroke-width="1.5" stroke-linejoin="round"/><path d="M12 22V12" stroke="currentColor" stroke-width="1.5"/><path d="M21 7l-9 5L3 7" stroke="currentColor" stroke-width="1.5"/><path d="M12 12V6" stroke="currentColor" stroke-width="2" stroke-linecap="round"/><path d="M9 9l3-3 3 3" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"/></svg>',
  },
  {
    route: '/vam-prefs',
    label: t('sidebar.vamPrefs'),
    icon: '<svg width="16" height="16" viewBox="0 0 24 24" fill="none"><path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z" stroke="currentColor" stroke-width="1.5" stroke-linejoin="round"/><path d="M14 2v6h6" stroke="currentColor" stroke-width="1.5" stroke-linejoin="round"/><path d="M8 13h8M8 17h5" stroke="currentColor" stroke-width="1.5" stroke-linecap="round"/></svg>',
  },
  {
    route: '/trash',
    label: t('sidebar.trash'),
    icon: '<svg width="16" height="16" viewBox="0 0 24 24" fill="none"><path d="M19 7l-.867 12.142A2 2 0 0116.138 21H7.862a2 2 0 01-1.995-1.858L5 7m5 4v6m4-6v6m1-10V4a1 1 0 00-1-1h-4a1 1 0 00-1 1v3M4 7h16" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"/></svg>',
  },
])

function navigate(path: string) {
  router.push(path)
}

function isActive(path: string): boolean {
  return route.path === path
}
</script>

<style scoped>
.app-sidebar {
  display: flex;
  flex-direction: column;
  width: var(--sidebar-width);
  height: 100%;
  background: var(--bg-surface);
  border-right: 1px solid var(--border-subtle);
  overflow: hidden;
  z-index: var(--z-sidebar);
  flex-shrink: 0;
}

/* ── Header ──────────────────────────────────────────────────── */
.sidebar-header {
  display: flex;
  align-items: center;
  height: 48px;
  padding: 0 var(--space-4);
  flex-shrink: 0;
}

.sidebar-brand {
  display: flex;
  align-items: center;
  gap: var(--space-2);
}

.brand-logo {
  flex-shrink: 0;
  display: flex;
  align-items: center;
  justify-content: center;
}

.brand-name {
  font-size: var(--text-base);
  font-weight: var(--font-semibold);
  letter-spacing: -0.01em;
  color: var(--text-primary);
  white-space: nowrap;
}

/* ── Navigation ──────────────────────────────────────────────── */
.sidebar-nav {
  flex: 1;
  overflow-y: auto;
  overflow-x: hidden;
  padding: 0 var(--space-3) var(--space-3);
}

.nav-group {
  margin-bottom: var(--space-5);
}

.nav-group-label {
  display: block;
  font-size: var(--text-xs);
  font-weight: var(--font-medium);
  color: var(--text-tertiary);
  padding: var(--space-2) var(--space-2) var(--space-1);
  white-space: nowrap;
}

.nav-item {
  position: relative;
  display: flex;
  align-items: center;
  width: 100%;
  height: 34px;
  padding: 0 var(--space-3);
  gap: var(--space-2);
  border-radius: var(--radius-md);
  color: var(--text-secondary);
  font-size: var(--text-sm);
  font-weight: var(--font-medium);
  white-space: nowrap;
  border: 1px solid transparent;
  background: transparent;
  cursor: pointer;
  transition:
    color var(--duration-fast) var(--ease),
    background var(--duration-fast) var(--ease),
    border-color var(--duration-fast) var(--ease);
}

.sidebar-nav .nav-item {
  margin-bottom: var(--space-1);
}

.nav-item:hover {
  color: var(--text-primary);
  background: var(--bg-hover);
}

.nav-item.active {
  color: var(--text-primary);
  background: var(--bg-active);
  border-color: var(--border-default);
}

.nav-item.active .nav-icon {
  color: var(--accent-primary);
}

.nav-icon {
  flex-shrink: 0;
  display: flex;
  align-items: center;
  justify-content: center;
  width: 16px;
  height: 16px;
  transition: color var(--duration-fast) var(--ease);
}

.nav-label {
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

/* ── Bottom ──────────────────────────────────────────────────── */
.sidebar-bottom {
  padding: var(--space-2);
  border-top: 1px solid var(--border-subtle);
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
  flex-shrink: 0;
}

/* ── Nav Badge ───────────────────────────────────────────────── */
.nav-badge {
  position: absolute;
  right: var(--space-2);
  top: 50%;
  transform: translateY(-50%);
  display: flex;
  align-items: center;
  justify-content: center;
  min-width: 18px;
  height: 18px;
  padding: 0 5px;
  border-radius: var(--radius-full);
  background: var(--accent-primary);
  color: #ffffff;
  font-size: 10px;
  font-weight: var(--font-bold);
  line-height: 1;
}

/* ── Start Game Button ───────────────────────────────────────── */
.start-game-btn {
  color: var(--text-primary);
  background: var(--accent-subtle);
  border: 1px solid rgba(110, 107, 240, 0.12) !important;
  font-weight: var(--font-semibold);
  transition:
    color var(--duration-fast) var(--ease),
    background var(--duration-fast) var(--ease),
    border-color var(--duration-fast) var(--ease);
}

.start-game-btn .nav-icon {
  color: var(--accent-primary);
}

.start-game-btn:hover {
  background: var(--accent-muted);
  border-color: rgba(110, 107, 240, 0.2) !important;
}

.start-game-btn:active {
  transform: scale(0.98);
}

.start-game-btn.is-launching {
  opacity: 0.7;
  pointer-events: none;
}
</style>
