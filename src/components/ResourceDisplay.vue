<template>
  <div class="resource-display">
    <!-- View Controls -->
    <div class="view-controls glass-panel">
      <div class="controls-left">
        <div class="view-toggle">
          <button :class="['toggle-btn', { active: viewMode === 'large-card' }]" :aria-label="t('packages.largeCardView')" :title="t('packages.largeCardView')" @click="$emit('update:viewMode', 'large-card')">
            <svg width="16" height="16" viewBox="0 0 16 16" fill="none">
              <rect x="0.5" y="0.5" width="7" height="7" rx="1" stroke="currentColor" stroke-width="1.2" />
              <rect x="8.5" y="0.5" width="7" height="7" rx="1" stroke="currentColor" stroke-width="1.2" />
              <rect x="0.5" y="8.5" width="15" height="7" rx="1" stroke="currentColor" stroke-width="1.2" />
            </svg>
          </button>
          <button :class="['toggle-btn', { active: viewMode === 'small-card' }]" :aria-label="t('packages.smallCardView')" :title="t('packages.smallCardView')" @click="$emit('update:viewMode', 'small-card')">
            <svg width="16" height="16" viewBox="0 0 16 16" fill="none">
              <rect x="0.5" y="0.5" width="4.5" height="4.5" rx="0.8" stroke="currentColor" stroke-width="1.2" />
              <rect x="5.8" y="0.5" width="4.5" height="4.5" rx="0.8" stroke="currentColor" stroke-width="1.2" />
              <rect x="11" y="0.5" width="4.5" height="4.5" rx="0.8" stroke="currentColor" stroke-width="1.2" />
              <rect x="0.5" y="5.8" width="4.5" height="4.5" rx="0.8" stroke="currentColor" stroke-width="1.2" />
              <rect x="5.8" y="5.8" width="4.5" height="4.5" rx="0.8" stroke="currentColor" stroke-width="1.2" />
              <rect x="11" y="5.8" width="4.5" height="4.5" rx="0.8" stroke="currentColor" stroke-width="1.2" />
              <rect x="0.5" y="11" width="4.5" height="4.5" rx="0.8" stroke="currentColor" stroke-width="1.2" />
              <rect x="5.8" y="11" width="4.5" height="4.5" rx="0.8" stroke="currentColor" stroke-width="1.2" />
              <rect x="11" y="11" width="4.5" height="4.5" rx="0.8" stroke="currentColor" stroke-width="1.2" />
            </svg>
          </button>
          <button :class="['toggle-btn', { active: viewMode === 'list' }]" :aria-label="t('packages.listView')" :title="t('packages.listView')" @click="$emit('update:viewMode', 'list')">
            <svg width="16" height="16" viewBox="0 0 16 16" fill="none">
              <path d="M1 3h14M1 8h14M1 13h14" stroke="currentColor" stroke-width="1.3" stroke-linecap="round" />
            </svg>
          </button>
        </div>
        <span class="result-count text-secondary text-sm">{{ resultCountLabel }}</span>
      </div>
      <div class="controls-right">
        <slot name="actions" />
      </div>
    </div>

    <!-- Content Area -->
    <div v-if="packages.length > 0" class="resource-content-area">
      <div class="resource-content-scrollable">

        <!-- Large Card Grid -->
        <div v-if="viewMode === 'large-card'" class="large-card-grid">
          <div v-for="pkg in paginatedPackages" :key="pkg.id" class="large-card glass-card-component" @click="selectPackage(pkg)">
            <!-- 缩略图区域 -->
            <div class="large-card-thumb">
              <img data-resource-preview v-if="thumbnails[pkg.id]" :src="thumbnails[pkg.id]" :alt="getVarFileName(pkg)" class="thumb-img" @error="onThumbError(pkg.id)" />
              <div v-else class="thumb-placeholder">
                <svg width="28" height="28" viewBox="0 0 24 24" fill="none">
                  <rect x="3" y="3" width="18" height="18" rx="2" stroke="currentColor" stroke-width="1.5" />
                  <circle cx="8.5" cy="8.5" r="1.5" stroke="currentColor" stroke-width="1.5" />
                  <path d="M21 15l-5-5-6 6-3-3-4 4" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" />
                </svg>
                <span>{{ $t('packages.noThumbnail') }}</span>
              </div>
              <div class="large-card-type-badge" :class="'type-' + (pkg.resource_types[0] || 'other')">{{ resourceTypeLabel(pkg.resource_types[0] || 'other') }}</div>
            </div>
            <!-- 信息区域 -->
            <div class="large-card-info">
              <h4 class="large-card-name" :title="getVarFileName(pkg)">{{ getVarFileName(pkg) }}</h4>
              <div class="large-card-meta">
                <span class="meta-tag">v{{ pkg.version }}</span>
                <span class="meta-tag">{{ formatSize(pkg.size_bytes) }}</span>
                <span class="meta-tag">{{ $t('packages.files', { count: pkg.content_count }) }}</span>
              </div>
              <div class="large-card-stats">
                <div class="stat-item">
                  <span class="stat-value">{{ pkg.dependency_count }}</span>
                  <span class="stat-label">{{ $t('packages.deps') }}</span>
                </div>
                <div class="stat-item">
                  <span class="stat-value">{{ pkg.dependents_count }}</span>
                  <span class="stat-label">{{ $t('packages.depsBy') }}</span>
                </div>
                <div class="stat-item">
                  <span class="stat-value">{{ formatTime(pkg.scan_time) }}</span>
                  <span class="stat-label">{{ $t('packages.importTime') }}</span>
                </div>
              </div>
            </div>
          </div>
        </div>

        <!-- Small Card Grid -->
        <div v-else-if="viewMode === 'small-card'" class="small-card-grid">
          <div v-for="pkg in paginatedPackages" :key="pkg.id" class="small-card glass-card-component" @click="selectPackage(pkg)">
            <div class="small-card-thumb">
              <img data-resource-preview v-if="thumbnails[pkg.id]" :src="thumbnails[pkg.id]" :alt="getVarFileName(pkg)" class="thumb-img" @error="onThumbError(pkg.id)" />
              <div v-else class="thumb-placeholder sm">
                <svg width="20" height="20" viewBox="0 0 24 24" fill="none">
                  <rect x="3" y="3" width="18" height="18" rx="2" stroke="currentColor" stroke-width="1.5" />
                  <circle cx="8.5" cy="8.5" r="1.5" stroke="currentColor" stroke-width="1.5" />
                  <path d="M21 15l-5-5-6 6-3-3-4 4" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" />
                </svg>
                <span>{{ $t('packages.noThumbnail') }}</span>
              </div>
            </div>
            <div class="small-card-name" :title="getVarFileName(pkg)">{{ getVarFileName(pkg) }}</div>
          </div>
        </div>

        <!-- List View -->
        <div v-else class="resource-list glass-panel">
          <div class="list-header text-xs text-tertiary">
            <span class="col-thumb"></span>
            <span class="col-name">{{ $t('packages.package') }}</span>
            <span class="col-size">{{ $t('packages.size') }}</span>
            <span class="col-deps">{{ $t('packages.deps') }}</span>
            <span class="col-deps-by">{{ $t('packages.depsBy') }}</span>
            <span class="col-time">{{ $t('packages.importTime') }}</span>
          </div>
          <div v-for="pkg in paginatedPackages" :key="pkg.id" class="list-row" @click="selectPackage(pkg)">
            <span class="list-cell col-thumb">
              <img data-resource-preview v-if="thumbnails[pkg.id]" :src="thumbnails[pkg.id]" :alt="getVarFileName(pkg)" class="list-thumb-img" @error="onThumbError(pkg.id)" />
              <div v-else class="list-thumb-placeholder">
                <span class="type-dot-sm" :class="'type-' + (pkg.resource_types[0] || 'other')" />
                <span>{{ $t('packages.noThumbnail') }}</span>
              </div>
            </span>
            <span class="list-cell col-name name" :title="getVarFileName(pkg)">{{ getVarFileName(pkg) }}</span>
            <span class="list-cell col-size size">{{ formatSize(pkg.size_bytes) }}</span>
            <span class="list-cell col-deps deps">{{ pkg.dependency_count }}</span>
            <span class="list-cell col-deps-by deps">{{ pkg.dependents_count }}</span>
            <span class="list-cell col-time deps">{{ formatTime(pkg.scan_time) }}</span>
          </div>
        </div>

      </div>
    </div>

    <!-- Pagination -->
    <div v-if="packages.length > 0" class="pagination-bar glass-panel">
      <div class="pagination-info text-xs text-tertiary">{{ paginationInfo }}</div>
      <div class="pagination-controls">
        <select v-model.number="pageSize" class="page-size-select text-xs" @change="currentPage = 1">
          <option v-for="s in pageSizes" :key="s" :value="s">{{ s }} / 页</option>
        </select>
        <button class="page-btn" :disabled="currentPage <= 1" @click="currentPage = 1">
          <svg width="14" height="14" viewBox="0 0 24 24" fill="none"><path d="M11 19l-7-7 7-7M18 19l-7-7 7-7" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" /></svg>
        </button>
        <button class="page-btn" :disabled="currentPage <= 1" @click="currentPage--">
          <svg width="14" height="14" viewBox="0 0 24 24" fill="none"><path d="M15 18l-6-6 6-6" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" /></svg>
        </button>
        <span class="page-indicator text-sm">{{ currentPage }} / {{ totalPages }}</span>
        <button class="page-btn" :disabled="currentPage >= totalPages" @click="currentPage++">
          <svg width="14" height="14" viewBox="0 0 24 24" fill="none"><path d="M9 18l6-6-6-6" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" /></svg>
        </button>
        <button class="page-btn" :disabled="currentPage >= totalPages" @click="currentPage = totalPages">
          <svg width="14" height="14" viewBox="0 0 24 24" fill="none"><path d="M13 19l7-7-7-7M6 19l7-7-7-7" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" /></svg>
        </button>
      </div>
    </div>

    <!-- Empty State -->
    <div v-else class="empty-wrapper">
      <EmptyState :icon="emptyIcon" :title="emptyTitle" :description="emptyDescription">
        <template #action>
          <slot name="empty-action" />
        </template>
      </EmptyState>
    </div>

    <!-- Detail Panel -->
    <Transition name="slide-right">
      <div v-if="selectedPackage" class="detail-panel glass-panel">
        <div class="detail-header">
          <button class="detail-close" :aria-label="$t('gameContent.close')" :disabled="extractingCharacter" @click="selectedPackage = null">
            <svg width="16" height="16" viewBox="0 0 24 24" fill="none"><path d="M18 6L6 18M6 6l12 12" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" /></svg>
          </button>
          <button
            class="detail-copy-btn"
            @click="copyPackageName"
            :title="$t('common.copy')"
            :aria-label="$t('common.copy')"
          >
            <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round">
              <rect x="9" y="9" width="13" height="13" rx="2" ry="2"></rect>
              <path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1"></path>
            </svg>
          </button>
          <h4 class="detail-title text-sm" :title="getVarFileName(selectedPackage)">{{ getVarFileName(selectedPackage) }}</h4>
        </div>
        <div class="detail-body">
          <PackageSceneExtractor :key="selectedPackage.id" :package-id="selectedPackage.id" @busy-change="extractingCharacter = $event">
            <template #actions="{ launch, extract, disabled, expanded, launching, panel }">
              <div class="detail-actions-bar">
                <button class="action-bar-btn share" @click="openShareModal" :title="$t('share.btn')">
                  <span>{{ $t('share.btn') }}</span>
                </button>
                <button
                  v-if="showCompleteDependencyButton"
                  class="action-bar-btn complete-deps"
                  :disabled="dependencyCompletionLoading"
                  title="补齐缺失依赖"
                  @click="completeSelectedPackageDependencies"
                >
                  <span>{{ dependencyCompletionLoading ? '查询中...' : '补齐依赖' }}</span>
                </button>
                <button v-if="props.showQuickDelete" class="action-bar-btn delete" :disabled="deleteLoading || extractingCharacter" @click="quickDeleteSelected(true)" :title="$t('common.delete')">
                  <span>{{ $t('common.delete') }}</span>
                </button>
                <button class="action-bar-btn open-location" @click="openPackageFolder" :title="$t('packages.openLocation')">
                  <span>{{ $t('packages.openLocation') }}</span>
                </button>
                <button class="action-bar-btn launch-scene" :disabled="disabled || deleteLoading" @click="launch" :title="$t('gameContent.launchScene')">
                  <span>{{ $t(launching ? 'gameContent.launchingScene' : 'gameContent.launchScene') }}</span>
                </button>
                <button class="action-bar-btn extract-preset" :disabled="disabled || deleteLoading" :aria-expanded="expanded" :aria-controls="panel" @click="extract" :title="$t('gameContent.extractPreset')">
                  <span>{{ $t('gameContent.extractPreset') }}</span>
                </button>
              </div>
            </template>
          </PackageSceneExtractor>

          <div class="detail-thumb">
            <img data-resource-preview v-if="detailThumb" :src="detailThumb" :alt="getVarFileName(selectedPackage)" class="detail-thumb-img" />
            <div v-else class="detail-thumb-placeholder">
              <svg width="40" height="40" viewBox="0 0 24 24" fill="none">
                <rect x="3" y="3" width="18" height="18" rx="2" stroke="currentColor" stroke-width="1.5" />
                <circle cx="8.5" cy="8.5" r="1.5" stroke="currentColor" stroke-width="1.5" />
                <path d="M21 15l-5-5-6 6-3-3-4 4" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" />
              </svg>
              <span>{{ $t('packages.noThumbnail') }}</span>
            </div>
          </div>
          <div class="detail-info-grid">
            <div class="detail-section">
              <span class="detail-label text-xs text-tertiary">{{ $t('packages.type') }}</span>
              <span class="detail-value">
                <span class="type-dot-detail" :class="'type-' + (selectedPackage.resource_types[0] || 'other')" />
                {{ resourceTypeLabel(selectedPackage.resource_types[0] || 'other') }}
              </span>
            </div>
            <div class="detail-section">
              <span class="detail-label text-xs text-tertiary">{{ $t('packages.version') }}</span>
              <span class="detail-value">v{{ selectedPackage.version }}</span>
            </div>
            <div class="detail-section">
              <span class="detail-label text-xs text-tertiary">{{ $t('packages.size') }}</span>
              <span class="detail-value">{{ formatSize(selectedPackage.size_bytes) }}</span>
            </div>
            <div class="detail-section">
              <span class="detail-label text-xs text-tertiary">{{ $t('packages.contents') }}</span>
              <span class="detail-value">{{ $t('packages.files', { count: selectedPackage.content_count }) }}</span>
            </div>
          </div>
          <div class="detail-section">
            <span class="detail-label text-xs text-tertiary">{{ $t('packages.images') }}</span>
            <div class="detail-image-list">
              <span v-if="imageListLoading" class="detail-muted">{{ $t('packages.loadingImages') }}</span>
              <span v-else-if="selectedPackageImages.length === 0" class="detail-muted">{{ $t('packages.noImages') }}</span>
              <div v-else>
                <div class="detail-image-summary">
                  {{ $t('packages.imageFiles', { count: selectedPackageImages.length }) }}
                </div>
                <div class="detail-image-grid">
                  <button
                    v-for="image in visiblePackageImages"
                    :key="image.path"
                    class="detail-image-tile"
                    :title="image.path"
                    @click="openImagePreview(image)"
                  >
                    <img data-resource-preview
                      v-if="packageImageUrls[image.path]"
                      :src="packageImageUrls[image.path]"
                      :alt="image.path"
                    />
                    <span v-else class="detail-image-loading">{{ $t('common.loading') }}</span>
                    <span class="detail-image-size">{{ formatSize(image.size_bytes) }}</span>
                  </button>
                </div>
                <button
                  v-if="selectedPackageImages.length > COLLAPSED_IMAGE_COUNT"
                  class="detail-image-toggle"
                  @click="toggleImageExpanded"
                >
                  {{ imagesExpanded ? $t('packages.collapseImages') : $t('packages.expandImages', { count: selectedPackageImages.length - COLLAPSED_IMAGE_COUNT }) }}
                </button>
              </div>
            </div>
          </div>
          <div class="detail-info-grid">
            <div class="detail-section">
              <span class="detail-label text-xs text-tertiary">{{ $t('packages.importTime') }}</span>
              <span class="detail-value">{{ formatTime(selectedPackage.scan_time) }}</span>
            </div>
            <div class="detail-section">
              <span class="detail-label text-xs text-tertiary">{{ $t('packages.types') }}</span>
              <span class="detail-value">{{ selectedPackage.resource_types.join(', ') || $t('resourceType.mixed') }}</span>
            </div>
          </div>
          <div class="dependency-card">
            <div class="dependency-card-title">依赖关系</div>
            <div class="dependency-groups">
              <div v-for="panel in dependencyPanels" :key="panel.key" class="dependency-group">
                <button class="dependency-toggle" @click="toggleDependencyPanel(panel.key)">
                  <svg :class="['dependency-caret', { expanded: dependencyPanelExpanded[panel.key] }]" width="14" height="14" viewBox="0 0 24 24" fill="none">
                    <path d="M9 18l6-6-6-6" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" />
                  </svg>
                  <span class="dependency-toggle-label">{{ panel.label }}</span>
                  <span class="dependency-toggle-count">({{ dependencyPanelCount(panel.key) }})</span>
                </button>
                <div v-if="dependencyPanelExpanded[panel.key]" class="dependency-panel-body">
                  <span v-if="dependencyRelationsLoading" class="dependency-empty">加载中...</span>
                  <span v-else-if="dependencyLoadError" class="dependency-empty error">{{ dependencyLoadError }}</span>
                  <div v-else-if="dependencyPanelRows(panel.key).length > 0" class="dependency-row-list">
                    <div v-for="row in dependencyPanelRows(panel.key)" :key="`${panel.key}-${row.id}`" class="dependency-row">
                      <span class="dependency-row-name" :title="row.id">{{ row.id }}</span>
                      <span v-if="row.tier && row.tier > 1" class="dependency-row-tier">Tier {{ row.tier }}</span>
                      <span class="dependency-row-status" :class="row.status">{{ row.statusText }}</span>
                    </div>
                  </div>
                  <span v-else class="dependency-empty">暂无数据</span>
                </div>
              </div>
            </div>
          </div>
        </div>
      </div>
    </Transition>

    <!-- Share Modal Overlay -->
    <ShareModal
      :visible="shareModalVisible"
      :package-id="selectedPackage?.id || ''"
      @close="shareModalVisible = false"
    />

    <div v-if="previewImage" class="image-preview-overlay" @click="closeImagePreview">
      <div class="image-preview-dialog" @click.stop>
        <div class="image-preview-header">
          <span class="image-preview-title" :title="previewImage.path">{{ previewImage.path }}</span>
          <button class="detail-close" @click="closeImagePreview">
            <svg width="16" height="16" viewBox="0 0 24 24" fill="none"><path d="M18 6L6 18M6 6l12 12" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" /></svg>
          </button>
        </div>
        <img data-resource-preview v-if="previewImageUrl" class="image-preview-img" :src="previewImageUrl" :alt="previewImage.path" />
        <div v-else class="image-preview-loading">{{ $t('common.loading') }}</div>
      </div>
    </div>

    <div v-if="deleteConfirm" class="delete-confirm-overlay" @click.self="cancelQuickDelete">
      <div class="delete-confirm-dialog">
        <div class="delete-confirm-header">
          <h3>{{ $t('packages.quickDeleteConfirmTitle') }}</h3>
          <button class="detail-close" :disabled="deleteLoading" @click="cancelQuickDelete">
            <svg width="16" height="16" viewBox="0 0 24 24" fill="none"><path d="M18 6L6 18M6 6l12 12" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" /></svg>
          </button>
        </div>
        <div class="delete-confirm-body">
          <p class="delete-main-msg">{{ deleteConfirmMessage }}</p>
          
          <div class="delete-deps-container">
            <div class="delete-deps-left">
              <label class="custom-checkbox">
                <input type="checkbox" v-model="deleteConfirm.includeDependencies" :disabled="deleteLoading" />
                <span class="checkbox-box"></span>
                <span class="checkbox-text">{{ $t('packages.deleteIncludeDeps') }}</span>
              </label>
            </div>
            <div class="delete-deps-right">
              <div class="delete-hint-card" :class="{ active: deleteConfirm.includeDependencies }">
                <svg width="14" height="14" viewBox="0 0 24 24" fill="none" class="hint-icon">
                  <path d="M12 9v4M12 17h.01M12 3a9 9 0 1 1 0 18 9 9 0 0 1 0-18Z" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"/>
                </svg>
                <span class="hint-text">
                  {{ deleteConfirm.includeDependencies ? $t('packages.quickDeleteDepsScope') : $t('packages.deleteIncludeDepsHint') }}
                </span>
              </div>
            </div>
          </div>
        </div>
        <div class="delete-confirm-actions">
          <button class="delete-cancel-btn" :disabled="deleteLoading" @click="cancelQuickDelete">
            {{ $t('common.cancel') }}
          </button>
          <button class="delete-confirm-btn" :disabled="deleteLoading" @click="confirmQuickDelete">
            {{ $t('packages.confirmDelete') }}
          </button>
        </div>
      </div>
    </div>

  </div>
