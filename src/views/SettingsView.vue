<template>
  <div class="settings-view animate-fadeIn b2-page">
    <PageHeader :title="$t('sidebar.settings')"></PageHeader>
    <div class="b2-page-body is-form">
      <GlassPanel :title="$t('settings.general')">
        <div class="setting-row">
          <div class="setting-info">
            <h4 class="setting-label">{{ $t('settings.vamDirectory') }}</h4>
            <p class="setting-description">{{ $t('settings.vamDirectoryDesc') }}</p>
          </div>
          <div class="setting-control path-control">
            <div class="path-display">
              <Icon :size="14" name="outline-45" />
              <span class="path-text truncate">
                {{ vamRootPath || $t('settings.notConfigured') }}
              </span>
            </div>
            <UiButton @click="browseDirectory" variant="secondary">
              {{ $t('settings.browse') }}
            </UiButton>
          </div>
        </div>

        <div class="setting-divider" />

        <div class="setting-row">
          <div class="setting-info">
            <h4 class="setting-label">{{ $t('settings.autoScan') }}</h4>
            <p class="setting-description">{{ $t('settings.autoScanDesc') }}</p>
          </div>
          <div class="setting-control">
            <UiSwitch
              :model-value="appStore.autoScan"
              :label="$t('settings.autoScan')"
              @update:model-value="toggleAutoScan"
            />
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
            <button
              class="instance-main"
              type="button"
              :aria-pressed="activeInstanceId === instance.id"
              @click="switchInstance(instance.id)"
            >
              <span class="instance-indicator" aria-hidden="true" />
              <span class="instance-text">
                <span class="instance-name">{{ instance.name }}</span>
                <span class="instance-path truncate">{{ instance.rootPath }}</span>
              </span>
            </button>
            <UiButton
              class="instance-delete"
              type="button"
              size="sm"
              :title="$t('settings.deleteInstance')"
              @click="deleteInstance(instance.id)"
              variant="ghost"
              icon-only
              :aria-label="$t('settings.deleteInstance')"
            >
              <Icon :size="14" name="outline-46" />
            </UiButton>
          </div>
        </div>

        <div class="setting-divider" />

        <div class="instance-add-row">
          <UiInput
            v-model="newInstanceName"
            class="setting-input instance-name-input"
            size="md"
            :placeholder="$t('settings.instanceNamePlaceholder')"
          />
          <UiButton
            :disabled="!canAddCurrentInstance"
            @click="addCurrentInstance"
            variant="secondary"
          >
            {{ $t('settings.addCurrentInstance') }}
          </UiButton>
        </div>
        <p v-if="isCurrentPathDuplicated" class="instance-tip">
          {{ $t('settings.duplicateInstancePath') }}
        </p>
      </GlassPanel>

      <SceneBrowserModPanel />

      <GlassPanel :title="$t('settings.downloadSettings')">
        <div class="setting-row">
          <div class="setting-info">
            <h4 class="setting-label">{{ $t('settings.maxConcurrentDownloads') }}</h4>
            <p class="setting-description">{{ $t('settings.maxConcurrentDownloadsDesc') }}</p>
          </div>
          <div class="setting-control">
            <div class="stepper-control">
              <UiButton
                type="button"
                :disabled="maxConcurrentDownloads <= 1"
                @click="changeMaxConcurrentDownloads(-1)"
                variant="secondary"
              >
                -
              </UiButton>
              <UiInput
                type="number"
                class="setting-input stepper-input"
                v-model.number="maxConcurrentDownloads"
                min="1"
                max="5"
                @blur="saveDownloadSettings"
                @change="saveDownloadSettings"
              />
              <UiButton
                type="button"
                :disabled="maxConcurrentDownloads >= 5"
                @click="changeMaxConcurrentDownloads(1)"
                variant="secondary"
              >
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
            <UiInput
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
            <select
              v-model="downloadTargetPolicy"
              class="setting-select"
              @change="saveDownloadSettings"
            >
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
            <select
              v-model="downloadAfterAction"
              class="setting-select"
              @change="saveDownloadSettings"
            >
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
            <p class="setting-description">
              {{ installContext?.downloadTargetDir || $t('settings.notConfigured') }}
            </p>
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
        <UiInput
          v-model="hubCookieDraft"
          class="cookie-input"
          :placeholder="$t('settings.hubCookiePlaceholder')"
          multiline
        />
        <div class="hub-actions">
          <UiButton @click="saveHubCookie" variant="secondary">
            {{ $t('settings.saveHubCookie') }}
          </UiButton>
          <UiButton @click="clearHubCookie" variant="secondary">
            {{ $t('settings.clearHubCookie') }}
          </UiButton>
        </div>
      </GlassPanel>

      <GlassPanel :title="$t('settings.trashSection')">
        <div class="setting-row">
          <div class="setting-info">
            <h4 class="setting-label">{{ $t('settings.trashRetention') }}</h4>
            <p class="setting-description">{{ $t('settings.trashRetentionDesc') }}</p>
          </div>
          <div class="setting-control retention-control">
            <UiInput
              v-model.number="cleanupPolicy.retention_days"
              type="number"
              size="md"
              class="setting-input retention-input"
              min="1"
              max="365"
              :aria-label="$t('settings.trashRetention')"
              :disabled="isSavingPolicy"
              @change="saveCleanupPolicy"
            />
            <span class="unit-label">{{ $t('settings.daysUnit') }}</span>
          </div>
        </div>

        <div class="setting-divider" />

        <div class="setting-row">
          <div class="setting-info">
            <h4 class="setting-label">{{ $t('settings.trashAutoPurge') }}</h4>
            <p class="setting-description">{{ $t('settings.trashAutoPurgeDesc') }}</p>
          </div>
          <div class="setting-control">
            <UiSwitch
              :model-value="cleanupPolicy.auto_purge"
              :label="$t('settings.trashAutoPurge')"
              :disabled="isSavingPolicy"
              @update:model-value="cleanupPolicy.auto_purge = $event; saveCleanupPolicy()"
            />
          </div>
        </div>

        <div class="setting-divider" />

        <div class="setting-row">
          <div class="setting-info">
            <h4 class="setting-label">{{ $t('settings.cleanupBackgroundCheck') }}</h4>
            <p class="setting-description">{{ $t('settings.cleanupBackgroundCheckDesc') }}</p>
          </div>
          <div class="setting-control">
            <UiSwitch
              :model-value="cleanupPolicy.background_index"
              :label="$t('settings.cleanupBackgroundCheck')"
              :disabled="isSavingPolicy"
              @update:model-value="cleanupPolicy.background_index = $event; saveCleanupPolicy()"
            />
          </div>
        </div>
      </GlassPanel>

      <GlassPanel :title="$t('settings.resourceManagement')">
        <div class="setting-row">
          <div class="setting-info">
            <h4 class="setting-label">{{ $t('settings.managedMode') }}</h4>
            <p class="setting-description">{{ $t('settings.managedModeDesc') }}</p>
          </div>
          <div class="setting-control">
            <UiSwitch
              :model-value="managedEnabled"
              :label="$t('settings.managedMode')"
              @update:model-value="toggleManagedMode"
            />
          </div>
        </div>

        <div class="setting-divider" />

        <div class="setting-row">
          <div class="setting-info">
            <h4 class="setting-label">{{ $t('settings.managedLibraryPath') }}</h4>
            <p class="setting-description">
              {{ installContext?.managedLibraryDir || $t('settings.notConfigured') }}
            </p>
          </div>
        </div>
      </GlassPanel>

      <GlassPanel :title="$t('settings.display')">
        <div class="setting-row">
          <div class="setting-info">
            <h4 id="blur-previews-label" class="setting-label">
              {{ $t('settings.blurPreviews') }}
            </h4>
            <p id="blur-previews-description" class="setting-description">
              {{ $t('settings.blurPreviewsDesc') }}
            </p>
          </div>
          <div class="setting-control">
            <UiSwitch
              type="button"
              role="switch"
              aria-labelledby="blur-previews-label"
              aria-describedby="blur-previews-description"
              :aria-checked="appStore.blurPreviews"
              :disabled="isSavingPreviewSettings"
              :model-value="appStore.blurPreviews"
              :label="$t('settings.blurPreviews')"
              @update:model-value="togglePreviewBlur"
            />
          </div>
        </div>

        <div class="setting-divider" />

        <div class="setting-row">
          <div class="setting-info">
            <h4 class="setting-label">{{ $t('settings.language') }}</h4>
            <p class="setting-description">{{ $t('settings.languageDesc') }}</p>
          </div>
          <div class="setting-control">
            <UiSegmented
              :model-value="locale"
              :label="$t('settings.language')"
              :options="[
                { value: 'zh-CN', label: '中文' },
                { value: 'en-US', label: 'English' },
              ]"
              @update:model-value="setLocale"
            />
          </div>
        </div>
      </GlassPanel>

      <GlassPanel :title="$t('settings.dataAndCache')">
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

        <div class="setting-divider" />

        <div class="setting-row">
          <div class="setting-info">
            <h4 class="setting-label">{{ $t('settings.clearThumbnails') }}</h4>
            <p class="setting-description">{{ $t('settings.clearThumbnailsDesc') }}</p>
          </div>
          <div class="setting-control">
            <UiButton
              :disabled="isClearingThumbnails"
              @click="handleClearThumbnails"
              variant="secondary"
            >
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
            <UiButton
              :disabled="isClearingLocalData"
              @click="confirmClearLocalData = true"
              variant="danger"
            >
              {{ isClearingLocalData ? $t('common.loading') : $t('settings.clearBtn') }}
            </UiButton>
          </div>
        </div>
      </GlassPanel>

      <GlassPanel :title="$t('settings.about')">
        <div class="about-content">
          <div class="about-row">
            <span class="about-label">{{ $t('settings.version') }}</span>
            <span class="about-value">{{ appVersion }}</span>
          </div>
          <div class="about-row">
            <span class="about-label">{{ $t('settings.repository') }}</span>
            <UiButton class="about-link" type="button" size="sm" @click="openRepository" variant="ghost">
              github.com/xyuqikzz/vam-library
            </UiButton>
          </div>
        </div>
      </GlassPanel>

      <UiModal
        v-if="confirmClearLocalData"
        :title="$t('settings.clearLocalDataConfirmTitle')"
        :description="$t('settings.clearLocalDataConfirmDesc')"
        :show-close="false"
        :close-disabled="isClearingLocalData"
        @close="closeClearLocalDataDialog"
      >
        <template #footer>
          <UiButton
            :disabled="isClearingLocalData"
            @click="closeClearLocalDataDialog"
            variant="secondary"
          >
            {{ $t('common.cancel') }}
          </UiButton>
          <UiButton :disabled="isClearingLocalData" @click="handleClearLocalData" variant="danger">
            {{ isClearingLocalData ? $t('common.loading') : $t('settings.clearLocalData') }}
          </UiButton>
        </template>
      </UiModal>
    </div>
  </div>
