<template>
  <div class="on-demand-view animate-fadeIn">
    <header class="page-header">
      <div>
        <h1 class="page-title">{{ t('onDemand.title') }}</h1>
        <p class="page-subtitle">{{ t('onDemand.subtitle') }}</p>
      </div>
      <div class="status-pills">
        <span :class="['status-pill', vamRootPath ? 'ok' : 'warn']">
          {{ vamRootPath ? t('onDemand.vamReady') : t('onDemand.vamNotSet') }}
        </span>
        <span class="status-pill info">{{ t('onDemand.linkFirst') }}</span>
        <span class="status-pill safe">{{ t('onDemand.previewOnly') }}</span>
      </div>
    </header>

    <div class="workspace-grid">
      <aside class="plans-panel glass-panel">
        <div class="panel-heading">
          <span>{{ t('onDemand.planList') }}</span>
          <button class="icon-action" :title="t('onDemand.newPlan')" @click="createPlan">
            <svg width="15" height="15" viewBox="0 0 24 24" fill="none">
              <path d="M12 5v14M5 12h14" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" />
            </svg>
          </button>
        </div>

        <button
          v-for="plan in plans"
          :key="plan.id"
          :class="['plan-card', { active: activePlanId === plan.id }]"
          @click="selectPlan(plan.id)"
        >
          <div class="plan-name">{{ plan.name }}</div>
          <div class="plan-meta">{{ t('onDemand.planPackageCount', { count: plan.packageIds.length }) }}</div>
        </button>

        <div class="option-block">
          <label class="switch-row">
            <input v-model="includeDependencies" type="checkbox" />
            <span>{{ t('onDemand.includeDependencies') }}</span>
          </label>
          <label class="switch-row">
            <input v-model="showOnlyActivated" type="checkbox" />
            <span>{{ t('onDemand.onlySelected') }}</span>
          </label>
        </div>

        <button class="add-var-btn" @click="openPackagePicker">
          <svg width="15" height="15" viewBox="0 0 24 24" fill="none">
            <path d="M12 5v14M5 12h14" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" />
          </svg>
          <span>{{ t('onDemand.addVarPackages') }}</span>
        </button>

        <div class="strategy-card">
          <div class="strategy-title">{{ t('onDemand.recommendedStrategy') }}</div>
          <p>{{ t('onDemand.strategyDesc') }}</p>
          <button class="strategy-action" :disabled="operationRunning || !vamRootPath" @click="migrateExistingPackages">
            {{ operationRunning ? t('onDemand.running') : t('onDemand.migrateExisting') }}
          </button>
          <button class="strategy-action restore" :disabled="operationRunning || !vamRootPath" @click="restoreAllPackages">
            {{ operationRunning ? t('onDemand.running') : t('onDemand.restoreAll') }}
          </button>
          <div v-if="operationProgress && operationRunning" class="operation-progress">
            <div class="operation-progress-meta">
              <span>{{ operationProgress.completed + operationProgress.failed + operationProgress.skipped }} / {{ operationProgress.total }}</span>
              <span>{{ t('onDemand.remainingFiles', { count: operationRemainingFiles }) }}</span>
            </div>
            <div class="operation-progress-track">
              <div class="operation-progress-fill" :style="{ width: `${operationProgressPercent}%` }" />
            </div>
          </div>
        </div>
      </aside>

      <section class="package-panel glass-panel">
        <div class="launch-hero">
          <div class="launch-hero-icon">
            <svg width="28" height="28" viewBox="0 0 24 24" fill="none">
              <path d="M8 5v14l11-7L8 5Z" stroke="currentColor" stroke-width="1.5" stroke-linejoin="round" />
              <path d="M4 6v12" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" />
            </svg>
          </div>
          <div>
            <h2>{{ t('onDemand.launchSelectionTitle') }}</h2>
            <p>{{ t('onDemand.launchSelectionDesc') }}</p>
          </div>
        </div>

        <div v-if="loading" class="empty-state compact">
          <div class="loader" />
          <span>{{ t('common.loading') }}</span>
        </div>

        <div v-else-if="packages.length === 0" class="empty-state compact">
          <span>{{ t('onDemand.noPackages') }}</span>
          <p>{{ t('onDemand.noPackagesDesc') }}</p>
        </div>

        <div v-else class="overview-grid">
          <button class="overview-action" @click="openPackagePicker">
            <strong>{{ packages.length }}</strong>
            <span>{{ t('onDemand.availablePackages') }}</span>
          </button>
          <div>
            <strong>{{ selectedPrimaryPackages.length }}</strong>
            <span>{{ t('onDemand.primaryPackages') }}</span>
          </div>
          <div>
            <strong>{{ selectedDependencyPackages.length }}</strong>
            <span>{{ t('onDemand.dependencyPackages') }}</span>
          </div>
          <div>
            <strong>{{ formatSize(totalSelectedSize) }}</strong>
            <span>{{ t('onDemand.totalSize') }}</span>
          </div>
        </div>

        <div class="selected-preview">
          <div class="section-title">{{ t('onDemand.currentPrimaryPackages') }}</div>
          <div v-if="selectedPrimaryPackages.length === 0" class="preview-empty">
            <span>{{ t('onDemand.selectHint') }}</span>
            <button class="ghost-btn" @click="openPackagePicker">{{ t('onDemand.addVarPackages') }}</button>
          </div>
          <div v-else class="preview-card-grid">
            <button
              v-for="pkg in selectedPrimaryPackages.slice(0, 8)"
              :key="pkg.id"
              class="selected-preview-card"
              @click="openPackagePicker"
            >
              <div class="preview-thumb">
                <img data-resource-preview v-if="thumbnails[pkg.id]" :src="thumbnails[pkg.id]" :alt="pkg.id + '.var'" />
                <span v-else>{{ t('packages.noThumbnail') }}</span>
              </div>
              <div class="preview-info">
                <strong>{{ pkg.id }}.var</strong>
                <span>{{ formatSize(pkg.size_bytes) }} · {{ t('packages.files', { count: pkg.content_count }) }}</span>
              </div>
            </button>
          </div>
          <div v-if="selectedPrimaryPackages.length > 8" class="muted">
            {{ t('onDemand.moreHidden', { count: selectedPrimaryPackages.length - 8 }) }}
          </div>
        </div>
      </section>

      <aside class="selection-panel glass-panel">
        <div class="panel-heading">
          <span>{{ t('onDemand.selectedPlan') }}</span>
          <span class="selected-count">{{ activePackageIds.size }}</span>
        </div>

        <div class="summary-grid">
          <div>
            <strong>{{ selectedPrimaryPackages.length }}</strong>
            <span>{{ t('onDemand.primaryPackages') }}</span>
          </div>
          <div>
            <strong>{{ selectedDependencyPackages.length }}</strong>
            <span>{{ t('onDemand.dependencyPackages') }}</span>
          </div>
          <div>
            <strong>{{ formatSize(totalSelectedSize) }}</strong>
            <span>{{ t('onDemand.totalSize') }}</span>
          </div>
        </div>

        <div class="path-preview">
          <div class="path-label">{{ t('onDemand.libraryPath') }}</div>
          <div class="path-value">{{ libraryPath }}</div>
          <div class="path-label">{{ t('onDemand.activePath') }}</div>
          <div class="path-value">{{ activePath }}</div>
        </div>

        <div class="selected-section">
          <div class="section-title">{{ t('onDemand.primaryPackages') }}</div>
          <div v-if="selectedPrimaryPackages.length === 0" class="muted">{{ t('onDemand.selectHint') }}</div>
          <div v-for="pkg in selectedPrimaryPackages" :key="pkg.id" class="selected-item">
            <span>{{ pkg.id }}.var</span>
            <button :title="t('common.delete')" @click="togglePrimary(pkg.id)">
              <svg width="13" height="13" viewBox="0 0 24 24" fill="none">
                <path d="M18 6L6 18M6 6l12 12" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" />
              </svg>
            </button>
          </div>
        </div>

        <div class="selected-section">
          <div class="section-title">{{ t('onDemand.dependencyPackages') }}</div>
          <div v-if="selectedDependencyPackages.length === 0" class="muted">{{ t('onDemand.noDependencySelected') }}</div>
          <div v-for="pkg in selectedDependencyPackages.slice(0, 12)" :key="pkg.id" class="selected-item dependency">
            <span>{{ pkg.id }}.var</span>
          </div>
          <div v-if="selectedDependencyPackages.length > 12" class="muted">
            {{ t('onDemand.moreHidden', { count: selectedDependencyPackages.length - 12 }) }}
          </div>
        </div>

        <div class="operation-preview">
          <div class="section-title">{{ t('onDemand.operationPreview') }}</div>
          <ol>
            <li>{{ t('onDemand.stepMoveToLibrary') }}</li>
            <li>{{ t('onDemand.stepClearLinks') }}</li>
            <li>{{ t('onDemand.stepCreateLinks') }}</li>
            <li>{{ t('onDemand.stepLaunchGame') }}</li>
          </ol>
        </div>

        <div v-if="operationMessage" :class="['operation-message', operationFailed ? 'error' : 'ok']">
          {{ operationMessage }}
        </div>

        <button
          class="execute-btn"
          :disabled="operationRunning || !vamRootPath || primaryIds.length === 0"
          @click="applyCurrentPlan"
        >
          {{ operationRunning ? t('onDemand.running') : t('onDemand.applyMapping') }}
        </button>
      </aside>
    </div>

    <Teleport to="body">
      <div v-if="pickerVisible" class="picker-overlay" @click.self="closePackagePicker">
        <div class="picker-modal">
          <header class="picker-header">
            <h2>{{ t('onDemand.varPickerTitle') }}</h2>
            <div class="picker-header-actions">
              <span>{{ t('onDemand.pickerSelectedCount', { count: tempActivePackageIds.size }) }}</span>
              <button class="ghost-btn" @click="closePackagePicker">{{ t('common.cancel') }}</button>
              <button class="save-picker-btn" @click="savePackagePicker">{{ t('onDemand.saveSelection') }}</button>
            </div>
          </header>

          <div class="picker-actions">
            <div class="search-box">
              <svg width="15" height="15" viewBox="0 0 24 24" fill="none">
                <circle cx="11" cy="11" r="7" stroke="currentColor" stroke-width="1.5" />
                <path d="M16.5 16.5L21 21" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" />
              </svg>
              <input v-model="localSearch" :placeholder="t('onDemand.searchPlaceholder')" />
            </div>
            <select v-model="selectedType" class="type-select">
              <option value="all">{{ t('packages.filterAll') }}</option>
              <option v-for="type in resourceTypes" :key="type" :value="type">{{ resourceTypeLabel(type) }}</option>
            </select>
            <select v-model="sortBy" class="type-select">
              <option value="created">{{ t('packages.sortByCreatedTime') }}</option>
              <option value="name">{{ t('packages.sortByName') }}</option>
              <option value="size">{{ t('packages.sortBySize') }}</option>
              <option value="deps">{{ t('packages.deps') }}</option>
            </select>
            <button class="ghost-btn" @click="showOnlyActivated = !showOnlyActivated">
              {{ showOnlyActivated ? t('onDemand.showAll') : t('onDemand.onlySelected') }}
            </button>
          </div>

          <div class="picker-bulk-actions">
            <button class="ghost-btn" @click="selectCurrentPage">{{ t('onDemand.selectPage') }}</button>
            <button class="ghost-btn" @click="invertCurrentPage">{{ t('onDemand.invertPage') }}</button>
            <button class="ghost-btn" @click="clearTempSelection">{{ t('common.clear') }}</button>
            <label class="picker-deps-toggle">
              <input v-model="includeDependencies" type="checkbox" />
              <span>{{ t('onDemand.withDeps') }}</span>
            </label>
          </div>

          <div class="picker-body">
            <aside class="picker-folder-tree">
              <div class="folder-tree-title">{{ t('onDemand.libraryCatalog') }}</div>
              <button
                v-for="folder in flatFolderNodes"
                :key="folder.path || '__root__'"
                :class="['folder-tree-item', { active: selectedFolder === folder.path }]"
                :style="{ paddingLeft: `${12 + folder.depth * 14}px` }"
                :title="folder.fullLabel"
                @click="selectedFolder = folder.path"
              >
                <svg width="13" height="13" viewBox="0 0 24 24" fill="none">
                  <path d="M3 6.5A2.5 2.5 0 0 1 5.5 4H9l2 2h7.5A2.5 2.5 0 0 1 21 8.5v9A2.5 2.5 0 0 1 18.5 20h-13A2.5 2.5 0 0 1 3 17.5v-11Z" stroke="currentColor" stroke-width="1.5" stroke-linejoin="round" />
                </svg>
                <span>{{ folder.label }}</span>
                <em>{{ folder.count }}</em>
              </button>
            </aside>

            <main class="picker-main">
              <div class="picker-viewbar">
                <div>
                  {{ t('onDemand.totalFiles', { count: filteredPackages.length }) }}
                </div>
                <div class="view-switch">
                  <button :class="{ active: pickerViewMode === 'grid' }" @click="pickerViewMode = 'grid'">
                    {{ t('onDemand.imageView') }}
                  </button>
                  <button :class="{ active: pickerViewMode === 'list' }" @click="pickerViewMode = 'list'">
                    {{ t('packages.listView') }}
                  </button>
                </div>
              </div>

              <div v-if="pickerViewMode === 'grid'" class="picker-grid">
                <button
                  v-for="pkg in pagedPickerPackages"
                  :key="pkg.id"
                  :class="['picker-card', { selected: tempActivePackageIds.has(pkg.id), dependency: tempDependencyIdSet.has(pkg.id), focused: focusedPackageId === pkg.id }]"
                  @click="focusPackage(pkg.id)"
                >
                  <span class="picker-check" @click.stop="toggleTempSelection(pkg.id)">
                    <svg v-if="tempActivePackageIds.has(pkg.id)" width="12" height="12" viewBox="0 0 16 16" fill="none">
                      <path d="M3 8l3 3 7-7" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" />
                    </svg>
                  </span>
                  <span class="picker-type" :class="`type-${pkg.resource_types[0] || 'other'}`">{{ resourceTypeLabel(pkg.resource_types[0] || 'other') }}</span>
                  <span class="picker-thumb">
                    <img data-resource-preview v-if="thumbnails[pkg.id]" :src="thumbnails[pkg.id]" :alt="pkg.id + '.var'" />
                    <span v-else>{{ t('packages.noThumbnail') }}</span>
                  </span>
                  <span class="picker-card-name" :title="pkg.id + '.var'">
                    {{ pkg.id }}.var
                  </span>
                  <span class="picker-card-meta">
                    {{ formatSize(pkg.size_bytes) }} · {{ t('packages.files', { count: pkg.content_count }) }} · {{ pkg.dependency_count }} {{ t('packages.deps') }}
                  </span>
                </button>
              </div>

              <div v-else class="picker-list">
                <button
                  v-for="pkg in pagedPickerPackages"
                  :key="pkg.id"
                  :class="['picker-list-row', { selected: tempActivePackageIds.has(pkg.id), dependency: tempDependencyIdSet.has(pkg.id), focused: focusedPackageId === pkg.id }]"
                  @click="focusPackage(pkg.id)"
                >
                  <span class="check-box" @click.stop="toggleTempSelection(pkg.id)">
                    <svg v-if="tempActivePackageIds.has(pkg.id)" width="12" height="12" viewBox="0 0 16 16" fill="none">
                      <path d="M3 8l3 3 7-7" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" />
                    </svg>
                  </span>
                  <span class="package-name" :title="pkg.id + '.var'">{{ pkg.id }}.var</span>
                  <span>{{ resourceTypeLabel(pkg.resource_types[0] || 'other') }}</span>
                  <span>{{ formatSize(pkg.size_bytes) }}</span>
                  <span>{{ t('packages.files', { count: pkg.content_count }) }}</span>
                  <span>{{ pkg.dependency_count }} / {{ pkg.dependents_count }}</span>
                </button>
              </div>
            </main>

            <aside class="picker-detail">
              <div class="detail-preview">
                <img data-resource-preview v-if="focusedPackage && thumbnails[focusedPackage.id]" :src="thumbnails[focusedPackage.id]" :alt="focusedPackage.id + '.var'" />
                <span v-else>{{ t('packages.noThumbnail') }}</span>
              </div>
              <template v-if="focusedPackage">
                <h3 :title="focusedPackage.id + '.var'">{{ focusedPackage.id }}.var</h3>
                <dl>
                  <dt>{{ t('packages.type') }}</dt>
                  <dd>{{ focusedPackage.resource_types.map(resourceTypeLabel).join(', ') }}</dd>
                  <dt>{{ t('packages.size') }}</dt>
                  <dd>{{ formatSize(focusedPackage.size_bytes) }}</dd>
                  <dt>{{ t('packages.contents') }}</dt>
                  <dd>{{ t('packages.files', { count: focusedPackage.content_count }) }}</dd>
                  <dt>{{ t('packages.deps') }} / {{ t('packages.depsBy') }}</dt>
                  <dd>{{ focusedPackage.dependency_count }} / {{ focusedPackage.dependents_count }}</dd>
                  <dt>{{ t('packages.importTime') }}</dt>
                  <dd>{{ formatTime(focusedPackage.scan_time) }}</dd>
                </dl>
              </template>
            </aside>
          </div>

          <footer class="picker-footer">
            <span>{{ paginationInfo }}</span>
            <div class="pagination-controls">
              <button class="page-btn" :disabled="pickerPage <= 1" @click="pickerPage--">‹</button>
              <span>{{ pickerPage }} / {{ totalPages }}</span>
              <button class="page-btn" :disabled="pickerPage >= totalPages" @click="pickerPage++">›</button>
              <select v-model.number="pickerPageSize" class="page-size-select">
                <option :value="60">60 / 页</option>
                <option :value="120">120 / 页</option>
                <option :value="200">200 / 页</option>
              </select>
            </div>
          </footer>
        </div>
      </div>
    </Teleport>
  </div>
