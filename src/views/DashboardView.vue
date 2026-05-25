<template>
  <div class="dashboard-view animate-fadeIn">
    <!-- Scan Banner (when scanning) -->
    <div v-if="isScanning" class="scan-banner glass-panel">
      <div class="scan-banner-content">
        <div class="scan-info">
          <div class="scan-spinner animate-spin" />
          <div class="scan-text">
            <span class="scan-title">{{ $t('toolbar.scanning') }}</span>
            <span v-if="scanProgress" class="scan-file text-tertiary">
              {{ scanProgress.current_file }}
            </span>
          </div>
        </div>
        <div class="scan-stats text-sm text-secondary">
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

    <!-- Stats Overview -->
    <section v-if="!loading" class="stats-grid stagger-children">
      <StatCard
        :title="$t('dashboard.totalPackages')"
        :value="stats.total_packages"
        :subtitle="$t('dashboard.totalPackagesSub')"
        :icon="icons.packages"
        trend="neutral"
        color="#7c5cfc"
      />
      <StatCard
        :title="$t('dashboard.totalSize')"
        :value="formatSize(stats.total_size_bytes)"
        :subtitle="$t('dashboard.totalSizeSub')"
        :icon="icons.disk"
        trend="neutral"
        color="#5b8def"
      />
      <StatCard
        :title="$t('dashboard.scenes')"
        :value="stats.scene_count"
        :subtitle="$t('dashboard.scenesSub')"
        :icon="icons.scenes"
        trend="neutral"
        color="#f59e0b"
      />
      <StatCard
        :title="$t('dashboard.issues')"
        :value="totalIssues"
        :subtitle="issuesSubtitle"
        :icon="icons.issues"
        :trend="totalIssues > 0 ? 'up' : 'neutral'"
        :color="totalIssues > 0 ? '#ef4444' : '#3ecf8e'"
      />
    </section>

    <!-- Skeleton loading for stats -->
    <section v-else class="stats-grid">
      <div v-for="i in 4" :key="i" class="skeleton-stat-card glass-panel">
        <SkeletonLoader variant="text" width="60%" height="12px" />
        <SkeletonLoader variant="text" width="40%" height="24px" />
        <SkeletonLoader variant="text" width="80%" height="10px" />
      </div>
    </section>

    <!-- Action Bar -->
    <div class="action-bar">
      <button class="scan-button" :disabled="isScanning || !vamRootPath" @click="handleScan">
        <span>{{ isScanning ? $t('toolbar.scanning') : $t('dashboard.scanVamDirectory') }}</span>
      </button>
      <button v-if="!vamRootPath" class="setup-button" @click="goToSettings">
        <svg width="16" height="16" viewBox="0 0 24 24" fill="none">
          <path d="M3 7V5a2 2 0 0 1 2-2h4l2 2h8a2 2 0 0 1 2 2v2" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" />
          <rect x="3" y="7" width="18" height="14" rx="2" stroke="currentColor" stroke-width="1.5" />
        </svg>
        <span>{{ $t('dashboard.selectVamDirectory') }}</span>
      </button>
    </div>

    <!-- Content Grid -->
    <div class="content-grid">
      <!-- Recent Packages -->
      <GlassPanel :title="$t('dashboard.recentPackages')">
        <div v-if="recentPackages.length > 0" class="recent-list">
          <div
            v-for="pkg in recentPackages"
            :key="pkg.id"
            class="recent-item"
          >
            <div class="recent-item-thumb">
              <img v-if="thumbnails[pkg.id]" :src="thumbnails[pkg.id]" :alt="pkg.creator + '.' + pkg.name" class="recent-thumb-img" />
              <div v-else class="recent-thumb-placeholder">
                <svg width="16" height="16" viewBox="0 0 24 24" fill="none">
                  <path d="M12 2L3 7V17L12 22L21 17V7L12 2Z" stroke="currentColor" stroke-width="1.5" stroke-linejoin="round"/>
                  <path d="M12 12L21 7" stroke="currentColor" stroke-width="1.5"/>
                  <path d="M12 12V22" stroke="currentColor" stroke-width="1.5"/>
                </svg>
              </div>
            </div>
            <div class="recent-item-info">
              <span class="recent-item-name">{{ pkg.creator }}.{{ pkg.name }}</span>
              <span class="recent-item-meta text-tertiary">{{ pkg.resource_types.length ? pkg.resource_types.map(t => resourceTypeLabel(t)).join(', ') : $t('resourceType.mixed') }}</span>
            </div>
            <span class="recent-item-size text-tertiary">{{ formatSize(pkg.size_bytes) }}</span>
          </div>
        </div>
        <EmptyState
          v-else
          :icon="icons.recentPackages"
          :title="$t('dashboard.noRecentPackages')"
          :description="$t('dashboard.noRecentPackagesDesc')"
        />
      </GlassPanel>

      <!-- Health Report -->
      <GlassPanel :title="$t('dashboard.healthReport')">
        <div v-if="totalIssues > 0" class="health-list">
          <!-- Missing Dependencies Group -->
          <div v-if="stats.missing_dependencies > 0" class="health-group">
            <div
              class="health-item health-warning clickable"
              @click="showMissingList = !showMissingList"
            >
              <div class="health-item-left">
                <svg width="14" height="14" viewBox="0 0 24 24" fill="none">
                  <path d="M12 22c5.523 0 10-4.477 10-10S17.523 2 12 2 2 6.477 2 12s4.477 10 10 10Z" stroke="currentColor" stroke-width="1.5"/>
                  <path d="M12 8v4M12 16h.01" stroke="currentColor" stroke-width="1.5" stroke-linecap="round"/>
                </svg>
                <span>{{ $t('dashboard.missingDependency', { count: stats.missing_dependencies }) }}</span>
              </div>
              <svg
                class="chevron-icon"
                :class="{ rotated: showMissingList }"
                width="14"
                height="14"
                viewBox="0 0 24 24"
                fill="none"
              >
                <path d="M9 5l7 7-7 7" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"/>
              </svg>
            </div>

            <transition name="expand">
              <div v-if="showMissingList && missingDependenciesList.length > 0" class="missing-deps-details glass-panel">
                <div class="missing-deps-list">
                  <div
                    v-for="(dep, idx) in missingDependenciesList"
                    :key="idx"
                    class="missing-dep-row"
                  >
                    <div class="missing-dep-info">
                      <span class="missing-dep-name" :title="dep.depends_on_id">{{ dep.depends_on_id }}</span>
                      <span class="missing-dep-ref text-tertiary">
                        {{ $t('dashboard.referencedBy', { name: dep.package_id }) }}
                      </span>
                    </div>
                    <button
                      class="copy-btn"
                      :title="$t('dashboard.copyId')"
                      @click.stop="copyToClipboard(dep.depends_on_id)"
                    >
                      <svg width="12" height="12" viewBox="0 0 24 24" fill="none">
                        <rect x="9" y="9" width="13" height="13" rx="2" stroke="currentColor" stroke-width="1.5"/>
                        <path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1" stroke="currentColor" stroke-width="1.5"/>
                      </svg>
                    </button>
                  </div>
                </div>
              </div>
            </transition>
          </div>

          <!-- Corrupted Packages Group -->
          <div v-if="stats.corrupted_packages > 0" class="health-group">
            <div
              class="health-item health-warning clickable"
              @click="showCorruptedList = !showCorruptedList"
            >
              <div class="health-item-left">
                <svg width="14" height="14" viewBox="0 0 24 24" fill="none">
                  <path d="M12 22c5.523 0 10-4.477 10-10S17.523 2 12 2 2 6.477 2 12s4.477 10 10 10Z" stroke="currentColor" stroke-width="1.5"/>
                  <path d="M12 8v4M12 16h.01" stroke="currentColor" stroke-width="1.5" stroke-linecap="round"/>
                </svg>
                <span>{{ $t('dashboard.corruptedPackageCount', { count: stats.corrupted_packages }) }}</span>
              </div>
              <svg
                class="chevron-icon"
                :class="{ rotated: showCorruptedList }"
                width="14"
                height="14"
                viewBox="0 0 24 24"
                fill="none"
              >
                <path d="M9 5l7 7-7 7" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"/>
              </svg>
            </div>

            <transition name="expand">
              <div v-if="showCorruptedList && corruptedPackagesList.length > 0" class="missing-deps-details glass-panel">
                <div class="missing-deps-list">
                  <div
                    v-for="(pkg, idx) in corruptedPackagesList"
                    :key="idx"
                    class="missing-dep-row"
                  >
                    <div class="missing-dep-info">
                      <span class="missing-dep-name" :title="pkg.package_id">{{ pkg.package_id }}</span>
                      <span class="missing-dep-ref text-tertiary" :title="pkg.error">
                        {{ pkg.error }}
                      </span>
                    </div>
                  </div>
                </div>
              </div>
            </transition>
          </div>

          <!-- Duplicate Resources Group -->
          <div v-if="stats.duplicate_resources > 0" class="health-item health-warning">
            <svg width="14" height="14" viewBox="0 0 24 24" fill="none">
              <rect x="3" y="3" width="12" height="12" rx="2" stroke="currentColor" stroke-width="1.5"/>
              <path d="M9 9H21V21H9" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"/>
            </svg>
            <span>{{ $t('dashboard.duplicateResource', { count: stats.duplicate_resources }) }}</span>
          </div>
        </div>
        <EmptyState
          v-else
          :icon="icons.health"
          :title="$t('dashboard.noHealthData')"
          :description="$t('dashboard.noHealthDataDesc')"
        />
      </GlassPanel>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, onMounted, ref, watch } from 'vue'
