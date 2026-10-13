<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { useRouter } from 'vue-router'
import { invoke } from '@tauri-apps/api/core'
import { useDownloadStore } from '@/stores/download'
import { useNotification } from '@/composables/useNotification'
import type { PackageImageEntry, ResourceDetailItem } from '@/types/package'
import type { GameContentRef, SceneDetailContext } from '@/types/presets'
import { formatSize } from '@/utils/bytes'
import { cachedResource, resourceInvoke, resourceKey, previewUrl, loadPackageThumbnails } from '@/utils/resourceCache'
import { resourceColor, resourceVersion } from '@/utils/resourcePresentation'
import { parseHubFiles, parsePackageId, selectHubFile, type HubFile, type HubFileCandidate } from '@/utils/hubFiles'
import PackageDetailPanel from './PackageDetailPanel.vue'
import PackageDependencyList, { type DetailDependency } from './PackageDependencyList.vue'
import PackageDetailModal from './PackageDetailModal.vue'
import PackageActions from './PackageActions.vue'
import SceneFavoriteButton from './SceneFavoriteButton.vue'
import PackageSceneExtractor from '@/components/PackageSceneExtractor.vue'
import ShareModal from '@/components/ShareModal.vue'
import UiButton from '@/components/ui/UiButton.vue'
import UiChip from '@/components/ui/UiChip.vue'
import UiSegmented from '@/components/ui/UiSegmented.vue'
import UiModal from '@/components/ui/UiModal.vue'
import UiCheckbox from '@/components/ui/UiCheckbox.vue'
import UiInput from '@/components/ui/UiInput.vue'

interface DependencyNode {
  id: string
  creator: string
  name: string
  version: number
  resource_type: string
  size_bytes: number
  dependents_count: number
  dependencies_count: number
  status: string
}

type DependencyPanelKey = 'direct' | 'sub' | 'reverse'

type DependencyRow = DetailDependency

interface DependencyRelation {
  id: string
  tier: number
  status: 'satisfied' | 'missing' | 'lower_version'
  required_version: string
  installed_version: number | null
}

interface DependencyRelationsResult {
  direct: DependencyRelation[]
  sub: DependencyRelation[]
}

interface QuickDeleteResult {
  deleted_package_ids: string[]
  skipped_package_ids: string[]
  deleted_count: number
  skipped_count: number
  deleted_file_count: number
  freed_bytes: number
}

interface DeleteConfirmState {
  packageId: string
  includeDependencies: boolean
}


interface HubPackageInfo {
  hubFiles?: HubFile[] | null
}


interface PackageContentEntry {
  path: string
  resource_type: string
  size_bytes: number
}

interface DependencyGroup {
  key: string
  creator: string
  name: string
  requiredVersion: number | null
}

interface DownloadQueueItem {
  id: string
  url: string
  filename: string
  creator: string
  name: string
  version: number
  total_bytes: number
}

const props = withDefaults(defineProps<{
  item: ResourceDetailItem
  image?: string | null
  scene?: SceneDetailContext
  expanded?: boolean
  showQuickDelete?: boolean
  loadError?: string
  loading?: boolean
  vamRoot?: string | null
}>(), { expanded: false, showQuickDelete: false })
const emit = defineEmits<{
  close: []
  busyChange: [busy: boolean]
  'update:expanded': [expanded: boolean]
  packageDeleted: [result: QuickDeleteResult]
  favoriteChange: [source: GameContentRef, favorite: boolean]
}>()
const { t } = useI18n()
const router = useRouter()
const notify = useNotification()
const downloadStore = useDownloadStore()
interface DependencyLocation { item: ResourceDetailItem; unavailable: boolean }
interface DetailHistoryEntry { location: DependencyLocation | null; tab: string; expanded: boolean }
const dependencyLocation = ref<DependencyLocation | null>(null)
const detailHistory = ref<DetailHistoryEntry[]>([])
const detailModal = ref<InstanceType<typeof PackageDetailModal> | null>(null)
const detailPanel = ref<InstanceType<typeof PackageDetailPanel> | null>(null)
const openingDependency = ref('')
let navigationRequest = 0
const selectedPackage = computed(() => dependencyLocation.value?.item || props.item)
const currentScene = computed(() => dependencyLocation.value ? undefined : props.scene)
const unavailable = computed(() => dependencyLocation.value?.unavailable || false)
const currentLoading = computed(() => !dependencyLocation.value && props.loading)
const currentLoadError = computed(() => dependencyLocation.value ? '' : props.loadError)
const suppliedImage = computed(() => dependencyLocation.value ? null : props.image)
const packageId = computed(() => currentScene.value ? currentScene.value.source.packageId : selectedPackage.value.id)
const cacheIdentity = computed(() => resourceKey(selectedPackage.value, props.vamRoot || ''))
const contentKey = computed(() => currentScene.value?.key || selectedPackage.value.id)
const detailsExpanded = computed({ get: () => props.expanded, set: value => emit('update:expanded', value) })
const thumbnail = ref<string | null>(null)
const subtitle = computed(() => [selectedPackage.value.creator,
  resourceVersion(selectedPackage.value) || null,
  selectedPackage.value.size_bytes !== undefined ? formatSize(selectedPackage.value.size_bytes) : null,
  selectedPackage.value.scan_time ? new Date(selectedPackage.value.scan_time).toLocaleDateString() : null,
].filter(Boolean).join(' · '))
let epoch = 0
let alive = true
const shareModalVisible = ref(false)
function openShareModal() {
  shareModalVisible.value = true
}

