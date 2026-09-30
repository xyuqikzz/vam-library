<template>
  <div class="settings-view animate-fadeIn">
    <SceneBrowserModPanel />
    <!-- General -->
    <GlassPanel :title="$t('settings.general')">
      <div class="setting-row">
        <div class="setting-info">
          <h4 class="setting-label">{{ $t('settings.vamDirectory') }}</h4>
          <p class="setting-description">{{ $t('settings.vamDirectoryDesc') }}</p>
        </div>
        <div class="setting-control path-control">
          <div class="path-display">
            <svg width="14" height="14" viewBox="0 0 24 24" fill="none">
              <path d="M3 7V5a2 2 0 0 1 2-2h4l2 2h8a2 2 0 0 1 2 2v2" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" />
              <rect x="3" y="7" width="18" height="14" rx="2" stroke="currentColor" stroke-width="1.5" />
            </svg>
            <span class="path-text truncate">{{ vamRootPath || $t('settings.notConfigured') }}</span>
          </div>
          <button class="browse-btn" @click="browseDirectory">{{ $t('settings.browse') }}</button>
        </div>
      </div>

      <div class="setting-divider" />

      <div class="setting-row">
        <div class="setting-info">
          <h4 class="setting-label">{{ $t('settings.autoScan') }}</h4>
          <p class="setting-description">{{ $t('settings.autoScanDesc') }}</p>
        </div>
        <div class="setting-control">
          <button :class="['toggle-switch', { on: appStore.autoScan }]" @click="toggleAutoScan">
            <span class="toggle-thumb" />
          </button>
        </div>
      </div>
    </GlassPanel>

    <GlassPanel :title="$t('settings.vamInstances')">
      <div class="instance-list">
        <p v-if="vamInstances.length === 0" class="instance-empty">
          {{ $t('settings.noInstances') }}
        </p>
        <div
          v-for="instance in vamInstances"
          :key="instance.id"
          :class="['instance-row', { active: activeInstanceId === instance.id }]"
        >
          <button class="instance-main" type="button" @click="switchInstance(instance.id)">
            <span class="instance-name">{{ instance.name }}</span>
            <span class="instance-path truncate">{{ instance.rootPath }}</span>
          </button>
          <button
            class="instance-delete"
            type="button"
            :title="$t('settings.deleteInstance')"
            @click="deleteInstance(instance.id)"
          >
            <svg width="14" height="14" viewBox="0 0 24 24" fill="none" aria-hidden="true">
              <path d="M3 6h18" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" />
              <path d="M8 6V4h8v2" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" />
              <path d="M6 6l1 14h10l1-14" stroke="currentColor" stroke-width="1.5" stroke-linejoin="round" />
              <path d="M10 10v6M14 10v6" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" />
            </svg>
          </button>
        </div>
      </div>

      <div class="setting-divider" />

      <div class="instance-add-row">
        <input v-model="newInstanceName" class="setting-input instance-name-input" :placeholder="$t('settings.instanceNamePlaceholder')" />
        <button class="browse-btn" :disabled="!canAddCurrentInstance" @click="addCurrentInstance">
          {{ $t('settings.addCurrentInstance') }}
        </button>
      </div>
      <p v-if="isCurrentPathDuplicated" class="instance-tip">
        {{ $t('settings.duplicateInstancePath') }}
      </p>
    </GlassPanel>

    <!-- Resource Management -->
    <GlassPanel :title="$t('settings.resourceManagement')">
      <div class="setting-row">
        <div class="setting-info">
          <h4 class="setting-label">{{ $t('settings.managedMode') }}</h4>
          <p class="setting-description">{{ $t('settings.managedModeDesc') }}</p>
        </div>
        <div class="setting-control">
          <button :class="['toggle-switch', { on: managedEnabled }]" @click="toggleManagedMode">
            <span class="toggle-thumb" />
          </button>
        </div>
      </div>

      <div class="setting-divider" />

      <div class="setting-row">
        <div class="setting-info">
          <h4 class="setting-label">{{ $t('settings.managedLibraryPath') }}</h4>
          <p class="setting-description">{{ installContext?.managedLibraryDir || $t('settings.notConfigured') }}</p>
        </div>
      </div>
    </GlassPanel>

    <GlassPanel :title="$t('settings.previewSettings')">
      <div class="setting-row">
        <div class="setting-info">
          <h4 id="blur-previews-label" class="setting-label">{{ $t('settings.blurPreviews') }}</h4>
          <p id="blur-previews-description" class="setting-description">{{ $t('settings.blurPreviewsDesc') }}</p>
        </div>
        <div class="setting-control">
          <button
            type="button"
            role="switch"
            aria-labelledby="blur-previews-label"
            aria-describedby="blur-previews-description"
            :aria-checked="appStore.blurPreviews"
            :class="['toggle-switch', { on: appStore.blurPreviews }]"
            :disabled="isSavingPreviewSettings"
            @click="togglePreviewBlur"
          >
            <span class="toggle-thumb" />
          </button>
        </div>
      </div>
    </GlassPanel>

    <!-- Language Settings -->
    <GlassPanel :title="$t('settings.language')">
      <div class="setting-row">
        <div class="setting-info">
          <h4 class="setting-label">{{ $t('settings.language') }}</h4>
          <p class="setting-description">{{ $t('settings.languageDesc') }}</p>
        </div>
        <div class="setting-control">
          <div class="theme-selector">
            <button
              :class="['theme-btn', { active: locale === 'zh-CN' }]"
              @click="setLocale('zh-CN')"
            >
              中文
            </button>
            <button
              :class="['theme-btn', { active: locale === 'en-US' }]"
              @click="setLocale('en-US')"
            >
              English
            </button>
          </div>
        </div>
      </div>
    </GlassPanel>

    <!-- Download Settings -->
    <GlassPanel :title="$t('settings.downloadSettings')">
      <div class="setting-row">
        <div class="setting-info">
          <h4 class="setting-label">{{ $t('settings.maxConcurrentDownloads') }}</h4>
          <p class="setting-description">{{ $t('settings.maxConcurrentDownloadsDesc') }}</p>
        </div>
        <div class="setting-control">
          <div class="stepper-control">
            <button
              class="stepper-btn"
              type="button"
              :disabled="maxConcurrentDownloads <= 1"
              @click="changeMaxConcurrentDownloads(-1)"
            >
              -
            </button>
            <input
              type="number"
              class="setting-input stepper-input"
              v-model.number="maxConcurrentDownloads"
              min="1"
              max="5"
              @blur="saveDownloadSettings"
              @change="saveDownloadSettings"
            />
            <button
              class="stepper-btn"
              type="button"
              :disabled="maxConcurrentDownloads >= 5"
              @click="changeMaxConcurrentDownloads(1)"
            >
              +
            </button>
          </div>
        </div>
      </div>

      <div class="setting-divider" />

      <div class="setting-row">
        <div class="setting-info">
          <h4 class="setting-label">{{ $t('settings.speedLimit') }}</h4>
          <p class="setting-description">{{ $t('settings.speedLimitDesc') }}</p>
        </div>
        <div class="setting-control speed-limit-control">
          <input
            type="number"
            class="setting-input"
            v-model.number="speedLimitKb"
            min="0"
            step="100"
            @change="saveDownloadSettings"
          />
          <span class="unit-label">{{ $t('settings.speedLimitUnit') }}</span>
        </div>
      </div>

      <div class="setting-divider" />

      <div class="setting-row">
        <div class="setting-info">
          <h4 class="setting-label">{{ $t('settings.downloadTargetPolicy') }}</h4>
          <p class="setting-description">{{ $t('settings.downloadTargetPolicyDesc') }}</p>
        </div>
        <div class="setting-control">
          <select v-model="downloadTargetPolicy" class="setting-select" @change="saveDownloadSettings">
            <option value="auto">{{ $t('settings.policyAuto') }}</option>
            <option value="real_addon">{{ $t('settings.policyRealAddon') }}</option>
            <option value="managed_library">{{ $t('settings.policyManagedLibrary') }}</option>
          </select>
        </div>
      </div>

      <div class="setting-divider" />

      <div class="setting-row">
        <div class="setting-info">
          <h4 class="setting-label">{{ $t('settings.downloadAfterAction') }}</h4>
          <p class="setting-description">{{ $t('settings.downloadAfterActionDesc') }}</p>
        </div>
        <div class="setting-control">
          <select v-model="downloadAfterAction" class="setting-select" @change="saveDownloadSettings">
            <option value="library_only">{{ $t('settings.afterLibraryOnly') }}</option>
            <option value="add_to_active_plan">{{ $t('settings.afterAddToActivePlan') }}</option>
            <option value="add_and_apply">{{ $t('settings.afterAddAndApply') }}</option>
          </select>
        </div>
      </div>

      <div class="setting-divider" />

      <div class="setting-row">
        <div class="setting-info">
          <h4 class="setting-label">{{ $t('settings.currentDownloadTarget') }}</h4>
          <p class="setting-description">{{ installContext?.downloadTargetDir || $t('settings.notConfigured') }}</p>
        </div>
      </div>
    </GlassPanel>

    <GlassPanel :title="$t('settings.hubLogin')">
      <div class="setting-row">
        <div class="setting-info">
          <h4 class="setting-label">{{ $t('settings.hubCookie') }}</h4>
          <p class="setting-description">
            {{ hubLoggedIn ? $t('settings.hubLoggedIn') : $t('settings.hubNotLoggedIn') }}
          </p>
        </div>
      </div>
      <textarea
        v-model="hubCookieDraft"
        class="cookie-input"
        :placeholder="$t('settings.hubCookiePlaceholder')"
      />
      <div class="hub-actions">
        <button class="browse-btn" @click="saveHubCookie">{{ $t('settings.saveHubCookie') }}</button>
        <button class="clear-cookie-btn" @click="clearHubCookie">{{ $t('settings.clearHubCookie') }}</button>
      </div>
    </GlassPanel>

    <!-- Cache Management -->
    <GlassPanel :title="$t('settings.cacheCleanup')">
      <div class="setting-row">
        <div class="setting-info">
          <h4 class="setting-label">{{ $t('settings.clearThumbnails') }}</h4>
          <p class="setting-description">{{ $t('settings.clearThumbnailsDesc') }}</p>
        </div>
        <div class="setting-control">
          <button class="clear-cookie-btn" :disabled="isClearingThumbnails" @click="handleClearThumbnails">
            {{ isClearingThumbnails ? $t('common.loading') : $t('settings.clearBtn') }}
          </button>
        </div>
      </div>

      <div class="setting-divider" />

      <div class="setting-row">
        <div class="setting-info">
          <h4 class="setting-label">{{ $t('settings.clearLocalData') }}</h4>
          <p class="setting-description">{{ $t('settings.clearLocalDataDesc') }}</p>
        </div>
        <div class="setting-control">
          <button class="danger-btn" :disabled="isClearingLocalData" @click="confirmClearLocalData = true">
            {{ isClearingLocalData ? $t('common.loading') : $t('settings.clearBtn') }}
          </button>
        </div>
      </div>
    </GlassPanel>
    
    <!-- Backup & Export -->
    <GlassPanel :title="$t('settings.exportPackageList')">
      <div class="setting-row">
        <div class="setting-info">
          <h4 class="setting-label">{{ $t('settings.exportPackageList') }}</h4>
          <p class="setting-description">{{ $t('settings.exportPackageListDesc') }}</p>
        </div>
        <div class="setting-control">
          <button class="browse-btn" @click="handleExportPackages">
            {{ $t('settings.exportPackageList') }}
          </button>
        </div>
      </div>
    </GlassPanel>

    <!-- About -->
    <GlassPanel :title="$t('settings.about')">
      <div class="about-content">
        <div class="about-row">
          <span class="about-label">{{ $t('settings.version') }}</span>
          <span class="about-value">0.1.1</span>
        </div>
        <div class="about-row">
          <span class="about-label">{{ $t('settings.framework') }}</span>
          <span class="about-value">Tauri 2 + Vue 3</span>
        </div>
        <div class="about-row">
          <span class="about-label">{{ $t('settings.repository') }}</span>
          <button class="about-link" type="button" @click="openRepository">
            github.com/xyuqikzz/vam-library
          </button>
        </div>
      </div>
    </GlassPanel>

    <div v-if="confirmClearLocalData" class="modal-overlay" @click.self="closeClearLocalDataDialog">
      <div class="confirm-dialog">
        <div class="confirm-dialog-header">
          <h3>{{ $t('settings.clearLocalDataConfirmTitle') }}</h3>
        </div>
        <div class="confirm-dialog-body">
          <p>{{ $t('settings.clearLocalDataConfirmDesc') }}</p>
        </div>
        <div class="confirm-dialog-actions">
          <button class="browse-btn" :disabled="isClearingLocalData" @click="closeClearLocalDataDialog">
            {{ $t('common.cancel') }}
          </button>
          <button class="danger-btn" :disabled="isClearingLocalData" @click="handleClearLocalData">
            {{ isClearingLocalData ? $t('common.loading') : $t('common.confirm') }}
          </button>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { useI18n } from 'vue-i18n'