import { useRouter } from 'vue-router'
import { invoke } from '@tauri-apps/api/core'
import { storeToRefs } from 'pinia'
import { useAppStore } from '@/stores/app'
import { useLocalLibraryStore } from '@/stores/localLibrary'
import { useNotification } from '@/composables/useNotification'
import { useI18n } from 'vue-i18n'
import StatCard from '@/components/common/StatCard.vue'
import GlassPanel from '@/components/common/GlassPanel.vue'
import EmptyState from '@/components/common/EmptyState.vue'
import SkeletonLoader from '@/components/common/SkeletonLoader.vue'

const router = useRouter()
const appStore = useAppStore()
const localLibraryStore = useLocalLibraryStore()
const { t } = useI18n()
const { vamRootPath, isScanning, scanProgress } = storeToRefs(appStore)
const {
  dashboardStats: stats,
  recentPackages,
  missingDependencies: missingDependenciesList,
  corruptedPackages: corruptedPackagesList,
  loading,
} = storeToRefs(localLibraryStore)
const notify = useNotification()

const showMissingList = ref(false)
const showCorruptedList = ref(false)
const thumbnails = ref<Record<string, string>>({})

const totalIssues = computed(() =>
  stats.value.missing_dependencies +
  stats.value.duplicate_resources +
  stats.value.corrupted_packages
)

