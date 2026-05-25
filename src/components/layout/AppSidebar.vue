<template>
  <aside :class="['app-sidebar', 'glass-sidebar', { collapsed }]">
    <!-- Sidebar Header (Brand & Collapse Toggle) -->
    <div class="sidebar-header">
      <div 
        :class="['sidebar-brand', { 'clickable': collapsed }]" 
        :title="collapsed ? $t('sidebar.expandHint') : undefined"
        @click="collapsed && toggleCollapse()"
        data-tauri-drag-region
      >
        <div class="brand-logo">
          <img src="/logo-square.png" alt="VAM Library" width="24" height="24" style="border-radius: 6px; object-fit: contain;" />
        </div>
        <span class="brand-name">{{ $t('app.name') }}</span>
      </div>

      <button
        class="collapse-toggle-btn"
        :class="{ 'is-collapsed': collapsed }"
        :title="collapsed ? $t('sidebar.expandHint') : $t('sidebar.collapseHint')"
        @click="toggleCollapse"
      >
        <svg
          width="14"
          height="14"
          viewBox="0 0 24 24"
          fill="none"
          class="collapse-icon"
        >
          <path d="M15 19l-7-7 7-7" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round" />
        </svg>
      </button>
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
          :title="collapsed ? item.label : undefined"
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
          :title="collapsed ? item.label : undefined"
          @click="navigate(item.route)"
        >
          <span class="nav-icon" v-html="item.icon" />
          <span class="nav-label">{{ item.label }}</span>
          <span
            v-if="item.route === '/download' && activeAndPendingCount > 0"
            :class="['nav-badge', { 'badge-collapsed': collapsed }]"
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
          :title="collapsed ? item.label : undefined"
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
        :title="collapsed ? $t('sidebar.startGame') : undefined"
        @click="startVamGame"
      >
        <span class="nav-icon">
          <svg width="18" height="18" viewBox="0 0 24 24" fill="none" class="play-icon">
            <path d="M8 5v14l11-7L8 5Z" fill="currentColor" stroke="currentColor" stroke-width="1.5" stroke-linejoin="round" />
          </svg>
        </span>
        <span class="nav-label">{{ isLaunching ? $t('onDemand.running') : $t('sidebar.startGame') }}</span>
      </button>
      <button
        :class="['nav-item', { active: isActive('/settings') }]"
        :title="collapsed ? $t('sidebar.settings') : undefined"
        @click="navigate('/settings')"
      >
        <span class="nav-icon">
          <svg width="18" height="18" viewBox="0 0 24 24" fill="none" class="settings-gear-icon">
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
import { computed, ref, onMounted, onUnmounted, watch } from 'vue'
import { useRouter, useRoute } from 'vue-router'
import { useI18n } from 'vue-i18n'
import { invoke } from '@tauri-apps/api/core'
import { useDownloadStore } from '@/stores/download'
import { useAppStore } from '@/stores/app'

interface Props {
  collapsed?: boolean
}

const props = withDefaults(defineProps<Props>(), {
  collapsed: false,
})

const emit = defineEmits<{
  'update:collapsed': [value: boolean]
}>()

const router = useRouter()
const route = useRoute()
const { t } = useI18n()

const collapsed = ref(props.collapsed)