</template>

<script setup lang="ts">
import { ref, watch, computed, onMounted, onUnmounted, nextTick } from 'vue'
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

interface HubFile {
  filename?: string | null
  file_size?: string | null
  urlHosted?: string | null
}

interface HubPackageInfo {
  hubFiles?: HubFile[] | null
}

interface HubFileCandidate {
  filename: string
  version: number
  sizeBytes: number
  url: string
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
  const fullName = getVarFileName(selectedPackage.value)
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
const dependencyPanels: Array<{ key: DependencyPanelKey; label: string }> = [
  { key: 'direct', label: '依赖列表' },
  { key: 'sub', label: '子依赖' },
  { key: 'reverse', label: '被依赖' },
]
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
      statusText: '引用',
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
  if (dep.status === 'lower_version') return '版本不足'
  if (dep.status === 'missing') return '缺失'
  return '拥有'
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

function selectHubFile(files: HubFileCandidate[], requiredVersion: number | null) {
  if (files.length === 0) return null
  const sorted = [...files].sort((a, b) => a.version - b.version)
  if (requiredVersion === null) return sorted[sorted.length - 1] || null
  return sorted.find((file) => file.version >= requiredVersion) || null
}

function parseHubFiles(files: HubFile[]) {
  return files
    .map((file) => {
      if (!file.filename || !file.urlHosted) return null
      const parsed = parsePackageId(file.filename)
      if (!parsed || parsed.version === null) return null
      return {
        filename: file.filename,
        version: parsed.version,
        sizeBytes: parseSizeValue(file.file_size),
        url: file.urlHosted,
      }
    })
    .filter((item): item is HubFileCandidate => Boolean(item))
}

function parsePackageId(packageId: string) {
  const cleaned = packageId.replace(/\.var$/i, '')
  const parts = cleaned.split('.')
  if (parts.length < 2) return null

  const creator = parts[0]
  if (parts.length === 2) {
    return { creator, name: parts[1], version: null as number | null }
  }

  const versionText = parts[parts.length - 1]
  const name = parts.slice(1, -1).join('.')
  const version = Number.parseInt(versionText, 10)
  return {
    creator,
    name,
    version: Number.isFinite(version) ? version : null,
  }
}

function parseRequiredVersion(requiredVersion: string) {
  const text = requiredVersion.trim()
  if (!text || text.toLowerCase() === 'latest') return null
  const version = Number.parseInt(text, 10)
  return Number.isFinite(version) ? version : null
}

function parseSizeValue(value: string | null | undefined) {
  if (!value) return 0
  const text = String(value).trim().toUpperCase()
  const match = text.match(/^([\d.]+)\s*(B|KB|MB|GB|TB)?$/)
  if (!match) return 0
  const size = Number.parseFloat(match[1])
  const unit = match[2] || 'B'
  const map: Record<string, number> = {
    B: 1,
    KB: 1024,
    MB: 1024 * 1024,
    GB: 1024 * 1024 * 1024,
    TB: 1024 * 1024 * 1024 * 1024,
  }
  return Math.round(size * (map[unit] || 1))
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
function formatSize(bytes: number): string {
  if (bytes === 0) return '0 B'
  const units = ['B', 'KB', 'MB', 'GB', 'TB']
  const i = Math.floor(Math.log(bytes) / Math.log(1024))
  return `${(bytes / Math.pow(1024, i)).toFixed(i > 0 ? 1 : 0)} ${units[i]}`
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
</script>

<style scoped>
.resource-display {
  display: flex; flex-direction: column; gap: var(--space-5);
  height: 100%; min-width: 0; min-height: 0; position: relative; overflow: hidden;
}

/* ── View Controls ────────────────────────────────────────── */
.view-controls {
  position: relative;
  z-index: calc(var(--z-dropdown) + 1);
  display: flex; align-items: center; justify-content: space-between; flex-wrap: wrap; gap: var(--space-3);
  padding: var(--space-3) var(--space-4); border-radius: var(--radius-md); flex-shrink: 0;
  overflow: visible;
}
.controls-left, .controls-right { display: flex; align-items: center; gap: var(--space-3); min-width: 0; }
.controls-right { justify-content: flex-end; flex-wrap: wrap; }
.view-toggle { display: flex; background: var(--bg-base); border-radius: var(--radius-sm); padding: 2px; gap: 2px; }
.toggle-btn {
  display: flex; align-items: center; justify-content: center;
  width: 30px; height: 28px; border-radius: var(--radius-xs);
  color: var(--text-tertiary); cursor: pointer;
  transition: color var(--duration-fast) var(--ease), background var(--duration-fast) var(--ease);
}
.toggle-btn:hover { color: var(--text-secondary); }
.toggle-btn.active { color: var(--text-primary); background: var(--bg-elevated); }
.result-count { padding-left: var(--space-2); border-left: 1px solid var(--border-subtle); }

/* ── Content + Pagination ─────────────────────────────────── */
.resource-content-area {
  position: relative;
  z-index: var(--z-base);
  flex: 1; display: flex; flex-direction: column; min-height: 0; gap: var(--space-3);
}
.resource-content-scrollable { flex: 1; overflow-y: auto; overflow-x: hidden; min-height: 0; min-width: 0; }

/* ── Large Card Grid ──────────────────────────────────────── */
.large-card-grid { display: grid; grid-template-columns: repeat(auto-fill, minmax(280px, 1fr)); gap: var(--space-4); }
.large-card {
  display: flex; flex-direction: column; cursor: pointer;
  background: var(--glass-bg); backdrop-filter: var(--glass-blur);
  border: 1px solid var(--border-default); border-radius: var(--radius-lg); overflow: hidden;
  transition: transform var(--duration-base) var(--ease), box-shadow var(--duration-base) var(--ease), border-color var(--duration-base) var(--ease);
}
.large-card:hover { transform: translateY(-2px); box-shadow: var(--glass-shadow-lg); border-color: var(--border-strong); }

.large-card-thumb { position: relative; width: 100%; height: 160px; background: #000; overflow: hidden; display: flex; align-items: center; justify-content: center; }
.large-card-thumb .thumb-img { width: 100%; height: 100%; object-fit: contain; }
.large-card-thumb .thumb-placeholder {
  display: flex; flex-direction: column; align-items: center; justify-content: center; gap: var(--space-2);
  width: 100%; height: 100%; color: var(--text-tertiary); opacity: 0.7; font-size: var(--text-xs);
}

.large-card-type-badge {
  position: absolute; top: var(--space-2); right: var(--space-2);
  font-size: 10px; font-weight: var(--font-semibold); text-transform: uppercase; letter-spacing: 0.06em;
  padding: 2px 8px; border-radius: var(--radius-full); background: var(--glass-bg); /* backdrop-filter removed */
}
.large-card-type-badge.type-scene { color: var(--color-scene); }
.large-card-type-badge.type-appearance { color: var(--color-appearance); }
.large-card-type-badge.type-morph { color: var(--color-morph); }
.large-card-type-badge.type-clothing { color: var(--color-clothing); }
.large-card-type-badge.type-plugin { color: var(--color-plugin); }
.large-card-type-badge.type-texture { color: var(--color-texture); }
.large-card-type-badge.type-other { color: var(--text-tertiary); }

.large-card-info { padding: var(--space-3) var(--space-4); display: flex; flex-direction: column; gap: var(--space-2); }
.large-card-name {
  font-size: var(--text-sm); font-weight: var(--font-medium); color: var(--text-primary);
  overflow: hidden; text-overflow: ellipsis; white-space: nowrap;
}
.large-card-meta { display: flex; gap: var(--space-2); flex-wrap: wrap; }
.meta-tag {
  font-size: 11px; color: var(--text-tertiary); padding: 1px 6px;
  border-radius: var(--radius-sm); background: var(--bg-hover); font-variant-numeric: tabular-nums;
}
.large-card-stats { display: flex; gap: var(--space-4); padding-top: var(--space-2); border-top: 1px solid var(--border-subtle); }
.stat-item { display: flex; flex-direction: column; align-items: center; gap: 2px; flex: 1; }
.stat-value { font-size: var(--text-sm); font-weight: var(--font-semibold); color: var(--text-primary); font-variant-numeric: tabular-nums; }
.stat-label { font-size: 10px; color: var(--text-tertiary); text-transform: uppercase; letter-spacing: 0.04em; }

/* ── Small Card Grid ──────────────────────────────────────── */
.small-card-grid { display: grid; grid-template-columns: repeat(auto-fill, minmax(120px, 1fr)); gap: var(--space-3); }
.small-card {
  cursor: pointer; background: var(--glass-bg); backdrop-filter: var(--glass-blur);
  border: 1px solid var(--border-default); border-radius: var(--radius-md); overflow: hidden;
  transition: transform var(--duration-fast) var(--ease), box-shadow var(--duration-fast) var(--ease), border-color var(--duration-fast) var(--ease);
}
.small-card:hover { transform: translateY(-2px); box-shadow: var(--glass-shadow); border-color: var(--border-strong); }
.small-card-thumb { width: 100%; aspect-ratio: 1; background: #000; overflow: hidden; display: flex; align-items: center; justify-content: center; }
.small-card-thumb .thumb-img { width: 100%; height: 100%; object-fit: contain; }
.small-card-thumb .thumb-placeholder.sm {
  display: flex; flex-direction: column; align-items: center; justify-content: center; gap: var(--space-1);
  width: 100%; height: 100%; color: var(--text-tertiary); opacity: 0.7; font-size: 10px;
}
.small-card-name {
  padding: var(--space-2); font-size: 11px; font-weight: var(--font-medium);
  color: var(--text-primary); text-align: center; overflow: hidden; text-overflow: ellipsis; white-space: nowrap;
}

/* ── List View ────────────────────────────────────────────── */
.resource-list { overflow: hidden; }
.list-header {
  display: grid; grid-template-columns: 72px 1fr 80px 60px 60px 100px; gap: var(--space-3);
  padding: var(--space-3) var(--space-4); border-bottom: 1px solid var(--border-subtle);
  font-weight: var(--font-semibold); text-transform: uppercase; letter-spacing: 0.06em; align-items: center;
}
.list-row {
  display: grid; grid-template-columns: 72px 1fr 80px 60px 60px 100px; gap: var(--space-3);
  padding: var(--space-2) var(--space-4); cursor: pointer; align-items: center;
  transition: background var(--duration-fast) var(--ease);
}
.list-row:hover { background: var(--bg-hover); }
.list-row + .list-row { border-top: 1px solid var(--border-subtle); }
.list-cell { font-size: var(--text-sm); color: var(--text-primary); }
.list-cell.name { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.list-cell.size { font-variant-numeric: tabular-nums; color: var(--text-secondary); }
.list-cell.deps { font-variant-numeric: tabular-nums; color: var(--text-secondary); }
.col-thumb { display: flex; align-items: center; }
.list-thumb-img { width: 28px; height: 28px; border-radius: var(--radius-xs); object-fit: cover; }
.list-thumb-placeholder {
  display: flex; align-items: center; gap: var(--space-1);
  color: var(--text-tertiary); font-size: 10px; white-space: nowrap;
}
.type-dot-sm { width: 10px; height: 10px; border-radius: 50%; }
.type-dot-sm.type-scene { background: var(--color-scene); }
.type-dot-sm.type-appearance { background: var(--color-appearance); }
.type-dot-sm.type-morph { background: var(--color-morph); }
.type-dot-sm.type-clothing { background: var(--color-clothing); }
.type-dot-sm.type-plugin { background: var(--color-plugin); }
.type-dot-sm.type-texture { background: var(--color-texture); }

/* ── Pagination ───────────────────────────────────────────── */
.pagination-bar {
  display: flex; align-items: center; justify-content: space-between;
  padding: var(--space-2) var(--space-4); border-radius: var(--radius-md); flex-shrink: 0;
}
.pagination-info { font-variant-numeric: tabular-nums; }
.pagination-controls { display: flex; align-items: center; gap: var(--space-1); }
.page-size-select {
  padding: 2px var(--space-2); height: 26px; background: var(--bg-base);
  border: 1px solid var(--border-subtle); border-radius: var(--radius-sm);
  color: var(--text-secondary); cursor: pointer; margin-right: var(--space-2);
}
.page-size-select:focus { border-color: var(--accent-primary); }
.page-btn {
  display: flex; align-items: center; justify-content: center; width: 26px; height: 26px;
  border-radius: var(--radius-xs); color: var(--text-secondary);
  transition: color var(--duration-fast) var(--ease), background var(--duration-fast) var(--ease);
}
.page-btn:hover:not(:disabled) { color: var(--text-primary); background: var(--bg-hover); }
.page-btn:disabled { opacity: 0.3; cursor: not-allowed; }
.page-indicator {
  font-variant-numeric: tabular-nums; color: var(--text-primary);
  font-weight: var(--font-medium); min-width: 70px; text-align: center;
}

/* ── Empty State ──────────────────────────────────────────── */
.empty-wrapper { flex: 1; display: flex; align-items: center; justify-content: center; }

/* ── Detail Panel ─────────────────────────────────────────── */
.detail-panel {
  position: absolute; top: 0; right: 0; width: min(520px, 88vw); height: 100%; z-index: calc(var(--z-overlay) + 10);
  border-radius: 0; border-left: 1px solid var(--border-subtle);
  background: var(--bg-surface);
  display: flex; flex-direction: column;
  box-shadow: -24px 0 60px rgba(0, 0, 0, 0.35);
}
.detail-header { display: flex; align-items: center; gap: var(--space-3); padding: var(--space-4); border-bottom: 1px solid var(--border-subtle); }
.detail-close {
  display: flex; align-items: center; justify-content: center; width: 28px; height: 28px;
  border-radius: var(--radius-sm); color: var(--text-secondary);
  transition: color var(--duration-fast) var(--ease), background var(--duration-fast) var(--ease);
}
.detail-close:hover { color: var(--text-primary); background: var(--bg-hover); }
.detail-copy-btn {
  display: flex; align-items: center; justify-content: center; width: 28px; height: 28px;
  border-radius: var(--radius-sm); color: var(--text-secondary);
  transition: color var(--duration-fast) var(--ease), background var(--duration-fast) var(--ease);
  cursor: pointer;
}
.detail-copy-btn:hover { color: var(--text-primary); background: var(--bg-hover); }
.detail-copy-btn:active { transform: scale(0.92); }
.detail-title {
  flex: 1;
  min-width: 0;
  font-weight: var(--font-semibold);
  color: var(--text-primary);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.detail-actions {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  flex-shrink: 0;
}
.detail-body { flex: 1; overflow-y: auto; padding: var(--space-5); display: flex; flex-direction: column; gap: var(--space-4); }
.detail-body > * { flex-shrink: 0; }
.detail-thumb { width: 100%; height: 220px; border-radius: var(--radius-md); overflow: hidden; background: #000; display: flex; align-items: center; justify-content: center; }
.detail-thumb-img { width: 100%; height: 100%; object-fit: contain; }
.detail-thumb-placeholder {
  display: flex; flex-direction: column; align-items: center; justify-content: center; gap: var(--space-2);
  width: 100%; height: 100%; color: var(--text-tertiary); opacity: 0.7; font-size: var(--text-xs);
}
.detail-info-grid {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: var(--space-3);
}
.detail-section {
  min-width: 0;
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
  padding: var(--space-3);
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-md);
  background: rgba(255, 255, 255, 0.025);
}
.detail-label { font-weight: var(--font-semibold); text-transform: uppercase; letter-spacing: 0.06em; }
.detail-value { min-width: 0; font-size: var(--text-sm); color: var(--text-primary); display: flex; align-items: center; gap: var(--space-2); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.detail-muted { font-size: var(--text-xs); color: var(--text-tertiary); }
.dependency-card {
  padding: var(--space-3);
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-md);
  background: rgba(255, 255, 255, 0.025);
}
.dependency-card-title {
  margin-bottom: var(--space-2);
  color: var(--text-primary);
  font-size: var(--text-sm);
  font-weight: var(--font-semibold);
}
.dependency-groups {
  overflow: hidden;
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-sm);
  background: rgba(255, 255, 255, 0.018);
}
.dependency-group + .dependency-group { border-top: 1px solid var(--border-subtle); }
.dependency-toggle {
  display: flex;
  align-items: center;
  width: 100%;
  min-height: 42px;
  padding: 0 var(--space-3);
  color: var(--text-secondary);
  text-align: left;
  transition: color var(--duration-fast) var(--ease), background var(--duration-fast) var(--ease);
}
.dependency-toggle:hover {
  color: var(--text-primary);
  background: rgba(255, 255, 255, 0.035);
}
.dependency-caret {
  flex-shrink: 0;
  margin-right: var(--space-2);
  transition: transform var(--duration-fast) var(--ease);
}
.dependency-caret.expanded { transform: rotate(90deg); }
.dependency-toggle-label {
  font-size: var(--text-sm);
  font-weight: var(--font-medium);
}
.dependency-toggle-count {
  margin-left: var(--space-1);
  color: var(--text-tertiary);
  font-size: var(--text-xs);
}
.dependency-panel-body {
  padding: 0 var(--space-3) var(--space-2) calc(var(--space-3) + 22px);
}
.dependency-row-list {
  max-height: 280px;
  overflow-y: auto;
}
.dependency-row {
  display: grid;
  grid-template-columns: minmax(0, 1fr) auto auto;
  align-items: center;
  gap: var(--space-2);
  min-height: 34px;
  border-top: 1px solid rgba(255, 255, 255, 0.055);
}
.dependency-row-name {
  min-width: 0;
  overflow: hidden;
  color: var(--text-secondary);
  font-size: var(--text-xs);
  text-overflow: ellipsis;
  white-space: nowrap;
}
.dependency-row-tier {
  color: var(--text-tertiary);
  font-size: var(--text-xs);
  font-variant-numeric: tabular-nums;
}
.dependency-row-status {
  padding: 2px 7px;
  border-radius: var(--radius-xs);
  font-size: var(--text-xs);
  font-weight: var(--font-semibold);
  line-height: 1.4;
}
.dependency-row-status.owned {
  color: var(--color-success);
  background: rgba(52, 211, 153, 0.12);
}
.dependency-row-status.referenced {
  color: var(--accent-secondary);
  background: rgba(91, 141, 239, 0.12);
}
.dependency-row-status.missing,
.dependency-row-status.lower-version {
  color: var(--color-error);
  background: rgba(248, 113, 113, 0.12);
}
.dependency-empty {
  display: block;
  padding: var(--space-2) 0;
  color: var(--text-tertiary);
  font-size: var(--text-xs);
}
.dependency-empty.error { color: var(--color-error); }
.detail-image-list { display: flex; flex-direction: column; gap: var(--space-2); min-width: 0; }
.detail-image-summary { font-size: var(--text-xs); color: var(--text-secondary); }
.detail-image-grid {
  display: grid;
  grid-template-columns: repeat(3, minmax(0, 1fr));
  gap: var(--space-2);
}
.detail-image-tile {
  position: relative;
  display: flex;
  align-items: center;
  justify-content: center;
  aspect-ratio: 1.25;
  min-width: 0;
  overflow: hidden;
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-sm);
  background: #050508;
  cursor: zoom-in;
}
.detail-image-tile:hover { border-color: var(--border-strong); }
.detail-image-tile img { width: 100%; height: 100%; object-fit: cover; }
.detail-image-loading {
  padding: var(--space-2);
  color: var(--text-tertiary);
  font-size: var(--text-xs);
}
.detail-image-size {
  position: absolute;
  right: 4px;
  bottom: 4px;
  padding: 1px 5px;
  border-radius: var(--radius-xs);
  background: rgba(0, 0, 0, 0.62);
  font-size: var(--text-xs);
  color: var(--text-secondary);
  font-variant-numeric: tabular-nums;
}
.detail-image-toggle {
  align-self: flex-start;
  color: var(--accent-secondary);
  font-size: var(--text-xs);
}
.type-dot-detail { width: 8px; height: 8px; border-radius: 50%; flex-shrink: 0; }
.type-dot-detail.type-scene { background: var(--color-scene); }
.type-dot-detail.type-appearance { background: var(--color-appearance); }
.type-dot-detail.type-morph { background: var(--color-morph); }
.type-dot-detail.type-clothing { background: var(--color-clothing); }
.type-dot-detail.type-plugin { background: var(--color-plugin); }
.type-dot-detail.type-texture { background: var(--color-texture); }

/* ── Slide Transition ─────────────────────────────────────── */
.slide-right-enter-active, .slide-right-leave-active { transition: transform var(--duration-base) var(--ease); }
.slide-right-enter-from, .slide-right-leave-to { transform: translateX(100%); }

.share-btn-primary {
  flex-shrink: 0;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  gap: var(--space-2);
  height: 30px;
  padding: 0 var(--space-3);
  background: var(--accent-gradient);
  color: white;
  font-size: var(--text-xs);
  font-weight: var(--font-semibold);
  border-radius: var(--radius-sm);
  border: none;
  cursor: pointer;
  box-shadow: 0 4px 12px rgba(110, 107, 240, 0.25);
  transition: all var(--transition-fast) var(--ease);
}

.share-btn-primary:hover {
  transform: translateY(-1px);
  box-shadow: 0 6px 16px rgba(110, 107, 240, 0.35);
}

.share-btn-primary:active {
  transform: translateY(0);
}

.btn-icon {
  flex-shrink: 0;
}

.quick-delete-btn {
  flex-shrink: 0;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  gap: var(--space-2);
  height: 30px;
  padding: 0 var(--space-3);
  color: var(--color-warning);
  font-size: var(--text-xs);
  font-weight: var(--font-semibold);
  border-radius: var(--radius-sm);
  background: rgba(251, 191, 36, 0.1);
  transition: color var(--duration-fast) var(--ease), background var(--duration-fast) var(--ease), opacity var(--duration-fast) var(--ease);
}

.quick-delete-btn:hover:not(:disabled) {
  background: rgba(251, 191, 36, 0.16);
}

.quick-delete-btn.danger {
  color: var(--color-error);
  background: rgba(248, 113, 113, 0.1);
}

.quick-delete-btn.danger:hover:not(:disabled) {
  background: rgba(248, 113, 113, 0.16);
}

.quick-delete-btn:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}

.image-preview-overlay {
  position: fixed;
  inset: 0;
  z-index: calc(var(--z-overlay) + 20);
  display: flex;
  align-items: center;
  justify-content: center;
  padding: var(--space-6);
  background: rgba(0, 0, 0, 0.72);
  /* backdrop-filter removed */
}

.image-preview-dialog {
  width: min(920px, 92vw);
  max-height: 88vh;
  display: flex;
  flex-direction: column;
  overflow: hidden;
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-lg);
  background: rgba(15, 15, 28, 0.96);
  box-shadow: var(--glass-shadow-lg);
}

.image-preview-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--space-3);
  padding: var(--space-3) var(--space-4);
  border-bottom: 1px solid var(--border-subtle);
}

.image-preview-title {
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  color: var(--text-secondary);
  font-size: var(--text-xs);
}

.image-preview-img {
  width: 100%;
  max-height: calc(88vh - 54px);
  object-fit: contain;
  background: #000;
}

.image-preview-loading {
  height: 420px;
  display: flex;
  align-items: center;
  justify-content: center;
  color: var(--text-secondary);
}

.delete-confirm-overlay {
  position: fixed;
  inset: 0;
  z-index: calc(var(--z-overlay) + 30);
  display: flex;
  align-items: center;
  justify-content: center;
  padding: var(--space-5);
  background: rgba(0, 0, 0, 0.68);
  /* backdrop-filter removed */
}

.delete-confirm-dialog {
  width: min(440px, 92vw);
  overflow: hidden;
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-md);
  background: rgba(18, 18, 32, 0.96);
  box-shadow: var(--glass-shadow-lg);
}