const issuesSubtitle = computed(() => {
  if (totalIssues.value === 0) return t('dashboard.noIssuesFound')
  return t('dashboard.issuesFound', { count: totalIssues.value })
})



onMounted(async () => {
  await localLibraryStore.ensureLoaded()
  await loadRecentThumbnails()
})

watch(recentPackages, () => {
  void loadRecentThumbnails()
})

async function loadRecentThumbnails() {
  await Promise.all(recentPackages.value.map(async (pkg) => {
    if (thumbnails.value[pkg.id]) return
    try {
      const thumb = await invoke<string | null>('get_package_thumbnail', { packageId: pkg.id })
      if (thumb) {
        thumbnails.value[pkg.id] = thumb
      }
    } catch {
      // 缩略图缺失不影响仪表盘数据。
    }
  }))
}

async function copyToClipboard(text: string) {
  try {
    await navigator.clipboard.writeText(text)
    notify.success(t('dashboard.idCopied'))
  } catch (e) {
    notify.error(String(e), t('common.error'))
  }
}

async function handleScan() {
  if (!vamRootPath.value || isScanning.value) return
  try {
    await appStore.startScan(vamRootPath.value)
    notify.success(t('dashboard.scanCompleted'))
  } catch (e) {
    notify.error(String(e), t('dashboard.scanFailed'))
  }
}

function goToSettings() {
  router.push('/settings')
}

function resourceTypeLabel(type: string): string {
  const key = `resourceType.${type}`
  const translated = t(key)
  return translated !== key ? translated : type
}

function formatSize(bytes: number): string {
  if (bytes === 0) return '0 B'
  const units = ['B', 'KB', 'MB', 'GB', 'TB']
  const i = Math.floor(Math.log(bytes) / Math.log(1024))
  const size = (bytes / Math.pow(1024, i)).toFixed(i > 0 ? 1 : 0)
  return `${size} ${units[i]}`
}