// Sync with prop changes
watch(() => props.collapsed, (val) => {
  collapsed.value = val
})

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
    icon: '<svg width="18" height="18" viewBox="0 0 24 24" fill="none"><rect x="3" y="3" width="7" height="7" rx="1.5" stroke="currentColor" stroke-width="1.5"/><rect x="14" y="3" width="7" height="7" rx="1.5" stroke="currentColor" stroke-width="1.5"/><rect x="3" y="14" width="7" height="7" rx="1.5" stroke="currentColor" stroke-width="1.5"/><rect x="14" y="14" width="7" height="7" rx="1.5" stroke="currentColor" stroke-width="1.5"/></svg>',
  },
  {
    route: '/packages',
    label: t('sidebar.packages'),
    icon: '<svg width="18" height="18" viewBox="0 0 24 24" fill="none"><path d="M12 2L3 7V17L12 22L21 17V7L12 2Z" stroke="currentColor" stroke-width="1.5" stroke-linejoin="round"/><path d="M12 12L21 7" stroke="currentColor" stroke-width="1.5"/><path d="M12 12V22" stroke="currentColor" stroke-width="1.5"/><path d="M12 12L3 7" stroke="currentColor" stroke-width="1.5"/></svg>',
  },
  {
    route: '/statistics',
    label: t('sidebar.statistics'),
    icon: '<svg width="18" height="18" viewBox="0 0 24 24" fill="none"><path d="M5 20V10" stroke="currentColor" stroke-width="1.5" stroke-linecap="round"/><path d="M12 20V4" stroke="currentColor" stroke-width="1.5" stroke-linecap="round"/><path d="M19 20v-7" stroke="currentColor" stroke-width="1.5" stroke-linecap="round"/><path d="M3 20h18" stroke="currentColor" stroke-width="1.5" stroke-linecap="round"/></svg>',
  },
])

const resourcesItems = computed(() => [
  {
    route: '/online',
    label: t('sidebar.onlineHub'),
    icon: '<svg width="18" height="18" viewBox="0 0 24 24" fill="none"><circle cx="12" cy="12" r="10" stroke="currentColor" stroke-width="1.5"/><path d="M2 12h20M12 2a15.3 15.3 0 0 1 4 10 15.3 15.3 0 0 1-4 10 15.3 15.3 0 0 1-4-10 15.3 15.3 0 0 1 4-10z" stroke="currentColor" stroke-width="1.5"/></svg>',
  },
  {
    route: '/download',
    label: t('sidebar.downloadCenter'),
    icon: '<svg width="18" height="18" viewBox="0 0 24 24" fill="none"><path d="M12 14V3m0 11l-4-4m4 4l4-4M4 16v2a2 2 0 002 2h12a2 2 0 002-2v-2" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"/></svg>',
  },
])

const toolsItems = computed(() => [
  {
    route: '/dependency-completion',
    label: t('sidebar.dependencyCompletion'),
    icon: '<svg width="18" height="18" viewBox="0 0 24 24" fill="none"><path d="M4 12h7m0 0-3-3m3 3-3 3M13 7h4a2 2 0 0 1 2 2v8a2 2 0 0 1-2 2h-4" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"/><path d="M17 12h3" stroke="currentColor" stroke-width="1.5" stroke-linecap="round"/></svg>',
  },
  {
    route: '/deduplication',
    label: t('sidebar.deduplication'),
    icon: '<svg width="18" height="18" viewBox="0 0 24 24" fill="none"><rect x="3" y="3" width="12" height="12" rx="2" stroke="currentColor" stroke-width="1.5"/><path d="M9 9H21V21H9" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"/></svg>',
  },
  {
    route: '/on-demand',
    label: t('sidebar.onDemand'),
    icon: '<svg width="18" height="18" viewBox="0 0 24 24" fill="none"><path d="M8 5v14l11-7L8 5Z" stroke="currentColor" stroke-width="1.5" stroke-linejoin="round"/><path d="M4 6v12" stroke="currentColor" stroke-width="1.5" stroke-linecap="round"/></svg>',
  },
  {
    route: '/migration',
    label: t('sidebar.migration'),
    icon: '<svg width="18" height="18" viewBox="0 0 24 24" fill="none"><path d="M4 12H20M20 12L16 8M20 12L16 16" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"/><path d="M20 6H4M4 6L8 2M4 6L8 10" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" opacity="0.4"/></svg>',
  },
  {
    route: '/unpack',
    label: t('sidebar.unpack'),
    icon: '<svg width="18" height="18" viewBox="0 0 24 24" fill="none"><path d="M12 2L3 7v10l9 5 9-5V7l-9-5z" stroke="currentColor" stroke-width="1.5" stroke-linejoin="round"/><path d="M12 22V12" stroke="currentColor" stroke-width="1.5"/><path d="M21 7l-9 5L3 7" stroke="currentColor" stroke-width="1.5"/><path d="M12 12V6" stroke="currentColor" stroke-width="2" stroke-linecap="round"/><path d="M9 9l3-3 3 3" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"/></svg>',
  },
  {
    route: '/trash',
    label: t('sidebar.trash'),
    icon: '<svg width="18" height="18" viewBox="0 0 24 24" fill="none"><path d="M19 7l-.867 12.142A2 2 0 0116.138 21H7.862a2 2 0 01-1.995-1.858L5 7m5 4v6m4-6v6m1-10V4a1 1 0 00-1-1h-4a1 1 0 00-1 1v3M4 7h16" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"/></svg>',
  },
])