</template>

<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { storeToRefs } from 'pinia'
import { useAppStore } from '@/stores/app'
import { useLocalLibraryStore } from '@/stores/localLibrary'
import type { PackageDisplayItem } from '@/types/package'

interface LaunchPlan {
  id: string
  name: string
  packageIds: string[]
}

interface OnDemandPlansState {
  plans: LaunchPlan[]
  activePlanId: string | null
}

interface FolderNode {
  label: string
  path: string
  count: number
  children: Map<string, FolderNode>
}

interface FlatFolderNode {
  label: string
  fullLabel: string
  path: string
  count: number
  depth: number
}

interface OnDemandOperationResult {
  completed: number
  failed: number
  skipped: number
  total: number
  errors: string[]
  launched: boolean
  launch_path: string | null
}

interface OnDemandProgress {
  phase: string
  completed: number
  failed: number
  skipped: number
  total: number
  current_file: string | null
}

const { t } = useI18n()
const appStore = useAppStore()
const localLibraryStore = useLocalLibraryStore()
const { vamRootPath, installContext } = storeToRefs(appStore)
const { packages, dependencyGraph: graph, packageFolders: folderPaths } = storeToRefs(localLibraryStore)

const loading = ref(false)
const localSearch = ref('')
const selectedType = ref('all')
const selectedFolder = ref('')
const sortBy = ref('created')
const includeDependencies = ref(true)
const showOnlyActivated = ref(false)
const pickerVisible = ref(false)
const pickerViewMode = ref<'grid' | 'list'>('grid')
const pickerPage = ref(1)
const pickerPageSize = ref(60)
const tempSelectedIds = ref<Set<string>>(new Set())
const focusedPackageId = ref('')
const thumbnails = ref<Record<string, string>>({})
const thumbnailErrors = ref<Set<string>>(new Set())
const operationRunning = ref(false)
const operationMessage = ref('')
const operationFailed = ref(false)
const operationProgress = ref<OnDemandProgress | null>(null)
const activePlanId = ref('default')
const plansLoaded = ref(false)
let unlistenOperationProgress: (() => void) | null = null
const plans = ref<LaunchPlan[]>([
  { id: 'default', name: t('onDemand.defaultPlan'), packageIds: [] },
])