import { computed, ref, watch } from 'vue'
import { storeToRefs } from 'pinia'
import { useAppStore } from '@/stores/app'
import { useLocalLibraryStore } from '@/stores/localLibrary'
import { useNotification } from '@/composables/useNotification'
import { invoke } from '@tauri-apps/api/core'
import GlassPanel from '@/components/common/GlassPanel.vue'
import SceneBrowserModPanel from '@/components/SceneBrowserModPanel.vue'

const { t } = useI18n()
const appStore = useAppStore()
const localLibraryStore = useLocalLibraryStore()
const {
  vamRootPath,
  locale,
  managedEnabled,
  downloadTargetPolicy,
  downloadAfterAction,
  installContext,
  maxConcurrentDownloads,
  speedLimitKb,
  vamInstances,
  activeInstanceId,
  hubAuthCookie,
  hubLoggedIn,
} = storeToRefs(appStore)

const notify = useNotification()
const isSavingPreviewSettings = ref(false)
async function togglePreviewBlur() {
  if (isSavingPreviewSettings.value) return
  isSavingPreviewSettings.value = true
  try {
    await appStore.setBlurPreviews(!appStore.blurPreviews)
  } catch (error) {
    notify.error(t('settings.previewSaveFailed', { error: String(error) }))
  } finally {
    isSavingPreviewSettings.value = false
  }
}

