<script setup lang="ts">
import { useRouter } from 'vue-router'
import PageHeader from '@/components/layout/PageHeader.vue'
import UiButton from '@/components/ui/UiButton.vue'
import UiProgress from '@/components/ui/UiProgress.vue'
import { loadPackageThumbnails, resourceCacheRevision } from '@/utils/resourceCache'
import PackageCard from '@/components/packages/PackageCard.vue'

import { computed, onMounted, onBeforeUnmount, ref, watch } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { storeToRefs } from 'pinia'
import { useI18n } from 'vue-i18n'
import { useAppStore } from '@/stores/app'
import { useLocalLibraryStore } from '@/stores/localLibrary'
import { useNotification } from '@/composables/useNotification'

import StatsBar from '@/components/dashboard/StatsBar.vue'
import QuickActions from '@/components/dashboard/QuickActions.vue'
import HealthStatus from '@/components/dashboard/HealthStatus.vue'
import ResourceDisplay from '@/components/ResourceDisplay.vue'

const appStore = useAppStore()
const localLibraryStore = useLocalLibraryStore()
const notify = useNotification()
const { t } = useI18n()

const { vamRootPath, isScanning, scanProgress } = storeToRefs(appStore)
const {
  dashboardStats: stats,
  recentPackages,
  missingDependencies: missingDependenciesList,
  corruptedPackages: corruptedPackagesList,
  loading,
} = storeToRefs(localLibraryStore)

const thumbnails = ref<Record<string, string | null>>({})

const recentPreviewPackages = computed(() => recentPackages.value.slice(0, 12))

const screenshotPath = computed(() => (
  vamRootPath.value ? `${vamRootPath.value}\\Saves\\scene` : null
))

const appearancePresetPath = computed(() => (
  vamRootPath.value ? `${vamRootPath.value}\\Custom\\Atom\\Person\\Appearance` : null
))

const clothingPresetPath = computed(() => (
  vamRootPath.value ? `${vamRootPath.value}\\Custom\\Atom\\Person\\Clothing` : null
))

onMounted(() => {
  window.requestAnimationFrame(() => {
    window.requestAnimationFrame(() => {
      void localLibraryStore.ensureLoaded().then(() => loadRecentThumbnails())
    })
  })
})

watch([recentPackages, resourceCacheRevision], () => {
  void loadRecentThumbnails()
})

let thumbnailEpoch = 0
onBeforeUnmount(() => { ++thumbnailEpoch })
async function loadRecentThumbnails() {
  const epoch = ++thumbnailEpoch
  await loadPackageThumbnails(recentPreviewPackages.value, (id, url) => { thumbnails.value[id] = url }, () => epoch === thumbnailEpoch)
}

async function openPath(path: string) {
  try {
    await invoke('open_path_in_explorer', { path })
  } catch (e) {
    notify.error(String(e))
  }
}

async function openGameConfig() {
  if (!vamRootPath.value) return
  try {
    await invoke('launch_vam_config', { vamRoot: vamRootPath.value })
    notify.success(t('dashboard.gameConfigLaunched'))
  } catch (e) {
    notify.error(String(e))
  }
}