const activePlan = computed(() => plans.value.find(plan => plan.id === activePlanId.value) || plans.value[0])
const primaryIds = computed(() => activePlan.value?.packageIds || [])
const primaryIdSet = computed(() => new Set(primaryIds.value))
const tempPrimaryIdSet = computed(() => new Set(tempSelectedIds.value))

const packageById = computed(() => {
  const map = new Map<string, PackageDisplayItem>()
  for (const pkg of packages.value) map.set(pkg.id, pkg)
  return map
})

const dependencyMap = computed(() => {
  const map = new Map<string, string[]>()
  for (const edge of graph.value.edges) {
    if (!map.has(edge.source)) map.set(edge.source, [])
    map.get(edge.source)!.push(edge.target)
  }
  return map
})

const dependencyIdSet = computed(() => {
  if (!includeDependencies.value) return new Set<string>()
  return collectDependencyIds(primaryIds.value, primaryIdSet.value)
})

const activePackageIds = computed(() => new Set([...primaryIds.value, ...dependencyIdSet.value]))

const tempDependencyIdSet = computed(() => {
  if (!includeDependencies.value) return new Set<string>()
  return collectDependencyIds([...tempSelectedIds.value], tempPrimaryIdSet.value)
})

const tempActivePackageIds = computed(() => new Set([...tempSelectedIds.value, ...tempDependencyIdSet.value]))

const resourceTypes = computed(() => {
  const types = new Set<string>()
  for (const pkg of packages.value) {
    for (const type of pkg.resource_types) types.add(type)
  }
  return [...types].sort()
})

const addonPackagesPath = computed(() => (
  vamRootPath.value ? normalizeSlashes(`${vamRootPath.value}/AddonPackages`) : ''
))

const libraryPackagesRootPath = computed(() => (
  vamRootPath.value ? normalizeSlashes(`${vamRootPath.value}/VAMBoxLibrary/AddonPackages`) : ''
))

const packageRootPaths = computed(() => (
  [addonPackagesPath.value, libraryPackagesRootPath.value].filter(Boolean)
))

const flatFolderNodes = computed(() => {
  const root: FolderNode = {
    label: t('packages.addonPackages'),
    path: '',
    count: packages.value.length,
    children: new Map(),
  }

  for (const folderPath of folderPaths.value) {
    ensureFolderPath(root, folderPath)
  }

  for (const pkg of packages.value) {
    const dir = packageRelativeDir(pkg)
    if (!dir) continue

    const nodes = ensureFolderPath(root, dir)
    for (const node of nodes) node.count += 1
  }

  const nodes: FlatFolderNode[] = []
  flattenFolder(root, 0, nodes)
  return nodes
})

const filteredPackages = computed(() => {
  const query = localSearch.value.trim().toLowerCase()
  let list = packages.value

  if (showOnlyActivated.value) {
    return [...list]
      .filter(pkg => tempActivePackageIds.value.has(pkg.id))
      .sort((a, b) => {
        const aPrimary = tempSelectedIds.value.has(a.id) ? 0 : 1
        const bPrimary = tempSelectedIds.value.has(b.id) ? 0 : 1
        return aPrimary - bPrimary || comparePackageName(a, b)
      })
  }

  if (selectedFolder.value) {
    list = list.filter(pkg => isPackageInSelectedFolder(pkg))
  }
  if (selectedType.value !== 'all') {
    list = list.filter(pkg => pkg.resource_types.includes(selectedType.value))
  }
  if (query) {
    list = list.filter(pkg => (
      pkg.id.toLowerCase().includes(query) ||
      `${pkg.id.toLowerCase()}.var`.includes(query) ||
      pkg.creator.toLowerCase().includes(query) ||
      pkg.name.toLowerCase().includes(query)
    ))
  }
  return [...list].sort((a, b) => {
    const aActive = tempActivePackageIds.value.has(a.id) ? 0 : 1
    const bActive = tempActivePackageIds.value.has(b.id) ? 0 : 1
    if (aActive !== bActive) return aActive - bActive
    switch (sortBy.value) {
      case 'size':
        return b.size_bytes - a.size_bytes || comparePackageName(a, b)
      case 'deps':
        return b.dependency_count - a.dependency_count || comparePackageName(a, b)
      case 'name':
        return comparePackageName(a, b)
      case 'created':
      default:
        return timeValue(b.created_time) - timeValue(a.created_time) || comparePackageName(a, b)
    }
  })
})

