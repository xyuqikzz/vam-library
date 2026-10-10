<script setup lang="ts">
import { useRouter } from 'vue-router'
import PageHeader from '@/components/layout/PageHeader.vue'
import UiButton from '@/components/ui/UiButton.vue'
import UiProgress from '@/components/ui/UiProgress.vue'
import PackageCard from '@/components/packages/PackageCard.vue'

import { computed, onMounted, ref, watch } from 'vue'
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

const thumbnails = ref<Record<string, string>>({})

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
</script><template><div class="dashboard-page"><PageHeader :title="t('sidebar.dashboard')" :subtitle="vamRootPath || t('gameContent.noGame')"><template #actions><UiButton icon="upload" @click="router.push('/ingestion')">{{ t('sidebar.ingestion') }}</UiButton><UiButton variant="primary" size="lg" icon="refresh" :loading="isScanning" :loading-text="t('toolbar.scanning')" :disabled="isScanning || !vamRootPath" @click="scanDashboard">{{ t('ui.startScan') }}</UiButton></template></PageHeader><div class="dashboard-content"><ResourceDisplay :packages="recentPreviewPackages" view-mode="small-card" :result-count-label="t('dashboard.recentPackages')" :empty-title="t('dashboard.noRecentPackages')" :empty-description="t('dashboard.noRecentPackagesDesc')" show-quick-delete @package-deleted="localLibraryStore.refreshAll('refreshing')"><template #browser="{ selectPackage, openDetails, launchPackage }"><div class="dashboard resource-content-scrollable"><div v-if="isScanning" class="scan-banner"><UiProgress circular :value="scanProgress?.processed_files || 0" :max="scanProgress?.total_files || 1" :label="t('toolbar.scanning')" /><div class="scan-text"><strong>{{ t('toolbar.scanning') }}</strong><span :title="scanProgress?.current_file">{{ scanProgress?.current_file }}</span><UiProgress :value="scanProgress?.processed_files || 0" :max="scanProgress?.total_files || 1" :indeterminate="!scanProgress?.total_files" :label="t('toolbar.scanning')" /></div><span class="scan-stats">{{ scanProgress?.processed_files || 0 }} / {{ scanProgress?.total_files || 0 }}</span></div><StatsBar :stats="stats" :loading="loading" /><div class="content-grid"><div class="dashboard-left"><section class="dashboard-section"><h2>{{ t('dashboard.recentPackages') }}</h2><div v-if="loading" class="recent-grid"><PackageCard v-for="i in 4" :key="i" :item="{ id: String(i), name: '' }" loading /></div><div v-else-if="recentPreviewPackages.length" class="recent-grid"><PackageCard v-for="pkg in recentPreviewPackages" :key="pkg.id" :item="pkg" :image="thumbnails[pkg.id]" :fresh="Date.now() - new Date(pkg.scan_time).getTime() < 86400000" :can-launch="pkg.resource_types.includes('scene')" @select="selectPackage(pkg)" @details="openDetails(pkg)" @launch="launchPackage(pkg)" @more="openDetails(pkg)" /></div><div v-else class="dashboard-empty"><p>{{ t('dashboard.noRecentPackagesDesc') }}</p><UiButton variant="primary" :disabled="isScanning || !vamRootPath" @click="scanDashboard">{{ t('ui.startScan') }}</UiButton></div></section><section class="dashboard-section"><h2>{{ t('dashboard.quickOpen') }}</h2><QuickActions :vam-root="vamRootPath" :screenshot-path="screenshotPath" :appearance-preset-path="appearancePresetPath" :clothing-preset-path="clothingPresetPath" @open="openPath" @game-config="openGameConfig" /></section></div><section class="health-panel"><header><h2>{{ t('dashboard.healthReport') }}</h2><span>{{ t('ui.issueCount', { count: totalIssues }) }}</span></header><HealthStatus :missing="stats.missing_dependencies" :corrupted="stats.corrupted_packages" :duplicates="stats.duplicate_resources" :missing-list="missingDependenciesList" :corrupted-list="corruptedPackagesList" /></section></div></div></template></ResourceDisplay></div></div></template><style scoped>
.dashboard-page { display: flex; flex-direction: column; height: 100%; min-height: 0; }.dashboard-content { flex: 1; min-height: 0; padding: 18px 24px; }.dashboard { display: flex; flex-direction: column; flex: 1; min-height: 0; min-width: 0; overflow-y: auto; gap: 20px; padding: 4px; }.scan-banner { display: flex; align-items: center; gap: 14px; padding: 16px; border-radius: var(--radius-lg); background: var(--bg-elevated); }.scan-text { display: flex; flex-direction: column; gap: 6px; flex: 1; min-width: 0; }.scan-text strong { font-size: var(--text-md); font-weight: var(--font-heavy); }.scan-text > span { white-space: nowrap; overflow: hidden; text-overflow: ellipsis; font-size: var(--text-sm); color: var(--text-tertiary); }.scan-stats { font-size: var(--text-base); color: var(--text-secondary); }.content-grid { display: grid; grid-template-columns: minmax(0, 1fr) minmax(280px, .65fr); gap: 20px; align-items: start; }.dashboard-left { display: flex; flex-direction: column; gap: 24px; min-width: 0; }.dashboard-section { min-width: 0; }.dashboard-section h2,.health-panel h2 { font-size: var(--text-lg); font-weight: var(--font-heavy); margin-bottom: 14px; }.recent-grid { display: grid; grid-template-columns: repeat(auto-fill, minmax(124px, 1fr)); gap: 14px; }.health-panel { padding: 18px; border-radius: var(--radius-lg); background: var(--bg-elevated); }.health-panel header { display: flex; align-items: center; justify-content: space-between; gap: 8px; margin-bottom: 16px; }.health-panel h2 { margin: 0; }.health-panel header > span { font-size: var(--text-xs); color: var(--color-warning); background: var(--color-warning-bg); border-radius: var(--radius-full); padding: 4px 8px; }.dashboard-empty { display: flex; flex-direction: column; align-items: center; gap: 16px; padding: 28px; background: var(--bg-elevated); border-radius: var(--radius-xl); color: var(--text-secondary); font-size: var(--text-base); }@media(max-width: 1050px) { .content-grid { grid-template-columns: 1fr; } }@media(max-width: 800px) { .dashboard-content { padding: 12px; } }
</style>