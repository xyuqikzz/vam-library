<template>
<div class="settings-view animate-fadeIn b2-page"><PageHeader :title="$t('sidebar.settings')" ></PageHeader><div class="b2-page-body">
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
            <Icon  :size="14" name="outline-45" />
            <span class="path-text truncate">{{ vamRootPath || $t('settings.notConfigured') }}</span>
          </div>
          <UiButton @click="browseDirectory" variant="secondary">{{ $t('settings.browse') }}</UiButton>
        </div>
      </div>

      <div class="setting-divider" />

      <div class="setting-row">
        <div class="setting-info">
          <h4 class="setting-label">{{ $t('settings.autoScan') }}</h4>
          <p class="setting-description">{{ $t('settings.autoScanDesc') }}</p>
        </div>
        <div class="setting-control">
          <UiSwitch  :model-value="appStore.autoScan" :label="$t('settings.autoScan')" @update:model-value="toggleAutoScan" />
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
          <UiButton class="instance-main" type="button" @click="switchInstance(instance.id)" variant="secondary">
            <span class="instance-name">{{ instance.name }}</span>
            <span class="instance-path truncate">{{ instance.rootPath }}</span>
          </UiButton>
          <UiButton class="instance-delete" type="button" :title="$t('settings.deleteInstance')" @click="deleteInstance(instance.id)" variant="danger" icon-only :aria-label="$t('settings.deleteInstance')">
            <Icon  :size="14" name="outline-46" />
          </UiButton>
        </div>
      </div>

      <div class="setting-divider" />

      <div class="instance-add-row">
        <UiInput v-model="newInstanceName" class="setting-input instance-name-input" :placeholder="$t('settings.instanceNamePlaceholder')" />
        <UiButton :disabled="!canAddCurrentInstance" @click="addCurrentInstance" variant="secondary">
          {{ $t('settings.addCurrentInstance') }}
        </UiButton>
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
          <UiSwitch  :model-value="managedEnabled" :label="$t('settings.managedMode')" @update:model-value="toggleManagedMode" />
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
          <UiSwitch type="button" role="switch" aria-labelledby="blur-previews-label" aria-describedby="blur-previews-description" :aria-checked="appStore.blurPreviews" :disabled="isSavingPreviewSettings" :model-value="appStore.blurPreviews" :label="$t('settings.blurPreviews')" @update:model-value="togglePreviewBlur" />
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
            <UiChip :class="['', { active: locale === 'zh-CN' }]" @click="setLocale('zh-CN')" :selected="locale === 'zh-CN'">
              中文
            </UiChip>
            <UiChip :class="['', { active: locale === 'en-US' }]" @click="setLocale('en-US')" :selected="locale === 'en-US'">
              English
            </UiChip>
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
            <UiButton type="button" :disabled="maxConcurrentDownloads <= 1" @click="changeMaxConcurrentDownloads(-1)" variant="secondary">
              -
            </UiButton>
            <UiInput type="number" class="setting-input stepper-input" v-model.number="maxConcurrentDownloads" min="1" max="5" @blur="saveDownloadSettings" @change="saveDownloadSettings" />
            <UiButton type="button" :disabled="maxConcurrentDownloads >= 5" @click="changeMaxConcurrentDownloads(1)" variant="secondary">
              +
            </UiButton>
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
          <UiInput type="number" class="setting-input" v-model.number="speedLimitKb" min="0" step="100" @change="saveDownloadSettings" />
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
      <UiInput v-model="hubCookieDraft" class="cookie-input" :placeholder="$t('settings.hubCookiePlaceholder')" multiline />
      <div class="hub-actions">
        <UiButton @click="saveHubCookie" variant="secondary">{{ $t('settings.saveHubCookie') }}</UiButton>
        <UiButton @click="clearHubCookie" variant="secondary">{{ $t('settings.clearHubCookie') }}</UiButton>
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
          <UiButton :disabled="isClearingThumbnails" @click="handleClearThumbnails" variant="secondary">
            {{ isClearingThumbnails ? $t('common.loading') : $t('settings.clearBtn') }}
          </UiButton>
        </div>
      </div>

      <div class="setting-divider" />

      <div class="setting-row">
        <div class="setting-info">
          <h4 class="setting-label">{{ $t('settings.clearLocalData') }}</h4>
          <p class="setting-description">{{ $t('settings.clearLocalDataDesc') }}</p>
        </div>
        <div class="setting-control">
          <UiButton :disabled="isClearingLocalData" @click="confirmClearLocalData = true" variant="danger">
            {{ isClearingLocalData ? $t('common.loading') : $t('settings.clearBtn') }}
          </UiButton>
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
          <UiButton @click="handleExportPackages" variant="secondary">
            {{ $t('settings.exportPackageList') }}
          </UiButton>
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
          <UiButton class="about-link" type="button" @click="openRepository" variant="secondary">
            github.com/xyuqikzz/vam-library
          </UiButton>
        </div>
      </div>
    </GlassPanel>

    <UiModal v-if="confirmClearLocalData" :show-close="false"  :close-disabled="isClearingLocalData" @close="closeClearLocalDataDialog">
        <div class="confirm-dialog-header">
          <h3>{{ $t('settings.clearLocalDataConfirmTitle') }}</h3>
        </div>
        <div class="confirm-dialog-body">
          <p>{{ $t('settings.clearLocalDataConfirmDesc') }}</p>
        </div>
        <div class="confirm-dialog-actions">
          <UiButton :disabled="isClearingLocalData" @click="closeClearLocalDataDialog" variant="secondary">
            {{ $t('common.cancel') }}
          </UiButton>
          <UiButton :disabled="isClearingLocalData" @click="handleClearLocalData" variant="danger">
            {{ isClearingLocalData ? $t('common.loading') : $t('common.confirm') }}
          </UiButton>
        </div>
      </UiModal>
  </div></div>