const totalPages = computed(() => Math.max(1, Math.ceil(filteredPackages.value.length / pickerPageSize.value)))
const pagedPickerPackages = computed(() => {
  const start = (pickerPage.value - 1) * pickerPageSize.value
  return filteredPackages.value.slice(start, start + pickerPageSize.value)
})
const paginationInfo = computed(() => {
  if (filteredPackages.value.length === 0) return '0 / 0'
  const start = (pickerPage.value - 1) * pickerPageSize.value + 1
  const end = Math.min(start + pickerPageSize.value - 1, filteredPackages.value.length)
  return `${start}-${end} / ${filteredPackages.value.length}`
})

const selectedPrimaryPackages = computed(() => (
  primaryIds.value.map(id => packageById.value.get(id)).filter(Boolean) as PackageDisplayItem[]
))

const selectedDependencyPackages = computed(() => (
  [...dependencyIdSet.value].map(id => packageById.value.get(id)).filter(Boolean) as PackageDisplayItem[]
))

const totalSelectedSize = computed(() => {
  let total = 0
  for (const id of activePackageIds.value) {
    total += packageById.value.get(id)?.size_bytes || 0
  }
  return total
})

const operationRemainingFiles = computed(() => {
  if (!operationProgress.value) return 0
  return Math.max(
    0,
    operationProgress.value.total -
      operationProgress.value.completed -
      operationProgress.value.failed -
      operationProgress.value.skipped,
  )
})

const operationProgressPercent = computed(() => {
  if (!operationProgress.value || operationProgress.value.total === 0) return 0
  const handled = operationProgress.value.completed + operationProgress.value.failed + operationProgress.value.skipped
  return Math.min(100, Math.round(handled / operationProgress.value.total * 100))
})

const libraryPath = computed(() => (
  installContext.value?.managedLibraryDir || (vamRootPath.value ? `${vamRootPath.value}\\VAMBoxLibrary\\AddonPackages` : t('onDemand.pathPending'))
))

const activePath = computed(() => (
  installContext.value?.realAddonDir || (vamRootPath.value ? `${vamRootPath.value}\\AddonPackages` : t('onDemand.pathPending'))
))

const focusedPackage = computed(() => {
  if (focusedPackageId.value) return packageById.value.get(focusedPackageId.value) || null
  return pagedPickerPackages.value[0] || null
})

onMounted(async () => {
  unlistenOperationProgress = await listen<OnDemandProgress>('on-demand-operation-progress', (event) => {
    operationProgress.value = event.payload
  })
  loadData()
})

onUnmounted(() => {
  if (unlistenOperationProgress) {
    unlistenOperationProgress()
    unlistenOperationProgress = null
  }
})

watch(vamRootPath, () => {
  localLibraryStore.refreshAll('loading').then(() => loadData())
})

watch(
  () => [localSearch.value, selectedType.value, selectedFolder.value, sortBy.value, showOnlyActivated.value, pickerPageSize.value],
  () => { pickerPage.value = 1 },
)

watch(pagedPickerPackages, () => {
  loadVisibleThumbnails()
})

watch(selectedPrimaryPackages, () => {
  loadSelectedThumbnails()
})

function applyPlanState(planState: OnDemandPlansState) {
  if (planState.plans.length > 0) {
    plans.value = planState.plans
    activePlanId.value = planState.activePlanId && planState.plans.some(plan => plan.id === planState.activePlanId)
      ? planState.activePlanId
      : planState.plans[0].id
  } else {
    plans.value = [{ id: 'default', name: t('onDemand.defaultPlan'), packageIds: [] }]
    activePlanId.value = 'default'
  }
}

async function loadData() {
  loading.value = true
  try {
    plansLoaded.value = false
    await localLibraryStore.ensureLoaded()
    const planState = await invoke<OnDemandPlansState>('list_on_demand_plans')
    applyPlanState(planState)
    focusedPackageId.value = packages.value[0]?.id || ''
    loadSelectedThumbnails()
  } catch {
    plansLoaded.value = true
  } finally {
    loading.value = false
  }
}

function updateActivePlan(packageIds: string[]) {
  plans.value = plans.value.map(plan => (
    plan.id === activePlanId.value ? { ...plan, packageIds } : plan
  ))
  savePlansToDatabase()
}

function togglePrimary(packageId: string) {
  const next = primaryIdSet.value.has(packageId)
    ? primaryIds.value.filter(id => id !== packageId)
    : [...primaryIds.value, packageId]
  updateActivePlan(next)
}

function openPackagePicker() {
  tempSelectedIds.value = new Set(primaryIds.value)
  pickerVisible.value = true
  focusedPackageId.value = primaryIds.value[0] || packages.value[0]?.id || ''
  loadVisibleThumbnails()
}

function closePackagePicker() {
  pickerVisible.value = false
}

function savePackagePicker() {
  updateActivePlan([...tempSelectedIds.value])
  pickerVisible.value = false
  loadSelectedThumbnails()
}

function toggleTempSelection(packageId: string) {
  const next = new Set(tempSelectedIds.value)
  if (next.has(packageId)) {
    next.delete(packageId)
  } else {
    next.add(packageId)
  }
  tempSelectedIds.value = next
}

function focusPackage(packageId: string) {
  focusedPackageId.value = packageId
  loadThumbnail(packageId)
}

function selectCurrentPage() {
  const next = new Set(tempSelectedIds.value)
  for (const pkg of pagedPickerPackages.value) next.add(pkg.id)
  tempSelectedIds.value = next
}

function invertCurrentPage() {
  const next = new Set(tempSelectedIds.value)
  for (const pkg of pagedPickerPackages.value) {
    if (next.has(pkg.id)) {
      next.delete(pkg.id)
    } else {
      next.add(pkg.id)
    }
  }
  tempSelectedIds.value = next
}

function clearTempSelection() {
  tempSelectedIds.value = new Set()
}

function collectDependencyIds(seedIds: string[], excludedIds: Set<string>) {
  const result = new Set<string>()
  const visited = new Set<string>()
  const stack = [...seedIds]

  while (stack.length > 0) {
    const current = stack.pop()!
    if (visited.has(current)) continue
    visited.add(current)

    for (const depId of dependencyMap.value.get(current) || []) {
      if (!excludedIds.has(depId)) result.add(depId)
      if (!visited.has(depId)) stack.push(depId)
    }
  }

  return result
}

function createPlan() {
  const index = plans.value.length + 1
  const id = `plan-${Date.now()}`
  plans.value = [...plans.value, { id, name: t('onDemand.planName', { index }), packageIds: [] }]
  activePlanId.value = id
  savePlansToDatabase()
}

function selectPlan(planId: string) {
  activePlanId.value = planId
  savePlansToDatabase()
}

async function savePlansToDatabase() {
  if (!plansLoaded.value) return
  try {
    await invoke('save_on_demand_plans', {
      plans: plans.value,
      activePlanId: activePlanId.value,
    })
  } catch (error) {
    operationFailed.value = true
    operationMessage.value = String(error)
  }
}

async function migrateExistingPackages() {
  if (!vamRootPath.value || operationRunning.value) return
  operationRunning.value = true
  operationMessage.value = ''
  operationFailed.value = false
  operationProgress.value = null

  try {
    const result = await invoke<OnDemandOperationResult>('migrate_on_demand_library', {
      vamRoot: vamRootPath.value,
    })
    operationFailed.value = result.failed > 0
    operationMessage.value = t('onDemand.migrateResult', {
      completed: result.completed,
      failed: result.failed,
      skipped: result.skipped,
    })
    await appStore.refreshInstallContext()
    await localLibraryStore.refreshAll('refreshing')
    await loadData()
  } catch (error) {
    operationFailed.value = true
    operationMessage.value = String(error)
  } finally {
    operationRunning.value = false
  }
}