</template>

<script setup lang="ts">
import UiSegmented from '@/components/ui/UiSegmented.vue'
import UiModal from '@/components/ui/UiModal.vue'
import PageHeader from '@/components/layout/PageHeader.vue'
import UiInput from '@/components/ui/UiInput.vue'
import UiSwitch from '@/components/ui/UiSwitch.vue'
import UiButton from '@/components/ui/UiButton.vue'
import Icon from '@/components/ui/Icon.vue'
import { useI18n } from 'vue-i18n'
import { computed, onMounted, ref, watch } from 'vue'
import { storeToRefs } from 'pinia'
import { useAppStore } from '@/stores/app'
import { useLocalLibraryStore } from '@/stores/localLibrary'
import { useNotification } from '@/composables/useNotification'
import { invoke } from '@tauri-apps/api/core'
import GlassPanel from '@/components/common/GlassPanel.vue'
import SceneBrowserModPanel from '@/components/SceneBrowserModPanel.vue'
import type { CleanupPolicy } from '@/types/cleanup'
import packageInfo from '../../package.json'

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
const appVersion = packageInfo.version

// 回收站策略：原本放在回收站页顶部，属于偏好设置，统一放到设置页。
const cleanupPolicy = ref<CleanupPolicy>({ retention_days: 30, auto_purge: false, background_index: true })
const isSavingPolicy = ref(false)
onMounted(async () => {
  try {
    cleanupPolicy.value = await invoke<CleanupPolicy>('get_cleanup_policy')
  } catch {
    /* keep defaults; saving will report errors */
  }
})
async function saveCleanupPolicy() {
  if (isSavingPolicy.value) return
  const days = Math.min(365, Math.max(1, Math.round(Number(cleanupPolicy.value.retention_days) || 30)))
  isSavingPolicy.value = true
  try {
    cleanupPolicy.value = await invoke<CleanupPolicy>('save_cleanup_policy', {
      retentionDays: days,
      autoPurge: cleanupPolicy.value.auto_purge,
      backgroundIndex: cleanupPolicy.value.background_index,
    })
  } catch (error) {
    notify.error(t('settings.policySaveFailed', { error: String(error) }))
  } finally {
    isSavingPolicy.value = false
  }
}
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
  const name = newInstanceName.value.trim() || t('settings.defaultInstanceName', { index: vamInstances.value.length + 1 })
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
.setting-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--space-6);
  min-height: var(--control-h-md);
}