const isClearingThumbnails = ref(false)
const isClearingLocalData = ref(false)
const confirmClearLocalData = ref(false)
const repositoryUrl = 'https://github.com/xyuqikzz/vam-library'

async function handleClearThumbnails() {
  if (isClearingThumbnails.value) return
  isClearingThumbnails.value = true
  try {
    await appStore.clearThumbnailCache()
    notify.success(t('settings.clearThumbnailsSuccess'))
  } catch (e) {
    notify.error(t('settings.clearThumbnailsFailed') + ': ' + String(e))
  } finally {
    isClearingThumbnails.value = false
  }
}

function closeClearLocalDataDialog() {
  if (isClearingLocalData.value) return
  confirmClearLocalData.value = false
}

async function handleClearLocalData() {
  if (isClearingLocalData.value) return
  isClearingLocalData.value = true
  try {
    await invoke('clear_local_database')
    localLibraryStore.resetState('loading')
    await localLibraryStore.refreshAll('loading')
    notify.success(t('settings.clearLocalDataSuccess'))
    confirmClearLocalData.value = false
  } catch (e) {
    notify.error(t('settings.clearLocalDataFailed') + ': ' + String(e))
  } finally {
    isClearingLocalData.value = false
  }
}

const newInstanceName = ref('')
const hubCookieDraft = ref(hubAuthCookie.value || '')