async function restoreAllPackages() {
  if (!vamRootPath.value || operationRunning.value) return
  operationRunning.value = true
  operationMessage.value = ''
  operationFailed.value = false
  operationProgress.value = null

  try {
    const result = await invoke<OnDemandOperationResult>('restore_on_demand_library', {
      vamRoot: vamRootPath.value,
    })
    operationFailed.value = result.failed > 0
    operationMessage.value = t('onDemand.restoreResult', {
      completed: result.completed,
      failed: result.failed,
      skipped: result.skipped,
    })
    await appStore.refreshInstallContext()
    await localLibraryStore.refreshAll('refreshing')
    await loadData()
  } catch (error) {
    operationFailed.value = true
    operationMessage.value = String(error)
  } finally {
    operationRunning.value = false
  }
}

async function applyCurrentPlan() {
  if (!vamRootPath.value || operationRunning.value || primaryIds.value.length === 0) return
  operationRunning.value = true
  operationMessage.value = ''
  operationFailed.value = false
  operationProgress.value = null

  try {
    const result = await invoke<OnDemandOperationResult>('apply_on_demand_plan', {
      vamRoot: vamRootPath.value,
      packageIds: primaryIds.value,
      includeDependencies: includeDependencies.value,
    })
    operationFailed.value = result.failed > 0
    operationMessage.value = t('onDemand.applyResult', {
      completed: result.completed,
      failed: result.failed,
      skipped: result.skipped,
    }) + (result.launched ? ` ${t('onDemand.gameLaunched')}` : '')
    await appStore.refreshInstallContext()
  } catch (error) {
    operationFailed.value = true
    operationMessage.value = String(error)
  } finally {
    operationRunning.value = false
  }
}

async function loadVisibleThumbnails() {
  if (!pickerVisible.value) return
  await Promise.all(pagedPickerPackages.value.slice(0, 36).map(pkg => loadThumbnail(pkg.id)))
}

async function loadSelectedThumbnails() {
  await Promise.all(selectedPrimaryPackages.value.slice(0, 8).map(pkg => loadThumbnail(pkg.id)))
}

async function loadThumbnail(packageId: string) {
  if (thumbnails.value[packageId] || thumbnailErrors.value.has(packageId)) return
  try {
    const result = await invoke<string | null>('get_package_thumbnail', { packageId })
    if (result) thumbnails.value = { ...thumbnails.value, [packageId]: result }
  } catch {
    const next = new Set(thumbnailErrors.value)
    next.add(packageId)
    thumbnailErrors.value = next
  }
}

function normalizeSlashes(path: string): string {
  return path.replace(/\\/g, '/').replace(/\/+$/, '')
}

function comparePath(path: string): string {
  return normalizeSlashes(path).toLowerCase()
}

function packageRelativeDir(pkg: PackageDisplayItem): string {
  if (packageRootPaths.value.length === 0) return ''
  const filePath = normalizeSlashes(pkg.file_path)
  const comparableFilePath = comparePath(filePath)

  for (const rootPath of packageRootPaths.value) {
    const comparableRootPath = comparePath(rootPath)
    if (comparableFilePath === comparableRootPath || comparableFilePath.startsWith(`${comparableRootPath}/`)) {
      const relativeFile = filePath.slice(rootPath.length).replace(/^\/+/, '')
      const lastSlash = relativeFile.lastIndexOf('/')
      return lastSlash >= 0 ? relativeFile.slice(0, lastSlash) : ''
    }
  }

  return ''
}

function isPackageInSelectedFolder(pkg: PackageDisplayItem): boolean {
  const dir = packageRelativeDir(pkg)
  return dir === selectedFolder.value || dir.startsWith(`${selectedFolder.value}/`)
}

function flattenFolder(node: FolderNode, depth: number, output: FlatFolderNode[]) {
  output.push({
    label: node.label,
    fullLabel: node.path || node.label,
    path: node.path,
    count: node.count,
    depth,
  })

  const children = [...node.children.values()].sort((a, b) => a.label.localeCompare(b.label))
  for (const child of children) flattenFolder(child, depth + 1, output)
}

function ensureFolderPath(root: FolderNode, folderPath: string): FolderNode[] {
  let current = root
  let currentPath = ''
  const nodes: FolderNode[] = []

  for (const segment of folderPath.split('/').filter(Boolean)) {
    currentPath = currentPath ? `${currentPath}/${segment}` : segment
    if (!current.children.has(segment)) {
      current.children.set(segment, {
        label: segment,
        path: currentPath,
        count: 0,
        children: new Map(),
      })
    }
    current = current.children.get(segment)!
    nodes.push(current)
  }

  return nodes
}

function resourceTypeLabel(type: string): string {
  const key = `resourceType.${type}`
  const translated = t(key)
  return translated !== key ? translated : type
}

function formatTime(iso: string): string {
  if (!iso) return '-'
  const date = new Date(iso)
  if (!Number.isFinite(date.getTime())) return iso.slice(0, 10)
  return `${date.getFullYear()}-${String(date.getMonth() + 1).padStart(2, '0')}-${String(date.getDate()).padStart(2, '0')}`
}

function timeValue(value?: string): number {
  const time = value ? new Date(value).getTime() : 0
  return Number.isFinite(time) ? time : 0
}

function comparePackageName(a: PackageDisplayItem, b: PackageDisplayItem): number {
  return a.creator.localeCompare(b.creator)
    || a.name.localeCompare(b.name)
    || b.version - a.version
}

function formatSize(bytes: number): string {
  if (bytes === 0) return '0 B'
  const units = ['B', 'KB', 'MB', 'GB', 'TB']
  const i = Math.floor(Math.log(bytes) / Math.log(1024))
  return `${(bytes / Math.pow(1024, i)).toFixed(i > 0 ? 1 : 0)} ${units[i]}`
}
</script>

<style scoped>
.on-demand-view {
  display: flex;
  flex-direction: column;
  gap: var(--space-5);
  height: 100%;
  min-height: 0;
}

.page-header {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: var(--space-4);
}

.page-title {
  font-size: var(--text-2xl);
  font-weight: var(--font-bold);
  color: var(--text-primary);
}

.page-subtitle {
  max-width: 720px;
  margin-top: var(--space-1);
  color: var(--text-secondary);
  font-size: var(--text-sm);
  line-height: var(--leading-relaxed);
}

.status-pills {
  display: flex;
  flex-wrap: wrap;
  justify-content: flex-end;
  gap: var(--space-2);
}

.status-pill {
  display: inline-flex;
  align-items: center;
  height: 26px;
  padding: 0 var(--space-3);
  border-radius: var(--radius-full);
  font-size: var(--text-xs);
  font-weight: var(--font-semibold);
  border: 1px solid var(--border-subtle);
  color: var(--text-secondary);
  background: var(--glass-bg);
}

.status-pill.ok,
.status-pill.safe { color: var(--color-success); background: var(--color-success-bg); }
.status-pill.warn { color: var(--color-warning); background: var(--color-warning-bg); }
.status-pill.info { color: var(--color-info); background: var(--color-info-bg); }

.workspace-grid {
  display: grid;
  grid-template-columns: 240px minmax(0, 1fr) 340px;
  gap: var(--space-4);
  min-height: 0;
  flex: 1;
}

.plans-panel,
.selection-panel,
.package-panel {
  min-height: 0;
}

.plans-panel,
.selection-panel {
  display: flex;
  flex-direction: column;
  gap: var(--space-3);
  padding: var(--space-4);
  overflow-y: auto;
}

.package-panel {
  display: flex;
  flex-direction: column;
  gap: var(--space-3);
  padding: var(--space-4);
  overflow: hidden;
}

.add-var-btn {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  gap: var(--space-2);
  width: 100%;
  height: 36px;
  border-radius: var(--radius-md);
  color: white;
  background: var(--accent-gradient);
  font-size: var(--text-sm);
  font-weight: var(--font-semibold);
  box-shadow: 0 4px 14px rgba(110, 107, 240, 0.26);
}

.launch-hero {
  display: flex;
  gap: var(--space-4);
  align-items: center;
  padding: var(--space-5);
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-md);
  background: rgba(255, 255, 255, 0.025);
}

.launch-hero-icon {
  display: flex;
  align-items: center;
  justify-content: center;
  flex-shrink: 0;
  width: 54px;
  height: 54px;
  border-radius: var(--radius-md);
  color: var(--accent-primary);
  background: rgba(110, 107, 240, 0.14);
}

