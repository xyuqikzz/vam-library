<script setup lang="ts">
import PackageCard from '@/components/packages/PackageCard.vue'
import PackageDetailPanel from '@/components/packages/PackageDetailPanel.vue'
import PackageDetailModal from '@/components/packages/PackageDetailModal.vue'
import PackageActions from '@/components/packages/PackageActions.vue'
import UiButton from '@/components/ui/UiButton.vue'
import UiChip from '@/components/ui/UiChip.vue'
import UiSegmented from '@/components/ui/UiSegmented.vue'
import UiMenu, { type MenuItem } from '@/components/ui/UiMenu.vue'
import UiModal from '@/components/ui/UiModal.vue'
import UiCheckbox from '@/components/ui/UiCheckbox.vue'
import { resourceColor } from '@/utils/resourcePresentation'

import { parseHubFiles, parsePackageId, selectHubFile, type HubFile, type HubFileCandidate } from '@/utils/hubFiles'
import { formatSize } from '@/utils/bytes'
import { ref, watch, computed, onMounted, onUnmounted, nextTick, useSlots } from 'vue'
import { useI18n } from 'vue-i18n'
import { useRouter } from 'vue-router'
import { invoke } from '@tauri-apps/api/core'
import EmptyState from '@/components/common/EmptyState.vue'
import ShareModal from '@/components/ShareModal.vue'
import PackageSceneExtractor from '@/components/PackageSceneExtractor.vue'
import { useNotification } from '@/composables/useNotification'
import { useDownloadStore } from '@/stores/download'
import type { PackageDisplayItem, PackageImageEntry } from '@/types/package'

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

interface MissingDependency {
  package_id: string
  depends_on_id: string
  required_version: string
  dependent_package: string
  status: string
  installed_version: number | null
}

type DependencyPanelKey = 'direct' | 'sub' | 'reverse'

interface DependencyRow {
  id: string
  status: 'owned' | 'missing' | 'lower-version' | 'referenced'
  statusText: string
  tier?: number
}

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

const props = withDefaults(
  defineProps<{
    packages: PackageDisplayItem[]
    viewMode: 'large-card' | 'small-card' | 'list'
    resultCountLabel: string
    emptyTitle: string
    emptyDescription: string
    emptyIcon?: string
    showQuickDelete?: boolean
    hideControls?: boolean
    missingCounts?: Record<string, number>
    organizing?: boolean
  }>(),
  {
    emptyIcon: 'M12 2L3 7V17L12 22L21 17V7L12 2ZM12 12L21 7M12 12V22M12 12L3 7',
    showQuickDelete: false,
  },
)

const emit = defineEmits<{
  'update:viewMode': [mode: 'large-card' | 'small-card' | 'list']
  'packageDeleted': [result: QuickDeleteResult]
}>()

const { t } = useI18n()
const router = useRouter()
const notify = useNotification()
const downloadStore = useDownloadStore()
const slots = useSlots()

function getVarFileName(pkg: { id: string; file_path?: string } | null): string {
  if (!pkg) return ''
  if (pkg.file_path) {
    const parts = pkg.file_path.split(/[/\\]/)
    const last = parts[parts.length - 1]
    if (last && last.endsWith('.var')) return last
  }
  return pkg.id ? `${pkg.id}.var` : ''
}

const shareModalVisible = ref(false)
function openShareModal() {
  shareModalVisible.value = true
}

async function openPackageFolder() {
  if (!selectedPackage.value) return
  try {
    await invoke('open_package_in_explorer', { filePath: selectedPackage.value.file_path })
  } catch (err) {
    notify.error('无法打开文件位置: ' + String(err))
  }
}