.setting-info {
  flex: 1;
  min-width: 0;
}

.setting-label {
  font-size: var(--text-md);
  font-weight: var(--font-medium);
  color: var(--text-primary);
  margin: 0 0 2px;
}

.setting-description {
  margin: 0;
  font-size: var(--text-sm);
  color: var(--text-tertiary);
  line-height: var(--leading-normal);
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
  height: var(--control-h-md);
  box-sizing: border-box;
  padding: 0 var(--space-4);
  background: var(--bg-input);
  border: none;
  border-radius: var(--radius-control);
  color: var(--text-secondary);
  font-size: var(--text-sm);
  max-width: 280px;
}

.path-text {
  max-width: 220px;
}

.about-content {
  display: flex;
  flex-direction: column;
}

.about-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  min-height: var(--control-h-md);
}

.about-row + .about-row {
  border-top: 1px solid var(--border-subtle);
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

/* 双类选择器保证覆盖 UiButton 幽灵按钮的文字色 */
.about-link.about-link {
  margin-right: calc(-1 * var(--space-3));
  color: var(--accent-primary);
}

.about-link.about-link:hover {
  color: var(--accent-primary-hover);
}

.setting-input {
  width: 120px;
}

/* 设置页右侧控件统一 40 高 */
.setting-control :deep(.ui-input-shell) {
  height: var(--control-h-md);
}

.stepper-control {
  display: grid;
  grid-template-columns: var(--control-h-md) 56px var(--control-h-md);
  align-items: center;
  overflow: hidden;
  height: var(--control-h-md);
  background: var(--bg-input);
  border: none;
  border-radius: var(--radius-control);
}

.stepper-control :deep(.ui-button) {
  width: var(--control-h-md);
  padding: 0;
  background: transparent;
}

.stepper-control :deep(.ui-button:hover:not(:disabled)) {
  background: var(--bg-control);
}

.stepper-control :deep(.ui-input-shell) {
  background: transparent;
}

.stepper-input {
  width: 56px;
  text-align: center;
  appearance: textfield;
  -moz-appearance: textfield;
}

.stepper-input :deep(input::-webkit-outer-spin-button),
.stepper-input :deep(input::-webkit-inner-spin-button) {
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
  align-items: center;
  gap: var(--space-2);
  padding-right: var(--space-2);
  border-radius: var(--radius-md);
  background: var(--bg-hover);
}

.instance-row.active {
  box-shadow: inset 0 0 0 2px var(--accent-primary);
}

.instance-main {
  display: flex;
  flex: 1;
  align-items: center;
  gap: var(--space-3);
  min-width: 0;
  min-height: 56px;
  padding: var(--space-2) var(--space-4);
  border: none;
  border-radius: var(--radius-md);
  background: transparent;
  color: inherit;
  font: inherit;
  text-align: left;
  cursor: pointer;
}

.instance-main:focus-visible {
  outline: none;
  box-shadow: var(--focus-ring);
}

.instance-indicator {
  width: 16px;
  height: 16px;
  flex-shrink: 0;
  box-sizing: border-box;
  border-radius: var(--radius-full);
  border: 2px solid var(--text-disabled);
}

.instance-row.active .instance-indicator {
  border: 5px solid var(--accent-primary);
}

.instance-text {
  display: flex;
  flex-direction: column;
  gap: 2px;
  min-width: 0;
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
  flex-shrink: 0;
  color: var(--text-tertiary);
  transition:
    color var(--duration-fast) var(--ease),
    background var(--duration-fast) var(--ease),
    border-color var(--duration-fast) var(--ease);
}

.instance-delete:hover {
  color: var(--color-error);
  background: var(--color-error-bg);
}

.instance-add-row {
  display: flex;
  gap: var(--space-item);
}

.retention-control {
  display: flex;
  align-items: center;
  gap: var(--space-2);
}

.retention-input {
  width: 96px;
}

.instance-name-input {
  flex: 0 1 280px;
}

.cookie-input {
  width: 100%;
  margin-top: var(--space-3);
}

/* 尺寸要落到 UiInput 内部的 textarea 上 */
.cookie-input :deep(textarea) {
  min-height: 88px;
  font-family: var(--font-mono);
  font-size: var(--text-sm);
}

.hub-actions {
  display: flex;
  gap: var(--space-item);
  margin-top: var(--space-3);
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
  margin: var(--space-2) 0 0;
  font-size: var(--text-xs);
  color: var(--text-tertiary);
}

</style>