.launch-hero h2 {
  color: var(--text-primary);
  font-size: var(--text-lg);
  font-weight: var(--font-bold);
}

.launch-hero p {
  margin-top: var(--space-1);
  color: var(--text-secondary);
  font-size: var(--text-sm);
  line-height: var(--leading-relaxed);
}

.overview-grid {
  display: grid;
  grid-template-columns: repeat(4, 1fr);
  gap: var(--space-3);
}

.overview-grid > div,
.overview-action {
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
  min-height: 76px;
  padding: var(--space-4);
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-md);
  background: rgba(255, 255, 255, 0.025);
  text-align: left;
}

.overview-action {
  cursor: pointer;
}

.overview-action:hover {
  border-color: var(--border-accent);
  background: rgba(110, 107, 240, 0.12);
}

.overview-grid strong {
  color: var(--text-primary);
  font-size: var(--text-xl);
  font-variant-numeric: tabular-nums;
}

.overview-grid span {
  color: var(--text-tertiary);
  font-size: var(--text-xs);
}

.selected-preview {
  display: flex;
  flex-direction: column;
  gap: var(--space-3);
  min-height: 0;
}

.preview-empty {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--space-3);
  padding: var(--space-4);
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-md);
  color: var(--text-secondary);
}

.preview-card-grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(220px, 1fr));
  gap: var(--space-3);
}

.selected-preview-card {
  display: grid;
  grid-template-columns: 74px minmax(0, 1fr);
  gap: var(--space-3);
  align-items: center;
  padding: var(--space-2);
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-md);
  background: rgba(255, 255, 255, 0.025);
  text-align: left;
}

.selected-preview-card:hover {
  border-color: var(--border-accent);
  background: rgba(110, 107, 240, 0.1);
}

.preview-thumb {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 74px;
  height: 74px;
  overflow: hidden;
  border-radius: var(--radius-sm);
  color: var(--text-tertiary);
  background: #050508;
  font-size: 10px;
}

.preview-thumb img {
  width: 100%;
  height: 100%;
  object-fit: cover;
}

.preview-info {
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
  min-width: 0;
}

.preview-info strong,
.preview-info span {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.preview-info strong {
  color: var(--text-primary);
  font-size: var(--text-sm);
}

.preview-info span {
  color: var(--text-tertiary);
  font-size: var(--text-xs);
}

.empty-state.compact {
  min-height: 240px;
  flex: 0 0 auto;
}

.panel-heading {
  display: flex;
  align-items: center;
  justify-content: space-between;
  color: var(--text-primary);
  font-size: var(--text-sm);
  font-weight: var(--font-semibold);
}

.icon-action {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 28px;
  height: 28px;
  border-radius: var(--radius-sm);
  color: var(--text-secondary);
  transition: background var(--duration-fast) var(--ease), color var(--duration-fast) var(--ease);
}

.icon-action:hover {
  color: var(--text-primary);
  background: var(--bg-hover);
}

.plan-card {
  display: flex;
  flex-direction: column;
  gap: 3px;
  width: 100%;
  padding: var(--space-3);
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-md);
  text-align: left;
  background: rgba(255, 255, 255, 0.03);
  transition: border-color var(--duration-fast) var(--ease), background var(--duration-fast) var(--ease);
}

.plan-card:hover,
.plan-card.active {
  border-color: var(--border-accent);
  background: rgba(110, 107, 240, 0.12);
}

.plan-name {
  color: var(--text-primary);
  font-size: var(--text-sm);
  font-weight: var(--font-semibold);
}

.plan-meta,
.muted {
  color: var(--text-tertiary);
  font-size: var(--text-xs);
}

.option-block,
.strategy-card {
  padding: var(--space-3);
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-md);
  background: rgba(255, 255, 255, 0.025);
}

.switch-row {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  min-height: 28px;
  color: var(--text-secondary);
  font-size: var(--text-sm);
}

.switch-row input {
  accent-color: var(--accent-primary);
}

.strategy-title {
  margin-bottom: var(--space-2);
  color: var(--text-primary);
  font-size: var(--text-sm);
  font-weight: var(--font-semibold);
}

.strategy-card p {
  color: var(--text-secondary);
  font-size: var(--text-xs);
  line-height: var(--leading-relaxed);
}

.strategy-action {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 100%;
  height: 32px;
  margin-top: var(--space-3);
  border: 1px solid var(--border-accent);
  border-radius: var(--radius-sm);
  color: var(--accent-primary);
  background: rgba(110, 107, 240, 0.12);
  font-size: var(--text-xs);
  font-weight: var(--font-semibold);
}

.strategy-action:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}

.strategy-action.restore {
  color: var(--color-error);
  border-color: rgba(248, 113, 113, 0.28);
  background: rgba(248, 113, 113, 0.1);
}

.operation-progress {
  margin-top: var(--space-3);
}

.operation-progress-meta {
  display: flex;
  justify-content: space-between;
  margin-bottom: var(--space-2);
  color: var(--text-secondary);
  font-size: var(--text-xs);
  font-variant-numeric: tabular-nums;
}

.operation-progress-track {
  height: 7px;
  overflow: hidden;
  border-radius: var(--radius-full);
  background: var(--bg-elevated);
}

.operation-progress-fill {
  height: 100%;
  border-radius: inherit;
  background: var(--accent-gradient);
  transition: width var(--duration-base) var(--ease);
}

.package-toolbar {
  display: flex;
  align-items: center;
  gap: var(--space-3);
  padding: var(--space-3);
  border-radius: var(--radius-md);
  flex-shrink: 0;
}

.search-box {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  flex: 1;
  height: 34px;
  padding: 0 var(--space-3);
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-md);
  color: var(--text-tertiary);
  background: var(--bg-input);
}

.search-box input {
  width: 100%;
  color: var(--text-primary);
  font-size: var(--text-sm);
  background: none;
  border: none;
  padding: 0;
}

.type-select,
.ghost-btn {
  height: 34px;
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-md);
  color: var(--text-secondary);
  background: var(--bg-input);
  font-size: var(--text-sm);
}

.type-select {
  min-width: 120px;
  padding: 0 var(--space-3);
}

.ghost-btn {
  padding: 0 var(--space-4);
}

.ghost-btn:hover {
  color: var(--text-primary);
  background: var(--bg-hover);
}

.empty-state {
  display: flex;
  flex: 1;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: var(--space-2);
  padding: var(--space-8);
  color: var(--text-secondary);
  border-radius: var(--radius-md);
}

.empty-state p {
  max-width: 420px;
  color: var(--text-tertiary);
  font-size: var(--text-sm);
  text-align: center;
}

.loader {
  width: 28px;
  height: 28px;
  border: 3px solid var(--accent-primary);
  border-top-color: transparent;
  border-radius: 50%;
  animation: spin 0.8s linear infinite;
}

@keyframes spin {
  to { transform: rotate(360deg); }
}

.package-list {
  min-height: 0;
  overflow-y: auto;
  border-radius: var(--radius-md);
}

.list-head,
.package-row {
  display: grid;
  grid-template-columns: minmax(240px, 1fr) 92px 86px 56px 138px;
  gap: var(--space-3);
  align-items: center;
}

.list-head {
  position: sticky;
  top: 0;
  z-index: 2;
  padding: var(--space-3) var(--space-4);
  color: var(--text-tertiary);
  background: var(--bg-surface);
  border-bottom: 1px solid var(--border-subtle);
  font-size: var(--text-xs);
  font-weight: var(--font-semibold);
  text-transform: uppercase;
}

.package-row {
  width: 100%;
  padding: var(--space-3) var(--space-4);
  border-bottom: 1px solid var(--border-subtle);
  color: var(--text-secondary);
  text-align: left;
  transition: background var(--duration-fast) var(--ease), color var(--duration-fast) var(--ease);
}

.package-row:hover {
  color: var(--text-primary);
  background: var(--bg-hover);
}

.package-row.primary {
  background: rgba(110, 107, 240, 0.16);
}

.package-row.dependency:not(.primary) {
  background: rgba(91, 141, 239, 0.08);
}