.delete-confirm-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--space-3);
  padding: var(--space-4);
  border-bottom: 1px solid var(--border-subtle);
}

.delete-confirm-header h3 {
  margin: 0;
  color: var(--text-primary);
  font-size: var(--text-md);
  font-weight: var(--font-semibold);
}

.delete-confirm-body {
  display: flex;
  flex-direction: column;
  gap: var(--space-3);
  padding: var(--space-4);
  color: var(--text-secondary);
  font-size: var(--text-sm);
  line-height: 1.6;
}

.delete-confirm-body p {
  margin: 0;
}

.delete-confirm-hint {
  padding: var(--space-3);
  border: 1px solid rgba(251, 191, 36, 0.22);
  border-radius: var(--radius-sm);
  background: rgba(251, 191, 36, 0.08);
  color: var(--color-warning);
}

.delete-confirm-actions {
  display: flex;
  justify-content: flex-end;
  gap: var(--space-2);
  padding: var(--space-4);
  border-top: 1px solid var(--border-subtle);
}

.delete-cancel-btn,
.delete-confirm-btn {
  height: 32px;
  padding: 0 var(--space-4);
  border-radius: var(--radius-sm);
  font-size: var(--text-sm);
  font-weight: var(--font-semibold);
  transition: opacity var(--duration-fast) var(--ease), background var(--duration-fast) var(--ease);
}