function navigate(path: string) {
  router.push(path)
}

function isActive(path: string): boolean {
  return route.path === path
}

function toggleCollapse() {
  collapsed.value = !collapsed.value
  emit('update:collapsed', collapsed.value)
}
</script>

<style scoped>
.app-sidebar {
  display: flex;
  flex-direction: column;
  width: var(--sidebar-width);
  height: 100vh;
  transition: width 350ms cubic-bezier(0.2, 0.8, 0.2, 1);
  overflow: hidden;
  z-index: var(--z-sidebar);
}

.app-sidebar.collapsed {
  width: var(--sidebar-collapsed-width);
}

/* ── Header (Brand & Collapse) ─────────────────────────────── */
.sidebar-header {
  position: relative;
  display: flex;
  align-items: center;
  justify-content: space-between;
  height: 50px;
  padding: 0 var(--space-4);
  margin-top: var(--space-2);
  margin-bottom: var(--space-3);
  transition: padding 350ms cubic-bezier(0.2, 0.8, 0.2, 1);
}

.collapsed .sidebar-header {
  padding: 0;
  justify-content: center;
}

.sidebar-brand {
  display: flex;
  align-items: center;
  gap: var(--space-3);
  height: 100%;
  flex: 1;
  transition: all 350ms cubic-bezier(0.2, 0.8, 0.2, 1);
}

.sidebar-brand.clickable {
  cursor: pointer;
}

.collapsed .sidebar-brand {
  justify-content: center;
  gap: 0;
}

.brand-logo {
  flex-shrink: 0;
  display: flex;
  align-items: center;
  justify-content: center;
  transition: opacity 350ms cubic-bezier(0.2, 0.8, 0.2, 1), transform 350ms cubic-bezier(0.2, 0.8, 0.2, 1);
}

.brand-name {
  font-size: var(--text-md);
  font-weight: var(--font-bold);
  letter-spacing: -0.02em;
  background: var(--accent-gradient);
  -webkit-background-clip: text;
  -webkit-text-fill-color: transparent;
  background-clip: text;
  white-space: nowrap;
  overflow: hidden;
  opacity: 1;
  max-width: 120px;
  transform: translateX(0);
  transition:
    opacity 300ms cubic-bezier(0.2, 0.8, 0.2, 1),
    max-width 350ms cubic-bezier(0.2, 0.8, 0.2, 1),
    transform 300ms cubic-bezier(0.2, 0.8, 0.2, 1);
}

.collapsed .brand-name {
  opacity: 0;
  max-width: 0;
  transform: translateX(-10px);
  pointer-events: none;
  transition:
    opacity 150ms cubic-bezier(0.2, 0.8, 0.2, 1),
    max-width 350ms cubic-bezier(0.2, 0.8, 0.2, 1),
    transform 150ms cubic-bezier(0.2, 0.8, 0.2, 1);
}

