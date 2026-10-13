<script setup lang="ts">
import PackageCard from '@/components/packages/PackageCard.vue'
import PackageDetails from '@/components/packages/PackageDetails.vue'
import UiButton from '@/components/ui/UiButton.vue'
import UiSegmented from '@/components/ui/UiSegmented.vue'
import UiMenu from '@/components/ui/UiMenu.vue'
import UiCheckbox from '@/components/ui/UiCheckbox.vue'
import { resourceColor, resourceThumbnail, resourceVersion } from '@/utils/resourcePresentation'

import { formatSize } from '@/utils/bytes'
import { ref, watch, computed, onUnmounted, useSlots } from 'vue'
import { useI18n } from 'vue-i18n'
import EmptyState from '@/components/common/EmptyState.vue'
import { useNotification } from '@/composables/useNotification'
import type { PackageDisplayItem } from '@/types/package'
import CleanupToolbar from '@/components/CleanupToolbar.vue'
import { useCleanupStore } from '@/stores/cleanup'
import { useResourceActions } from '@/composables/useResourceActions'
import { useFavoritesStore } from '@/stores/favorites'
import { loadPackageThumbnails, resourceCacheRevision } from '@/utils/resourceCache'

export interface StatusTag {
  label: string
  tone: 'error' | 'warning' | 'neutral'
}

interface QuickDeleteResult {
  deleted_package_ids: string[]
  skipped_package_ids: string[]
  deleted_count: number
  skipped_count: number
  deleted_file_count: number
  freed_bytes: number
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
    /** Short status tags per package id, e.g. problems found by the cleanup check. */
    statusTags?: Record<string, StatusTag[]>
  }>(),
  {
    emptyIcon: 'M12 2L3 7V17L12 22L21 17V7L12 2ZM12 12L21 7M12 12V22M12 12L3 7',
    showQuickDelete: false,
    organizing: false,
  },
)

const emit = defineEmits<{
  'update:viewMode': [mode: 'large-card' | 'small-card' | 'list']
  'packageDeleted': [result: QuickDeleteResult]
}>()

const { t } = useI18n()
const notify = useNotification()
const slots = useSlots()
const cleanup = useCleanupStore()
const favorites = useFavoritesStore()
async function unprotect(id: string) { try { await cleanup.update('unprotect', [id]) } catch (e) { notify.error(String(e)) } }