.delete-cancel-btn {
  color: var(--text-secondary);
  background: var(--bg-hover);
}

.delete-confirm-btn {
  color: white;
  background: var(--color-error);
}

.delete-cancel-btn:hover:not(:disabled) {
  color: var(--text-primary);
}

.delete-confirm-btn:hover:not(:disabled) {
  opacity: 0.88;
}

.delete-cancel-btn:disabled,
.delete-confirm-btn:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}

/* ── Elegant Action Bar above the image cover ── */
.detail-actions-bar {
  display: flex;
  flex-wrap: wrap;
  gap: var(--space-2);
  margin-bottom: var(--space-4);
  width: 100%;
}

.action-bar-btn {
  flex: 1 1 calc(50% - var(--space-2));
  display: inline-flex;
  align-items: center;
  justify-content: center;
  gap: var(--space-2);
  height: 32px;
  padding: 0 var(--space-3);
  border-radius: var(--radius-sm);
  border: 1px solid var(--border-subtle);
  background: rgba(255, 255, 255, 0.03);
  color: var(--text-secondary);
  font-size: var(--text-xs);
  font-weight: var(--font-semibold);
  min-width: 0;
  cursor: pointer;
  transition: all var(--transition-fast) var(--ease);
}

.action-bar-btn .btn-icon {
  flex-shrink: 0;
}