/* Collapse Toggle Button */
.collapse-toggle-btn {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 26px;
  height: 26px;
  border-radius: var(--radius-md);
  color: var(--text-tertiary);
  background: transparent;
  border: 1px solid transparent;
  cursor: pointer;
  transition: all 250ms cubic-bezier(0.2, 0.8, 0.2, 1);
}

.sidebar-header .collapse-toggle-btn {
  position: relative;
  opacity: 1;
  pointer-events: auto;
}

.collapsed .collapse-toggle-btn {
  position: absolute;
  left: 50%;
  top: 50%;
  transform: translate(-50%, -50%) scale(0.8);
  opacity: 0;
  pointer-events: none;
  z-index: 2;
}

/* Hover effects in collapsed mode: fade out logo, show button */
.collapsed .sidebar-header:hover .brand-logo {
  opacity: 0;
  transform: scale(0.8);
}

.collapsed .sidebar-header:hover .collapse-toggle-btn {
  opacity: 1;
  pointer-events: auto;
  transform: translate(-50%, -50%) scale(1);
}

.collapse-toggle-btn:hover {
  color: var(--text-primary);
  background: var(--bg-hover);
  border-color: var(--border-subtle);
  box-shadow: var(--glass-shadow);
}

.collapse-icon {
  transition: transform 350ms cubic-bezier(0.2, 0.8, 0.2, 1);
}

.is-collapsed .collapse-icon {
  transform: rotate(180deg);
}

/* ── Navigation ────────────────────────────────────────────── */
.sidebar-nav {
  flex: 1;
  overflow-y: auto;
  overflow-x: hidden;
  padding: 0 var(--space-3);
  transition: padding 350ms cubic-bezier(0.2, 0.8, 0.2, 1);
}

.collapsed .sidebar-nav {
  padding: 0 var(--space-2);
}

.nav-group {
  margin-bottom: var(--space-5);
}

.nav-group-label {
  display: block;
  font-size: 10px;
  font-weight: var(--font-semibold);
  text-transform: uppercase;
  letter-spacing: 0.1em;
  color: var(--text-tertiary);
  padding: 0 var(--space-3);
  margin-bottom: var(--space-2);
  white-space: nowrap;
  overflow: hidden;
  opacity: 1;
  max-height: 20px;
  transition:
    opacity 300ms cubic-bezier(0.2, 0.8, 0.2, 1),
    max-height 350ms cubic-bezier(0.2, 0.8, 0.2, 1),
    margin-bottom 350ms cubic-bezier(0.2, 0.8, 0.2, 1);
}

.collapsed .nav-group-label {
  opacity: 0;
  max-height: 0;
  margin-bottom: 0;
  pointer-events: none;
  transition:
    opacity 150ms cubic-bezier(0.2, 0.8, 0.2, 1),
    max-height 350ms cubic-bezier(0.2, 0.8, 0.2, 1),
    margin-bottom 350ms cubic-bezier(0.2, 0.8, 0.2, 1);
}

.nav-item {
  position: relative;
  display: flex;
  align-items: center;
  width: 100%;
  height: 38px;
  padding: 0 12px;
  gap: var(--space-3);
  border-radius: var(--radius-md);
  color: var(--text-secondary);
  font-size: var(--text-sm);
  font-weight: var(--font-medium);
  white-space: nowrap;
  transition:
    color var(--duration-fast) var(--ease),
    background var(--duration-fast) var(--ease),
    padding 350ms cubic-bezier(0.2, 0.8, 0.2, 1),
    gap 350ms cubic-bezier(0.2, 0.8, 0.2, 1);
}

.sidebar-nav .nav-item {
  margin-bottom: 4px;
}

.sidebar-nav .nav-item:last-child {
  margin-bottom: 0;
}

.collapsed .nav-item {
  padding: 0 17px;
  gap: 0;
}

.nav-item:hover {
  color: var(--text-primary);
  background: var(--bg-hover);
}

.nav-item.active {
  color: var(--text-primary);
  background: rgba(124, 92, 252, 0.15);
}