async function openPackageFolder() {
  if (!selectedPackage.value) return
  try {
    if (packageId.value && selectedPackage.value.file_path) {
      await invoke('open_package_in_explorer', { filePath: selectedPackage.value.file_path })
    } else if (!packageId.value && currentScene.value && props.vamRoot) {
      await invoke('open_path_in_explorer', { path: `${props.vamRoot.replace(/[\\/]+$/, '')}/${currentScene.value.source.path}` })
    }
  } catch (err) {
    notify.error(t('ui.openLocationFailed', { error: String(err) }))
  }
}

function unversionedName(id: string) {
  return id.replace(/\.var$/i, '').replace(/\.(?:\d+|latest)$/i, '')
}
async function copyResourceName(id: string, stripVersion = true) {
  try {
    await navigator.clipboard.writeText(stripVersion ? unversionedName(id) : id)
    notify.success(t('common.copySuccess'))
  } catch {
    notify.error(t('ui.copyNameFailed'))
  }
}
function copyPackageName() {
  return copyResourceName(packageId.value || selectedPackage.value.name, !!packageId.value)
}

async function openDependency(dependency: DependencyRow) {
  if (busy.value || openingDependency.value) return
  const request = ++navigationRequest, requestEpoch = epoch
  openingDependency.value = dependency.id
  try {
    const id = dependency.installedId || dependency.id
    const item = dependency.installedId
      ? await cachedResource(`summary:${cacheIdentity.value}:${id}`, () => resourceInvoke<ResourceDetailItem | null>('get_package_summary', { packageId: id }))
      : null
    if (!alive || request !== navigationRequest || requestEpoch !== epoch) return
    const parsed = parsePackageId(id)
    detailHistory.value.push({ location: dependencyLocation.value, tab: detailTab.value, expanded: detailsExpanded.value })
    dependencyLocation.value = {
      item: item || { id, creator: parsed?.creator || '', name: parsed?.name || id, resource_types: [], ...(parsed?.version !== null && parsed?.version !== undefined ? { version: parsed.version } : {}) },
      unavailable: !item,
    }
    detailTab.value = 'overview'
    await nextTick()
    detailModal.value?.focusBack()
  } catch {
    if (alive && request === navigationRequest && requestEpoch === epoch) notify.error(t('ui.dependencyDetailsFailed'))
  } finally {
    if (request === navigationRequest) openingDependency.value = ''
  }
}
function goBack() {
  if (busy.value) return
  const previous = detailHistory.value.pop()
  if (!previous) return
  ++navigationRequest
  openingDependency.value = ''
  dependencyLocation.value = previous.location
  detailsExpanded.value = previous.expanded
  detailTab.value = previous.tab
  void nextTick(() => detailPanel.value?.focusTitle())
}
function resetNavigation() {
  ++navigationRequest
  openingDependency.value = ''
  dependencyLocation.value = null
  detailHistory.value = []
  detailTab.value = 'overview'
}
watch(() => [props.item.id, props.scene?.key], resetNavigation)

async function completeUnavailableDependency() {
  if (busy.value) return
  const parsed = parsePackageId(selectedPackage.value.id)
  if (!parsed) return
  const requestEpoch = epoch
  dependencyCompletionLoading.value = true
  try {
    const file = await resolveDependencyDownload({ key: selectedPackage.value.id, creator: parsed.creator, name: parsed.name, requiredVersion: parsed.version })
    if (!alive || requestEpoch !== epoch) return
    if (!file) { notify.warning(t('ui.missingNoSource')); return }
    await downloadStore.addItems([{ id: file.filename.replace(/\.var$/i, ''), url: file.url, filename: file.filename, creator: parsed.creator, name: parsed.name, version: file.version, total_bytes: file.sizeBytes }])
    notify.success(t('ui.queuedDependencies', { count: 1 }))
  } catch (error) {
    notify.error(t('ui.completeFailed', { error: String(error) }))
  } finally { if (alive && requestEpoch === epoch) dependencyCompletionLoading.value = false }
}

const deleteLoading = ref(false)
const deleteConfirm = ref<DeleteConfirmState | null>(null)
const deleteConfirmMessage = computed(() => {
  if (!deleteConfirm.value) return ''
  const key = deleteConfirm.value.includeDependencies
    ? 'packages.confirmQuickDeleteWithDeps'
    : 'packages.confirmQuickDelete'
  return t(key, { name: deleteConfirm.value.packageId })
})

function quickDeleteSelected(includeDependencies: boolean) {
  if (!selectedPackage.value || deleteLoading.value) return

  deleteConfirm.value = {
    packageId: selectedPackage.value.id,
    includeDependencies,
  }
}

function cancelQuickDelete() {
  if (deleteLoading.value) return
  deleteConfirm.value = null
}

async function confirmQuickDelete() {
  if (!deleteConfirm.value || deleteLoading.value) return
  const { packageId, includeDependencies } = deleteConfirm.value
  deleteLoading.value = true
  try {
    const result = await invoke<QuickDeleteResult>('quick_delete_package', {
      packageId,
      includeDependencies,
    })
    emit('close')
    deleteConfirm.value = null
    shareModalVisible.value = false
    emit('packageDeleted', result)

    const skippedText = result.skipped_count > 0
      ? t('packages.quickDeleteSkipped', { count: result.skipped_count })
      : ''
    notify.success(`${t('packages.quickDeleteSuccess', { count: result.deleted_count })}${skippedText}`)
  } catch (err) {
    notify.error(t('packages.quickDeleteFailed', { error: String(err) }))
  } finally {
    deleteLoading.value = false
  }
}