.action-bar-btn:hover:not(:disabled) {
  background: rgba(255, 255, 255, 0.08);
  color: var(--text-primary);
  border-color: rgba(255, 255, 255, 0.15);
  transform: translateY(-1px);
}

.action-bar-btn:active:not(:disabled) {
  transform: translateY(0);
}

.action-bar-btn:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}

.action-bar-btn.launch-scene,
.action-bar-btn.extract-preset[aria-expanded="true"] {
  background: var(--accent-muted);
  color: var(--accent-primary);
  border-color: var(--accent-primary);
}

.action-bar-btn.share {
  background: rgba(110, 107, 240, 0.1);
  color: var(--accent-primary);
  border-color: rgba(110, 107, 240, 0.22);
}

.action-bar-btn.share:hover {
  background: rgba(110, 107, 240, 0.18);
  color: #a78bfa;
  border-color: rgba(110, 107, 240, 0.4);
}

.action-bar-btn.delete {
  background: rgba(248, 113, 113, 0.1);
  color: var(--color-error);
  border-color: rgba(248, 113, 113, 0.22);
}

.action-bar-btn.delete:hover {
  background: rgba(248, 113, 113, 0.18);
  color: #fca5a5;
  border-color: rgba(248, 113, 113, 0.4);
}

.action-bar-btn.complete-deps {
  background: rgba(52, 211, 153, 0.1);
  color: var(--color-success);
  border-color: rgba(52, 211, 153, 0.22);
}

