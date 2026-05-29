<template>
  <div class="dashboard animate-fadeIn">
    <!-- Scan Banner -->
    <div v-if="isScanning" class="scan-banner">
      <div class="scan-banner-content">
        <div class="scan-info">
          <div class="scan-spinner animate-spin" />
          <div class="scan-text">
            <span class="scan-title">{{ $t('toolbar.scanning') }}</span>
            <span v-if="scanProgress" class="scan-file">
              {{ scanProgress.current_file }}
            </span>
          </div>
        </div>
        <div class="scan-stats">
          {{ scanProgress ? `${scanProgress.processed_files} / ${scanProgress.total_files}` : '' }}
        </div>
      </div>
      <div v-if="scanProgress" class="scan-progress-bar">
        <div
          class="scan-progress-fill"
          :style="{ width: scanProgress.total_files > 0 ? (scanProgress.processed_files / scanProgress.total_files * 100) + '%' : '0%' }"
        />
      </div>
    </div>

    <!-- Stats Row -->
    <StatsBar v-if="!loading" :stats="stats" :loading="false" />
    <div v-else class="stats-skeleton">
      <SkeletonLoader v-for="i in 4" :key="i" variant="text" width="80px" height="24px" />
    </div>

    <section class="dashboard-section">
      <h2 class="section-title">{{ $t('dashboard.quickOpen') }}</h2>
      <GlassPanel>
        <QuickActions
          :vam-root="vamRootPath"
          :screenshot-path="screenshotPath"
          @open="openPath"
        />
      </GlassPanel>
    </section>

    <div class="content-grid">
      <section class="dashboard-section">
        <h2 class="section-title">{{ $t('dashboard.recentPackages') }}</h2>
        <GlassPanel class="dashboard-panel recent-panel">
          <div v-if="recentPreviewPackages.length > 0" class="recent-grid">
            <div
              v-for="pkg in recentPreviewPackages"
              :key="pkg.id"
              class="recent-card"
              :title="`${pkg.creator}.${pkg.name}`"
            >
              <img
                v-if="thumbnails[pkg.id]"
                :src="thumbnails[pkg.id]"
                :alt="pkg.creator + '.' + pkg.name"
                class="recent-card-image"
              />
              <div v-else class="recent-card-placeholder">
                <svg width="24" height="24" viewBox="0 0 24 24" fill="none">
                  <path d="M12 2L3 7V17L12 22L21 17V7L12 2Z" stroke="currentColor" stroke-width="1.5" stroke-linejoin="round"/>
                  <path d="M12 12L21 7M12 12V22M12 12L3 7" stroke="currentColor" stroke-width="1.5"/>
                </svg>
              </div>
              <div class="recent-card-overlay">
                <div class="recent-card-name">
                  {{ pkg.creator }}.{{ pkg.name }}
                </div>
              </div>
            </div>
          </div>
          <EmptyState
            v-else
            :icon="icons.recentPackages"
            :title="$t('dashboard.noRecentPackages')"
            :description="$t('dashboard.noRecentPackagesDesc')"
          />
        </GlassPanel>
      </section>

      <section class="dashboard-section">
        <h2 class="section-title">{{ $t('dashboard.healthReport') }}</h2>
        <GlassPanel class="dashboard-panel health-panel">
          <HealthStatus
            :missing="stats.missing_dependencies"
            :corrupted="stats.corrupted_packages"
            :duplicates="stats.duplicate_resources"
            :missing-list="missingDependenciesList"
            :corrupted-list="corruptedPackagesList"
          />
        </GlassPanel>
      </section>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, onMounted, ref, watch } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { storeToRefs } from 'pinia'
import { useAppStore } from '@/stores/app'
import { useLocalLibraryStore } from '@/stores/localLibrary'
import { useNotification } from '@/composables/useNotification'

import StatsBar from '@/components/dashboard/StatsBar.vue'
import QuickActions from '@/components/dashboard/QuickActions.vue'
import HealthStatus from '@/components/dashboard/HealthStatus.vue'
import EmptyState from '@/components/common/EmptyState.vue'
import GlassPanel from '@/components/common/GlassPanel.vue'
import SkeletonLoader from '@/components/common/SkeletonLoader.vue'

const appStore = useAppStore()
const localLibraryStore = useLocalLibraryStore()
const notify = useNotification()

const { vamRootPath, isScanning, scanProgress } = storeToRefs(appStore)
const {
  dashboardStats: stats,
  recentPackages,
  missingDependencies: missingDependenciesList,
  corruptedPackages: corruptedPackagesList,
  loading,
} = storeToRefs(localLibraryStore)

const thumbnails = ref<Record<string, string>>({})

const recentPreviewPackages = computed(() => recentPackages.value.slice(0, 6))

const screenshotPath = computed(() => (
  vamRootPath.value ? `${vamRootPath.value}\\Saves\\scene` : null
))

onMounted(async () => {
  await localLibraryStore.ensureLoaded()
  await loadRecentThumbnails()
})