const icons = {
  packages: 'M12 2L3 7V17L12 22L21 17V7L12 2ZM12 12L21 7M12 12V22M12 12L3 7',
  disk: 'M4 7V4a2 2 0 0 1 2-2h8.5L20 7.5V20a2 2 0 0 1-2 2H6a2 2 0 0 1-2-2v-3M2 12h16M6 9v6',
  scenes: 'M2 4h20v16H2zM2 9h20M5 6.5h0M7.5 6.5h0M10 6.5h0',
  issues: 'M12 22c5.523 0 10-4.477 10-10S17.523 2 12 2 2 6.477 2 12s4.477 10 10 10ZM12 8v4M12 16h.01',
  recentPackages: 'M12 8v4l3 3M21 12a9 9 0 1 1-18 0 9 9 0 0 1 18 0Z',
  health: 'M22 12h-4l-3 9L9 3l-3 9H2',
}
</script>

<style scoped>
.dashboard-view {
  display: flex;
  flex-direction: column;
  gap: var(--space-5);
}

.scan-error-panel {
  display: flex;
  flex-direction: column;
  gap: var(--space-3);
  padding: var(--space-4);
  border: 1px solid rgba(248, 113, 113, 0.22);
}

.scan-error-header {
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
}

.scan-error-header strong {
  color: var(--color-error);
  font-size: var(--text-sm);
}

.scan-error-header span,
.scan-error-item {
  color: var(--text-secondary);
  font-size: var(--text-xs);
}

.scan-error-list {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
}

.scan-error-item {
  overflow: hidden;
  padding: var(--space-2);
  border-radius: var(--radius-sm);
  background: rgba(248, 113, 113, 0.06);
  text-overflow: ellipsis;
  white-space: nowrap;
}

/* ── Scan Banner ──────────────────────────────────────────── */
.scan-banner {
  overflow: hidden;
}

.scan-banner-content {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: var(--space-4) var(--space-5);
}

.scan-info {
  display: flex;
  align-items: center;
  gap: var(--space-3);
}

.scan-spinner {
  width: 18px;
  height: 18px;
  border: 2px solid var(--accent-primary);
  border-top-color: transparent;
  border-radius: 50%;
}