async function copyPackageName() {
  if (!selectedPackage.value) return
  const fullName = selectedPackage.value.id
  try {
    await navigator.clipboard.writeText(fullName)
    notify.success(t('common.copySuccess'))
  } catch (err) {
    notify.error(t('common.error') + ': ' + String(err))
  }
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
    selectedPackage.value = null
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

// ── 分页 ─────────────────────────────────────────────────────
const pageSizes = [50, 100, 200, 400]
const currentPage = ref(1)
const pageSize = ref(100)

watch(
  () => [props.packages.length, props.viewMode],
  () => { currentPage.value = 1 },
)

const totalPages = computed(() => Math.max(1, Math.ceil(props.packages.length / pageSize.value)))
const paginatedPackages = computed(() => {
  const start = (currentPage.value - 1) * pageSize.value
  return props.packages.slice(start, start + pageSize.value)
})
const paginationInfo = computed(() => {
  const start = (currentPage.value - 1) * pageSize.value + 1
  const end = Math.min(start + pageSize.value - 1, props.packages.length)
  return `${start}-${end} / ${props.packages.length}`
})

// ── 缩略图缓存（LRU，最多 50 张） ────────────────────────────────
const MAX_THUMB_CACHE = 50
const thumbnails = ref<Record<string, string>>({})
const thumbErrors = ref<Set<string>>(new Set())

// ── IntersectionObserver 懒加载缩略图 ───────────────────────────
let observer: IntersectionObserver | null = null
const observedEls = new Map<Element, { id: string }>()

function setupObserver() {
  if (observer) observer.disconnect()
  if (slots.browser) return
  observer = new IntersectionObserver(
    (entries) => {
      for (const entry of entries) {
        if (entry.isIntersecting) {
          const info = observedEls.get(entry.target)
          if (info) {
            observedEls.delete(entry.target)
            observer!.unobserve(entry.target)
            loadThumbnail(info.id)
          }
        }
      }
    },
    { rootMargin: '200px' },
  )
}

function observeElement(el: Element, packageId: string) {
  if (!observer) return
  if (thumbnails.value[packageId] || thumbErrors.value.has(packageId)) return
  observer.observe(el)
  observedEls.set(el, { id: packageId })
}

async function loadThumbnail(packageId: string) {
  if (thumbnails.value[packageId] || thumbErrors.value.has(packageId)) return
  try {
    const result = await invoke<string | null>('get_package_thumbnail', { packageId })
    if (result) {
      const keys = Object.keys(thumbnails.value)
      if (keys.length >= MAX_THUMB_CACHE) delete thumbnails.value[keys[0]]
      thumbnails.value[packageId] = result
    }
  } catch { /* 静默失败 */ }
}

function onThumbError(packageId: string) {
  thumbErrors.value.add(packageId)
  delete thumbnails.value[packageId]
}

// 数据/视图/分页变化时重建 observer
watch(
  [() => props.packages, () => props.viewMode, () => currentPage.value],
  () => {
    const currentIds = new Set(props.packages.map((p) => p.id))
    for (const id of Object.keys(thumbnails.value)) {
      if (!currentIds.has(id)) delete thumbnails.value[id]
    }
    for (const id of thumbErrors.value) {
      if (!currentIds.has(id)) thumbErrors.value.delete(id)
    }
    observer?.disconnect()
    observedEls.clear()
    setupObserver()
    nextTick(() => bindObserverToCards())
  },
)

function bindObserverToCards() {
  const container = document.querySelector('.resource-content-scrollable')
  if (!container || !observer) return
  let selector: string
  if (props.viewMode === 'large-card') selector = '.large-card-thumb'
  else if (props.viewMode === 'small-card') selector = '.small-card-thumb'
  else selector = '.list-row'
  const elements = container.querySelectorAll(selector)
  elements.forEach((el, index) => {
    const pkg = paginatedPackages.value[index]
    if (pkg) observeElement(el, pkg.id)
  })
}

onMounted(() => {
  setupObserver()
  nextTick(() => bindObserverToCards())
})

onUnmounted(() => { observer?.disconnect() })

// ── 选中与详情 ────────────────────────────────────────────────
const selectedPackage = ref<PackageDisplayItem | null>(null)
const extractingCharacter = ref(false)
const selectedPackageImages = ref<PackageImageEntry[]>([])
const imageListLoading = ref(false)
const packageImageUrls = ref<Record<string, string>>({})
const previewImage = ref<PackageImageEntry | null>(null)
const directDependencyRelations = ref<DependencyRelation[]>([])
const subDependencyRelations = ref<DependencyRelation[]>([])
const reverseDependencies = ref<DependencyNode[]>([])
const missingDependencies = ref<MissingDependency[]>([])
const dependencyRelationsLoading = ref(false)
const dependencyRelationsLoaded = ref(false)
const dependencyLoadError = ref('')
const missingDependenciesLoading = ref(false)
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
const detailThumb = computed(() => {
  if (!selectedPackage.value) return null
  return thumbnails.value[selectedPackage.value.id] || null
})

const showCompleteDependencyButton = computed(() => (
  Boolean(selectedPackage.value)
  && !missingDependenciesLoading.value
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

function selectPackage(pkg: PackageDisplayItem) {
  if (extractingCharacter.value) return
  selectedPackage.value = pkg
  previewImage.value = null
  imagesExpanded.value = false
  resetDependencyRelations()
  loadPackageImages(pkg.id)
  loadPackageMissingDependencies(pkg.id)
  loadDependencyRelations()
  if (!thumbnails.value[pkg.id] && !thumbErrors.value.has(pkg.id)) {
    loadThumbnail(pkg.id)
  }
}

function resetDependencyRelations() {
  directDependencyRelations.value = []
  subDependencyRelations.value = []
  reverseDependencies.value = []
  missingDependencies.value = []
  dependencyRelationsLoading.value = false
  dependencyRelationsLoaded.value = false
  dependencyLoadError.value = ''
  missingDependenciesLoading.value = false
  dependencyCompletionLoading.value = false
  dependencyPanelExpanded.value = {
    direct: false,
    sub: false,
    reverse: false,
  }
}

async function loadPackageMissingDependencies(packageId: string) {
  if (!selectedPackage.value || selectedPackage.value.id !== packageId) return
  if (selectedPackage.value.dependency_count <= 0) {
    missingDependencies.value = []
    return
  }

  missingDependenciesLoading.value = true
  try {
    const allMissing = await invoke<MissingDependency[]>('find_missing_dependencies')
    if (selectedPackage.value?.id === packageId) {
      missingDependencies.value = allMissing.filter((dep) => dep.package_id === packageId)
    }
  } catch {
    if (selectedPackage.value?.id === packageId) {
      missingDependencies.value = []
    }
  } finally {
    if (selectedPackage.value?.id === packageId) {
      missingDependenciesLoading.value = false
    }
  }
}

async function completeSelectedPackageDependencies() {
  const packageId = selectedPackage.value?.id
  if (!packageId || dependencyCompletionLoading.value) return

  dependencyCompletionLoading.value = true
  try {
    if (selectedPackage.value?.id !== packageId) return
    await loadDependencyRelations(true)
    if (selectedPackage.value?.id !== packageId) return

    const targetIncomplete = [...directDependencyRelations.value, ...subDependencyRelations.value]
      .filter((dep) => dep.status !== 'satisfied')
    if (targetIncomplete.length === 0) {
      notify.success('该资源没有缺失依赖')
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
      notify.warning(noSourceCount > 0 ? '缺失依赖未找到可下载来源' : '没有可加入下载队列的依赖')
      return
    }

    const results = await downloadStore.addItems(items)
    const skippedCount = Array.isArray(results)
      ? results.filter((result) => result && typeof result === 'object' && 'Skipped' in result).length
      : 0
    const addedCount = Math.max(items.length - skippedCount, 0)

    if (addedCount > 0) notify.success(`已加入 ${addedCount} 个依赖下载任务`)
    if (skippedCount > 0) notify.info(`已跳过 ${skippedCount} 个本地已存在的任务`)
    if (noSourceCount > 0) notify.warning(`${noSourceCount} 个缺失依赖未找到下载源`)

    router.push('/download')
  } catch (err) {
    notify.error(`补齐依赖失败: ${String(err)}`)
  } finally {
    dependencyCompletionLoading.value = false
  }
}

function dependencyPanelCount(key: DependencyPanelKey): number {
  if (!selectedPackage.value) return 0
  if (key === 'direct') return selectedPackage.value.dependency_count
  if (key === 'sub') return subDependencyRows.value.length
  return selectedPackage.value.dependents_count
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
    loadDependencyRelations()
  }
}

async function loadDependencyRelations(force = false) {
  const packageId = selectedPackage.value?.id
  if (!packageId || (!force && dependencyRelationsLoaded.value) || dependencyRelationsLoading.value) return

  dependencyRelationsLoading.value = true
  dependencyLoadError.value = ''
  try {
    const [relations, reverse, allMissing] = await Promise.all([
      invoke<DependencyRelationsResult>('get_package_dependency_relations', { packageId }),
      invoke<DependencyNode[]>('get_reverse_dependencies', { packageId }),
      invoke<MissingDependency[]>('find_missing_dependencies'),
    ])

    if (selectedPackage.value?.id !== packageId) return
    directDependencyRelations.value = relations.direct || []
    subDependencyRelations.value = relations.sub || []
    reverseDependencies.value = reverse
    const dependencyPackageIds = new Set([
      packageId,
      ...directDependencyRelations.value
        .filter((dep) => dep.status === 'satisfied')
        .map((dep) => dep.id),
      ...subDependencyRelations.value
        .filter((dep) => dep.status === 'satisfied')
        .map((dep) => dep.id),
    ].map((id) => id.toLowerCase()))
    missingDependencies.value = allMissing.filter((dep) => dependencyPackageIds.has(dep.package_id.toLowerCase()))
    dependencyRelationsLoaded.value = true
  } catch (err) {
    if (selectedPackage.value?.id === packageId) {
      dependencyLoadError.value = `加载失败: ${String(err)}`
    }
  } finally {
    if (selectedPackage.value?.id === packageId) {
      dependencyRelationsLoading.value = false
    }
  }
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
  imageListLoading.value = true
  selectedPackageImages.value = []
  packageImageUrls.value = {}
  try {
    const images = await invoke<PackageImageEntry[]>('list_package_images', { packageId })
    if (selectedPackage.value?.id === packageId) {
      selectedPackageImages.value = images
      loadVisiblePackageImages(packageId)
    }
  } catch {
    if (selectedPackage.value?.id === packageId) {
      selectedPackageImages.value = []
    }
  } finally {
    if (selectedPackage.value?.id === packageId) {
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
  if (packageImageUrls.value[imagePath]) return
  try {
    const result = await invoke<string | null>('get_package_image', { packageId, imagePath })
    if (result && selectedPackage.value?.id === packageId) {
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

function closeImagePreview() {
  previewImage.value = null
}

// ── 工具函数 ────────────────────────────────────────────────

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

const detailsExpanded = ref(false)
const detailTab = ref('overview')
const sceneActions = ref<InstanceType<typeof PackageSceneExtractor> | null>(null)
const queuedAction = ref<{ id: string; action: 'launch' | 'extract' } | null>(null)
const cardMenuOpen = ref(false)
const cardMenuAnchor = ref<HTMLElement | null>(null)
const cardMenuPoint = ref<{ x?: number; y?: number }>({})
const viewOptions = computed(() => [
 { value: 'large-card', label: t('packages.largeCardView'), icon: 'grid' as const },
 { value: 'small-card', label: t('packages.smallCardView'), icon: 'landscape' as const },
 { value: 'list', label: t('packages.listView'), icon: 'list' as const },
])
const detailTabs = computed(() => [{ value: 'overview', label: t('ui.overview') }, ...dependencyPanels.value.map(panel => ({ value: panel.key, label: panel.label + ' ' + dependencyPanelCount(panel.key) })), { value: 'contents', label: t('ui.contents') + ' ' + (selectedPackage.value?.content_count || 0) }])
const cardMenuItems = computed<MenuItem[]>(() => [
 { value: 'launch', label: t('gameContent.launchScene'), icon: 'play', shortcut: 'Enter', disabled: !selectedPackage.value?.resource_types.includes('scene') || extractingCharacter.value },
 { value: 'open', label: t('packages.openLocation'), icon: 'folder', shortcut: 'Ctrl O' },
 { value: 'copy', label: t('ui.copyId'), icon: 'copy', shortcut: 'Ctrl C' },
 { value: 'share', label: t('share.btn'), icon: 'share' },
 { value: 'extract', label: t('gameContent.extractPreset'), icon: 'user', disabled: !selectedPackage.value?.resource_types.includes('scene') || extractingCharacter.value },
 { value: 'complete', label: t('ui.allDependencies'), icon: 'link', disabled: !showCompleteDependencyButton.value || dependencyCompletionLoading.value },
 ...(props.showQuickDelete ? [{ value: 'separator', label: '', separator: true }, { value: 'delete', label: t('ui.toTrash'), icon: 'trash' as const, shortcut: 'Del', danger: true, disabled: deleteLoading.value || extractingCharacter.value }] : []),
])
function openDetails(pkg: PackageDisplayItem) { if (extractingCharacter.value) return; if (selectedPackage.value?.id !== pkg.id) selectPackage(pkg); if (!imagesExpanded.value) toggleImageExpanded(); detailsExpanded.value = true; detailTab.value = 'overview' }
function openCardMenu(pkg: PackageDisplayItem, event: MouseEvent) { if (extractingCharacter.value) return; if (selectedPackage.value?.id !== pkg.id) selectPackage(pkg); cardMenuAnchor.value = event.currentTarget instanceof HTMLElement ? event.currentTarget : null; cardMenuPoint.value = event.type === 'contextmenu' ? { x: event.clientX, y: event.clientY } : {}; cardMenuOpen.value = true }
function queueSceneAction(pkg: PackageDisplayItem, action: 'launch' | 'extract') { if (extractingCharacter.value) return; if (selectedPackage.value?.id !== pkg.id) selectPackage(pkg); queuedAction.value = { id: pkg.id, action }; if (action === 'extract') detailsExpanded.value = true; runQueuedAction() }
function runQueuedAction() { const pending = queuedAction.value; if (!pending || selectedPackage.value?.id !== pending.id || !sceneActions.value?.ready) return; queuedAction.value = null; if (pending.action === 'launch') void sceneActions.value.launch(); else sceneActions.value.extract() }
watch(() => sceneActions.value?.ready, runQueuedAction)
watch(selectedPackage, pkg => { if (!pkg) { detailsExpanded.value = false; queuedAction.value = null } else if (queuedAction.value?.id !== pkg.id) queuedAction.value = null })
function handleCardAction(item: MenuItem) { const pkg = selectedPackage.value; if (!pkg) return; if (item.value === 'launch' || item.value === 'extract') queueSceneAction(pkg, item.value); else if (item.value === 'open') void openPackageFolder(); else if (item.value === 'copy') void copyPackageName(); else if (item.value === 'share') openShareModal(); else if (item.value === 'complete') void completeSelectedPackageDependencies(); else if (item.value === 'delete') quickDeleteSelected(true) }
function packageShortcut(event: KeyboardEvent, pkg: PackageDisplayItem) {
  if (event.defaultPrevented || extractingCharacter.value || (event.target as HTMLElement).closest('input, textarea, select, [contenteditable="true"]')) return
  const action = event.ctrlKey || event.metaKey ? ({ c: 'copy', o: 'open' } as Record<string, string>)[event.key.toLowerCase()] : event.key === 'Delete' ? 'delete' : undefined
  if (!action) return
  const item = cardMenuItems.value.find(item => item.value === action)
  if (!item || item.disabled) return
  event.preventDefault()
  event.stopPropagation()
  if (selectedPackage.value?.id !== pkg.id) selectPackage(pkg)
  handleCardAction(item)
}
function setDetailTab(value: string) { detailTab.value = value; if (['direct','sub','reverse'].includes(value) && !dependencyPanelExpanded.value[value as DependencyPanelKey]) toggleDependencyPanel(value as DependencyPanelKey) }
function closeDetails() { if (extractingCharacter.value) return; closeImagePreview(); selectedPackage.value = null }
function viewChanged(value: string) { if (value === 'large-card' || value === 'small-card' || value === 'list') emit('update:viewMode', value) }
defineExpose({ selectPackage, openDetails })
</script>
<template>
  <div class="resource-display">
    <div class="resource-browser">
      <slot
        name="browser"
        :select-package="selectPackage"
        :open-details="openDetails"
        :launch-package="(pkg: PackageDisplayItem) => queueSceneAction(pkg, 'launch')"
      >
        <div v-if="!hideControls" class="view-controls">
          <span class="result-count">{{ resultCountLabel }}</span>
          <div class="controls-right">
            <slot name="actions" />
            <UiSegmented
              :model-value="viewMode"
              :options="viewOptions"
              :label="t('ui.view')"
              icon-only
              @update:model-value="viewChanged"
            />
          </div>
        </div>
        <div v-if="packages.length" class="resource-content-area">
          <div class="resource-content-scrollable">
            <div
              v-if="viewMode !== 'list'"
              class="package-grid"
              :class="{ 'landscape-grid': viewMode === 'small-card' }"
            >
              <div v-for="pkg in paginatedPackages" :key="pkg.id" class="package-cell">
                <PackageCard
                  :item="pkg"
                  @keydown="packageShortcut($event, pkg)"
                  :image="thumbnails[pkg.id]"
                  :landscape="viewMode === 'small-card'"
                  :selected="selectedPackage?.id === pkg.id"
                  :missing="missingCounts?.[pkg.id]"
                  :can-launch="pkg.resource_types.includes('scene')"
                  @select="selectPackage(pkg)"
                  @details="openDetails(pkg)"
                  @launch="queueSceneAction(pkg, 'launch')"
                  @more="openCardMenu(pkg, $event)"
                  @image-error="onThumbError(pkg.id)"
                />
              </div>
            </div>
            <div v-else class="resource-list" role="table">
              <div class="list-header" role="row">
                <span>{{ t('packages.package') }}</span>
                <span>{{ t('packages.type') }}</span>
                <span>{{ t('packages.size') }}</span>
                <span>{{ t('packages.deps') }}</span>
                <span>{{ t('ui.status') }}</span>
                <span>{{ t('packages.importTime') }}</span>
              </div>
              <div
                v-for="pkg in paginatedPackages"
                :key="pkg.id"
                class="list-row"
                :class="{ selected: selectedPackage?.id === pkg.id }"
                role="row"
                @contextmenu.prevent="openCardMenu(pkg, $event)"
              >
                <div class="list-package" role="cell">
                  <button
                    type="button"
                    class="list-open"
                    @keydown="packageShortcut($event, pkg)"
                    @click="selectPackage(pkg)"
                    @dblclick="openDetails(pkg)"
                    @keydown.space.prevent="openDetails(pkg)"
                  >
                    <img
                      v-if="thumbnails[pkg.id]"
                      data-resource-preview
                      :src="thumbnails[pkg.id]"
                      :alt="getVarFileName(pkg)"
                      @error="onThumbError(pkg.id)"
                    />
                    <span v-else class="list-placeholder" />
                    <span>
                      <strong>{{ pkg.name }}</strong>
                      <small>{{ pkg.creator }} · v{{ pkg.version }}</small>
                    </span>
                  </button>
                </div>
                <span
                  role="cell"
                  class="list-type"
                  :style="{ color: resourceColor(pkg.resource_types[0]) }"
                >
                  {{ resourceTypeLabel(pkg.resource_types[0] || 'other') }}
                </span>
                <span role="cell">{{ formatSize(pkg.size_bytes) }}</span>
                <span role="cell">{{ pkg.dependency_count }}</span>
                <span role="cell" :class="{ warning: missingCounts?.[pkg.id] }">
                  {{ missingCounts?.[pkg.id] ? t('ui.missing', { count: missingCounts[pkg.id] }) : t('ui.installed') }}
                </span>
                <span role="cell">{{ formatTime(pkg.scan_time) }}</span>
              </div>
            </div>
          </div>
        </div>
        <div v-if="packages.length" class="pagination-bar">
          <span class="pagination-info">{{ paginationInfo }}</span>
          <div class="pagination-controls">
            <select
              v-model.number="pageSize"
              :aria-label="t('ui.perPageLabel')"
              class="page-size-select"
              @change="currentPage = 1"
            >
              <option v-for="size in pageSizes" :key="size" :value="size">
                {{ t('ui.perPage', { count: size }) }}
              </option>
            </select>
            <UiButton
              size="sm"
              variant="ghost"
              icon="chevrons-left"
              icon-only
              :aria-label="t('ui.firstPage')"
              :disabled="currentPage <= 1"
              @click="currentPage = 1"
            />
            <UiButton
              size="sm"
              variant="ghost"
              icon="chevron-left"
              icon-only
              :aria-label="t('gameContent.previous')"
              :disabled="currentPage <= 1"
              @click="currentPage--"
            />
            <span>{{ currentPage }} / {{ totalPages }}</span>
            <UiButton
              size="sm"
              variant="ghost"
              icon="chevron-right"
              icon-only
              :aria-label="t('gameContent.next')"
              :disabled="currentPage >= totalPages"
              @click="currentPage++"
            />
            <UiButton
              size="sm"
              variant="ghost"
              icon="chevrons-right"
              icon-only
              :aria-label="t('ui.lastPage')"
              :disabled="currentPage >= totalPages"
              @click="currentPage = totalPages"
            />
          </div>
        </div>
        <div v-else class="empty-wrapper">
          <EmptyState :icon="emptyIcon" :title="emptyTitle" :description="emptyDescription">
            <template v-if="$slots['empty-action']" #action><slot name="empty-action" /></template>
          </EmptyState>
        </div>
      </slot>
    </div>
    <PackageDetailModal
      v-if="selectedPackage"
      :open="detailsExpanded"
      :title="selectedPackage.name"
      :images="selectedPackageImages"
      :image-urls="packageImageUrls"
      :fallback-image="detailThumb"
      :busy="extractingCharacter"
      @close="detailsExpanded = false"
      @image="openImagePreview"
    >
      <PackageDetailPanel
        @keydown="packageShortcut($event, selectedPackage)"
        :item="selectedPackage"
        :image="previewImageUrl || detailThumb"
        :active-image="previewImage?.path"
        :images="selectedPackageImages"
        :image-urls="packageImageUrls"
        :expanded="detailsExpanded"
        :busy="extractingCharacter"
        :dependencies="directDependencyRows"
        :missing="incompleteDependencyCount"
        :loading="dependencyRelationsLoading"
        :error="dependencyLoadError"
        @close="closeDetails"
        @image="openImagePreview"
        @details="openDetails(selectedPackage)"
        @complete="completeSelectedPackageDependencies"
      >
        <template #actions>
          <PackageSceneExtractor
            ref="sceneActions"
            :key="selectedPackage.id + ':' + selectedPackage.file_path + ':' + selectedPackage.scan_time"
            :package-id="selectedPackage.id"
            @busy-change="extractingCharacter = $event"
          >
            <template #actions="{ launch, extract, disabled, expanded, launching, panel }">
              <PackageActions
                :disabled="disabled"
                :launching="launching"
                :expanded="expanded"
                :panel="panel"
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
        <template v-if="detailsExpanded" #tabs>
          <UiSegmented
            :model-value="detailTab"
            :options="detailTabs"
            :label="t('ui.overview')"
            class="detail-tabs"
            @update:model-value="setDetailTab"
          />
        </template>
        <template v-if="detailsExpanded" #default>
          <div v-if="detailTab === 'overview'" class="detail-overview">
            <p>{{ t('ui.overviewHelp') }}</p>
            <dl class="detail-metadata">
              <div>
                <dt>{{ t('packages.version') }}</dt>
                <dd>v{{ selectedPackage.version }}</dd>
              </div>
              <div>
                <dt>{{ t('ui.files') }}</dt>
                <dd>{{ selectedPackage.content_count }}</dd>
              </div>
              <div>
                <dt>{{ t('packages.size') }}</dt>
                <dd>{{ formatSize(selectedPackage.size_bytes) }}</dd>
              </div>
              <div>
                <dt>{{ t('ui.libraryDate') }}</dt>
                <dd>{{ formatTime(selectedPackage.scan_time) }}</dd>
              </div>
              <div class="metadata-path">
                <dt>{{ t('ui.directory') }}</dt>
                <dd>{{ selectedPackage.file_path }}</dd>
              </div>
            </dl>
            <div class="content-types">
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
              <span>{{ t('ui.missing', { count: incompleteDependencyCount }) }}</span>
              <UiButton
                size="sm"
                :loading="dependencyCompletionLoading"
                @click="completeSelectedPackageDependencies"
              >
                {{ t('ui.allDependencies') }}
              </UiButton>
            </div>
          </div>
          <div v-else-if="detailTab === 'contents'">
            <p>{{ t('packages.files', { count: selectedPackage.content_count }) }}</p>
            <div class="content-types">
              <UiChip
                v-for="type in selectedPackage.resource_types"
                :key="type"
                :color="resourceColor(type)"
                disabled
              >
                {{ resourceTypeLabel(type) }}
              </UiChip>
            </div>
          </div>
          <div v-else class="expanded-dependencies">
            <p v-if="dependencyRelationsLoading" role="status">{{ t('common.loading') }}</p>
            <p v-else-if="dependencyLoadError" role="alert">{{ dependencyLoadError }}</p>
            <div
              v-for="dependency in dependencyPanelRows(detailTab as DependencyPanelKey)"
              :key="dependency.id"
              class="expanded-dependency"
            >
              <span>{{ dependency.id }}</span>
              <small>{{ dependency.statusText }}</small>
              <UiButton
                v-if="dependency.status === 'missing' || dependency.status === 'lower-version'"
                size="sm"
                :loading="dependencyCompletionLoading"
                @click="completeSelectedPackageDependencies"
              >
                {{ t('ui.complete') }}
              </UiButton>
            </div>
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
    <UiMenu
      v-model:open="cardMenuOpen"
      :anchor="cardMenuAnchor"
      :x="cardMenuPoint.x"
      :y="cardMenuPoint.y"
      :items="cardMenuItems"
      :label="t('ui.packageActions')"
      @select="handleCardAction"
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
  </div>
</template>
<style scoped>
.resource-display {
  display: flex;
  gap: 16px;
  height: 100%;
  min-width: 0;
  min-height: 0;
  position: relative;
  overflow: hidden;
}
.resource-browser {
  flex: 1;
  min-width: 0;
  min-height: 0;
  display: flex;
  flex-direction: column;
  gap: 14px;
}
.view-controls {
  display: flex;
  align-items: center;
  justify-content: space-between;
  flex-wrap: wrap;
  gap: var(--space-3);
}
.controls-right {
  display: flex;
  gap: var(--space-2);
  align-items: center;
  flex-wrap: wrap;
}
.result-count,
.pagination-info {
  font-size: var(--text-sm);
  color: var(--text-tertiary);
}
.resource-content-area {
  flex: 1;
  min-height: 0;
  display: flex;
}
.resource-content-scrollable {
  flex: 1;
  overflow: auto;
  min-height: 0;
  min-width: 0;
  padding: 4px;
}
.package-grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(168px, 1fr));
  gap: 14px;
}
.landscape-grid {
  grid-template-columns: repeat(auto-fill, minmax(220px, 1fr));
}
.package-cell {
  min-width: 0;
  position: relative;
}
.cleanup-unprotect {
  position: absolute;
  bottom: 58px;
  left: 8px;
  z-index: 4;
  background: var(--bg-on-image);
}
.list-header,
.list-row {
  display: grid;
  grid-template-columns: minmax(200px, 1fr) 80px 80px 55px 95px 100px;
  align-items: center;
  gap: 12px;
  min-width: 680px;
}
.list-header {
  height: 36px;
  font-size: var(--text-xs);
  color: var(--text-tertiary);
}
.list-row {
  min-height: 52px;
  border-radius: var(--radius-sm);
  font-size: var(--text-sm);
  padding: 0 8px;
}
.list-row + .list-row {
  border-top: 1px solid var(--border-subtle);
}
.list-row:hover {
  background: var(--bg-hover);
}
.list-row.selected {
  background: var(--accent-subtle);
  box-shadow: inset 3px 0 var(--accent-primary);
}
.list-package,
.list-open {
  display: flex;
  align-items: center;
  min-width: 0;
  gap: 10px;
}
.list-open {
  width: 100%;
  text-align: left;
  border-radius: var(--radius-sm);
}
.list-open img,
.list-placeholder {
  width: 40px;
  height: 30px;
  flex-shrink: 0;
  object-fit: cover;
  border-radius: var(--radius-xs);
  background: var(--thumb-other);
}
.list-open > span:last-child {
  min-width: 0;
}
.list-open strong,
.list-open small {
  display: block;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}
.list-open strong {
  font-size: var(--text-base);
}
.list-open small {
  color: var(--text-tertiary);
  font-size: var(--text-xs);
}
.warning {
  color: var(--color-warning);
}
.pagination-bar {
  display: flex;
  justify-content: space-between;
  align-items: center;
  flex-wrap: wrap;
  gap: var(--space-2);
  padding: 4px;
}
.pagination-controls {
  display: flex;
  align-items: center;
  gap: 4px;
  font-size: var(--text-sm);
}
.page-size-select {
  min-width: 88px;
  border: none;
  border-radius: var(--radius-full);
}
.empty-wrapper {
  flex: 1;
  display: grid;
  place-items: center;
}
.detail-tabs {
  max-width: 100%;
  overflow-x: auto;
}
.detail-overview > p {
  color: var(--text-secondary);
  font-size: var(--text-base);
}
.detail-metadata {
  display: grid;
  grid-template-columns: repeat(3, minmax(0, 1fr));
  gap: 8px;
  margin-top: 14px;
}
.detail-metadata div {
  padding: 12px;
  border-radius: var(--radius-icon);
  background: var(--bg-input);
}
.detail-metadata dt {
  color: var(--text-tertiary);
  font-size: var(--text-xs);
}
.detail-metadata dd {
  margin-top: 4px;
  font-size: var(--text-sm);
  overflow-wrap: anywhere;
}
.metadata-path {
  grid-column: span 2;
}
.content-types {
  display: flex;
  flex-wrap: wrap;
  gap: 6px;
  margin-top: 12px;
}
.missing-notice {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--space-2);
  padding: 12px;
  margin-top: 14px;
  border-radius: var(--radius-icon);
  background: var(--color-warning-subtle);
  color: var(--color-warning);
  font-size: var(--text-base);
}
.expanded-dependency {
  display: flex;
  align-items: center;
  gap: 8px;
  min-height: 44px;
  font-size: var(--text-sm);
  border-bottom: 1px solid var(--border-subtle);
}
.expanded-dependency span {
  flex: 1;
  overflow-wrap: anywhere;
}
.expanded-dependency small {
  color: var(--text-tertiary);
}
.delete-scope {
  margin-top: var(--space-3);
  font-size: var(--text-base);
  color: var(--text-secondary);
}
@media (max-width: 760px) {
  .resource-display {
    flex-direction: column;
  }
}
</style>