function getVarFileName(pkg: { id: string; file_path?: string } | null): string {
  if (!pkg) return ''
  if (pkg.file_path) {
    const parts = pkg.file_path.split(/[/\\]/)
    const last = parts[parts.length - 1]
    if (last && last.endsWith('.var')) return last
  }
  return pkg.id ? `${pkg.id}.var` : ''
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

// URLs and negative results are shared across pages; no 50-image eviction loop.
const thumbnails = ref<Record<string, string | null>>({})
const thumbErrors = ref<Set<string>>(new Set())
let thumbnailEpoch = 0
function onThumbError(id: string) { thumbErrors.value.add(id); delete thumbnails.value[id] }
watch([paginatedPackages, resourceCacheRevision], () => {
  const epoch = ++thumbnailEpoch
  thumbErrors.value.clear()
  if (slots.browser) return
  void loadPackageThumbnails(paginatedPackages.value, (id, url, error) => {
    if (error) thumbErrors.value.add(id)
    else thumbnails.value[id] = url
  }, () => epoch === thumbnailEpoch)
}, { immediate: true })
onUnmounted(() => { ++thumbnailEpoch })

// Shared details own all preview, action and dependency behavior.
const selectedPackage = ref<PackageDisplayItem | null>(null)
const packageDetails = ref<InstanceType<typeof PackageDetails> | null>(null)
const extractingCharacter = ref(false)
watch(() => props.packages, packages => {
  const previous = selectedPackage.value
  if (!previous) return
  const latest = packages.find(pkg => pkg.id === previous.id)
  if (!latest) return
  selectedPackage.value = latest
  if (latest.file_path !== previous.file_path || latest.scan_time !== previous.scan_time) {
    delete thumbnails.value[latest.id]; thumbErrors.value.delete(latest.id)
  }
})
function selectPackage(pkg: PackageDisplayItem) {
  if (extractingCharacter.value) return
  packageDetails.value?.resetNavigation()
  selectedPackage.value = pkg
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
const viewOptions = computed(() => [
 { value: 'large-card', label: t('packages.cardView'), icon: 'grid' as const },
 { value: 'list', label: t('packages.listView'), icon: 'list' as const },
])
const { cardMenuOpen, cardMenuAnchor, cardMenuPoint, cardMenuItems, openDetails, openCardMenu, queueSceneAction, handleCardAction, packageShortcut } = useResourceActions({
  selected: selectedPackage, details: packageDetails, busy: extractingCharacter, expanded: detailsExpanded,
  key: pkg => pkg.id, select: selectPackage, isScene: pkg => pkg.resource_types.includes('scene'),
  isPackage: () => true, canDelete: () => props.showQuickDelete,
})
function closeDetails() { if (!extractingCharacter.value) selectedPackage.value = null }
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
              :model-value="viewMode === 'small-card' ? 'large-card' : viewMode"
              :options="viewOptions"
              :label="t('ui.view')"
              icon-only
              @update:model-value="viewChanged"
            />
          </div>
        </div>
        <slot name="filters" />
        <div v-if="packages.length" class="resource-content-area">
          <div class="resource-content-scrollable">
            <div
              v-if="viewMode !== 'list'"
              class="package-grid resource-grid"
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
                  :favorite="favorites.countFor(pkg.id) > 0"
                  :favorite-title="t('ui.favoriteScenes', { count: favorites.countFor(pkg.id) })"
                  :can-launch="pkg.resource_types.includes('scene')"
                  :multi-select="organizing"
                  :checked="cleanup.selected.has(pkg.id.toLowerCase())"
                  :selection-disabled="organizing && cleanup.protectedIds.has(pkg.id.toLowerCase())"
                  @toggle="cleanup.toggle(pkg.id, paginatedPackages.map(p => p.id), $event.shiftKey)"
                  @select="openDetails(pkg)"
                  @details="openDetails(pkg)"
                  @launch="queueSceneAction(pkg, 'launch')"
                  @more="openCardMenu(pkg, $event)"
                  @image-error="onThumbError(pkg.id)"
                >
                  <template v-if="statusTags?.[pkg.id]?.length" #badge>
                    <span class="status-tags">
                      <span
                        v-for="tag in statusTags[pkg.id]"
                        :key="tag.label"
                        class="status-tag"
                        :class="tag.tone"
                      >{{ tag.label }}</span>
                    </span>
                  </template>
                </PackageCard>
                <UiButton
                  v-if="organizing && cleanup.markMap.get(pkg.id.toLowerCase())?.protected"
                  class="cleanup-unprotect"
                  size="sm"
                  variant="ghost"
                  @click="unprotect(pkg.id)"
                >
                  {{ t('cleanup.unprotect') }}
                </UiButton>
              </div>
            </div>
            <div v-else class="resource-list" role="table">
              <div class="list-header" role="row">
                <span>{{ t('packages.package') }}</span>
                <span>{{ t('packages.type') }}</span>
                <span>{{ t('packages.size') }}</span>
                <span>{{ t('packages.deps') }}</span>
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
                  <UiCheckbox
                    v-if="organizing"
                    :model-value="cleanup.selected.has(pkg.id.toLowerCase())"
                    :disabled="cleanup.protectedIds.has(pkg.id.toLowerCase())"
                    :label="pkg.id"
                    @update:model-value="cleanup.toggle(pkg.id, paginatedPackages.map(p => p.id), false)"
                  />
                  <button
                    type="button"
                    class="list-open"
                    @keydown="packageShortcut($event, pkg)"
                    @click="openDetails(pkg)"
                    @dblclick="openDetails(pkg)"
                    @keydown.space.prevent="openDetails(pkg)"
                  >
                    <img
                      v-if="thumbnails[pkg.id]"
                      data-resource-preview
                      :src="thumbnails[pkg.id] || undefined"
                      :alt="getVarFileName(pkg)"
                      @error="onThumbError(pkg.id)"
                    />
                    <span
                      v-else
                      class="list-placeholder"
                      :style="{ background: resourceThumbnail(pkg.resource_types[0]) }"
                    />
                    <span>
                      <strong>{{ pkg.name }}</strong>
                      <small>{{ pkg.creator }} · {{ resourceVersion(pkg) }}</small>
                    </span>
                  </button>
                  <span v-if="statusTags?.[pkg.id]?.length" class="list-tags">
                    <span
                      v-for="tag in statusTags[pkg.id]"
                      :key="tag.label"
                      class="status-tag"
                      :class="tag.tone"
                    >{{ tag.label }}</span>
                  </span>
                </div>
                <span
                  role="cell"
                  class="list-type"
                  :style="{ color: resourceColor(pkg.resource_types[0]) }"
                >
                  {{ resourceTypeLabel(pkg.resource_types[0] || 'other') }}
                </span>
                <span role="cell">{{ formatSize(pkg.size_bytes) }}</span>
                <!-- 依赖数后面直接标出缺几个；不缺就不额外写字，避免整列都是"已安装" -->
                <span role="cell" class="list-deps">
                  {{ pkg.dependency_count }}
                  <span v-if="missingCounts?.[pkg.id]" class="warning">
                    · {{ t('ui.missing', { count: missingCounts[pkg.id] }) }}
                  </span>
                </span>
                <span role="cell">{{ formatTime(pkg.scan_time) }}</span>
              </div>
            </div>
          </div>
        </div>
        <div v-else class="empty-wrapper">
          <EmptyState :icon="emptyIcon" :title="emptyTitle" :description="emptyDescription">
            <template v-if="$slots['empty-action']" #action><slot name="empty-action" /></template>
          </EmptyState>
        </div>
        <CleanupToolbar
          v-if="organizing"
          :packages="packages"
          :page-ids="paginatedPackages.map(p => p.id)"
        />
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
      </slot>
    </div>
    <PackageDetails v-if="selectedPackage" ref="packageDetails" :key="selectedPackage.id"
      :item="selectedPackage" :image="thumbnails[selectedPackage.id]" v-model:expanded="detailsExpanded"
      :show-quick-delete="showQuickDelete" @busy-change="extractingCharacter = $event" @close="closeDetails"
      @package-deleted="emit('packageDeleted', $event)" />
    <UiMenu
      v-model:open="cardMenuOpen"
      :anchor="cardMenuAnchor"
      :x="cardMenuPoint.x"
      :y="cardMenuPoint.y"
      :items="cardMenuItems"
      :label="t('ui.packageActions')"
      @select="handleCardAction"
    />

  </div>
</template>
<style scoped>
.resource-display {
  display: flex;
  gap: var(--space-4);
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
  gap: var(--space-4);
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
  padding: var(--space-1);
}

.package-cell {
  min-width: 0;
  position: relative;
}
/* 卡片上的状态标签放在类型角标下面，和它左对齐。 */
.status-tags {
  position: absolute;
  top: calc(var(--space-2) + 22px + var(--space-1));
  left: var(--space-2);
  z-index: 2;
  display: flex;
  flex-direction: column;
  align-items: flex-start;
  gap: var(--space-1);
  pointer-events: none;
}
.list-tags {
  display: flex;
  flex-shrink: 0;
  gap: var(--space-1);
}
.status-tag {
  display: inline-flex;
  align-items: center;
  min-height: 22px;
  padding: 0 var(--space-2);
  border-radius: var(--radius-full);
  font-size: var(--text-xs);
  font-weight: var(--font-bold);
  white-space: nowrap;
  background: var(--bg-on-image);
  color: var(--text-primary);
}
.status-tag.error {
  background: var(--color-error);
  color: var(--color-error-ink);
}
.status-tag.warning {
  background: var(--color-warning);
  color: var(--color-warning-ink);
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
  grid-template-columns: minmax(200px, 1fr) 88px 96px 96px 112px;
  align-items: center;
  gap: var(--space-3);
  min-width: 680px;
}
.list-header {
  height: 36px;
  padding: 0 var(--space-2);
  font-size: var(--text-xs);
  color: var(--text-tertiary);
}
.list-row {
  min-height: 52px;
  border-radius: var(--radius-sm);
  font-size: var(--text-sm);
  padding: 0 var(--space-2);
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
  gap: var(--space-3);
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
  padding: var(--space-1);
}
.pagination-controls {
  display: flex;
  align-items: center;
  gap: var(--space-1);
  font-size: var(--text-sm);
}
/* 与旁边 32 高的翻页按钮同高 */
.page-size-select {
  height: var(--control-h-sm);
  min-width: 96px;
  margin-right: var(--space-2);
  padding: 0 28px 0 var(--space-3);
  border: none;
  border-radius: var(--radius-control-sm);
  font-size: var(--text-sm);
  background-position:
    right 16px center,
    right 12px center;
}
.pagination-controls > span {
  min-width: 48px;
  text-align: center;
  color: var(--text-secondary);
  font-variant-numeric: tabular-nums;
}
.empty-wrapper {
  flex: 1;
  min-height: 0;
  overflow: auto;
  display: flex;
}
.empty-wrapper > :deep(.empty-state) {
  margin: auto;
}
@media (max-width: 760px) {
  .resource-display {
    flex-direction: column;
  }
}
</style>