function normalizeInstancePath(path: string) {
  return path.trim().replace(/[\\/]+/g, '\\').replace(/\\+$/g, '').toLowerCase()
}

const currentInstancePathKey = computed(() => {
  if (!vamRootPath.value) return ''
  return normalizeInstancePath(vamRootPath.value)
})

const isCurrentPathDuplicated = computed(() => {
  if (!currentInstancePathKey.value) return false
  return vamInstances.value.some((instance) => {
    if (activeInstanceId.value && instance.id === activeInstanceId.value) return false
    return normalizeInstancePath(instance.rootPath) === currentInstancePathKey.value
  })
})

const canAddCurrentInstance = computed(() => Boolean(vamRootPath.value) && !isCurrentPathDuplicated.value)

function clampMaxConcurrentDownloads(value: number) {
  if (!Number.isFinite(value)) return 1
  return Math.min(5, Math.max(1, Math.trunc(value)))
}

watch(hubAuthCookie, (value) => {
  hubCookieDraft.value = value || ''
})

async function saveDownloadSettings() {
  maxConcurrentDownloads.value = clampMaxConcurrentDownloads(maxConcurrentDownloads.value)
  if (speedLimitKb.value === undefined || speedLimitKb.value === null || speedLimitKb.value < 0) {
    speedLimitKb.value = 0
  }
  await appStore.saveSettings()
  await appStore.refreshInstallContext()
}