const extractingCharacter = ref(false)
const selectedPackageImages = ref<PackageImageEntry[]>([])
const imageListLoading = ref(false)
const packageImageUrls = ref<Record<string, string>>({})
const previewImage = ref<PackageImageEntry | null>(null)
const directDependencyRelations = ref<DependencyRelation[]>([])
const subDependencyRelations = ref<DependencyRelation[]>([])
const reverseDependencies = ref<DependencyNode[]>([])
const reverseDependenciesLoading = ref(false)
const dependencyRelationsLoading = ref(false)
const dependencyRelationsLoaded = ref(false)
const dependencyLoadError = ref('')
const dependencyCompletionLoading = ref(false)
const dependencyPanelExpanded = ref<Record<DependencyPanelKey, boolean>>({
  direct: false,
  sub: false,
  reverse: false,
})
const imagesExpanded = ref(false)
const COLLAPSED_IMAGE_COUNT = 3
const EXPANDED_IMAGE_COUNT = 18
const dependencyPanels = computed<Array<{ key: DependencyPanelKey; label: string }>>(() => [
  { key: 'direct', label: t('ui.dependencies') },
  { key: 'sub', label: t('ui.subDependencies') },
  { key: 'reverse', label: t('ui.dependents') },
])
const visiblePackageImages = computed(() => (
  selectedPackageImages.value.slice(0, imagesExpanded.value ? EXPANDED_IMAGE_COUNT : COLLAPSED_IMAGE_COUNT)
))
const previewImageUrl = computed(() => (
  previewImage.value ? packageImageUrls.value[previewImage.value.path] || null : null
))
const detailThumb = computed(() => suppliedImage.value || thumbnail.value)

const showCompleteDependencyButton = computed(() => (
  Boolean(selectedPackage.value)
  && !dependencyRelationsLoading.value
  && (selectedPackage.value?.dependency_count || 0) > 0
  && incompleteDependencyCount.value > 0
))

const directDependencyRows = computed<DependencyRow[]>(() => {
  return dependencyRelationRows(directDependencyRelations.value)
})

const subDependencyRows = computed<DependencyRow[]>(() => {
  return dependencyRelationRows(subDependencyRelations.value)
})

const reverseDependencyRows = computed<DependencyRow[]>(() => (
  reverseDependencies.value
    .map((dep) => ({
      id: dep.id,
      installedId: dep.id,
      status: 'referenced' as const,
      statusText: t('ui.referenced'),
    }))
    .sort((a, b) => a.id.localeCompare(b.id))
))

const incompleteDependencyCount = computed(() => (
  [...directDependencyRelations.value, ...subDependencyRelations.value]
    .filter((dep) => dep.status !== 'satisfied')
    .length
))

function dependencyRelationRows(relations: DependencyRelation[]): DependencyRow[] {
  return relations
    .map((dep) => ({
      id: dep.id,
      tier: dep.tier,
      installedId: dep.installed_version != null ? `${unversionedName(dep.id)}.${dep.installed_version}` : dep.status === 'satisfied' ? dep.id : undefined,
      status: dependencyRelationStatus(dep),
      statusText: dependencyRelationStatusText(dep),
    }))
    .sort((a, b) => {
      if ((a.tier || 1) !== (b.tier || 1)) return (a.tier || 1) - (b.tier || 1)
      return a.id.localeCompare(b.id)
    })
}

function dependencyRelationStatus(dep: DependencyRelation): DependencyRow['status'] {
  if (dep.status === 'lower_version') return 'lower-version'
  if (dep.status === 'missing') return 'missing'
  return 'owned'
}

function dependencyRelationStatusText(dep: DependencyRelation): string {
  if (dep.status === 'lower_version') return t('ui.lowerVersion')
  if (dep.status === 'missing') return t('ui.missingStatus')
  return t('ui.installed')
}

function resetDependencyRelations() {
  directDependencyRelations.value = []
  subDependencyRelations.value = []
  reverseDependencies.value = []
  reverseDependenciesLoading.value = false
  dependencyRelationsLoading.value = false
  dependencyRelationsLoaded.value = false
  dependencyLoadError.value = ''
  dependencyCompletionLoading.value = false
  dependencyPanelExpanded.value = {
    direct: false,
    sub: false,
    reverse: false,
  }
}

async function completeSelectedPackageDependencies() {
  const id = packageId.value
  if (!id || dependencyCompletionLoading.value) return

  dependencyCompletionLoading.value = true
  try {
    if (packageId.value !== id) return
    await loadDependencyRelations(true)
    if (packageId.value !== id) return

    const targetIncomplete = [...directDependencyRelations.value, ...subDependencyRelations.value]
      .filter((dep) => dep.status !== 'satisfied')
    if (targetIncomplete.length === 0) {
      notify.success(t('ui.noMissingDependencies'))
      return
    }

    const groups = groupDependencyRelations(targetIncomplete)
    const items: DownloadQueueItem[] = []
    let noSourceCount = 0

    for (const group of groups) {
      const file = await resolveDependencyDownload(group)
      if (!file) {
        noSourceCount += 1
        continue
      }
      items.push({
        id: `${group.creator}.${group.name}.${file.version}`,
        url: file.url,
        filename: file.filename,
        creator: group.creator,
        name: group.name,
        version: file.version,
        total_bytes: file.sizeBytes,
      })
    }

    if (items.length === 0) {
      notify.warning(t(noSourceCount > 0 ? 'ui.missingNoSource' : 'ui.nothingToQueue'))
      return
    }

    const results = await downloadStore.addItems(items)
    const skippedCount = Array.isArray(results)
      ? results.filter((result) => result && typeof result === 'object' && 'Skipped' in result).length
      : 0
    const addedCount = Math.max(items.length - skippedCount, 0)

    if (addedCount > 0) notify.success(t('ui.queuedDependencies', { count: addedCount }))
    if (skippedCount > 0) notify.info(t('ui.skippedQueued', { count: skippedCount }))
    if (noSourceCount > 0) notify.warning(t('ui.noSourceCount', { count: noSourceCount }))

    router.push('/download')
  } catch (err) {
    notify.error(t('ui.completeFailed', { error: String(err) }))
  } finally {
    dependencyCompletionLoading.value = false
  }
}