watch(recentPackages, () => {
  void loadRecentThumbnails()
})

async function loadRecentThumbnails() {
  await Promise.all(recentPreviewPackages.value.map(async (pkg) => {
    if (thumbnails.value[pkg.id]) return
    try {
      const thumb = await invoke<string | null>('get_package_thumbnail', { packageId: pkg.id })
      if (thumb) {
        thumbnails.value[pkg.id] = thumb
      }
    } catch {
      // thumbnail missing is fine
    }
  }))
}

async function openPath(path: string) {
  try {
    await invoke('open_path_in_explorer', { path })
  } catch (e) {
    notify.error(String(e))
  }
}

const icons = {
  recentPackages: 'M12 8v4l3 3M21 12a9 9 0 1 1-18 0 9 9 0 0 1 18 0Z',
}
</script>

<style scoped>
.dashboard {
  display: flex;
  flex-direction: column;
  gap: var(--space-4);
  width: 100%;
}

/* ── Scan Banner ──────────────────────────────────────────── */
.scan-banner {
  background: var(--bg-surface);
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-lg);
  overflow: hidden;
}

.scan-banner-content {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: var(--space-3) var(--space-4);
}

.scan-info {
  display: flex;
  align-items: center;
  gap: var(--space-2);
}

.scan-spinner {
  width: 14px;
  height: 14px;
  border: 2px solid var(--accent-primary);
  border-top-color: transparent;
  border-radius: 50%;
}

.scan-text {
  display: flex;
  flex-direction: column;
  gap: 1px;
}

.scan-title {
  font-size: var(--text-sm);
  font-weight: var(--font-medium);
  color: var(--accent-primary);
}

.scan-file {
  font-size: var(--text-xs);
  color: var(--text-tertiary);
  max-width: 300px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.scan-stats {
  font-size: var(--text-sm);
  color: var(--text-secondary);
  font-variant-numeric: tabular-nums;
}

.scan-progress-bar {
  height: 2px;
  background: var(--bg-hover);
}

.scan-progress-fill {
  height: 100%;
  background: var(--accent-gradient);
  transition: width 200ms ease;
}

/* ── Stats Skeleton ───────────────────────────────────────── */
.stats-skeleton {
  display: flex;
  gap: var(--space-6);
  padding: var(--space-3) 0;
}

/* ── Section ──────────────────────────────────────────────── */
.dashboard-section {
  display: flex;
  flex-direction: column;
  gap: var(--space-3);
  min-width: 0;
}

.section-title {
  font-size: var(--text-sm);
  font-weight: var(--font-medium);
  color: var(--text-secondary);
  margin: 0;
}

/* ── Content Grid ─────────────────────────────────────────── */
.content-grid {
  display: grid;
  grid-template-columns: minmax(0, 2fr) minmax(320px, 1fr);
  gap: var(--space-4);
  align-items: start;
}

.dashboard-panel {
  min-height: 100%;
}

.recent-panel :deep(.panel-content) {
  min-height: 340px;
}

.health-panel :deep(.panel-content) {
  min-height: 340px;
}

/* ── Recent Grid ──────────────────────────────────────────── */
.recent-grid {
  display: grid;
  grid-template-columns: repeat(3, 1fr);
  gap: var(--space-3);
}

@media (max-width: 800px) {
  .recent-grid {
    grid-template-columns: repeat(2, 1fr);
  }
}

.recent-card {
  position: relative;
  overflow: hidden;
  aspect-ratio: 4 / 3;
  border-radius: var(--radius-lg);
  background: var(--bg-elevated);
  cursor: default;
  transition:
    transform var(--duration-base) var(--ease),
    box-shadow var(--duration-base) var(--ease);
}

.recent-card:hover {
  transform: translateY(-1px);
  box-shadow: var(--shadow-md);
}

.recent-card-image {
  width: 100%;
  height: 100%;
  object-fit: cover;
  display: block;
  transition: transform var(--duration-base) var(--ease);
}

.recent-card:hover .recent-card-image {
  transform: scale(1.02);
}

.recent-card-placeholder {
  width: 100%;
  height: 100%;
  display: flex;
  align-items: center;
  justify-content: center;
  color: var(--text-disabled);
  background: var(--bg-elevated);
}

.recent-card-overlay {
  position: absolute;
  inset: auto 0 0 0;
  padding: var(--space-4) var(--space-3) var(--space-3);
  background: linear-gradient(180deg, transparent, rgba(22, 22, 24, 0.85) 60%);
}

.recent-card-name {
  color: #fff;
  font-size: var(--text-xs);
  font-weight: var(--font-medium);
  line-height: 1.3;
  text-shadow: 0 1px 4px rgba(0, 0, 0, 0.4);
  display: -webkit-box;
  overflow: hidden;
  -webkit-box-orient: vertical;
  -webkit-line-clamp: 2;
}

@media (max-width: 1100px) {
  .content-grid {
    grid-template-columns: 1fr;
  }
}
</style>