async function changeMaxConcurrentDownloads(delta: number) {
  maxConcurrentDownloads.value = clampMaxConcurrentDownloads(maxConcurrentDownloads.value + delta)
  await saveDownloadSettings()
}

function toggleManagedMode() {
  appStore.setManagedSettings(!managedEnabled.value, appStore.managedLibraryPath)
}

function toggleAutoScan() {
  appStore.setAutoScan(!appStore.autoScan)
}

async function addCurrentInstance() {
  if (!canAddCurrentInstance.value || !vamRootPath.value) return
  const name = newInstanceName.value.trim() || `VAM 实例 ${vamInstances.value.length + 1}`
  const id = `instance_${Date.now()}`
  await appStore.saveInstances(
    [...vamInstances.value, {
      id,
      name,
      rootPath: vamRootPath.value,
      managedEnabled: managedEnabled.value,
      managedLibraryPath: appStore.managedLibraryPath,
      downloadTargetPolicy: downloadTargetPolicy.value,
      downloadAfterAction: downloadAfterAction.value,
    }],
    id,
  )
  newInstanceName.value = ''
}

async function deleteInstance(id: string) {
  const nextInstances = vamInstances.value.filter((instance) => instance.id !== id)
  const nextActiveId = activeInstanceId.value === id ? nextInstances[0]?.id ?? null : activeInstanceId.value
  await appStore.saveInstances(nextInstances, nextActiveId)
}

async function switchInstance(id: string) {
  await appStore.saveInstances(vamInstances.value, id)
}

async function saveHubCookie() {
  await appStore.saveHubCookie(hubCookieDraft.value)
}

async function clearHubCookie() {
  hubCookieDraft.value = ''
  await appStore.saveHubCookie(null)
}

async function openRepository() {
  try {
    const { openUrl } = await import('@tauri-apps/plugin-opener')
    await openUrl(repositoryUrl)
  } catch (e) {
    notify.error(String(e))
  }
}

function setLocale(localeKey: string) {
  appStore.setLocale(localeKey)
}

async function browseDirectory() {
  try {
    const { open } = await import('@tauri-apps/plugin-dialog')
    const selected = await open({
      directory: true,
      title: t('settings.selectVamDirectory'),
    })
    if (selected && typeof selected === 'string') {
      appStore.setVamRoot(selected)
    }
  } catch {
    // Dialog cancelled or not available
  }
}

async function handleExportPackages() {
  try {
    const { save } = await import('@tauri-apps/plugin-dialog')
    const selectedPath = await save({
      title: t('settings.exportPackageList'),
      filters: [{
        name: 'Text File',
        extensions: ['txt']
      }],
      defaultPath: 'vam_packages_list.txt'
    })

    if (!selectedPath) return // User cancelled

    await invoke('export_installed_packages', { targetPath: selectedPath })
    notify.success(t('settings.exportSuccess'))
  } catch (e) {
    notify.error(t('settings.exportFailed') + ': ' + String(e))
  }
}
</script>

<style scoped>
.settings-view {
  display: flex;
  flex-direction: column;
  gap: var(--space-5);
  max-width: 720px;
}

.setting-row {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: var(--space-6);
}

.setting-info {
  flex: 1;
  min-width: 0;
}

.setting-label {
  font-size: var(--text-md);
  font-weight: var(--font-medium);
  color: var(--text-primary);
  margin-bottom: var(--space-1);
}

.setting-description {
  font-size: var(--text-sm);
  color: var(--text-secondary);
}

.setting-control {
  flex-shrink: 0;
}

.setting-divider {
  height: 1px;
  background: var(--border-subtle);
  margin: var(--space-4) 0;
}

.path-control {
  display: flex;
  align-items: center;
  gap: var(--space-2);
}

.path-display {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  padding: var(--space-2) var(--space-3);
  background: var(--bg-input);
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-sm);
  color: var(--text-secondary);
  font-size: var(--text-sm);
  max-width: 220px;
}

.path-text {
  max-width: 160px;
}