const router = useRouter()
async function scanDashboard() { if (!vamRootPath.value || isScanning.value) return; try { await appStore.startScan(vamRootPath.value) } catch (error) { notify.error(String(error), t('dashboard.scanFailed')) } }
const totalIssues = computed(() => stats.value.missing_dependencies + stats.value.corrupted_packages + stats.value.duplicate_resources)
</script><template>
  <div class="dashboard-page">
    <PageHeader :title="t('sidebar.dashboard')" :subtitle="vamRootPath || t('gameContent.noGame')">
      <template #actions>
        <UiButton icon="upload" @click="router.push('/ingestion')">
          {{ t('sidebar.ingestion') }}
        </UiButton>
        <UiButton
          variant="primary"
          icon="refresh"
          :loading="isScanning"
          :loading-text="t('toolbar.scanning')"
          :disabled="isScanning || !vamRootPath"
          @click="scanDashboard"
        >
          {{ t('ui.startScan') }}
        </UiButton>
      </template>
    </PageHeader>
    <div class="dashboard-content">
      <ResourceDisplay
        :packages="recentPreviewPackages"
        view-mode="small-card"
        :result-count-label="t('dashboard.recentPackages')"
        :empty-title="t('dashboard.noRecentPackages')"
        :empty-description="t('dashboard.noRecentPackagesDesc')"
        show-quick-delete
        @package-deleted="localLibraryStore.refreshAll('refreshing')"
      >
        <template #browser="{ openDetails, launchPackage }">
          <div class="dashboard resource-content-scrollable">
            <div v-if="isScanning" class="scan-banner">
              <UiProgress
                circular
                :value="scanProgress?.processed_files || 0"
                :max="scanProgress?.total_files || 1"
                :label="t('toolbar.scanning')"
              />
              <div class="scan-text">
                <strong>{{ t('toolbar.scanning') }}</strong>
                <span :title="scanProgress?.current_file">{{ scanProgress?.current_file }}</span>
                <UiProgress
                  :value="scanProgress?.processed_files || 0"
                  :max="scanProgress?.total_files || 1"
                  :indeterminate="!scanProgress?.total_files"
                  :label="t('toolbar.scanning')"
                />
              </div>
              <span class="scan-stats">
                {{ scanProgress?.processed_files || 0 }} / {{ scanProgress?.total_files || 0 }}
              </span>
            </div>
            <section class="dashboard-section quick-open-section" :aria-label="t('dashboard.quickOpen')">
              <QuickActions
                :vam-root="vamRootPath"
                :screenshot-path="screenshotPath"
                :appearance-preset-path="appearancePresetPath"
                :clothing-preset-path="clothingPresetPath"
                @open="openPath"
                @game-config="openGameConfig"
              />
            </section>
            <StatsBar :stats="stats" :loading="loading" />
            <div class="content-grid">
              <div class="dashboard-left">
                <section class="dashboard-section">
                  <h2>{{ t('dashboard.recentPackages') }}</h2>
                  <div v-if="loading" class="recent-grid resource-grid">
                    <PackageCard
                      v-for="i in 4"
                      :key="i"
                      :item="{ id: String(i), name: '' }"
                      loading
                    />
                  </div>
                  <div v-else-if="recentPreviewPackages.length" class="recent-grid resource-grid">
                    <PackageCard
                      v-for="pkg in recentPreviewPackages"
                      :key="pkg.id"
                      :item="pkg"
                      :image="thumbnails[pkg.id]"
                      :fresh="Date.now() - new Date(pkg.scan_time).getTime() < 86400000"
                      :can-launch="pkg.resource_types.includes('scene')"
                      @select="openDetails(pkg)"
                      @details="openDetails(pkg)"
                      @launch="launchPackage(pkg)"
                      @more="openDetails(pkg)"
                    />
                  </div>
                  <div v-else class="dashboard-empty">
                    <p>{{ t('dashboard.noRecentPackagesDesc') }}</p>
                    <UiButton
                      variant="primary"
                      :disabled="isScanning || !vamRootPath"
                      @click="scanDashboard"
                    >
                      {{ t('ui.startScan') }}
                    </UiButton>
                  </div>
                </section>
              </div>
              <section class="dashboard-section">
                <header class="health-head">
                  <h2>{{ t('dashboard.healthReport') }}</h2>
                  <span :class="{ 'is-ok': !totalIssues }">{{ totalIssues ? t('ui.issueCount', { count: totalIssues }) : t('ui.noIssues') }}</span>
                </header>
                <div class="health-panel">
                <HealthStatus
                  :missing="stats.missing_dependencies"
                  :corrupted="stats.corrupted_packages"
                  :duplicates="stats.duplicate_resources"
                  :missing-list="missingDependenciesList"
                  :corrupted-list="corruptedPackagesList"
                />
                </div>
              </section>
            </div>
          </div>
        </template>
      </ResourceDisplay>
    </div>
  </div>
</template><style scoped>
.dashboard-page {
  display: flex;
  flex-direction: column;
  height: 100%;
  min-height: 0;
}
.dashboard-content {
  flex: 1;
  min-height: 0;
  padding: var(--space-page-y) var(--space-page-x);
}
.dashboard {
  display: flex;
  flex-direction: column;
  flex: 1;
  min-height: 0;
  min-width: 0;
  overflow-y: auto;
  gap: var(--space-section);
  /* 给选中外环和键盘焦点留空间，用负 margin 抵消，保证内容左缘与页头对齐 */
  padding: var(--focus-room);
  margin: calc(-1 * var(--focus-room));
}
.scan-banner {
  display: flex;
  align-items: center;
  gap: var(--space-4);
  padding: var(--space-4) var(--space-panel);
  border-radius: var(--radius-lg);
  background: var(--bg-elevated);
}
.scan-text {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
  flex: 1;
  min-width: 0;
}
.scan-text strong {
  font-size: var(--text-md);
  font-weight: var(--font-heavy);
}
.scan-text > span {
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
  font-size: var(--text-sm);
  color: var(--text-tertiary);
}
.scan-stats {
  font-size: var(--text-base);
  color: var(--text-secondary);
}
.content-grid {
  display: grid;
  grid-template-columns: minmax(0, 1fr) minmax(300px, 0.6fr);
  gap: var(--space-section);
  align-items: start;
}
.dashboard-left {
  display: flex;
  flex-direction: column;
  gap: var(--space-section);
  min-width: 0;
}
.dashboard-section {
  min-width: 0;
}
.dashboard-section h2 {
  font-size: var(--text-lg);
  font-weight: var(--font-heavy);
  line-height: var(--leading-tight);
  margin: 0 0 var(--space-stack);
}

.health-panel {
  padding: var(--space-panel);
  border-radius: var(--radius-lg);
  background: var(--bg-elevated);
}
.health-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--space-item);
  margin-bottom: var(--space-stack);
}
.health-head h2 {
  margin: 0;
}
.health-head > span {
  font-size: var(--text-xs);
  color: var(--color-warning);
  background: var(--color-warning-bg);
  border-radius: var(--radius-full);
  padding: var(--space-1) var(--space-2);
}
.health-head > span.is-ok {
  color: var(--color-success);
  background: var(--color-success-bg);
}
.dashboard-empty {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: var(--space-4);
  padding: var(--space-8) var(--space-6);
  background: var(--bg-elevated);
  border-radius: var(--radius-xl);
  color: var(--text-secondary);
  font-size: var(--text-base);
}
@media (max-width: 1050px) {
  .content-grid {
    grid-template-columns: 1fr;
  }
}
@media (max-width: 800px) {
  .dashboard-content {
    padding: var(--space-3);
  }
}
</style>