.action-bar-btn.complete-deps:hover {
  background: rgba(52, 211, 153, 0.18);
  color: #86efac;
  border-color: rgba(52, 211, 153, 0.4);
}

.action-bar-btn.open-location {
  background: rgba(96, 165, 250, 0.1);
  color: var(--color-info);
  border-color: rgba(96, 165, 250, 0.22);
}

.action-bar-btn.open-location:hover {
  background: rgba(96, 165, 250, 0.18);
  color: #93c5fd;
  border-color: rgba(96, 165, 250, 0.4);
}

/* ── Delete Dependencies Checkbox & Hint Layout ── */
.delete-confirm-dialog {
  width: min(520px, 92vw) !important;
}

.delete-main-msg {
  margin-bottom: var(--space-4) !important;
}

.delete-deps-container {
  display: flex;
  flex-direction: column;
  gap: var(--space-4);
  margin-top: var(--space-2);
  padding-top: var(--space-4);
  border-top: 1px solid var(--border-subtle);
}

.delete-deps-left {
  display: flex;
  flex-direction: column;
  justify-content: center;
}

.custom-checkbox {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  cursor: pointer;
  user-select: none;
  font-size: var(--text-sm);
  color: var(--text-primary);
  font-weight: var(--font-semibold);
}

.custom-checkbox input {
  position: absolute;
  opacity: 0;
  cursor: pointer;
  height: 0;
  width: 0;
}