.browse-btn {
  padding: var(--space-2) var(--space-4);
  height: 34px;
  background: var(--bg-elevated);
  color: var(--text-primary);
  font-size: var(--text-sm);
  font-weight: var(--font-medium);
  border-radius: var(--radius-sm);
  border: 1px solid var(--border-subtle);
  transition:
    background var(--duration-fast) var(--ease),
    border-color var(--duration-fast) var(--ease);
}

.browse-btn:hover {
  background: var(--bg-hover);
  border-color: var(--border-default);
}

.toggle-switch {
  position: relative;
  width: 44px;
  height: 24px;
  border-radius: var(--radius-full);
  background: var(--bg-hover);
  border: 1px solid var(--border-subtle);
  cursor: pointer;
  transition:
    background var(--duration-base) var(--ease),
    border-color var(--duration-base) var(--ease);
}

.toggle-switch.on {
  background: var(--accent-primary);
  border-color: var(--accent-primary);
}

.toggle-thumb {
  position: absolute;
  top: 2px;
  left: 2px;
  width: 18px;
  height: 18px;
  border-radius: 50%;
  background: white;
  box-shadow: 0 1px 3px rgba(0, 0, 0, 0.3);
  transition: transform var(--duration-base) var(--ease);
}

.toggle-switch.on .toggle-thumb {
  transform: translateX(20px);
}

.theme-selector {
  display: flex;
  background: var(--bg-base);
  border-radius: var(--radius-sm);
  padding: 2px;
  gap: 2px;
}

.theme-btn {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  padding: var(--space-1) var(--space-3);
  height: 30px;
  border-radius: var(--radius-xs);
  color: var(--text-tertiary);
  font-size: var(--text-sm);
  font-weight: var(--font-medium);
  transition:
    color var(--duration-fast) var(--ease),
    background var(--duration-fast) var(--ease);
}

.theme-btn:hover {
  color: var(--text-secondary);
}

.theme-btn.active {
  color: var(--text-primary);
  background: var(--bg-elevated);
}

.about-content {
  display: flex;
  flex-direction: column;
  gap: var(--space-3);
}

.about-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
}

.about-label {
  font-size: var(--text-sm);
  color: var(--text-secondary);
}

.about-value {
  font-size: var(--text-sm);
  color: var(--text-primary);
  font-weight: var(--font-medium);
}

.about-link {
  font-size: var(--text-sm);
  color: var(--accent-secondary);
  font-weight: var(--font-medium);
  cursor: pointer;
}

.about-link:hover {
  color: var(--accent-secondary-hover);
}

.setting-input {
  width: 100px;
  height: 30px;
  padding: 0 var(--space-2);
  background: var(--bg-input);
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-sm);
  color: var(--text-primary);
  font-size: var(--text-sm);
  text-align: right;
  transition: border-color var(--transition-fast) var(--ease);
}

.stepper-control {
  display: grid;
  grid-template-columns: 30px 44px 30px;
  align-items: center;
  overflow: hidden;
  height: 32px;
  background: var(--bg-input);
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-sm);
}

.stepper-btn {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 30px;
  height: 30px;
  color: var(--text-secondary);
  font-size: var(--text-lg);
  line-height: 1;
  transition:
    background var(--duration-fast) var(--ease),
    color var(--duration-fast) var(--ease);
}

.stepper-btn:hover:not(:disabled) {
  background: var(--bg-hover);
  color: var(--text-primary);
}

.stepper-btn:disabled {
  color: var(--text-tertiary);
  cursor: not-allowed;
  opacity: 0.5;
}

.stepper-input {
  width: 44px;
  height: 30px;
  padding: 0;
  border-width: 0 1px;
  border-color: var(--border-subtle);
  border-radius: 0;
  background: transparent;
  text-align: center;
  appearance: textfield;
  -moz-appearance: textfield;
}

.stepper-input::-webkit-outer-spin-button,
.stepper-input::-webkit-inner-spin-button {
  margin: 0;
  appearance: none;
  -webkit-appearance: none;
}

.instance-list {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
}

.instance-row {
  display: flex;
  align-items: stretch;
  gap: var(--space-2);
  padding: var(--space-2);
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-sm);
  background: rgba(255, 255, 255, 0.02);
}