.package-main {
  display: flex;
  align-items: center;
  gap: var(--space-3);
  min-width: 0;
}

.check-box {
  display: flex;
  align-items: center;
  justify-content: center;
  flex-shrink: 0;
  width: 18px;
  height: 18px;
  border: 1px solid var(--border-strong);
  border-radius: var(--radius-xs);
  color: white;
  background: rgba(110, 107, 240, 0.18);
}

.package-text {
  display: flex;
  flex-direction: column;
  min-width: 0;
}

.package-name {
  overflow: hidden;
  color: var(--text-primary);
  font-size: var(--text-sm);
  font-weight: var(--font-semibold);
  text-overflow: ellipsis;
  white-space: nowrap;
}

.package-id {
  overflow: hidden;
  color: var(--text-tertiary);
  font-size: var(--text-xs);
  text-overflow: ellipsis;
  white-space: nowrap;
}

.type-badge {
  display: inline-flex;
  max-width: 84px;
  height: 22px;
  align-items: center;
  padding: 0 var(--space-2);
  border-radius: var(--radius-full);
  font-size: var(--text-xs);
  font-weight: var(--font-semibold);
}

.type-scene { color: var(--color-scene); background: var(--color-scene-bg); }
.type-appearance { color: var(--color-appearance); background: var(--color-appearance-bg); }
.type-morph { color: var(--color-morph); background: var(--color-morph-bg); }
.type-clothing { color: var(--color-clothing); background: var(--color-clothing-bg); }
.type-plugin { color: var(--color-plugin); background: var(--color-plugin-bg); }
.type-texture { color: var(--color-texture); background: var(--color-texture-bg); }
.type-asset { color: var(--color-asset); background: var(--color-asset-bg); }
.type-hair { color: var(--color-hair); background: var(--color-hair-bg); }
.type-other { color: var(--text-tertiary); background: var(--bg-hover); }

.mono {
  font-variant-numeric: tabular-nums;
  white-space: nowrap;
}

.row-actions {
  display: flex;
  gap: var(--space-2);
  justify-content: flex-end;
}

.mini-btn {
  height: 26px;
  padding: 0 var(--space-2);
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-sm);
  color: var(--text-secondary);
  background: rgba(255, 255, 255, 0.03);
  font-size: var(--text-xs);
}

.mini-btn:hover,
.mini-btn.accent {
  color: var(--text-primary);
  border-color: var(--border-accent);
  background: rgba(110, 107, 240, 0.16);
}

.list-more {
  padding: var(--space-3);
  color: var(--text-tertiary);
  font-size: var(--text-xs);
  text-align: center;
}

.selected-count {
  color: var(--accent-primary);
  font-variant-numeric: tabular-nums;
}

.summary-grid {
  display: grid;
  grid-template-columns: repeat(3, 1fr);
  gap: var(--space-2);
}

.summary-grid div {
  display: flex;
  flex-direction: column;
  gap: 2px;
  padding: var(--space-3);
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-md);
  background: rgba(255, 255, 255, 0.025);
}

.summary-grid strong {
  color: var(--text-primary);
  font-size: var(--text-md);
  font-variant-numeric: tabular-nums;
}

.summary-grid span {
  color: var(--text-tertiary);
  font-size: var(--text-xs);
}

.path-preview {
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
  padding: var(--space-3);
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-md);
  background: rgba(255, 255, 255, 0.025);
}

.path-label {
  margin-top: var(--space-1);
  color: var(--text-tertiary);
  font-size: var(--text-xs);
}

.path-value {
  overflow: hidden;
  color: var(--text-secondary);
  font-family: var(--font-mono);
  font-size: 10px;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.selected-section,
.operation-preview {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
}

.section-title {
  color: var(--text-primary);
  font-size: var(--text-sm);
  font-weight: var(--font-semibold);
}

.selected-item {
  display: grid;
  grid-template-columns: minmax(0, 1fr) auto;
  align-items: center;
  gap: var(--space-2);
  min-height: 30px;
  padding: 0 var(--space-2);
  border-radius: var(--radius-sm);
  color: var(--text-secondary);
  background: rgba(110, 107, 240, 0.1);
  font-size: var(--text-xs);
}

.selected-item.dependency {
  background: rgba(91, 141, 239, 0.08);
}

.selected-item span {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.selected-item button {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 22px;
  height: 22px;
  border-radius: var(--radius-xs);
  color: var(--text-tertiary);
}

.selected-item button:hover {
  color: var(--text-primary);
  background: var(--bg-hover);
}

.operation-preview ol {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
  margin: 0;
  padding-left: 18px;
  color: var(--text-secondary);
  font-size: var(--text-xs);
  line-height: var(--leading-relaxed);
}

.execute-btn {
  flex-shrink: 0;
  height: 38px;
  margin-top: auto;
  border-radius: var(--radius-md);
  color: white;
  background: var(--accent-gradient);
  font-size: var(--text-sm);
  font-weight: var(--font-semibold);
}

.execute-btn:disabled {
  opacity: 0.55;
  cursor: not-allowed;
}

.operation-message {
  padding: var(--space-3);
  border-radius: var(--radius-md);
  font-size: var(--text-xs);
  line-height: var(--leading-relaxed);
}

.operation-message.ok {
  color: var(--color-success);
  background: var(--color-success-bg);
}

.operation-message.error {
  color: var(--color-error);
  background: var(--color-error-bg);
}

.picker-overlay {
  position: fixed;
  inset: 0;
  z-index: var(--z-overlay);
  display: flex;
  align-items: center;
  justify-content: center;
  padding: var(--space-5);
  background: rgba(0, 0, 0, 0.54);
  /* backdrop-filter removed */
}

.picker-modal {
  display: grid;
  grid-template-rows: auto auto auto minmax(0, 1fr) auto;
  width: min(1680px, calc(100vw - 40px));
  height: min(920px, calc(100vh - 40px));
  overflow: hidden;
  border: 1px solid var(--border-default);
  border-radius: var(--radius-lg);
  background: var(--bg-surface);
  box-shadow: var(--glass-shadow-lg);
}

.picker-header,
.picker-actions,
.picker-bulk-actions,
.picker-footer {
  display: flex;
  align-items: center;
  gap: var(--space-3);
  padding: var(--space-3) var(--space-4);
  border-bottom: 1px solid var(--border-subtle);
}

.picker-header {
  justify-content: space-between;
}

.picker-header h2 {
  color: var(--text-primary);
  font-size: var(--text-lg);
  font-weight: var(--font-bold);
}

.picker-header-actions {
  display: flex;
  align-items: center;
  gap: var(--space-3);
  color: var(--text-secondary);
  font-size: var(--text-sm);
}

.save-picker-btn {
  height: 34px;
  padding: 0 var(--space-4);
  border-radius: var(--radius-md);
  color: white;
  background: var(--accent-gradient);
  font-size: var(--text-sm);
  font-weight: var(--font-semibold);
}

.picker-actions .search-box {
  max-width: 360px;
}

.picker-bulk-actions {
  border-bottom: 1px solid var(--border-subtle);
}

.picker-deps-toggle {
  display: inline-flex;
  align-items: center;
  gap: var(--space-2);
  height: 34px;
  padding: 0 var(--space-3);
  border: 1px solid var(--border-accent);
  border-radius: var(--radius-md);
  color: var(--accent-primary);
  background: rgba(110, 107, 240, 0.12);
  font-size: var(--text-sm);
  font-weight: var(--font-semibold);
}

.picker-deps-toggle input {
  accent-color: var(--accent-primary);
}

.ghost-btn.accent {
  color: var(--accent-primary);
  border-color: var(--border-accent);
}

.picker-body {
  display: grid;
  grid-template-columns: 210px minmax(0, 1fr) 280px;
  min-height: 0;
}

.picker-folder-tree,
.picker-detail {
  min-height: 0;
  overflow-y: auto;
  border-right: 1px solid var(--border-subtle);
  background: rgba(255, 255, 255, 0.018);
}

.picker-folder-tree {
  padding: var(--space-3);
}

.folder-tree-title {
  padding: 0 var(--space-2) var(--space-2);
  color: var(--text-tertiary);
  font-size: var(--text-xs);
  font-weight: var(--font-semibold);
}

.folder-tree-item {
  display: grid;
  grid-template-columns: auto minmax(0, 1fr) auto;
  align-items: center;
  gap: var(--space-2);
  width: 100%;
  min-height: 30px;
  padding-right: var(--space-2);
  border-radius: var(--radius-sm);
  color: var(--text-secondary);
  font-size: var(--text-sm);
  text-align: left;
}

.folder-tree-item:hover,
.folder-tree-item.active {
  color: var(--text-primary);
  background: var(--bg-hover);
}

.folder-tree-item.active {
  color: var(--accent-primary);
}

.folder-tree-item span {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.folder-tree-item em {
  color: var(--text-tertiary);
  font-size: var(--text-xs);
  font-style: normal;
}

.picker-main {
  display: flex;
  flex-direction: column;
  min-height: 0;
  overflow: hidden;
}

.picker-viewbar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  flex-shrink: 0;
  padding: var(--space-3) var(--space-4);
  border-bottom: 1px solid var(--border-subtle);
  color: var(--text-secondary);
  font-size: var(--text-sm);
}

.view-switch {
  display: flex;
  gap: 2px;
  padding: 2px;
  border-radius: var(--radius-sm);
  background: var(--bg-base);
}

.view-switch button {
  height: 28px;
  padding: 0 var(--space-3);
  border-radius: var(--radius-xs);
  color: var(--text-secondary);
  font-size: var(--text-xs);
}

.view-switch button.active {
  color: var(--text-primary);
  background: var(--bg-elevated);
}

.picker-grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(150px, 1fr));
  grid-auto-rows: minmax(176px, auto);
  align-content: start;
  flex: 1;
  gap: var(--space-3);
  min-height: 0;
  overflow-y: auto;
  padding: var(--space-4);
}