</template>

<script setup lang="ts">
import UiChip from '@/components/ui/UiChip.vue'
import UiModal from '@/components/ui/UiModal.vue'
import PageHeader from '@/components/layout/PageHeader.vue'
import UiInput from '@/components/ui/UiInput.vue'
import UiSwitch from '@/components/ui/UiSwitch.vue'
import UiButton from '@/components/ui/UiButton.vue'
import Icon from '@/components/ui/Icon.vue'
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
  await saveWithFeedback(async () => {
    await appStore.saveSettings()
    await appStore.refreshInstallContext()
  })
}

async function saveWithFeedback(action: () => Promise<unknown>): Promise<boolean> {
  try {
    await action()
    return true
  } catch (error) {
    notify.error(String(error))
    await appStore.loadSettings()
    return false
  }
}

async function changeMaxConcurrentDownloads(delta: number) {
  maxConcurrentDownloads.value = clampMaxConcurrentDownloads(maxConcurrentDownloads.value + delta)
  await saveDownloadSettings()
}

function toggleManagedMode() {
  void saveWithFeedback(() => appStore.setManagedSettings(!managedEnabled.value, appStore.managedLibraryPath))
}

function toggleAutoScan() {
  void saveWithFeedback(() => appStore.setAutoScan(!appStore.autoScan))
}

async function addCurrentInstance() {
  const rootPath = vamRootPath.value
  if (!canAddCurrentInstance.value || !rootPath) return
  const name = newInstanceName.value.trim() || `VAM 实例 ${vamInstances.value.length + 1}`
  const id = `instance_${Date.now()}`
  const saved = await saveWithFeedback(() => appStore.saveInstances(
    [...vamInstances.value, {
      id,
      name,
      rootPath,
      managedEnabled: managedEnabled.value,
      managedLibraryPath: appStore.managedLibraryPath,
      downloadTargetPolicy: downloadTargetPolicy.value,
      downloadAfterAction: downloadAfterAction.value,
    }],
    id,
  ))
  if (saved) newInstanceName.value = ''
}

async function deleteInstance(id: string) {
  const nextInstances = vamInstances.value.filter((instance) => instance.id !== id)
  const nextActiveId = activeInstanceId.value === id ? nextInstances[0]?.id ?? null : activeInstanceId.value
  await saveWithFeedback(() => appStore.saveInstances(nextInstances, nextActiveId))
}

async function switchInstance(id: string) {
  await saveWithFeedback(() => appStore.saveInstances(vamInstances.value, id))
}

async function saveHubCookie() {
  await saveWithFeedback(() => appStore.saveHubCookie(hubCookieDraft.value))
}

async function clearHubCookie() {
  if (await saveWithFeedback(() => appStore.saveHubCookie(null))) hubCookieDraft.value = ''
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
      await saveWithFeedback(() => appStore.setVamRoot(selected))
    }
  } catch (error) {
    notify.error(String(error))
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
  border: none;
  border-radius: var(--radius-sm);
  color: var(--text-secondary);
  font-size: var(--text-sm);
  max-width: 220px;
}

.path-text {
  max-width: 160px;
}

.toggle-switch {
  position: relative;
  width: 44px;
  height: 24px;
  border-radius: var(--radius-full);
  background: var(--bg-hover);
  border: none;
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
  border-radius: var(--radius-xl)%;
  background: var(--text-primary);
  box-shadow: none;
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
  border: none;
  border-radius: var(--radius-sm);
}

.stepper-input {
  width: 44px;
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
  border: none;
  border-radius: var(--radius-sm);
  background: var(--bg-subtle);
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
  background: var(--accent-muted);
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
  border: none;
  border-radius: var(--radius-sm);
  color: var(--text-tertiary);
  transition:
    color var(--duration-fast) var(--ease),
    background var(--duration-fast) var(--ease),
    border-color var(--duration-fast) var(--ease);
}

.instance-delete:hover {
  color: var(--color-error);
  background: var(--color-error-bg);
  border-color: var(--color-error-bg);
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
  resize: vertical;
}

.hub-actions {
  display: flex;
  gap: var(--space-2);
  margin-top: var(--space-3);
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

.b2-page { display: flex; flex-direction: column; height: 100%; min-height: 0; max-width: none; margin: 0; padding: 0; gap: 0; overflow: hidden; }
.b2-page-body { flex: 1; min-height: 0; min-width: 0; overflow: auto; padding: 20px 24px; display: flex; flex-direction: column; gap: 20px; }
.b2-page :deep(.page-header) { width: 100%; }
@media(max-width: 800px) { .b2-page-body { padding: 12px; } }
</style>