.instance-main {
  display: flex;
  flex: 1;
  flex-direction: column;
  gap: 2px;
  min-width: 0;
  padding: var(--space-1) var(--space-2);
  text-align: left;
}

.instance-row.active {
  border-color: var(--accent-primary);
  background: rgba(110, 107, 240, 0.12);
}

.instance-name {
  color: var(--text-primary);
  font-size: var(--text-sm);
  font-weight: var(--font-semibold);
}

.instance-path {
  color: var(--text-tertiary);
  font-size: var(--text-xs);
}

.instance-delete {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 32px;
  height: 32px;
  flex-shrink: 0;
  border: 1px solid transparent;
  border-radius: var(--radius-sm);
  color: var(--text-tertiary);
  transition:
    color var(--duration-fast) var(--ease),
    background var(--duration-fast) var(--ease),
    border-color var(--duration-fast) var(--ease);
}

.instance-delete:hover {
  color: var(--color-error);
  background: rgba(248, 113, 113, 0.08);
  border-color: rgba(248, 113, 113, 0.2);
}

.instance-add-row {
  display: flex;
  gap: var(--space-2);
}

.instance-name-input {
  width: 180px;
  text-align: left;
}

.cookie-input {
  width: 100%;
  min-height: 82px;
  padding: var(--space-3);
  resize: vertical;
  background: var(--bg-input);
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-sm);
  color: var(--text-primary);
  font-size: var(--text-xs);
}

.hub-actions {
  display: flex;
  gap: var(--space-2);
  margin-top: var(--space-3);
}

.clear-cookie-btn {
  padding: var(--space-2) var(--space-4);
  height: 34px;
  border: 1px solid rgba(248, 113, 113, 0.2);
  border-radius: var(--radius-sm);
  background: rgba(248, 113, 113, 0.08);
  color: var(--color-error);
  font-size: var(--text-sm);
  font-weight: var(--font-medium);
}

.danger-btn {
  padding: var(--space-2) var(--space-4);
  height: 34px;
  border: 1px solid rgba(248, 113, 113, 0.2);
  border-radius: var(--radius-sm);
  background: rgba(248, 113, 113, 0.1);
  color: var(--color-error);
  font-size: var(--text-sm);
  font-weight: var(--font-medium);
}

.setting-select {
  min-width: 180px;
  height: 30px;
  padding: 0 var(--space-2);
  background: var(--bg-input);
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-sm);
  color: var(--text-primary);
  font-size: var(--text-sm);
}

.setting-input:focus {
  border-color: var(--accent-primary);
  outline: none;
}

.speed-limit-control {
  display: flex;
  align-items: center;
  gap: var(--space-2);
}

.unit-label {
  font-size: var(--text-sm);
  color: var(--text-secondary);
}

.instance-empty,
.instance-tip {
  margin-top: var(--space-2);
  font-size: var(--text-xs);
  color: var(--text-tertiary);
}

.browse-btn:disabled {
  opacity: 0.45;
  cursor: not-allowed;
}

.danger-btn:disabled {
  opacity: 0.45;
  cursor: not-allowed;
}

.modal-overlay {
  position: fixed;
  inset: 0;
  z-index: 720;
  display: flex;
  align-items: center;
  justify-content: center;
  padding: var(--space-6);
  background: rgba(22, 22, 24, 0.56);
  /* backdrop-filter removed */

}

.confirm-dialog {
  width: min(460px, 100%);
  border: 1px solid rgba(248, 113, 113, 0.18);
  border-radius: var(--radius-lg);
  background: rgba(20, 21, 35, 0.96);
  box-shadow: 0 24px 64px rgba(0, 0, 0, 0.35);
}

.confirm-dialog-header,
.confirm-dialog-body,
.confirm-dialog-actions {
  padding: var(--space-5);
}

.confirm-dialog-header {
  border-bottom: 1px solid var(--border-subtle);
}

.confirm-dialog-header h3 {
  color: var(--text-primary);
  font-size: var(--text-lg);
  font-weight: var(--font-semibold);
}

.confirm-dialog-body {
  color: var(--text-secondary);
  font-size: var(--text-sm);
  line-height: 1.7;
}

.confirm-dialog-actions {
  display: flex;
  justify-content: flex-end;
  gap: var(--space-3);
  border-top: 1px solid var(--border-subtle);
}
</style>