.picker-card {
  position: relative;
  display: flex;
  flex-direction: column;
  min-width: 0;
  min-height: 176px;
  overflow: hidden;
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-md);
  background: rgba(255, 255, 255, 0.025);
  text-align: left;
}

.picker-card.selected,
.picker-card.focused {
  border-color: var(--border-accent);
  background: rgba(110, 107, 240, 0.1);
}

.picker-card.dependency:not(.selected) {
  border-color: rgba(91, 141, 239, 0.32);
}

.picker-check {
  position: absolute;
  top: var(--space-2);
  left: var(--space-2);
  z-index: 1;
  display: flex;
  align-items: center;
  justify-content: center;
  width: 18px;
  height: 18px;
  border: 1px solid var(--border-strong);
  border-radius: var(--radius-xs);
  color: white;
  background: rgba(20, 20, 40, 0.74);
  cursor: pointer;
}

.picker-card.selected .picker-check {
  background: var(--accent-primary);
}

.picker-list-row .check-box {
  cursor: pointer;
}

.picker-type {
  position: absolute;
  top: var(--space-2);
  right: var(--space-2);
  z-index: 1;
  max-width: 72px;
  height: 22px;
  padding: 3px var(--space-2);
  border-radius: var(--radius-sm);
  font-size: 10px;
  font-weight: var(--font-semibold);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.picker-thumb {
  display: flex;
  align-items: center;
  justify-content: center;
  flex-shrink: 0;
  width: 100%;
  aspect-ratio: 1.18;
  overflow: hidden;
  color: var(--text-tertiary);
  background: #050508;
  font-size: var(--text-xs);
}

.picker-thumb img {
  width: 100%;
  height: 100%;
  object-fit: cover;
}

.picker-card-name {
  display: block;
  width: 100%;
  min-width: 0;
  padding: var(--space-2) var(--space-2) 2px;
  color: var(--text-primary);
  font-size: var(--text-sm);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.picker-card-name strong {
  color: var(--accent-secondary);
}

.picker-card-meta {
  display: block;
  width: 100%;
  min-width: 0;
  padding: 0 var(--space-2) var(--space-2);
  color: var(--text-tertiary);
  font-size: 10px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.picker-list {
  min-height: 0;
  overflow-y: auto;
  padding: var(--space-3);
}

.picker-list-row {
  display: grid;
  grid-template-columns: 24px minmax(220px, 1fr) 88px 84px 88px 78px;
  gap: var(--space-3);
  align-items: center;
  width: 100%;
  min-height: 38px;
  padding: 0 var(--space-3);
  border-bottom: 1px solid var(--border-subtle);
  color: var(--text-secondary);
  font-size: var(--text-sm);
  text-align: left;
}

.picker-list-row.selected,
.picker-list-row.focused {
  color: var(--text-primary);
  background: var(--bg-hover);
}

.picker-list-row.dependency.selected {
  background: rgba(91, 141, 239, 0.12);
}

.picker-list-row .package-name {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.picker-detail {
  display: flex;
  flex-direction: column;
  gap: var(--space-3);
  padding: var(--space-4);
  border-right: none;
  border-left: 1px solid var(--border-subtle);
}

.detail-preview {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 100%;
  aspect-ratio: 1.25;
  overflow: hidden;
  border-radius: var(--radius-md);
  color: var(--text-tertiary);
  background: #050508;
  font-size: var(--text-xs);
}

.detail-preview img {
  width: 100%;
  height: 100%;
  object-fit: contain;
}

.picker-detail h3 {
  color: var(--text-primary);
  font-size: var(--text-sm);
  font-weight: var(--font-semibold);
  line-height: var(--leading-normal);
  overflow-wrap: anywhere;
}

.picker-detail dl {
  display: grid;
  grid-template-columns: 92px minmax(0, 1fr);
  gap: var(--space-2) var(--space-3);
  margin: 0;
}

.picker-detail dt {
  color: var(--text-tertiary);
  font-size: var(--text-xs);
}

.picker-detail dd {
  margin: 0;
  color: var(--text-secondary);
  font-size: var(--text-xs);
  overflow-wrap: anywhere;
}

.picker-footer {
  justify-content: space-between;
  border-top: 1px solid var(--border-subtle);
  border-bottom: none;
  color: var(--text-tertiary);
  font-size: var(--text-xs);
}

.pagination-controls {
  display: flex;
  align-items: center;
  gap: var(--space-2);
}

.page-btn {
  width: 28px;
  height: 28px;
  border-radius: var(--radius-sm);
  color: var(--text-secondary);
  background: var(--bg-input);
}

.page-btn:disabled {
  opacity: 0.4;
  cursor: not-allowed;
}

.page-size-select {
  height: 28px;
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-sm);
  color: var(--text-secondary);
  background: var(--bg-input);
  font-size: var(--text-xs);
}

@media (max-width: 1280px) {
  .workspace-grid {
    grid-template-columns: 220px minmax(0, 1fr);
  }

  .selection-panel {
    grid-column: 1 / -1;
    min-height: 260px;
  }

  .picker-body {
    grid-template-columns: 190px minmax(0, 1fr);
  }

  .picker-detail {
    display: none;
  }
}

@media (max-width: 900px) {
  .page-header,
  .package-toolbar {
    flex-direction: column;
    align-items: stretch;
  }

  .workspace-grid {
    grid-template-columns: 1fr;
  }

  .plans-panel {
    max-height: 320px;
  }

  .overview-grid {
    grid-template-columns: repeat(2, 1fr);
  }

  .list-head,
  .package-row {
    grid-template-columns: minmax(180px, 1fr) 76px 74px;
  }

  .list-head span:nth-child(4),
  .list-head span:nth-child(5),
  .package-row > span:nth-child(4),
  .package-row > span:nth-child(5) {
    display: none;
  }

  .picker-overlay {
    padding: var(--space-2);
  }

  .picker-modal {
    width: calc(100vw - 16px);
    height: calc(100vh - 16px);
  }

  .picker-header,
  .picker-actions,
  .picker-bulk-actions {
    align-items: stretch;
    flex-direction: column;
  }

  .picker-header-actions {
    justify-content: space-between;
  }

  .picker-body {
    grid-template-columns: 1fr;
  }

  .picker-folder-tree {
    display: none;
  }

  .picker-list-row {
    grid-template-columns: 24px minmax(160px, 1fr) 76px;
  }

  .picker-list-row span:nth-child(n + 4) {
    display: none;
  }
}
</style>