.scan-text {
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.scan-title {
  font-size: var(--text-sm);
  font-weight: var(--font-semibold);
  color: var(--accent-primary);
}

.scan-file {
  font-size: var(--text-xs);
  max-width: 300px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.scan-stats {
  font-variant-numeric: tabular-nums;
}

.scan-progress-bar {
  height: 3px;
  background: var(--bg-hover);
}

.scan-progress-fill {
  height: 100%;
  background: var(--accent-gradient);
  transition: width 200ms ease;
}

/* ── Stats ────────────────────────────────────────────────── */
.stats-grid {
  display: grid;
  grid-template-columns: repeat(4, 1fr);
  gap: var(--space-5);
}

@media (max-width: 1200px) {
  .stats-grid {
    grid-template-columns: repeat(2, 1fr);
  }
}

/* ── Action Bar ───────────────────────────────────────────── */
.action-bar {
  display: flex;
  gap: var(--space-3);
}

.scan-button,
.setup-button {
  display: inline-flex;
  align-items: center;
  gap: var(--space-2);
  padding: var(--space-2) var(--space-5);
  height: 36px;
  font-size: var(--text-sm);
  font-weight: var(--font-semibold);
  border-radius: var(--radius-md);
  transition:
    opacity var(--duration-fast) var(--ease),
    transform var(--duration-fast) var(--ease);
}

.scan-button {
  background: var(--accent-gradient);
  color: white;
}

.scan-button:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}

.scan-button:not(:disabled):hover {
  opacity: 0.9;
  transform: translateY(-1px);
}

.setup-button {
  background: var(--bg-elevated);
  color: var(--text-primary);
  border: 1px solid var(--border-subtle);
}

.setup-button:hover {
  background: var(--bg-hover);
  border-color: var(--border-default);
}

/* ── Content Grid ─────────────────────────────────────────── */
.content-grid {
  display: grid;
  grid-template-columns: repeat(2, 1fr);
  gap: var(--space-5);
}

@media (max-width: 1000px) {
  .content-grid {
    grid-template-columns: 1fr;
  }
}

/* ── Recent List ──────────────────────────────────────────── */
.recent-list {
  display: flex;
  flex-direction: column;
  gap: 1px;
}

.recent-item {
  display: flex;
  align-items: center;
  gap: var(--space-3);
  padding: var(--space-3) 0;
}

.recent-item + .recent-item {
  border-top: 1px solid var(--border-subtle);
}

.recent-item-thumb {
  width: 36px;
  height: 36px;
  border-radius: var(--radius-md);
  overflow: hidden;
  background: var(--bg-base);
  flex-shrink: 0;
  display: flex;
  align-items: center;
  justify-content: center;
  border: 1px solid var(--border-subtle);
}

.recent-thumb-img {
  width: 100%;
  height: 100%;
  object-fit: cover;
}

.recent-thumb-placeholder {
  color: var(--accent-primary);
  opacity: 0.6;
  display: flex;
  align-items: center;
  justify-content: center;
}

.recent-item-info {
  flex: 1;
  min-width: 0;
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.recent-item-name {
  font-size: var(--text-sm);
  color: var(--text-primary);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.recent-item-meta {
  font-size: var(--text-xs);
}

.recent-item-size {
  font-size: var(--text-xs);
  flex-shrink: 0;
  font-variant-numeric: tabular-nums;
}

/* ── Health List ──────────────────────────────────────────── */
.health-list {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
}

.health-item {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  padding: var(--space-2) var(--space-3);
  border-radius: var(--radius-sm);
  font-size: var(--text-sm);
}

.health-warning {
  background: var(--color-warning-bg);
  color: var(--color-warning);
}

.clickable {
  cursor: pointer;
  user-select: none;
  transition: background-color var(--transition-fast), transform var(--transition-fast);
}

.clickable:hover {
  background-color: rgba(251, 191, 36, 0.15);
}

.health-group {
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
}

.health-item-left {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  flex: 1;
}

.chevron-icon {
  transition: transform var(--transition-normal);
  opacity: 0.8;
}

.chevron-icon.rotated {
  transform: rotate(90deg);
}

/* Collapsible container for missing dependencies details */
.missing-deps-details {
  margin-top: var(--space-1);
  padding: var(--space-3) !important;
  border-radius: var(--radius-md) !important;
  background: rgba(35, 35, 60, 0.4) !important;
  border: 1px solid rgba(255, 255, 255, 0.05) !important;
  max-height: 280px;
  overflow-y: auto;
  scrollbar-width: thin;
  scrollbar-color: rgba(255, 255, 255, 0.1) transparent;
}

/* Custom scrollbar for webkit */
.missing-deps-details::-webkit-scrollbar {
  width: 4px;
}
.missing-deps-details::-webkit-scrollbar-track {
  background: transparent;
}
.missing-deps-details::-webkit-scrollbar-thumb {
  background: rgba(255, 255, 255, 0.1);
  border-radius: var(--radius-full);
}
.missing-deps-details::-webkit-scrollbar-thumb:hover {
  background: rgba(255, 255, 255, 0.2);
}

.missing-deps-list {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
}

.missing-dep-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--space-3);
  padding: var(--space-2) var(--space-3);
  border-radius: var(--radius-sm);
  background: rgba(255, 255, 255, 0.02);
  border: 1px solid rgba(255, 255, 255, 0.02);
  transition: background-color var(--transition-fast), border-color var(--transition-fast);
}

.missing-dep-row:hover {
  background: rgba(255, 255, 255, 0.04);
  border-color: rgba(255, 255, 255, 0.05);
}

.missing-dep-info {
  display: flex;
  flex-direction: column;
  gap: 2px;
  min-width: 0;
  flex: 1;
}

.missing-dep-name {
  font-size: var(--text-xs);
  font-weight: var(--font-semibold);
  color: var(--text-primary);
  font-family: monospace;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.missing-dep-ref {
  font-size: 10px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

/* Copy Button */
.copy-btn {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 24px;
  height: 24px;
  border-radius: var(--radius-sm);
  background: rgba(255, 255, 255, 0.05);
  color: var(--text-secondary);
  border: 1px solid rgba(255, 255, 255, 0.05);
  cursor: pointer;
  transition: all var(--transition-fast);
  flex-shrink: 0;
}

.copy-btn:hover {
  background: var(--accent-gradient);
  color: white;
  border-color: transparent;
  transform: scale(1.05);
}

.copy-btn:active {
  transform: scale(0.95);
}

/* CSS transitions for expansion */
.expand-enter-active,
.expand-leave-active {
  transition: all 0.25s cubic-bezier(0.4, 0, 0.2, 1);
  max-height: 280px;
  opacity: 1;
  overflow: hidden;
}
.expand-enter-from,
.expand-leave-to {
  max-height: 0;
  opacity: 0;
  padding-top: 0 !important;
  padding-bottom: 0 !important;
  margin-top: 0 !important;
}

/* ── Skeleton Loading ─────────────────────────────────────── */
.skeleton-stat-card {
  display: flex;
  flex-direction: column;
  gap: var(--space-3);
  padding: var(--space-5);
  border-radius: var(--radius-lg);
}
</style>
