import { defineStore } from 'pinia'
import { ref } from 'vue'
import i18n from '../i18n'
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'

export interface ScanProgress {
  total_files: number
  processed_files: number
  current_file: string
  phase: string
}

export interface ScanResult {
  packages_found: number
  errors: string[]
  duration_ms: number
  total_files?: number
  skipped_files?: number
}

export type DownloadTargetPolicy = 'auto' | 'real_addon' | 'managed_library'
export type DownloadAfterAction = 'library_only' | 'add_to_active_plan' | 'add_and_apply'

export interface InstallContext {
  vamRoot: string
  realAddonDir: string
  managedLibraryDir: string
  downloadTempDir: string
  manifestPath: string
  managedStatePath: string
  isManaged: boolean
  activeOnDemandPlanId: string | null
  downloadTargetDir: string
  downloadTargetPolicy: DownloadTargetPolicy
  downloadAfterAction: DownloadAfterAction
  modeLabel: 'real_addon' | 'managed_library' | string
}

export interface VamInstance {
  id: string
  name: string
  rootPath: string
  managedEnabled?: boolean
  managedLibraryPath?: string | null
  downloadTargetPolicy?: DownloadTargetPolicy
  downloadAfterAction?: DownloadAfterAction
}

export const useAppStore = defineStore('app', () => {
  // ── State ──────────────────────────────────────────────────
  const vamRootPath = ref<string | null>(null)
  const isScanning = ref(false)
  const scanProgress = ref<ScanProgress | null>(null)
  const lastScanResult = ref<ScanResult | null>(null)
  const sidebarCollapsed = ref(false)
  const currentView = ref('dashboard')
  const searchQuery = ref('')
  const locale = ref<string>(localStorage.getItem('vamlibrary-locale') || 'zh-CN')
  const autoScan = ref(false)
  const theme = ref('dark')
  const managedEnabled = ref(false)
  const managedLibraryPath = ref<string | null>(null)
  const downloadTargetPolicy = ref<DownloadTargetPolicy>('auto')
  const downloadAfterAction = ref<DownloadAfterAction>('library_only')
  const installContext = ref<InstallContext | null>(null)
  const maxConcurrentDownloads = ref(1)
  const speedLimitKb = ref(0)
  const vamInstances = ref<VamInstance[]>([])
  const activeInstanceId = ref<string | null>(null)
  const hubAuthCookie = ref<string | null>(null)
  const hubLoggedIn = ref(false)


  // ── Scan Event Listener ────────────────────────────────────
  let unlistenScan: (() => void) | null = null

  function clampMaxConcurrentDownloads(value: number | undefined | null) {
    if (value === undefined || value === null || !Number.isFinite(value)) return 1
    return Math.min(5, Math.max(1, Math.trunc(value)))
  }

  async function setupScanListener() {
    // Clean up existing listener
    if (unlistenScan) {
      unlistenScan()
      unlistenScan = null
    }

    unlistenScan = await listen<ScanProgress>('scan-progress', (event) => {
      const progress = event.payload
      scanProgress.value = progress

      if (progress.phase === 'scanning') {
        isScanning.value = true
      } else if (progress.phase === 'done') {
        isScanning.value = false
        // Keep last progress visible briefly
        setTimeout(() => {
          scanProgress.value = null
        }, 2000)
      }
    })
  }

  // ── Actions ────────────────────────────────────────────────
  async function startScan(path: string) {
    if (isScanning.value) return

    isScanning.value = true
    scanProgress.value = null

    // Ensure listener is active
    await setupScanListener()

    try {
      const result: ScanResult = await invoke('scan_vam_directory', { path })
      lastScanResult.value = result
      return result
    } catch (error) {
      isScanning.value = false
      scanProgress.value = null
      throw error
    }
  }

  async function saveSettings() {
    try {
      maxConcurrentDownloads.value = clampMaxConcurrentDownloads(maxConcurrentDownloads.value)
      await invoke('save_settings', {
        settings: {
          vam_root_path: vamRootPath.value,
          auto_scan: autoScan.value,
          theme: theme.value,
          managed_enabled: managedEnabled.value,
          managed_library_path: managedLibraryPath.value,
          download_target_policy: downloadTargetPolicy.value,
          download_after_action: downloadAfterAction.value,
          max_concurrent_downloads: maxConcurrentDownloads.value,
          speed_limit_kb: speedLimitKb.value,
          vam_instances: withCurrentInstanceSettings(vamInstances.value),
          active_instance_id: activeInstanceId.value,
          hub_auth_cookie: hubAuthCookie.value,
          hub_logged_in: hubLoggedIn.value,
        },
      })
      // Sync setting parameters dynamically to Tauri background Downloader
      await invoke('set_download_settings', {
        maxConcurrent: maxConcurrentDownloads.value,
        speedLimitKb: speedLimitKb.value
      })
    } catch (e) {
      console.error('Failed to save settings:', e)
    }
  }

  async function loadSettings() {
    try {
      const settings = await invoke<{
        vam_root_path: string | null
        auto_scan: boolean
        theme: string
        managed_enabled?: boolean
        managed_library_path?: string | null
        download_target_policy?: DownloadTargetPolicy
        download_after_action?: DownloadAfterAction
        max_concurrent_downloads?: number
        speed_limit_kb?: number
        vam_instances?: VamInstance[]
        active_instance_id?: string | null
        hub_auth_cookie?: string | null
        hub_logged_in?: boolean
      }>('get_settings')
      
      vamRootPath.value = settings.vam_root_path
      autoScan.value = settings.auto_scan
      theme.value = settings.theme || 'dark'
      managedEnabled.value = settings.managed_enabled || false
      managedLibraryPath.value = settings.managed_library_path || null
      downloadTargetPolicy.value = settings.download_target_policy || 'auto'
      downloadAfterAction.value = settings.download_after_action || 'library_only'
      maxConcurrentDownloads.value = clampMaxConcurrentDownloads(settings.max_concurrent_downloads)
      speedLimitKb.value = settings.speed_limit_kb || 0
      vamInstances.value = settings.vam_instances || []
      activeInstanceId.value = settings.active_instance_id || null
      hubAuthCookie.value = settings.hub_auth_cookie || null
      hubLoggedIn.value = settings.hub_logged_in || false
      await refreshInstallContext()
      
      // Apply theme to document element
      applyTheme(theme.value)
    } catch (e) {
      console.error('Failed to load settings:', e)
    }
  }

  function applyTheme(themeValue: string) {
    if (themeValue === 'light') {
      document.documentElement.classList.add('light-theme')
    } else {
      document.documentElement.classList.remove('light-theme')
    }
  }

  async function setVamRoot(path: string | null) {
    vamRootPath.value = path
    if (path && vamInstances.value.length === 0) {
      const id = `instance_${Date.now()}`
      vamInstances.value = [withInstanceSettings({ id, name: '默认实例', rootPath: path })]
      activeInstanceId.value = id
    }
    await saveSettings()
    await refreshInstallContext()
  }

  async function saveInstances(instances: VamInstance[], activeId: string | null) {
    const nextInstances = withCurrentInstanceSettings(instances)
    const settings = await invoke<{
      vam_root_path: string | null
      vam_instances: VamInstance[]
      active_instance_id: string | null
    }>('save_vam_instances', {
      instances: nextInstances,
      activeInstanceId: activeId,
    })
    vamInstances.value = settings.vam_instances || []
    activeInstanceId.value = settings.active_instance_id || null
    vamRootPath.value = settings.vam_root_path
    await refreshInstallContext()
  }

  function withInstanceSettings(instance: VamInstance): VamInstance {
    return {
      ...instance,
      managedEnabled: managedEnabled.value,
      managedLibraryPath: managedLibraryPath.value,
      downloadTargetPolicy: downloadTargetPolicy.value,
      downloadAfterAction: downloadAfterAction.value,
    }
  }

  function withCurrentInstanceSettings(instances: VamInstance[]): VamInstance[] {
    return instances.map(instance => (
      instance.id === activeInstanceId.value ? withInstanceSettings(instance) : instance
    ))
  }

  async function saveHubCookie(cookie: string | null) {
    const settings = await invoke<{
      hub_auth_cookie?: string | null
      hub_logged_in?: boolean
    }>('save_hub_auth_cookie', { cookie })
    hubAuthCookie.value = settings.hub_auth_cookie || null
    hubLoggedIn.value = settings.hub_logged_in || false
  }

  async function setAutoScan(enabled: boolean) {
    autoScan.value = enabled
    await saveSettings()
  }

  async function setTheme(themeValue: string) {
    theme.value = themeValue
    applyTheme(themeValue)
    await saveSettings()
  }

  function setLocale(localeKey: string) {
    locale.value = localeKey
    localStorage.setItem('vamlibrary-locale', localeKey)
    try {
      const globalLocale = i18n.global.locale
      if (typeof globalLocale === 'object' && globalLocale && 'value' in globalLocale) {
        (globalLocale as any).value = localeKey
      } else {
        (i18n.global as any).locale = localeKey
      }
    } catch (e) {
      console.warn('Failed to set locale on global i18n:', e)
    }
  }

  async function setDownloadSettings(concurrent: number, limitKb: number) {
    maxConcurrentDownloads.value = clampMaxConcurrentDownloads(concurrent)
    speedLimitKb.value = limitKb
    await saveSettings()
  }

  async function setManagedSettings(enabled: boolean, libraryPath: string | null) {
    managedEnabled.value = enabled
    managedLibraryPath.value = libraryPath
    await saveSettings()
    await refreshInstallContext()
  }

  async function setDownloadPolicy(policy: DownloadTargetPolicy) {
    downloadTargetPolicy.value = policy
    await saveSettings()
    await refreshInstallContext()
  }

  async function setDownloadAfterAction(action: DownloadAfterAction) {
    downloadAfterAction.value = action
    await saveSettings()
  }

  async function refreshInstallContext() {
    if (!vamRootPath.value) {
      installContext.value = null
      return
    }
    try {
      installContext.value = await invoke<InstallContext>('get_install_context')
      managedEnabled.value = installContext.value.isManaged
      managedLibraryPath.value = installContext.value.managedLibraryDir
    } catch (e) {
      console.error('Failed to load install context:', e)
      installContext.value = null
    }
  }

  async function clearThumbnailCache() {
    await invoke('clear_thumbnail_cache')
  }

  return {
    vamRootPath,
    isScanning,
    scanProgress,
    lastScanResult,
    sidebarCollapsed,
    currentView,
    searchQuery,
    locale,
    autoScan,
    theme,
    managedEnabled,
    managedLibraryPath,
    downloadTargetPolicy,
    downloadAfterAction,
    installContext,
    maxConcurrentDownloads,
    speedLimitKb,
    vamInstances,
    activeInstanceId,
    hubAuthCookie,
    hubLoggedIn,
    setDownloadSettings,
    setManagedSettings,
    setDownloadPolicy,
    setDownloadAfterAction,
    saveInstances,
    saveHubCookie,
    refreshInstallContext,
    setLocale,
    setVamRoot,
    setAutoScan,
    setTheme,
    startScan,
    setupScanListener,
    loadSettings,
    saveSettings,
    clearThumbnailCache,
  }
})