.checkbox-box {
  width: 18px;
  height: 18px;
  border-radius: var(--radius-sm);
  border: 1px solid var(--border-strong);
  background: rgba(255, 255, 255, 0.05);
  display: flex;
  align-items: center;
  justify-content: center;
  transition: all var(--transition-fast) var(--ease);
  flex-shrink: 0;
}

.custom-checkbox:hover input ~ .checkbox-box {
  background: rgba(255, 255, 255, 0.1);
  border-color: var(--text-primary);
}

.custom-checkbox input:checked ~ .checkbox-box {
  background: var(--accent-gradient);
  border-color: transparent;
}

.checkbox-box::after {
  content: "";
  display: none;
  width: 4px;
  height: 8px;
  border: solid white;
  border-width: 0 2px 2px 0;
  transform: rotate(45deg);
  margin-bottom: 2px;
}

.custom-checkbox input:checked ~ .checkbox-box::after {
  display: block;
}

.delete-hint-card {
  display: flex;
  gap: var(--space-2);
  padding: var(--space-3);
  background: rgba(255, 255, 255, 0.03);
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-md);
  color: var(--text-secondary);
  line-height: 1.5;
  transition: all var(--transition-normal) var(--ease);
}

.delete-hint-card.active {
  background: rgba(248, 113, 113, 0.08);
  border-color: rgba(248, 113, 113, 0.22);
  color: var(--color-error);
}

.hint-icon {
  flex-shrink: 0;
  margin-top: 2px;
}

.hint-text {
  font-size: var(--text-xs);
}

@media (max-width: 720px) {
  .detail-panel { width: 100%; }
  .detail-header { flex-wrap: wrap; }
  .detail-actions { width: 100%; justify-content: flex-end; flex-wrap: wrap; }
  .detail-body { padding: var(--space-4); }
  .detail-thumb { height: 180px; }
  .detail-info-grid { grid-template-columns: 1fr; }
  .detail-image-grid { grid-template-columns: repeat(2, minmax(0, 1fr)); }
}
</style>