.nav-item.active .nav-icon {
  color: var(--accent-primary);
}

.nav-icon {
  flex-shrink: 0;
  display: flex;
  align-items: center;
  justify-content: center;
  width: 18px;
  height: 18px;
  transition: color var(--duration-fast) var(--ease);
}

.nav-label {
  white-space: nowrap;
  overflow: hidden;
  text-overflow: clip;
  opacity: 1;
  max-width: 150px;
  transform: translateX(0);
  transition:
    opacity 300ms cubic-bezier(0.2, 0.8, 0.2, 1),
    max-width 350ms cubic-bezier(0.2, 0.8, 0.2, 1),
    transform 300ms cubic-bezier(0.2, 0.8, 0.2, 1);
}

.collapsed .nav-label {
  opacity: 0;
  max-width: 0;
  transform: translateX(-10px);
  pointer-events: none;
  transition:
    opacity 150ms cubic-bezier(0.2, 0.8, 0.2, 1),
    max-width 350ms cubic-bezier(0.2, 0.8, 0.2, 1),
    transform 150ms cubic-bezier(0.2, 0.8, 0.2, 1);
}

/* ── Bottom ────────────────────────────────────────────────── */
.sidebar-bottom {
  padding: var(--space-3);
  border-top: 1px solid var(--border-subtle);
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
  transition: padding 350ms cubic-bezier(0.2, 0.8, 0.2, 1);
}

.collapsed .sidebar-bottom {
  padding: var(--space-2);
}

/* ── Nav Badge ──────────────────────────────────────────────── */
.nav-badge {
  position: absolute;
  right: var(--space-3);
  top: 50%;
  transform: translateY(-50%);
  display: flex;
  align-items: center;
  justify-content: center;
  min-width: 18px;
  height: 18px;
  padding: 0 var(--space-1);
  border-radius: var(--radius-full);
  background: var(--accent-primary);
  color: #ffffff;
  font-size: 10px;
  font-weight: var(--font-bold);
  line-height: 1;
  box-shadow: 0 2px 6px rgba(124, 92, 252, 0.4);
  animation: pulse-badge 2s infinite;
  transition: all 350ms cubic-bezier(0.2, 0.8, 0.2, 1);
}

.nav-badge.badge-collapsed {
  right: 14px;
  top: 6px;
  transform: none;
  min-width: 8px;
  max-width: 8px;
  height: 8px;
  padding: 0;
  border-radius: 50%;
  box-shadow: 0 0 0 2px var(--bg-surface);
  background: var(--accent-primary);
  font-size: 0;
  color: transparent;
  animation: none;
}

@keyframes pulse-badge {
  0% {
    box-shadow: 0 0 0 0 rgba(124, 92, 252, 0.6);
  }
  70% {
    box-shadow: 0 0 0 6px rgba(124, 92, 252, 0);
  }
  100% {
    box-shadow: 0 0 0 0 rgba(124, 92, 252, 0);
  }
}

.settings-gear-icon {
  transition: transform var(--duration-base) var(--ease);
}

.nav-item:hover .settings-gear-icon {
  transform: rotate(20deg);
}

/* ── Start Game Button ────────────────────────────────────── */
.start-game-btn {
  color: var(--text-primary);
  background: rgba(124, 92, 252, 0.08);
  border: 1px solid rgba(124, 92, 252, 0.15);
  font-weight: var(--font-semibold);
  transition: all var(--transition-fast) var(--ease);
}

.start-game-btn:hover {
  background: var(--accent-gradient);
  color: white;
  border-color: transparent;
  box-shadow: 0 4px 12px rgba(124, 92, 252, 0.3);
  transform: translateY(-1px);
}

.start-game-btn:active {
  transform: translateY(0);
}

.start-game-btn .play-icon {
  transition: transform var(--transition-fast) var(--ease);
}

.start-game-btn:hover .play-icon {
  transform: scale(1.1) translateX(1px);
}
</style>