function dependencyPanelCount(key: DependencyPanelKey): number {
  if (!selectedPackage.value) return 0
  if (key === 'direct') return selectedPackage.value.dependency_count || 0
  if (key === 'sub') return subDependencyRows.value.length
  return selectedPackage.value.dependents_count || 0
}

function dependencyPanelRows(key: DependencyPanelKey): DependencyRow[] {
  if (key === 'direct') return directDependencyRows.value
  if (key === 'sub') return subDependencyRows.value
  if (key === 'reverse') return reverseDependencyRows.value
  return []
}

function toggleDependencyPanel(key: DependencyPanelKey) {
  dependencyPanelExpanded.value[key] = !dependencyPanelExpanded.value[key]
  if (dependencyPanelExpanded.value[key]) {
    if (key === 'reverse') void loadReverseDependencies()
    else void loadDependencyRelations()
  }
}

async function loadDependencyRelations(force = false) {
  const requestEpoch = epoch
  const id = packageId.value
  if (!id || (!force && dependencyRelationsLoaded.value) || dependencyRelationsLoading.value) return

  dependencyRelationsLoading.value = true
  dependencyLoadError.value = ''
  try {
    const relations = await cachedResource(`relations:${cacheIdentity.value}`, () => resourceInvoke<DependencyRelationsResult>('get_package_dependency_relations', { packageId: id }), force)
    if (!alive || requestEpoch !== epoch || packageId.value !== id) return
    directDependencyRelations.value = relations.direct || []
    subDependencyRelations.value = relations.sub || []
    dependencyRelationsLoaded.value = true
  } catch (err) {
    if (alive && requestEpoch === epoch && selectedPackage.value?.id === id) {
      dependencyLoadError.value = t('ui.loadFailed', { error: String(err) })
    }
  } finally {
    if (alive && requestEpoch === epoch && selectedPackage.value?.id === id) {
      dependencyRelationsLoading.value = false
    }
  }
}

async function loadReverseDependencies() {
  const requestEpoch = epoch, id = packageId.value
  if (!id || reverseDependenciesLoading.value) return
  reverseDependenciesLoading.value = true
  dependencyLoadError.value = ''
  try {
    const result = await cachedResource(`reverse:${cacheIdentity.value}`, () => resourceInvoke<DependencyNode[]>('get_reverse_dependencies', { packageId: id }))
    if (alive && requestEpoch === epoch) reverseDependencies.value = result
  } catch (error) {
    if (alive && requestEpoch === epoch) dependencyLoadError.value = t('ui.loadFailed', { error: String(error) })
  } finally { if (alive && requestEpoch === epoch) reverseDependenciesLoading.value = false }
}

function groupDependencyRelations(list: DependencyRelation[]): DependencyGroup[] {
  const map = new Map<string, DependencyGroup>()

  for (const item of list) {
    const parsed = parsePackageId(item.id)
    if (!parsed) continue

    const key = `${parsed.creator.toLowerCase()}::${parsed.name.toLowerCase()}`
    const requiredVersion = parseRequiredVersion(item.required_version)
    const existing = map.get(key)
    if (!existing) {
      map.set(key, {
        key,
        creator: parsed.creator,
        name: parsed.name,
        requiredVersion,
      })
      continue
    }

    if (requiredVersion !== null) {
      existing.requiredVersion = existing.requiredVersion === null
        ? requiredVersion
        : Math.max(existing.requiredVersion, requiredVersion)
    }
  }

  return [...map.values()]
}

async function resolveDependencyDownload(group: DependencyGroup): Promise<HubFileCandidate | null> {
  try {
    const detail = await invoke<HubPackageInfo>('fetch_hub_package_info', {
      packageName: `${group.creator}.${group.name}.latest`,
    })
    const files = parseHubFiles(detail.hubFiles || [])
    return selectHubFile(files, group.requiredVersion)
  } catch {
    return null
  }
}




function parseRequiredVersion(requiredVersion: string) {
  const text = requiredVersion.trim()
  if (!text || text.toLowerCase() === 'latest') return null
  const version = Number.parseInt(text, 10)
  return Number.isFinite(version) ? version : null
}


async function loadPackageImages(packageId: string) {
  const requestEpoch = epoch
  imageListLoading.value = true
  selectedPackageImages.value = []
  packageImageUrls.value = {}
  try {
    const images = await cachedResource(`images:${cacheIdentity.value}`, () => resourceInvoke<PackageImageEntry[]>('list_package_images', { packageId }))
    if (alive && requestEpoch === epoch && selectedPackage.value?.id === packageId) {
      selectedPackageImages.value = images
      loadVisiblePackageImages(packageId)
    }
  } catch {
    if (alive && requestEpoch === epoch && selectedPackage.value?.id === packageId) {
      selectedPackageImages.value = []
    }
  } finally {
    if (alive && requestEpoch === epoch && selectedPackage.value?.id === packageId) {
      imageListLoading.value = false
    }
  }
}

