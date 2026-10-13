<script setup lang="ts">
import Icon from '@/components/ui/Icon.vue'
import UiMenu, { type MenuItem } from '@/components/ui/UiMenu.vue'
import type { IconName } from '@/components/ui/icons'
import { useNotification } from '@/composables/useNotification'

import { computed, ref, onMounted, onUnmounted } from 'vue'
import { useRouter, useRoute } from 'vue-router'
import { useI18n } from 'vue-i18n'
import { invoke } from '@tauri-apps/api/core'
import { useDownloadStore } from '@/stores/download'
import { useAppStore } from '@/stores/app'

const router = useRouter()
const route = useRoute()
const { t } = useI18n()
const notify = useNotification()

const downloadStore = useDownloadStore()
const appStore = useAppStore()
const activeAndPendingCount = computed(() => downloadStore.activeAndPendingCount)

const isLaunching = ref(false)

async function startVamGame() {
  if (isLaunching.value) return
  const rootPath = appStore.vamRootPath
  if (!rootPath) {
    notify.warning(t('onDemand.vamNotSet'))
    return
  }
  isLaunching.value = true
  try {
    await invoke('launch_vam_direct', { vamRoot: rootPath })
  } catch (err) {
    console.error('Failed to launch VAM:', err)
    notify.error(String(err))
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

interface NavItem { route: string; label: string; short: string; icon: IconName }
const browseItems = computed<NavItem[]>(() => [
  { route: '/dashboard', label: t('sidebar.dashboard'), short: t('sidebar.short.dashboard'), icon: 'dashboard' },
  { route: '/packages', label: t('sidebar.packages'), short: t('sidebar.short.packages'), icon: 'package' },
  { route: '/statistics', label: t('sidebar.statistics'), short: t('sidebar.short.statistics'), icon: 'chart' },
])
const resourcesItems = computed<NavItem[]>(() => [
  { route: '/appearances', label: t('sidebar.appearances'), short: t('sidebar.short.appearances'), icon: 'user' },
  { route: '/scenes', label: t('sidebar.scenes'), short: t('sidebar.short.scenes'), icon: 'star' },
  { route: '/online', label: t('sidebar.onlineHub'), short: t('sidebar.short.online'), icon: 'globe' },
  { route: '/download', label: t('sidebar.downloadCenter'), short: t('sidebar.short.download'), icon: 'download' },
])
const toolMenuItems = computed<MenuItem[]>(() => [
  { value: '/dependency-completion', label: t('sidebar.dependencyCompletion'), icon: 'link' },
  { value: '/cleanup', label: t('ui.cleanup'), icon: 'filter' },
  { value: '/deduplication', label: t('sidebar.deduplication'), icon: 'copy' },
  { value: '/migration', label: t('sidebar.migration'), icon: 'move' },
  { value: '/ingestion', label: t('sidebar.ingestion'), icon: 'upload' },
  { value: '/vam-prefs', label: t('sidebar.vamPrefs'), icon: 'settings' },
  { value: '/trash', label: t('sidebar.trash'), icon: 'trash' },
])
const toolsOpen = ref(false)
const toolsAnchor = ref<HTMLElement | null>(null)
const toolsActive = computed(() => toolMenuItems.value.some(item => isActive(item.value)))
const toolsPosition = ref<{ x?: number; y?: number }>({})
function toggleTools(event: MouseEvent) {
  const button = event.currentTarget as HTMLElement
  toolsAnchor.value = button
  // 浮层贴着整条侧边栏的右缘弹出（而不是按钮右缘），顶部与「工具」按钮对齐
  const rail = (button.closest('.app-sidebar') as HTMLElement | null)?.getBoundingClientRect()
  const rect = button.getBoundingClientRect()
  toolsPosition.value = { x: (rail?.right ?? rect.right) + 8, y: rect.top }
  toolsOpen.value = !toolsOpen.value
}
function navigate(path: string) {
  toolsOpen.value = false
  router.push(path)
}

function isActive(path: string): boolean {
  return route.path === path
}
</script>
<template>
  <aside class="app-sidebar">
    <nav class="sidebar-nav" :aria-label="t('app.name')">
      <div v-for="(group, index) in [browseItems, resourcesItems]" :key="index" class="nav-group">
        <button
          v-for="item in group"
          :key="item.route"
          type="button"
          class="nav-item"
          :class="{ active: isActive(item.route) }"
          :aria-label="item.label"
          :aria-current="isActive(item.route) ? 'page' : undefined"
          :title="item.label"
          @click="navigate(item.route)"
        >
          <span class="nav-icon">
            <Icon :name="item.icon" />
            <span v-if="item.route === '/download' && activeAndPendingCount > 0" class="nav-badge">
              {{ activeAndPendingCount }}
            </span>
          </span>
          <span class="nav-label">{{ item.short }}</span>
        </button>
      </div>
      <div class="nav-group">
        <button
          type="button"
          class="nav-item"
          :class="{ active: toolsActive || toolsOpen }"
          aria-haspopup="menu"
          :aria-expanded="toolsOpen"
          :aria-label="t('ui.tools')"
          @click="toggleTools"
        >
          <span class="nav-icon"><Icon name="tools" /></span>
          <span class="nav-label">{{ t('sidebar.short.tools') }}</span>
        </button>
      </div>
    </nav>
    <div class="sidebar-bottom">
      <button
        type="button"
        class="nav-item start-game-btn"
        :disabled="isLaunching"
        :aria-label="t('sidebar.startGame')"
        :title="t('sidebar.startGame')"
        @click="startVamGame"
      >
        <span class="nav-icon"><Icon name="play" :size="22" /></span>
        <span class="nav-label">
          {{ isLaunching ? t('onDemand.running') : t('sidebar.short.startGame') }}
        </span>
      </button>
      <button
        type="button"
        class="nav-item"
        :class="{ active: isActive('/settings') }"
        :aria-current="isActive('/settings') ? 'page' : undefined"
        :title="t('sidebar.settings')"
        @click="navigate('/settings')"
      >
        <span class="nav-icon"><Icon name="settings" /></span>
        <span class="nav-label">{{ t('sidebar.short.settings') }}</span>
      </button>
    </div>
    <UiMenu
      v-model:open="toolsOpen"
      :anchor="toolsAnchor"
      :items="toolMenuItems"
      :label="t('ui.tools')"
      :x="toolsPosition.x"
      :y="toolsPosition.y"
      @select="navigate($event.value)"
    />
  </aside>
</template>
<style scoped>
.app-sidebar {
  display: flex;
  flex-direction: column;
  width: var(--sidebar-width);
  height: 100%;
  min-height: 0;
  padding: var(--space-3) var(--space-2) var(--space-4);
  border-radius: var(--radius-shell);
  background: var(--glass-sidebar-bg);
  flex-shrink: 0;
  z-index: var(--z-sidebar);
}
.sidebar-nav {
  flex: 1;
  min-height: 0;
  overflow-y: auto;
  overflow-x: hidden;
  scrollbar-width: none;
}
.nav-group + .nav-group::before {
  content: '';
  display: block;
  height: 1px;
  margin: var(--space-2) var(--space-4);
  background: var(--divider);
}
.nav-item {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: var(--space-1);
  width: 100%;
  padding: var(--space-1) 0;
  color: var(--text-tertiary);
  border-radius: var(--radius-icon);
}
.nav-icon {
  position: relative;
  display: grid;
  place-items: center;
  width: 44px;
  height: 32px;
  border-radius: var(--radius-icon);
}
.nav-item:hover .nav-icon {
  background: var(--bg-hover);
}
.nav-item.active {
  color: var(--text-primary);
}
.nav-item.active .nav-icon {
  background: var(--bg-active);
  color: var(--accent-primary);
}
.nav-label {
  width: 100%;
  font-size: var(--text-xs);
  font-weight: var(--font-bold);
  line-height: 1.2;
  text-align: center;
  overflow-wrap: anywhere;
}
.nav-badge {
  position: absolute;
  top: -3px;
  right: -2px;
  display: grid;
  place-items: center;
  min-width: 16px;
  height: 16px;
  padding: 0 var(--space-1);
  border: 2px solid var(--glass-sidebar-bg);
  border-radius: var(--radius-full);
  background: var(--accent-primary);
  color: var(--accent-ink);
  font-size: var(--text-xs);
  font-weight: var(--font-bold);
  line-height: 1;
}
.sidebar-bottom {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
  padding-top: var(--space-2);
  flex-shrink: 0;
}
.start-game-btn .nav-icon {
  width: 48px;
  height: 48px;
  border-radius: var(--radius-card);
  background: var(--accent-primary);
  color: var(--accent-ink);
}
.start-game-btn:hover .nav-icon {
  background: var(--accent-primary-hover);
}
.start-game-btn:disabled {
  opacity: 0.6;
  cursor: not-allowed;
}
</style>