async function loadVisiblePackageImages(packageId: string) {
  for (const image of visiblePackageImages.value) {
    loadPackageImage(packageId, image.path)
  }
}

async function loadPackageImage(packageId: string, imagePath: string) {
  const requestEpoch = epoch
  if (packageImageUrls.value[imagePath]) return
  try {
    const result = await cachedResource(`image:${cacheIdentity.value}:${imagePath}`, async () => previewUrl(await resourceInvoke<string | null>('get_package_image', { packageId, imagePath })))
    if (result && alive && requestEpoch === epoch && selectedPackage.value?.id === packageId) {
      packageImageUrls.value = { ...packageImageUrls.value, [imagePath]: result }
    }
  } catch {
    // 图片预览失败时保留占位状态
  }
}

function openImagePreview(image: PackageImageEntry) {
  if (!selectedPackage.value) return
  previewImage.value = image
  loadPackageImage(selectedPackage.value.id, image.path)
}

function toggleImageExpanded() {
  imagesExpanded.value = !imagesExpanded.value
  if (selectedPackage.value) {
    loadVisiblePackageImages(selectedPackage.value.id)
  }
}

function formatTime(iso: string): string {
  if (!iso) return '-'
  try {
    const d = new Date(iso)
    return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, '0')}-${String(d.getDate()).padStart(2, '0')}`
  } catch { return iso.slice(0, 10) }
}

function resourceTypeLabel(type: string): string {
  const key = `resourceType.${type}`
  const translated = t(key)
  return translated !== key ? translated : type
}

const detailTabs = computed(() => [
  { value: 'overview', label: t('ui.overview') },
  ...dependencyPanels.value.map(panel => ({ value: panel.key, label: panel.label, count: dependencyPanelCount(panel.key) })),
  { value: 'contents', label: t('ui.contents'), count: selectedPackage.value?.content_count || 0 },
])

// 「内容」页：包内文件清单（来自入库时的记录，不读压缩包），切到这一页才加载
const CONTENT_PAGE_SIZE = 200
const CONTENT_TYPE_ORDER = ['scene', 'appearance', 'clothing', 'hair', 'morph', 'plugin', 'asset', 'texture', 'sound', 'other']
const packageContents = ref<PackageContentEntry[]>([])
const contentsLoading = ref(false)
const contentsLoaded = ref(false)
const contentsError = ref('')
const contentFilter = ref('all')
const contentQuery = ref('')
const contentLimit = ref(CONTENT_PAGE_SIZE)
const contentTypeCounts = computed(() => {
  const counts = new Map<string, number>()
  for (const entry of packageContents.value) counts.set(entry.resource_type, (counts.get(entry.resource_type) || 0) + 1)
  const rank = (type: string) => { const index = CONTENT_TYPE_ORDER.indexOf(type); return index < 0 ? CONTENT_TYPE_ORDER.length : index }
  return [...counts].sort((a, b) => rank(a[0]) - rank(b[0]) || a[0].localeCompare(b[0]))
})
const filteredContents = computed(() => {
  const query = contentQuery.value.trim().toLowerCase()
  return packageContents.value.filter(entry =>
    (contentFilter.value === 'all' || entry.resource_type === contentFilter.value)
    && (!query || entry.path.toLowerCase().includes(query)))
})
const visibleContents = computed(() => filteredContents.value.slice(0, contentLimit.value).map(entry => {
  const cut = Math.max(entry.path.lastIndexOf('/'), entry.path.lastIndexOf('\\')) + 1
  return { ...entry, folder: entry.path.slice(0, cut), fileName: entry.path.slice(cut) }
}))
watch([contentFilter, contentQuery], () => { contentLimit.value = CONTENT_PAGE_SIZE })
function resetContents() {
  packageContents.value = []
  contentsLoading.value = false
  contentsLoaded.value = false
  contentsError.value = ''
  contentFilter.value = 'all'
  contentQuery.value = ''
  contentLimit.value = CONTENT_PAGE_SIZE
}
async function loadPackageContents() {
  const requestEpoch = epoch, id = packageId.value
  if (!id || contentsLoaded.value || contentsLoading.value) return
  contentsLoading.value = true
  contentsError.value = ''
  try {
    const result = await cachedResource(`contents:${cacheIdentity.value}`, () => resourceInvoke<PackageContentEntry[]>('list_package_contents', { packageId: id }))
    if (alive && requestEpoch === epoch) {
      packageContents.value = result || []
      contentsLoaded.value = true
    }
  } catch (error) {
    if (alive && requestEpoch === epoch) contentsError.value = t('ui.loadFailed', { error: String(error) })
  } finally {
    if (alive && requestEpoch === epoch) contentsLoading.value = false
  }
}
const detailTab = ref('overview')
const dependencyThumbnails = ref<Record<string, string | null>>({})
const visibleDependencyRows = computed(() => detailsExpanded.value
  ? dependencyPanelRows(detailTab.value as DependencyPanelKey)
  : directDependencyRows.value)
watch([visibleDependencyRows, cacheIdentity], () => {
  const requestEpoch = epoch
  const rows = visibleDependencyRows.value.filter(row => row.installedId)
  dependencyThumbnails.value = {}
  void loadPackageThumbnails(rows.map(row => ({ id: row.installedId!, name: row.id, creator: '', resource_types: [] })), (id, url) => {
    for (const row of rows) if (row.installedId === id) dependencyThumbnails.value[row.id] = url
  }, () => alive && requestEpoch === epoch)
}, { flush: 'post' })
const sceneActions = ref<InstanceType<typeof PackageSceneExtractor> | null>(null)
const busy = computed(() => extractingCharacter.value || deleteLoading.value || dependencyCompletionLoading.value)
watch(busy, value => emit('busyChange', value), { flush: 'sync' })
function openDetails() { if (busy.value) return; if (!imagesExpanded.value) toggleImageExpanded(); detailsExpanded.value = true; detailTab.value = 'overview' }
function setDetailTab(value: string) {
  detailTab.value = value
  if (['direct', 'sub', 'reverse'].includes(value) && !dependencyPanelExpanded.value[value as DependencyPanelKey]) toggleDependencyPanel(value as DependencyPanelKey)
  if (value === 'contents') void loadPackageContents()
}
watch(detailsExpanded, expanded => {
  if (expanded && packageId.value && !unavailable.value) {
    imagesExpanded.value = true
    void loadPackageImages(packageId.value)
  }
})
// 关闭详情就是取消选中：弹窗收起，父组件卸载这份详情
function closeDetails() { if (busy.value) return; detailsExpanded.value = false; emit('close') }
function shortcut(event: KeyboardEvent) {
  if (event.defaultPrevented || busy.value || (event.target as HTMLElement).closest('input, textarea, select, [contenteditable="true"]')) return
  const action = event.ctrlKey || event.metaKey ? ({ c: copyPackageName, o: openPackageFolder } as Record<string, () => unknown>)[event.key.toLowerCase()] : event.key === 'Delete' && props.showQuickDelete && !unavailable.value ? () => quickDeleteSelected(true) : undefined
  if (!action) return
  event.preventDefault(); event.stopPropagation(); void action()
}
watch(() => [cacheIdentity.value, contentKey.value, currentLoading.value, unavailable.value], () => {
  const requestEpoch = ++epoch
  thumbnail.value = null; previewImage.value = null; imagesExpanded.value = detailsExpanded.value
  selectedPackageImages.value = []; packageImageUrls.value = {}; resetDependencyRelations(); resetContents()
  if (!packageId.value || currentLoading.value || unavailable.value) return
  const id = packageId.value
  if (detailsExpanded.value) void loadPackageImages(id)
  void loadDependencyRelations()
  if (detailsExpanded.value && detailTab.value === 'reverse') void loadReverseDependencies()
  if (detailsExpanded.value && detailTab.value === 'contents') void loadPackageContents()
  if (!suppliedImage.value) void cachedResource(`thumbnail:${resourceKey(selectedPackage.value)}`, async () => previewUrl(await resourceInvoke<string | null>('get_package_thumbnail', { packageId: id }))).then(image => {
    if (alive && requestEpoch === epoch) thumbnail.value = image
  }).catch(() => {})
}, { immediate: true })
onBeforeUnmount(() => { alive = false; ++epoch; emit('busyChange', false) })
defineExpose({
  resetNavigation,
  ready: computed(() => !!sceneActions.value?.ready),
  canComplete: showCompleteDependencyButton, completing: dependencyCompletionLoading, deleting: deleteLoading,
  launch: () => sceneActions.value?.launch(), extract: () => sceneActions.value?.extract(),
  share: openShareModal, openLocation: openPackageFolder, copyName: copyPackageName,
  delete: () => quickDeleteSelected(true), complete: completeSelectedPackageDependencies,
})
</script>

<template>
  <PackageDetailModal
    ref="detailModal"
    :can-go-back="detailHistory.length > 0"
    @back="goBack"
    :open="detailsExpanded"
    :title="selectedPackage.name"
    :package-id="selectedPackage.id"
    :images="selectedPackageImages"
    :image-urls="packageImageUrls"
    :fallback-image="detailThumb"
    :busy="busy"
    @close="closeDetails"
    @image="openImagePreview"
  >
    <template #header-actions>
      <SceneFavoriteButton
        v-if="selectedPackage.resource_types.includes('scene') && sceneActions?.selectedScene"
        :favorite="!!sceneActions.selectedScene.favorite"
        :disabled="busy || (!sceneActions.selectedScene.available && !sceneActions.selectedScene.favorite)"
        :loading="sceneActions.favoriting"
        @toggle="sceneActions.toggleFavorite()"
      />
    </template>
    <PackageDetailPanel
      ref="detailPanel"
      @keydown="shortcut"
      :item="selectedPackage"
      :subtitle="subtitle"
      :image="previewImageUrl || detailThumb"
      :active-image="previewImage?.path"
      :images="selectedPackageImages"
      :image-urls="packageImageUrls"
      :expanded="detailsExpanded"
      :busy="busy || !!openingDependency"
      :can-go-back="detailHistory.length > 0"
      :dependency-thumbnails="dependencyThumbnails"
      :opening-dependency="openingDependency"
      @copy="copyPackageName"
      @copy-dependency="copyResourceName"
      @dependency-details="openDependency"
      :dependencies="directDependencyRows"
      :missing="incompleteDependencyCount"
      :error="currentLoadError || dependencyLoadError"
      :loading="currentLoading || dependencyRelationsLoading || reverseDependenciesLoading"
      @close="closeDetails"
      @image="openImagePreview"
      @details="openDetails"
      @complete="completeSelectedPackageDependencies"
    >
      <template #actions>
        <div v-if="unavailable" class="unavailable-dependency">
          <p role="status">{{ t('ui.dependencyNotInstalled') }}</p>
          <UiButton size="sm" :loading="dependencyCompletionLoading" @click="completeUnavailableDependency">{{ t('ui.complete') }}</UiButton>
        </div>
        <PackageSceneExtractor
          v-else
          ref="sceneActions"
          :key="contentKey + ':' + (selectedPackage.file_path || '') + ':' + (selectedPackage.scan_time || '')"
          :package-id="packageId || undefined"
          :cache-key="cacheIdentity"
          :scene-eligible="selectedPackage.resource_types.includes('scene')"
          :source="currentScene?.source"
          :scene-name="currentScene?.name"
          :favorite="currentScene?.favorite"
          :available="currentScene?.available"
          @favorite-change="(source, favorite) => emit('favoriteChange', source, favorite)"
          @busy-change="extractingCharacter = $event"
        >
          <template #actions="{ launch, extract, disabled, launching }">
            <PackageActions
              :disabled="disabled"
              :launching="launching"
              :can-open="!!selectedPackage.file_path || !packageId"
              :can-share="!!packageId"
              :can-delete="showQuickDelete"
              :deleting="deleteLoading"
              :missing="showCompleteDependencyButton"
              :completing="dependencyCompletionLoading"
              @launch="launch"
              @extract="extract"
              @share="openShareModal"
              @open="openPackageFolder"
              @copy="copyPackageName"
              @delete="quickDeleteSelected(true)"
              @complete="completeSelectedPackageDependencies"
            />
          </template>
        </PackageSceneExtractor>
      </template>
      <template v-if="detailsExpanded && !unavailable" #tabs>
        <UiSegmented
          :model-value="detailTab"
          :options="detailTabs"
          :label="t('ui.overview')"
          class="detail-tabs"
          @update:model-value="setDetailTab"
        />
      </template>
      <template v-if="unavailable" #default><p class="detail-unavailable">{{ t('ui.dependencyPreviewUnavailable') }}</p></template>
      <template v-else-if="detailsExpanded" #default>
        <div v-if="detailTab === 'overview'" class="detail-overview">
          <dl class="detail-metadata">
            <div v-if="selectedPackage.version !== undefined">
              <dt>{{ t('packages.version') }}</dt>
              <dd>{{ resourceVersion(selectedPackage) }}</dd>
            </div>
            <div v-if="selectedPackage.content_count !== undefined">
              <dt>{{ t('ui.files') }}</dt>
              <dd>{{ selectedPackage.content_count }}</dd>
            </div>
            <div v-if="selectedPackage.size_bytes !== undefined">
              <dt>{{ t('packages.size') }}</dt>
              <dd>{{ formatSize(selectedPackage.size_bytes || 0) }}</dd>
            </div>
            <div v-if="selectedPackage.scan_time">
              <dt>{{ t('ui.libraryDate') }}</dt>
              <dd>{{ formatTime(selectedPackage.scan_time || '') }}</dd>
            </div>
            <div v-if="selectedPackage.file_path" class="metadata-path">
              <dt>{{ t('ui.directory') }}</dt>
              <dd>{{ selectedPackage.file_path }}</dd>
            </div>
          </dl>
          <!-- 只有一种类型时，顶部的类型标签已经说明了，不再重复 -->
          <div v-if="selectedPackage.resource_types.length > 1" class="content-types">
            <UiChip
              v-for="type in selectedPackage.resource_types"
              :key="type"
              :color="resourceColor(type)"
              disabled
            >
              {{ resourceTypeLabel(type) }}
            </UiChip>
          </div>
          <div v-if="incompleteDependencyCount" class="missing-notice">
            <span>{{ t('ui.missingDependencyNotice', { count: incompleteDependencyCount }) }}</span>
            <UiButton
              size="sm"
              :loading="dependencyCompletionLoading"
              @click="completeSelectedPackageDependencies"
            >
              {{ t('ui.allDependencies') }}
            </UiButton>
          </div>
        </div>
        <div v-else-if="detailTab === 'contents'" class="detail-contents">
          <p v-if="contentsLoading" class="detail-hint" role="status">{{ t('common.loading') }}</p>
          <p v-else-if="contentsError" class="detail-hint is-error" role="alert">{{ contentsError }}</p>
          <p v-else-if="contentsLoaded && !packageContents.length" class="detail-hint">{{ t('ui.noContents') }}</p>
          <template v-else-if="packageContents.length">
            <UiInput
              v-model="contentQuery"
              class="contents-search"
              search
              size="md"
              :placeholder="t('ui.searchContents')"
              :aria-label="t('ui.searchContents')"
            />
            <div class="chip-row contents-filters">
              <UiChip :selected="contentFilter === 'all'" :count="packageContents.length" @click="contentFilter = 'all'">{{ t('ui.all') }}</UiChip>
              <UiChip
                v-for="[type, count] in contentTypeCounts"
                :key="type"
                :color="resourceColor(type)"
                :count="count"
                :selected="contentFilter === type"
                @click="contentFilter = type"
              >{{ resourceTypeLabel(type) }}</UiChip>
            </div>
            <ul v-if="visibleContents.length" class="content-list">
              <li v-for="entry in visibleContents" :key="entry.path" class="content-row" :title="entry.path">
                <i class="content-dot" :style="{ background: resourceColor(entry.resource_type) }" aria-hidden="true" />
                <span class="content-name">
                  <strong>{{ entry.fileName }}</strong>
                  <small v-if="entry.folder">{{ entry.folder }}</small>
                </span>
                <span class="content-size">{{ formatSize(entry.size_bytes) }}</span>
              </li>
            </ul>
            <p v-else class="detail-hint">{{ t('ui.noMatchingContents') }}</p>
            <div v-if="filteredContents.length > visibleContents.length" class="contents-more">
              <span>{{ t('ui.shown', { shown: visibleContents.length, total: filteredContents.length }) }}</span>
              <UiButton size="sm" variant="ghost" @click="contentLimit += CONTENT_PAGE_SIZE">{{ t('ui.showMore') }}</UiButton>
            </div>
          </template>
        </div>
        <div v-else class="expanded-dependencies">
          <p v-if="dependencyRelationsLoading || reverseDependenciesLoading" role="status">{{ t('common.loading') }}</p>
          <p v-else-if="dependencyLoadError" role="alert">{{ dependencyLoadError }}</p>
          <PackageDependencyList
            :dependencies="dependencyPanelRows(detailTab as DependencyPanelKey)"
            :thumbnails="dependencyThumbnails"
            :busy="busy || !!openingDependency"
            :opening="openingDependency"
            @copy="copyResourceName"
            @details="openDependency"
            @complete="completeSelectedPackageDependencies"
          />
          <p
            v-if="!dependencyRelationsLoading && !dependencyLoadError && !dependencyPanelRows(detailTab as DependencyPanelKey).length"
          >
            {{ t('ui.noData') }}
          </p>
        </div>
      </template>
    </PackageDetailPanel>
  </PackageDetailModal>
  <ShareModal
    :visible="shareModalVisible"
    :package-id="selectedPackage?.id || ''"
    @close="shareModalVisible = false"
  />
  <UiModal
    v-if="deleteConfirm"
    :title="t('packages.quickDeleteConfirmTitle')"
    :description="deleteConfirmMessage"
    :close-disabled="deleteLoading"
    @close="cancelQuickDelete"
  >
    <UiCheckbox v-model="deleteConfirm.includeDependencies" :disabled="deleteLoading">
      {{ t('packages.deleteIncludeDeps') }}
    </UiCheckbox>
    <p class="delete-scope">
      {{ deleteConfirm.includeDependencies ? t('packages.quickDeleteDepsScope') : t('packages.deleteIncludeDepsHint') }}
    </p>
    <template #footer>
      <UiButton :disabled="deleteLoading" @click="cancelQuickDelete">
        {{ t('common.cancel') }}
      </UiButton>
      <UiButton variant="danger" :loading="deleteLoading" @click="confirmQuickDelete">
        {{ t('packages.confirmDelete') }}
      </UiButton>
    </template>
  </UiModal>
</template>

<style scoped>
/* 标签栏铺满整行、各项之间留间距；右栏滚动时吸顶，长列表里也能随时切换 */
.detail-tabs {
  position: sticky;
  top: 0;
  z-index: 2;
  display: flex;
  gap: var(--space-1);
  width: 100%;
  overflow-x: auto;
  scrollbar-width: none;
  /* 用面板底色把吸顶时从下面滚过的内容盖住 */
  box-shadow: 0 var(--space-item) 0 var(--space-item) var(--bg-elevated);
}
.detail-tabs::-webkit-scrollbar { display: none; }
.detail-tabs :deep(.ui-button) {
  flex: 1 0 auto;
}
.detail-overview > p {
  color: var(--text-secondary);
  font-size: var(--text-base);
}
.detail-metadata {
  display: grid;
  grid-template-columns: repeat(3, minmax(0, 1fr));
  gap: var(--space-2);
}
.detail-metadata div {
  padding: var(--space-3);
  border-radius: var(--radius-icon);
  background: var(--bg-input);
}
.detail-metadata dt {
  color: var(--text-tertiary);
  font-size: var(--text-xs);
}
.detail-metadata dd {
  margin-top: var(--space-1);
  font-size: var(--text-sm);
  overflow-wrap: anywhere;
}
.metadata-path {
  grid-column: span 2;
}
.content-types {
  display: flex;
  flex-wrap: wrap;
  gap: var(--space-2);
  margin-top: var(--space-3);
}
.missing-notice {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--space-2);
  padding: var(--space-3);
  margin-top: var(--space-4);
  border-radius: var(--radius-icon);
  background: var(--color-warning-subtle);
  color: var(--color-warning);
  font-size: var(--text-base);
}
.detail-contents {
  display: flex;
  flex-direction: column;
  gap: var(--space-stack);
  min-width: 0;
}
.detail-hint {
  color: var(--text-tertiary);
  font-size: var(--text-sm);
}
.detail-hint.is-error { color: var(--color-error); }
.content-list {
  display: flex;
  flex-direction: column;
  margin: 0;
  padding: 0;
  list-style: none;
}
.content-row {
  display: flex;
  align-items: center;
  gap: var(--space-stack);
  padding: var(--space-item) 0;
  min-width: 0;
}
.content-row + .content-row {
  border-top: 1px solid var(--border-subtle);
}
.content-dot {
  flex-shrink: 0;
  width: 6px;
  height: 6px;
  border-radius: var(--radius-full);
}
.content-name {
  flex: 1;
  min-width: 0;
  display: flex;
  flex-direction: column;
  gap: 2px;
}
.content-name strong,
.content-name small {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.content-name strong {
  font-size: var(--text-sm);
  font-weight: var(--font-semibold);
}
.content-name small {
  color: var(--text-tertiary);
  font-family: var(--font-mono);
  font-size: var(--text-xs);
}
.content-size {
  flex-shrink: 0;
  color: var(--text-secondary);
  font-size: var(--text-xs);
}
.contents-more {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--space-item);
  color: var(--text-tertiary);
  font-size: var(--text-sm);
}
.unavailable-dependency { display: flex; flex-direction: column; align-items: flex-start; gap: var(--space-stack); color: var(--color-warning); }
.detail-unavailable { color: var(--text-tertiary); font-size: var(--text-sm); }
.delete-scope {
  margin-top: var(--space-3);
  font-size: var(--text-base);
  color: var(--text-secondary);
}
</style>
