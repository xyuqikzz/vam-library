<template>
  <div class="online-hub-view animate-fadeIn b2-page">
    <PageHeader :title="$t('sidebar.onlineHub')">
      <template #actions>
        <div class="hub-search-bar">
          <UiInput
            v-model="searchQuery"
            type="text"
            class="hub-search-input"
            :placeholder="t('online.searchPlaceholder')"
            @keyup.enter="handleSearch"
            search
          />
          <UiButton :disabled="loading" @click="handleSearch" variant="secondary">
            <Icon v-if="loading" class="spinner" :size="14" name="outline-34" />
            <Icon v-else :size="14" name="outline-35" />
            <span>{{ t('online.search') }}</span>
          </UiButton>
        </div>
      </template>
      <template #filters>
        <div class="hub-filters">
        <!-- Pricing Filter -->
        <div class="filter-group">
          <select v-model="selectedPricing" class="hub-select" @change="resetAndSearch">
            <option value="free">{{ t('online.filters.free') }}</option>
            <option value="all">{{ t('online.filters.allPricing') }}</option>
            <option value="premium">{{ t('online.filters.premium') }}</option>
          </select>
        </div>

        <!-- Category Select -->
        <div class="filter-group">
          <select v-model="selectedCategory" class="hub-select" @change="resetAndSearch">
            <option value="">{{ t('online.filters.allCategories') }}</option>
            <option value="Scenes">{{ t('online.types.scenes') }}</option>
            <option value="Looks">{{ t('online.types.looks') }}</option>
            <option value="Clothing">{{ t('online.types.clothing') }}</option>
            <option value="Hairstyles">{{ t('online.types.hair') }}</option>
            <option value="Plugins + Scripts">{{ t('online.types.plugins') }}</option>
            <option value="Morphs">{{ t('online.types.morphs') }}</option>
            <option value="Assets + Accessories">{{ t('online.types.assets') }}</option>
            <option value="Toolkits + Templates">{{ t('online.types.toolkits') }}</option>
            <option value="Audio">{{ t('online.types.audio') }}</option>
          </select>
        </div>

        <!-- Sorting Select -->
        <div class="filter-group">
          <select v-model="selectedSort" class="hub-select" @change="resetAndSearch">
            <option value="last_update">{{ t('online.sort.lastUpdate') }}</option>
            <option value="resource_date">{{ t('online.sort.resourceDate') }}</option>
            <option value="reaction_score">{{ t('online.sort.reactionScore') }}</option>
            <option value="download_count">{{ t('online.sort.downloads') }}</option>
            <option value="rating_weighted">{{ t('online.sort.rating') }}</option>
            <option value="rating_count">{{ t('online.sort.ratingCount') }}</option>
            <option value="title">{{ t('online.sort.title') }}</option>
            <option value="username">{{ t('online.sort.creator') }}</option>
          </select>
        </div>

        </div>
      </template>
    </PageHeader>
    <div class="b2-page-body">
      <!-- Header Controls -->

      <!-- Error State -->
      <div v-if="error" class="hub-error-msg animate-fadeIn">
        <Icon :size="18" name="outline-14" />
        <span>{{ error }}</span>
      </div>

      <!-- Main Content Area -->
      <div class="hub-workspace">
      <div
        :class="['hub-content-scroll', { 'is-loading': loading && filteredAndSortedResources.length > 0 }]"
        ref="scrollContainer"
      >
        <div class="hub-results-section">
          <!-- Loading Overlay -->
          <div v-if="loading && filteredAndSortedResources.length === 0" class="hub-loading-state">
            <div class="loading-spinner"></div>
            <p class="text-secondary text-sm">{{ t('online.loading') }}</p>
          </div>

          <div
            v-if="loading && filteredAndSortedResources.length > 0"
            class="hub-inline-loading-overlay"
          >
            <div class="hub-inline-loading-card">
              <div class="loading-spinner"></div>
              <p class="text-secondary text-sm">{{ t('online.loadingPage') }}</p>
            </div>
          </div>

          <!-- Resources Grid -->
          <div v-else-if="filteredAndSortedResources.length > 0" class="resources-grid resource-grid">
            <PackageCard
              v-for="item in filteredAndSortedResources"
              :key="item.resource_id"
              :item="{ id: String(item.resource_id), name: item.title || $t('ui.untitled'), creator: item.username || $t('ui.unknown'), resource_types: [hubCardType(item.type)] }"
              :type-label="translateType(item.type)"
              :image="item.image_url || item.icon_url"
              :title="item.tag_line"
              @select="openPackageDetail(item)"
              @details="openPackageDetail(item)"
              @more="openPackageDetail(item)"
            >
              <template #meta>
                <span>{{ item.username }}</span>
                <span
                  :title="$t('ui.hubMetrics', { downloads: item.download_count, views: item.view_count, rating: item.rating_avg || '—' })"
                >
                  <Icon name="download" :size="12" />
                  {{ formatCompactNumber(item.download_count) }}
                </span>
              </template>
              <template #badge>
                <span v-if="item.category" class="hub-pricing">
                  {{ $t(item.category === 'Free' ? 'ui.free' : 'ui.premium') }}
                </span>
              </template>
            </PackageCard>
          </div>

          <!-- Empty State -->
          <div v-else-if="!loading" class="hub-empty-state animate-fadeIn">
            <Icon :size="48" name="outline-35" />
            <h3>{{ t('online.emptyTitle') }}</h3>
            <p class="text-secondary text-sm">{{ t('online.emptyDesc') }}</p>
          </div>
        </div>
      </div>

      <!-- Keep pagination outside the scroll area so it stays visible without covering cards. -->
      <!-- 与包管理等页面相同的紧凑分页：左边总数，右边页码 -->
      <div
        v-if="filteredAndSortedResources.length > 0 && totalPages > 1"
        class="hub-pagination-bar"
      >
        <span class="pagination-info">{{ t('online.foundCount', { found: totalFound }) }}</span>
        <div class="pagination-controls">
          <UiButton
            size="sm"
            variant="ghost"
            icon="chevrons-left"
            icon-only
            :aria-label="t('ui.firstPage')"
            :disabled="currentPage <= 1 || loading"
            @click="changePage(1)"
          />
          <UiButton
            size="sm"
            variant="ghost"
            icon="chevron-left"
            icon-only
            :aria-label="t('online.previousPage')"
            :disabled="currentPage <= 1 || loading"
            @click="changePage(currentPage - 1)"
          />
          <span class="page-indicator">{{ currentPage }} / {{ totalPages }}</span>
          <UiButton
            size="sm"
            variant="ghost"
            icon="chevron-right"
            icon-only
            :aria-label="t('online.nextPage')"
            :disabled="currentPage >= totalPages || loading"
            @click="changePage(currentPage + 1)"
          />
          <UiButton
            size="sm"
            variant="ghost"
            icon="chevrons-right"
            icon-only
            :aria-label="t('ui.lastPage')"
            :disabled="currentPage >= totalPages || loading"
            @click="changePage(totalPages)"
          />
        </div>
      </div>

      <!-- 资源详情：和本地资源详情同一个弹窗，左图右信息 -->
      <UiDetailModal
        v-if="detailPanelOpen"
        class="hub-detail-modal"
        :label="detailPackage.title || t('online.detailTitle')"
        :fallback-image="detailPackage.image_url || null"
        :content-key="detailPackage.resource_id"
        @close="closeDetailPanel"
      >
        <template #placeholder>
          <img
            v-if="detailPackage.icon_url"
            data-resource-preview
            :src="detailPackage.icon_url"
            class="panel-icon-img"
            alt="icon"
          />
          <Icon v-else :size="40" name="outline-36" class="panel-img-placeholder" />
        </template>
        <div class="panel-body">
          <div v-if="detailLoading && !detailPackage.title" class="panel-loading">
            <div class="loading-spinner"></div>
            <p class="text-secondary text-sm">{{ t('online.loadingDetail') }}</p>
          </div>

          <div v-else class="panel-scroll-content">
            <!-- Summary -->
            <div class="panel-hero-row">
              <div class="panel-meta-info">
                <div class="panel-version-row">
                  <span
                    class="panel-category-tag"
                    v-if="detailPackage.category"
                    :class="detailPackage.category === 'Free' ? 'pricing-free' : 'pricing-paid'"
                  >
                    {{ detailPackage.category === 'Free' ? t('online.filters.free') : t('online.filters.premium') }}
                  </span>
                  <span class="badge-version" v-if="detailPackage.version_string">
                    {{ formatHubVersion(detailPackage.version_string) }}
                  </span>
                  <span
                    class="card-type-badge panel-type-badge"
                    v-if="detailPackage.type"
                    :class="getTypeBadgeClass(detailPackage.type)"
                  >
                    {{ translateType(detailPackage.type) }}
                  </span>
                </div>
                <h2 class="panel-title">{{ detailPackage.title || t('online.detailTitle') }}</h2>
                <p v-if="detailPackage.tag_line" class="panel-tagline">
                  {{ detailPackage.tag_line }}
                </p>
                <div class="panel-creator">
                  {{ t('online.by') }} <span class="creator-name">{{ detailPackage.username }}</span>
                </div>
                <div class="panel-stat-badges">
                  <span class="badge-stat">
                    {{ t('online.downloads', { count: formatNumber(detailPackage.download_count) }) }}
                  </span>
                  <span class="badge-stat">
                    {{ t('online.views', { count: formatNumber(detailPackage.view_count) }) }}
                  </span>
                  <span class="badge-stat rating" v-if="Number(detailPackage.rating_count) > 0 && Number(detailPackage.rating_avg) > 0">
                    {{ t('online.rating', { score: parseFloat(detailPackage.rating_avg).toFixed(1), count: detailPackage.rating_count }) }}
                  </span>
                </div>
              </div>

              <div class="panel-actions-row">
                <UiButton
                  :disabled="detailLoading"
                  @click="openBatchDownloadModal"
                  variant="primary"
                  size="lg"
                  icon="download"
                >
                  <span>{{ t('online.downloadAll') }}</span>
                </UiButton>
              </div>
            </div>

            <!-- Description -->
            <div class="panel-section">
              <h4 class="section-title">{{ t('online.description') }}</h4>
              <div v-if="descLoading" class="desc-loading text-secondary text-sm">
                <div class="loading-spinner-sm"></div>
                <span>{{ t('online.loadingDescription') }}</span>
              </div>
              <div
                v-else-if="detailPackage.description"
                class="desc-content text-sm text-secondary"
                data-resource-preview-content
                v-html="formatDescription(detailPackage.description)"
              ></div>
              <div v-else class="text-tertiary text-sm">{{ t('online.noDescription') }}</div>
            </div>

            <!-- Files list -->
            <div class="panel-section">
              <h4 class="section-title">
                <span>{{ t('online.files', { count: detailPackage.hubFiles?.length || 0 }) }}</span>
              </h4>

              <div v-if="descLoading" class="desc-loading text-secondary text-sm">
                <div class="loading-spinner-sm"></div>
                <span>{{ t('online.loadingFiles') }}</span>
              </div>
              <div
                v-else-if="detailPackage.hubFiles?.length"
                class="panel-list files-list-container"
              >
                <div
                  v-for="file in detailPackage.hubFiles"
                  :key="file.filename"
                  class="list-item file-item glass-card"
                >
                  <div class="item-info">
                    <div class="item-name text-sm text-primary" :title="file.filename">
                      {{ file.filename }}
                    </div>
                    <div class="item-meta text-xs text-secondary">
                      <span v-if="file.file_size">
                        {{ formatDisplaySize(file.file_size) }}
                      </span>
                      <span v-if="file.licenseType" class="license-type">
                        {{ t('online.license', { name: file.licenseType }) }}
                      </span>
                    </div>
                  </div>

                  <div class="download-action">
                    <!-- If installed locally -->
                    <div
                      v-if="isFileInstalled(file.filename) || (downloadStates[file.filename] && downloadStates[file.filename].status === 'completed')"
                      class="download-status completed text-success text-xs"
                    >
                      <span>{{ t('online.downloaded') }}</span>
                    </div>

                    <!-- If no download URL available -->
                    <div v-else-if="!file.urlHosted" class="download-status no-source text-xs">
                      <span>{{ t('online.noLink') }}</span>
                    </div>

                    <!-- If not downloading / completed / error -->
                    <UiButton
                      v-else-if="!downloadStates[file.filename] || downloadStates[file.filename].status === 'idle'"
                      @click="handleDownload(file)"
                      variant="secondary"
                      size="sm"
                    >
                      <span>{{ t('online.download') }}</span>
                    </UiButton>

                    <!-- If downloading -->
                    <div
                      v-else-if="downloadStates[file.filename].status === 'downloading'"
                      class="download-progress-container"
                    >
                      <div class="progress-bar-wrapper">
                        <div
                          class="progress-bar-fill"
                          :style="{ width: downloadStates[file.filename].progress + '%' }"
                        ></div>
                      </div>
                      <span class="progress-percentage text-xs text-accent">
                        {{ Math.round(downloadStates[file.filename].progress) }}%
                      </span>
                    </div>

                    <!-- If error -->
                    <UiButton
                      v-else-if="downloadStates[file.filename].status === 'error'"
                      :title="downloadStates[file.filename].errorMsg"
                      @click="handleDownload(file)"
                      variant="danger"
                      size="sm"
                    >
                      <span>{{ t('online.retry') }}</span>
                    </UiButton>
                  </div>
                </div>
              </div>
              <div v-else class="empty-list-info text-xs text-secondary">{{ t('online.noFiles') }}</div>
            </div>

            <!-- Dependencies list -->
            <div class="panel-section">
              <h4 class="section-title">
                <span>
                  {{ t('online.dependencies', { count: uniqueDependencies.length }) }}
                </span>
              </h4>

              <div v-if="dependenciesLoading" class="desc-loading text-secondary text-sm">
                <div class="loading-spinner-sm"></div>
                <span>{{ t('online.loadingDependencies') }}</span>
              </div>
              <div
                v-else-if="uniqueDependencies.length"
                class="panel-list dependencies-list-container"
              >
                <div
                  v-for="dep in uniqueDependencies"
                  :key="dep.filename || dep.packageName"
                  class="list-item dep-item glass-card"
                >
                  <div class="item-info">
                    <div
                      class="item-name text-sm text-primary"
                      :title="dep.packageName || dep.filename"
                    >
                      {{ dep.packageName || dep.filename }}
                    </div>
                    <div class="item-meta text-xs text-secondary">
                      <span v-if="dep.file_size">{{ formatDisplaySize(dep.file_size) }}</span>
                      <span v-if="dep.username" class="dep-author">{{ dep.username }}</span>
                      <span
                        v-if="dep.dependencyType"
                        class="dep-tier"
                        :class="dep.dependencyType === 'sub' ? 'sub' : 'direct'"
                      >
                        {{ formatDependencyType(dep) }}
                      </span>
                    </div>
                  </div>

                  <div class="dep-action">
                    <!-- Installed Badge (version sufficient) -->
                    <div
                      v-if="getDependencyInstallStatus(dep) === 'installed'"
                      class="dep-status installed text-success text-xs"
                    >
                      <span class="badge success">{{ t('online.installed') }}</span>
                    </div>

                    <!-- Upgrade Badge (installed but version too low) -->
                    <div
                      v-else-if="getDependencyInstallStatus(dep) === 'upgrade'"
                      class="dep-status-download"
                    >
                      <template v-if="dep.downloadUrl">
                        <UiButton
                          v-if="!downloadStates[dep.filename || getFilenameFromUrl(dep.downloadUrl)] || downloadStates[dep.filename || getFilenameFromUrl(dep.downloadUrl)].status === 'idle'"
                          @click="handleDownloadDependency(dep)"
                          variant="primary"
                          size="sm"
                        >
                          <span>{{ t('online.upgrade') }}</span>
                        </UiButton>

                        <div
                          v-else-if="downloadStates[dep.filename || getFilenameFromUrl(dep.downloadUrl)].status === 'downloading'"
                          class="download-progress-container mini"
                        >
                          <div class="progress-bar-wrapper">
                            <div
                              class="progress-bar-fill"
                              :style="{ width: downloadStates[dep.filename || getFilenameFromUrl(dep.downloadUrl)].progress + '%' }"
                            ></div>
                          </div>
                          <span class="progress-percentage text-xs text-accent">
                            {{ Math.round(downloadStates[dep.filename || getFilenameFromUrl(dep.downloadUrl)].progress)
                            }}%
                          </span>
                        </div>

                        <div
                          v-else-if="downloadStates[dep.filename || getFilenameFromUrl(dep.downloadUrl)].status === 'completed'"
                          class="dep-status installed text-success text-xs"
                        >
                          <span class="badge success">{{ t('online.installed') }}</span>
                        </div>

                        <UiButton
                          v-else-if="downloadStates[dep.filename || getFilenameFromUrl(dep.downloadUrl)].status === 'error'"
                          @click="handleDownloadDependency(dep)"
                          variant="danger"
                          size="sm"
                        >
                          <span>{{ t('online.retry') }}</span>
                        </UiButton>
                      </template>
                      <span v-else class="badge upgrade">{{ t('online.needsUpgrade') }}</span>
                    </div>

                    <!-- Not Installed / Download Action -->
                    <div v-else class="dep-status-download">
                      <template v-if="dep.downloadUrl">
                        <!-- Direct download if URL exists and we are not downloading it -->
                        <UiButton
                          v-if="!downloadStates[dep.filename || getFilenameFromUrl(dep.downloadUrl)] || downloadStates[dep.filename || getFilenameFromUrl(dep.downloadUrl)].status === 'idle'"
                          @click="handleDownloadDependency(dep)"
                          variant="secondary"
                          size="sm"
                        >
                          <span>{{ t('online.download') }}</span>
                        </UiButton>

                        <!-- Downloading state for dependency -->
                        <div
                          v-else-if="downloadStates[dep.filename || getFilenameFromUrl(dep.downloadUrl)].status === 'downloading'"
                          class="download-progress-container mini"
                        >
                          <div class="progress-bar-wrapper">
                            <div
                              class="progress-bar-fill"
                              :style="{ width: downloadStates[dep.filename || getFilenameFromUrl(dep.downloadUrl)].progress + '%' }"
                            ></div>
                          </div>
                          <span class="progress-percentage text-xs text-accent">
                            {{ Math.round(downloadStates[dep.filename || getFilenameFromUrl(dep.downloadUrl)].progress)
                            }}%
                          </span>
                        </div>

                        <!-- Completed state -->
                        <div
                          v-else-if="downloadStates[dep.filename || getFilenameFromUrl(dep.downloadUrl)].status === 'completed'"
                          class="dep-status installed text-success text-xs"
                        >
                          <span class="badge success">{{ t('online.installed') }}</span>
                        </div>

                        <!-- Error state -->
                        <UiButton
                          v-else-if="downloadStates[dep.filename || getFilenameFromUrl(dep.downloadUrl)].status === 'error'"
                          @click="handleDownloadDependency(dep)"
                          variant="danger"
                          size="sm"
                        >
                          <span>{{ t('online.retry') }}</span>
                        </UiButton>
                      </template>

                      <!-- No direct download URL -->
                      <span v-else class="badge warning">{{ t('online.notInstalled') }}</span>
                    </div>
                  </div>
                </div>
              </div>
              <div v-else class="empty-list-info text-xs text-secondary">
                {{ t('online.noDependencies') }}
              </div>
            </div>
          </div>
        </div>
      </UiDetailModal>
      </div>

      <!-- Batch Download Modal -->
      <UiModal
        v-if="batchModalOpen"
        large
        :title="$t('download.modal.title')"
        :close-disabled="false"
        @close="closeBatchModal"
      >
        <div v-if="batchResolving" class="batch-loading" role="status">
          <span class="spinner-ring" aria-hidden="true" />
          <p class="batch-loading-title">{{ $t('download.modal.loading') }}</p>
          <div class="resolve-progress">
            <div class="resolve-track">
              <div class="resolve-fill" :style="{ width: resolveProgress.percentage + '%' }" />
            </div>
            <span class="resolve-count">{{ resolveProgress.resolved }} / {{ resolveProgress.total }}</span>
          </div>
          <p v-if="resolveProgress.currentPackage" class="resolve-current" :title="resolveProgress.currentPackage">
            {{ $t('download.modal.resolving', { name: resolveProgress.currentPackage }) }}
          </p>
        </div>

        <div v-else class="batch-content">
          <p class="batch-summary">
            {{ $t('download.modal.summary', { total: checklistItems.length, size: formattedTotalSize }) }}
          </p>
          <section v-for="group in batchGroups" :key="group.key" class="batch-group">
            <div class="batch-group-head">
              <button
                type="button"
                class="batch-group-toggle"
                :aria-expanded="!collapsedGroups[group.key]"
                @click="collapsedGroups[group.key] = !collapsedGroups[group.key]"
              >
                <Icon class="batch-chevron" :class="{ collapsed: collapsedGroups[group.key] }" name="chevron-down" :size="16" />
                <span class="batch-group-title">{{ $t(group.label) }}</span>
                <span class="batch-group-count">{{ group.items.length }}</span>
              </button>
              <span v-if="group.hint" class="batch-group-hint">{{ $t(group.hint) }}</span>
              <UiCheckbox
                v-if="group.selectable"
                class="batch-group-check"
                :model-value="isGroupAllChecked(group.key)"
                :indeterminate="isGroupSomeChecked(group.key)"
                :label="$t('download.modal.selectGroup')"
                @update:model-value="toggleGroupSelection(group.key)"
              >
                {{ $t('download.modal.selectGroup') }}
              </UiCheckbox>
            </div>
            <ul v-show="!collapsedGroups[group.key]" class="batch-items">
              <li v-for="item in group.items" :key="item.id" class="batch-item" :class="{ selectable: group.selectable }">
                <UiCheckbox
                  v-if="group.selectable"
                  v-model="item.checked"
                  class="batch-item-check"
                  :label="item.filename"
                />
                <span class="batch-item-name" :title="item.filename">{{ item.filename }}</span>
                <span v-if="group.key === 'upgraded' && (item.installed_version ?? item.queued_version) != null" class="batch-item-version">
                  v{{ item.installed_version ?? item.queued_version }} → v{{ item.version }}
                </span>
                <span class="batch-item-size">{{ formatDisplaySize(item.file_size) }}</span>
                <UiButton
                  v-if="group.key === 'no_source'"
                  size="sm"
                  variant="ghost"
                  icon="copy"
                  @click="copyManualDependency(item)"
                >
                  {{ $t('download.modal.manualMark') }}
                </UiButton>
              </li>
            </ul>
          </section>
        </div>

        <template v-if="!batchResolving" #footer>
          <span class="batch-footer-stats">
            {{ $t('download.modal.selectedSummary', { count: checkedCount, size: formattedCheckedSize }) }}
          </span>
          <UiButton @click="closeBatchModal">{{ $t('common.cancel') }}</UiButton>
          <UiButton variant="primary" :disabled="checkedCount === 0" @click="confirmBatchDownload">
            {{ $t('download.modal.action', { count: checkedCount }) }}
          </UiButton>
        </template>
      </UiModal>
    </div>
  </div>
</template>

<script setup lang="ts">
import PackageCard from '@/components/packages/PackageCard.vue'
import UiModal from '@/components/ui/UiModal.vue'
import PageHeader from '@/components/layout/PageHeader.vue'
import UiButton from '@/components/ui/UiButton.vue'
import UiInput from '@/components/ui/UiInput.vue'
import UiCheckbox from '@/components/ui/UiCheckbox.vue'
import Icon from '@/components/ui/Icon.vue'
import { parseSizeValue as parseSizeStringToBytes } from '@/utils/bytes'
import { formatDescription } from '@/utils/hubDescription'
import { computed, onMounted, onUnmounted, ref } from 'vue'
import { useRouter } from 'vue-router'
import UiDetailModal from '@/components/ui/UiDetailModal.vue'
import { useI18n } from 'vue-i18n'
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { storeToRefs } from 'pinia'
import { useAppStore } from '@/stores/app'
import { useDownloadStore } from '@/stores/download'
import { useLocalLibraryStore } from '@/stores/localLibrary'
import { useNotification } from '@/composables/useNotification'

const appStore = useAppStore()
const downloadStore = useDownloadStore()
const localLibraryStore = useLocalLibraryStore()
const router = useRouter()
const { t } = useI18n()
const notify = useNotification()

const { vamRootPath } = storeToRefs(appStore)
const { packages: localPackages } = storeToRefs(localLibraryStore)

// Batch download state
const batchModalOpen = ref(false)
let returnToDetail = false
const batchResolving = ref(false)
const checklistItems = ref<any[]>([])
const collapsedGroups = ref({
  need_download: false,
  upgraded: false,
  queued: true,
  installed: true,
  no_source: true
})

interface ResolveProgressPayload {
  resolved_count: number
  total_discovered: number
  current_package: string
}

const resolveProgress = ref({
  resolved: 0,
  total: 0,
  percentage: 0,
  currentPackage: ''
})

let unlistenResolveProgress: (() => void) | null = null

// Browsing list state
const resources = ref<any[]>([])
const searchQuery = ref('')
const selectedCategory = ref('')
const selectedPricing = ref('free') // Default to Free
const selectedSort = ref('last_update') // Default to Recently Updated
const currentPage = ref(1)
const totalPages = ref(1)
const totalFound = ref(0)
const loading = ref(false)
const error = ref<string | null>(null)
const scrollContainer = ref<HTMLElement | null>(null)

// Computed property for reactive, instant client-side filtering and sorting of VAM Hub results
const filteredAndSortedResources = computed(() => {
  let list = [...resources.value]

  // Pricing filter is handled server-side via the 'categorytype' API parameter,
  // so no client-side filtering needed here.

  // 2. Sorting based on the requested options
  list.sort((a, b) => {
    switch (selectedSort.value) {
      case 'last_update': {
        const valA = parseInt(a.last_update) || 0
        const valB = parseInt(b.last_update) || 0
        return valB - valA // Recently updated (descending)
      }
      case 'resource_date': {
        const valA = parseInt(a.resource_date) || 0
        const valB = parseInt(b.resource_date) || 0
        return valB - valA // Submission date (descending)
      }
      case 'reaction_score': {
        const valA = parseInt(a.reaction_score) || 0
        const valB = parseInt(b.reaction_score) || 0
        return valB - valA // Reaction score (descending)
      }
      case 'download_count': {
        const valA = parseInt(a.download_count) || 0
        const valB = parseInt(b.download_count) || 0
        return valB - valA // Downloads (descending)
      }
      case 'rating_weighted': {
        const valA = parseFloat(a.rating_weighted) || 0
        const valB = parseFloat(b.rating_weighted) || 0
        return valB - valA // Favorites/Weighted Rating (descending)
      }
      case 'rating_count': {
        const valA = parseInt(a.rating_count) || 0
        const valB = parseInt(b.rating_count) || 0
        return valB - valA // Likes/Rating count (descending)
      }
      case 'title': {
        const titleA = a.title || ''
        const titleB = b.title || ''
        return titleA.localeCompare(titleB, 'zh-CN') // Title (ascending)
      }
      case 'username': {
        const userA = a.username || ''
        const userB = b.username || ''
        return userA.localeCompare(userB, 'zh-CN') // Username (ascending)
      }
      default:
        return 0
    }
  })

  return list
})

// Detail panel state
const detailPanelOpen = ref(false)
const detailPackage = ref<any>({})
const detailLoading = ref(false)
const descLoading = ref(false)
const dependenciesLoading = ref(false)
const currentLoadingPkgName = ref('')

// Download status tracker mapped directly from downloadStore.queue for reactive synchronization
const downloadStates = computed(() => {
  const states: Record<string, { progress: number; status: 'idle' | 'downloading' | 'completed' | 'error'; errorMsg?: string }> = {}
  for (const item of downloadStore.queue) {
    let statusMapped: 'idle' | 'downloading' | 'completed' | 'error' = 'idle'
    if (item.status === 'Downloading' || item.status === 'Pending') {
      statusMapped = 'downloading'
    } else if (item.status === 'Completed') {
      statusMapped = 'completed'
    } else if (item.status === 'Failed') {
      statusMapped = 'error'
    } else if (item.status === 'Paused') {
      statusMapped = 'idle'
    }
    states[item.filename] = {
      progress: item.progress,
      status: statusMapped,
      errorMsg: item.error_msg || undefined
    }
  }
  return states
})

// Check if a search query is a specific package name format (contains dots and no spaces)
function isSpecificPackageName(str: string): boolean {
  const trimmed = str.trim()
  if (!trimmed) return false
  if (trimmed.includes(' ')) return false
  return trimmed.includes('.')
}

onMounted(async () => {
  await localLibraryStore.ensureLoaded()
  await searchOnlineResources()
  await downloadStore.fetchQueue()

  unlistenResolveProgress = await listen<ResolveProgressPayload>('resolve-dependencies-progress', (event) => {
    const payload = event.payload
    const resolved = payload.resolved_count
    const total = payload.total_discovered
    const currentPackage = payload.current_package
    const percentage = total > 0 ? Math.min(100, Math.round((resolved / total) * 100)) : 0

    resolveProgress.value = {
      resolved,
      total,
      percentage,
      currentPackage
    }
  })
})

onUnmounted(() => {
  if (unlistenResolveProgress) {
    unlistenResolveProgress()
    unlistenResolveProgress = null
  }
})

// Reset page parameters and trigger search
function resetAndSearch() {
  currentPage.value = 1
  searchOnlineResources()
}

// Handle search button/enter click
function handleSearch() {
  const query = searchQuery.value.trim()
  if (isSpecificPackageName(query)) {
    // Open detail panel directly for specific package
    openPackageDetailByName(query)
  } else {
    // Regular search
    resetAndSearch()
  }
}

// Call backend `browse_hub_packages` command to browse online packages
async function searchOnlineResources() {
  loading.value = true
  error.value = null

  if (scrollContainer.value) {
    scrollContainer.value.scrollTop = 0
  }

  try {
    const res = await invoke<any>('browse_hub_packages', {
      search: searchQuery.value.trim(),
      page: currentPage.value,
      resourceType: selectedCategory.value,
      pricing: selectedPricing.value,
      sort: selectedSort.value,
    })

    if (res && res.status === 'success') {
      resources.value = res.resources || []
      if (res.pagination) {
        totalPages.value = parseInt(res.pagination.total_pages) || 1
        totalFound.value = parseInt(res.pagination.total_found) || 0
      } else {
        totalPages.value = 1
        totalFound.value = resources.value.length
      }
    } else {
      resources.value = []
      totalPages.value = 1
      totalFound.value = 0
      if (res && res.error) {
        error.value = res.error
      }
    }
  } catch (err: any) {
    error.value = err.toString() || t('online.networkError')
    resources.value = []
  } finally {
    loading.value = false
  }
}

// Change page handler
function changePage(page: number) {
  if (page < 1 || page > totalPages.value) return
  currentPage.value = page
  searchOnlineResources()
}

// Open package detail from grid item click
async function openPackageDetail(item: any) {
  detailPanelOpen.value = true
  // Merge item into detailPackage so that basic info is visible immediately!
  detailPackage.value = {
    ...item,
    description: '',
    hubFiles: item.hubFiles || [],
    dependencies: item.dependencies || {}
  }
  detailLoading.value = false // Instant panel open, no giant loading blocker
  descLoading.value = true
  dependenciesLoading.value = true

  const searchName = item.resource_id ? `${item.username}.${item.title.replace(/\s+/g, '_')}.latest` : ''
  const queryName = item.resource_id || searchName || item.title
  currentLoadingPkgName.value = queryName

  // 1. Fetch basic info first (extremely fast, ~100ms)
  try {
    const basicDetail = await invoke<any>('fetch_hub_package_info_basic', {
      packageName: queryName
    })
    if (currentLoadingPkgName.value !== queryName) return
    if (basicDetail && (basicDetail.title || basicDetail.resource_id)) {
      detailPackage.value = { ...detailPackage.value, ...basicDetail }
    }
  } catch (err) {
    console.error('Failed to load basic package details:', err)
  } finally {
    if (currentLoadingPkgName.value === queryName) {
      descLoading.value = false
    }
  }

  // 2. Fetch full enriched details in background (scrapes dependencies, ~1s)
  try {
    const fullDetail = await invoke<any>('fetch_hub_package_info', {
      packageName: queryName
    })
    if (currentLoadingPkgName.value !== queryName) return
    if (fullDetail && (fullDetail.title || fullDetail.resource_id)) {
      detailPackage.value = { ...detailPackage.value, ...fullDetail }
    }
  } catch (err) {
    console.error('Failed to load dependency details:', err)
  } finally {
    if (currentLoadingPkgName.value === queryName) {
      dependenciesLoading.value = false
    }
  }
}

// Fetch package details directly by exact string key (e.g. Creator.Name.version)
async function openPackageDetailByName(pkgName: string) {
  detailPanelOpen.value = true
  detailPackage.value = { title: pkgName, description: '', hubFiles: [], dependencies: {} }
  detailLoading.value = false
  descLoading.value = true
  dependenciesLoading.value = true
  currentLoadingPkgName.value = pkgName

  // 1. Fetch basic info
  try {
    const basicDetail = await invoke<any>('fetch_hub_package_info_basic', {
      packageName: pkgName
    })
    if (currentLoadingPkgName.value !== pkgName) return
    if (basicDetail && (basicDetail.title || basicDetail.resource_id)) {
      detailPackage.value = { ...detailPackage.value, ...basicDetail }
    } else {
      detailPackage.value = { title: pkgName, description: t('online.detailNotFound') }
    }
  } catch (err: any) {
    if (currentLoadingPkgName.value !== pkgName) return
    detailPackage.value = { title: pkgName, description: t('online.detailFailed', { error: String(err) }) }
    descLoading.value = false
    dependenciesLoading.value = false
    return
  } finally {
    if (currentLoadingPkgName.value === pkgName) {
      descLoading.value = false
    }
  }

  // 2. Fetch full enriched details
  try {
    const fullDetail = await invoke<any>('fetch_hub_package_info', {
      packageName: pkgName
    })
    if (currentLoadingPkgName.value !== pkgName) return
    if (fullDetail && (fullDetail.title || fullDetail.resource_id)) {
      detailPackage.value = { ...detailPackage.value, ...fullDetail }
    }
  } catch (err: any) {
    console.error('Failed to load dependency details by name:', err)
  } finally {
    if (currentLoadingPkgName.value === pkgName) {
      dependenciesLoading.value = false
    }
  }
}

function closeDetailPanel() {
  detailPanelOpen.value = false
}

// 拉平并去重详情中的依赖列表，保留官网 Direct/Sub-dependencies 标记。
const uniqueDependencies = computed(() => {
  if (!detailPackage.value || !detailPackage.value.dependencies) return []

  const map: Record<string, any> = {}
  for (const [_, depList] of Object.entries(detailPackage.value.dependencies)) {
    if (Array.isArray(depList)) {
      for (const dep of depList) {
        if (!dep) continue
        const identifier = dep.packageName || dep.filename
        if (identifier) {
          map[identifier] = {
            ...dep,
            dependencyType: dep.dependencyType || (Number(dep.tier || 1) > 1 ? 'sub' : 'direct')
          }
        }
      }
    }
  }
  return Object.values(map).sort((a: any, b: any) => {
    const typeA = a.dependencyType === 'sub' ? 1 : 0
    const typeB = b.dependencyType === 'sub' ? 1 : 0
    if (typeA !== typeB) return typeA - typeB
    const tierA = Number(a.tier || 1)
    const tierB = Number(b.tier || 1)
    if (tierA !== tierB) return tierA - tierB
    return String(a.packageName || a.filename || '').localeCompare(String(b.packageName || b.filename || ''), 'zh-CN')
  })
})

function formatDependencyType(dep: any): string {
  if (dep.dependencyType === 'sub') {
    return t('online.indirectDependency', { level: dep.tier || 2 })
  }
  return t('online.directDependency')
}

// Check dependency install status: 'installed' (version sufficient), 'upgrade' (installed but version too low), 'not_installed'
function getDependencyInstallStatus(dep: any): 'installed' | 'upgrade' | 'not_installed' {
  if (!dep) return 'not_installed'
  let identifier = dep.packageName || dep.package_name || ''
  if (!identifier && dep.filename) {
    identifier = dep.filename.replace(/\.var$/i, '')
  }
  if (!identifier) return 'not_installed'

  const parsed = parsePackageMeta(identifier)
  if (!parsed.creator) return 'not_installed'

  const creatorLower = parsed.creator.toLowerCase()
  const nameLower = parsed.name.toLowerCase()

  // 计算需要的版本号：优先使用 latest_version / latestVersion / version 字段
  let requiredVersion = parsed.version
  const latestVer = parseInt(dep.latest_version || dep.latestVersion || '')
  if (Number.isFinite(latestVer) && latestVer > 0 && latestVer > requiredVersion) requiredVersion = latestVer
  const depVer = parseInt(dep.version || '')
  if (Number.isFinite(depVer) && depVer > 0 && depVer > requiredVersion) requiredVersion = depVer

  // 在本地包列表中查找匹配
  const matchedPkg = localPackages.value.find(p => {
    if (p.creator.toLowerCase() !== creatorLower) return false
    return p.name.toLowerCase() === nameLower
  })

  if (!matchedPkg) return 'not_installed'
  if (matchedPkg.version >= requiredVersion) return 'installed'
  return 'upgrade'  // 本地版本低于需求
}

// Extract filename from download URL
function getFilenameFromUrl(url: string): string {
  if (!url) return 'unknown.var'
  try {
    const u = new URL(url)
    const pathname = u.pathname
    const filename = pathname.substring(pathname.lastIndexOf('/') + 1)
    return decodeURIComponent(filename) || 'unknown.var'
  } catch {
    const lastSlash = url.lastIndexOf('/')
    if (lastSlash !== -1) {
      return url.substring(lastSlash + 1) || 'unknown.var'
    }
    return 'unknown.var'
  }
}

// Direct file download handler
async function handleDownload(file: any) {
  if (!file || !file.filename) return
  if (!file.urlHosted) {
    notify.error(t('online.noLinkError'))
    return
  }
  if (!vamRootPath.value) {
    notify.error(t('online.noGameFolder'))
    return
  }

  const filename = file.filename
  const url = file.urlHosted
  const parsed = parsePackageMeta(filename)

  const item = {
    id: `${parsed.creator}.${parsed.name}.${parsed.version}`,
    url,
    filename,
    creator: parsed.creator,
    name: parsed.name,
    version: parsed.version,
    total_bytes: parseSizeStringToBytes(file.file_size)
  }

  try {
    const results = await downloadStore.addItems([item])
    if (results && results[0] && typeof results[0] === 'object' && results[0].Skipped) {
      notify.info(t('online.skipped', { reason: results[0].Skipped }))
    } else {
      notify.success(t('download.toast.added', { count: 1 }))
    }
  } catch (error: any) {
    console.error('Failed to add single download:', error)
    notify.error(t('download.toast.addFailed', { error: String(error) }))
  }
}

// 依赖下载处理
async function handleDownloadDependency(dep: any) {
  if (!dep) return
  if (!dep.downloadUrl) {
    notify.error(t('online.noLinkError'))
    return
  }
  if (!vamRootPath.value) {
    notify.error(t('online.noGameFolder'))
    return
  }

  const url = dep.downloadUrl
  const filename = dep.filename || getFilenameFromUrl(url)
  const parsed = parsePackageMeta(filename)
  const version = resolveDependencyVersion(dep, parsed.version)

  const item = {
    id: `${parsed.creator}.${parsed.name}.${version}`,
    url,
    filename,
    creator: dep.username || parsed.creator || 'Hub',
    name: dep.packageName || parsed.name || 'Dependency',
    version,
    total_bytes: parseSizeStringToBytes(dep.file_size)
  }

  try {
    const results = await downloadStore.addItems([item])
    if (results && results[0] && typeof results[0] === 'object' && results[0].Skipped) {
      notify.info(t('online.skipped', { reason: results[0].Skipped }))
    } else {
      notify.success(t('download.toast.added', { count: 1 }))
    }
  } catch (error: any) {
    console.error('Failed to add dependency download:', error)
    notify.error(t('download.toast.addFailed', { error: String(error) }))
  }
}

// Get VAM Hub package query identifier
function getPackageIdentifier(): string {
  if (detailPackage.value.username && detailPackage.value.title) {
    return `${detailPackage.value.username}.${detailPackage.value.title.replace(/\s+/g, '_')}.latest`
  }
  return detailPackage.value.title || ''
}

// Open batch download modal and resolve dependencies recursively
async function openBatchDownloadModal() {
  if (!vamRootPath.value) {
    notify.error(t('online.noGameFolder'))
    return
  }

  if (!detailPackage.value.hubFiles || detailPackage.value.hubFiles.length === 0) {
    notify.warning(t('online.noDownloadableFiles'))
    return
  }

  // 不叠两层弹窗：先收起详情，取消时再回到详情
  returnToDetail = detailPanelOpen.value
  detailPanelOpen.value = false
  batchModalOpen.value = true
  batchResolving.value = true
  checklistItems.value = []
  collapsedGroups.value = {
    need_download: false,
    upgraded: false,
    queued: true,
    installed: true,
    no_source: true
  }

  resolveProgress.value = {
    resolved: 0,
    total: 0,
    percentage: 0,
    currentPackage: ''
  }

  const pkgName = detailPackage.value.resource_id || getPackageIdentifier()
  try {
    const res = await invoke<any>('resolve_hub_dependencies', { packageName: pkgName })
    const items: any[] = []

    // 1. Add main files from detailPackage
    if (detailPackage.value.hubFiles && Array.isArray(detailPackage.value.hubFiles)) {
      for (const file of detailPackage.value.hubFiles) {
        if (!file.filename) continue

        const filename = file.filename
        const parsed = parsePackageMeta(filename)

        // Check if installed and get installed version
        const creatorLower = parsed.creator.toLowerCase()
        const nameLower = parsed.name.toLowerCase()
        let installedVer: number | null = null
        if (localPackages.value && Array.isArray(localPackages.value)) {
          const matchedPkg = localPackages.value.find(p =>
            p.creator.toLowerCase() === creatorLower && p.name.toLowerCase() === nameLower
          )
          if (matchedPkg) installedVer = matchedPkg.version
        }

        // Check if queued
        let queuedVer: number | null = null
        for (const q of downloadStore.queue) {
          const qp = parsePackageMeta(q.filename)
          if (qp.creator.toLowerCase() === creatorLower && qp.name.toLowerCase() === nameLower) {
            queuedVer = qp.version
            break
          }
        }

        let status = 'need_download'
        if (installedVer !== null && installedVer >= parsed.version) {
          status = 'installed'
        } else if (installedVer !== null && installedVer < parsed.version) {
          // 本地版本低于需求，标记为 upgraded（需要升级）
          status = 'upgraded'
        } else if (queuedVer !== null) {
          if (queuedVer >= parsed.version) {
            status = 'queued'
          } else {
            status = 'upgraded'
          }
        } else if (!file.urlHosted) {
          status = 'no_source'
        }

        items.push({
          id: `${parsed.creator}.${parsed.name}.${parsed.version}`,
          filename,
          creator: parsed.creator,
          name: parsed.name,
          version: parsed.version,
          file_size: file.file_size ? (isNaN(Number(file.file_size)) ? file.file_size : formatBytes(Number(file.file_size))) : '',
          download_url: file.urlHosted || null,
          status,
          installed_version: installedVer,
          queued_version: queuedVer,
          checked: status === 'need_download' || status === 'upgraded',
          isMainFile: true
        })
      }
    }

    // 2. Add dependencies
    if (res.dependencies && Array.isArray(res.dependencies)) {
      for (const dep of res.dependencies) {
        items.push({
          ...dep,
          checked: dep.status === 'need_download' || dep.status === 'upgraded',
          isMainFile: false
        })
      }
    }

    checklistItems.value = items
  } catch (err: any) {
    console.error('Failed to resolve dependencies:', err)
    const errMsg = err.toString()
    if (errMsg !== 'CANCELLED') {
      notify.error(t('download.modal.resolveFailed', { error: errMsg }))
    }
    batchModalOpen.value = false
    if (returnToDetail) detailPanelOpen.value = true
    returnToDetail = false
  } finally {
    batchResolving.value = false
  }
}

function closeBatchModal() {
  if (batchResolving.value) {
    const pkgName = detailPackage.value.resource_id || getPackageIdentifier()
    invoke('cancel_hub_dependency_resolution', { packageName: pkgName }).catch(err => {
      console.error('Failed to cancel dependency resolution:', err)
    })
  }
  batchModalOpen.value = false
  if (returnToDetail) detailPanelOpen.value = true
  returnToDetail = false
}

async function copyManualDependency(item: any) {
  const text = item.id || item.filename
  try {
    await navigator.clipboard.writeText(text)
    notify.success(t('download.modal.manualCopied', { name: text }))
  } catch (err: any) {
    notify.error(String(err))
  }
}

// 分组按"会下载的在前、只做说明的在后"排列；只有前两组可以勾选，其余仅展示。
const batchGroups = computed(() => [
  { key: 'need_download', label: 'download.modal.groupToDownload', hint: '', selectable: true, items: needDownloadGroup.value },
  { key: 'upgraded', label: 'download.modal.groupUpgraded', hint: 'download.modal.groupUpgradedHint', selectable: true, items: upgradedGroup.value },
  { key: 'no_source', label: 'download.modal.groupNoSource', hint: 'download.modal.groupNoSourceHint', selectable: false, items: noSourceGroup.value },
  { key: 'queued', label: 'download.modal.groupInQueue', hint: 'download.modal.groupInQueueHint', selectable: false, items: queuedGroup.value },
  { key: 'installed', label: 'download.modal.groupInstalled', hint: 'download.modal.groupInstalledHint', selectable: false, items: installedGroup.value },
].filter((group) => group.items.length > 0) as { key: keyof typeof collapsedGroups.value; label: string; hint: string; selectable: boolean; items: any[] }[])

// Group computed segments
const needDownloadGroup = computed(() => {
  return checklistItems.value.filter(item => item.status === 'need_download')
})

const upgradedGroup = computed(() => {
  return checklistItems.value.filter(item => item.status === 'upgraded')
})

const installedGroup = computed(() => {
  return checklistItems.value.filter(item => item.status === 'installed')
})

const queuedGroup = computed(() => {
  return checklistItems.value.filter(item => item.status === 'queued')
})

const noSourceGroup = computed(() => {
  return checklistItems.value.filter(item => item.status === 'no_source')
})

// Checklist selection states
function isGroupAllChecked(status: string): boolean {
  const group = checklistItems.value.filter(item => item.status === status)
  if (!group.length) return false
  return group.every(item => item.checked)
}

function isGroupSomeChecked(status: string): boolean {
  const group = checklistItems.value.filter(item => item.status === status)
  if (!group.length) return false
  const checkedCount = group.filter(item => item.checked).length
  return checkedCount > 0 && checkedCount < group.length
}

function toggleGroupSelection(status: string) {
  const group = checklistItems.value.filter(item => item.status === status)
  if (!group.length) return
  const allChecked = group.every(item => item.checked)
  for (const item of group) {
    item.checked = !allChecked
  }
}

const checkedCount = computed(() => {
  return checklistItems.value.filter(item => item.checked && item.status !== 'no_source').length
})

const formattedCheckedSize = computed(() => {
  const totalBytes = checklistItems.value
    .filter(item => item.checked && item.status !== 'no_source')
    .reduce((sum, item) => sum + parseSizeStringToBytes(item.file_size), 0)
  return formatBytes(totalBytes)
})

const formattedTotalSize = computed(() => {
  const totalBytes = checklistItems.value
    .reduce((sum, item) => sum + parseSizeStringToBytes(item.file_size), 0)
  return formatBytes(totalBytes)
})

// Confirm batch download and push to queue
async function confirmBatchDownload() {
  const itemsToEnqueue = checklistItems.value
    .filter(item => item.checked && item.status !== 'no_source' && item.download_url)
    .map(item => {
      const parsed = parsePackageMeta(item.filename)
      return {
        id: item.id || `${parsed.creator}.${parsed.name}.${parsed.version}`,
        url: item.download_url || '',
        filename: item.filename,
        creator: item.creator || parsed.creator || 'Hub',
        name: item.name || parsed.name || 'Dependency',
        version: item.version || parsed.version || 1,
        total_bytes: parseSizeStringToBytes(item.file_size)
      }
    })

  if (!itemsToEnqueue.length) return

  try {
    const results = await downloadStore.addItems(itemsToEnqueue)

    let addedCount = 0
    let skippedCount = 0
    if (Array.isArray(results)) {
      results.forEach(res => {
        if (res && typeof res === 'object' && res.Skipped) skippedCount++
        else addedCount++
      })
    } else {
      addedCount = itemsToEnqueue.length
    }

    if (addedCount > 0) {
      notify.success(t('download.toast.added', { count: addedCount }))
    }
    if (skippedCount > 0) {
      notify.info(t('download.toast.skipped', { count: skippedCount }))
    }

    batchModalOpen.value = false
    detailPanelOpen.value = false
    returnToDetail = false
    router.push('/download')
  } catch (err: any) {
    console.error('Failed to batch download:', err)
    notify.error(t('download.toast.addFailed', { error: String(err) }))
  }
}

// 解析包文件名，latest 由调用方提供的 latest_version 再修正。
function parsePackageMeta(filename: string) {
  const cleaned = filename.replace(/\.var$/i, '')
  const parts = cleaned.split('.')
  if (parts.length >= 3) {
    const creator = parts[0]
    const versionStr = parts[parts.length - 1]
    const version = parseInt(versionStr) || 1
    const name = parts.slice(1, -1).join('.')
    return { creator, name, version }
  } else if (parts.length === 2) {
    const creator = parts[0]
    const name = parts[1]
    return { creator, name, version: 1 }
  } else {
    return { creator: 'Hub', name: cleaned, version: 1 }
  }
}

function resolveDependencyVersion(dep: any, fallbackVersion: number): number {
  const rawVersion = dep.latest_version || dep.latestVersion || dep.version
  const parsedVersion = Number.parseInt(rawVersion)
  return Number.isFinite(parsedVersion) && parsedVersion > 0 ? parsedVersion : fallbackVersion
}

function formatBytes(bytes: number): string {
  if (!bytes || bytes === 0) return '0 B'
  const units = ['B', 'KB', 'MB', 'GB']
  const i = Math.min(Math.floor(Math.log(bytes) / Math.log(1024)), units.length - 1)
  return `${(bytes / Math.pow(1024, i)).toFixed(1)} ${units[i]}`
}

// 统一显示大小，兼容字节数和已格式化文本
function formatDisplaySize(size: string | number | null | undefined): string {
  if (size === null || size === undefined || size === '') return t('online.unknownSize')
  if (typeof size === 'number') return formatBytes(size)

  const text = String(size).trim()
  if (!text) return t('online.unknownSize')
  if (/^\d+(\.\d+)?$/.test(text)) return formatBytes(Number(text))
  return text
}

function isFileInstalled(filename: string): boolean {
  if (!filename) return false
  const parsed = parsePackageMeta(filename)
  const creatorLower = parsed.creator.toLowerCase()
  const nameLower = parsed.name.toLowerCase()
  return localPackages.value.some(p => {
    return p.creator.toLowerCase() === creatorLower && p.name.toLowerCase() === nameLower && p.version >= parsed.version
  })
}

// Formatting utils

function formatNumber(num: any): string {
  if (num === null || num === undefined) return '0'
  const val = typeof num === 'number' ? num : parseInt(num)
  if (isNaN(val)) return num.toString()
  return val.toLocaleString()
}

function formatCompactNumber(num: any): string {
  if (num === null || num === undefined) return '0'
  const val = typeof num === 'number' ? num : parseInt(num)
  if (isNaN(val)) return num.toString()
  if (val >= 1000000) {
    return (val / 1000000).toFixed(1) + 'M'
  }
  if (val >= 1000) {
    return (val / 1000).toFixed(1) + 'K'
  }
  return val.toString()
}

// Get CSS class for resource type badge color
function getTypeBadgeClass(type: string): string {
  const t = (type || '').toLowerCase()
  if (t.includes('scene')) return 'type-scenes'
  if (t.includes('look')) return 'type-looks'
  if (t.includes('cloth')) return 'type-clothing'
  if (t.includes('hair')) return 'type-hair'
  if (t.includes('plugin') || t.includes('script')) return 'type-plugins'
  if (t.includes('morph')) return 'type-morphs'
  if (t.includes('asset') || t.includes('accessor')) return 'type-assets'
  if (t.includes('toolkit') || t.includes('template')) return 'type-toolkits'
  if (t.includes('audio')) return 'type-audio'
  return 'type-default'
}

// Hub 的版本号有时是数字（5），有时是日期（2026-10-09）；只有纯数字版本才加 v 前缀。
function formatHubVersion(value: unknown): string {
  const text = String(value ?? '').trim().replace(/^v/i, '')
  return /^\d+(\.\d+)*$/.test(text) ? `v${text}` : text
}

function translateType(type: string): string {
  const key = (type || '').toLowerCase()
  if (key.includes('scene')) return t('online.types.scenesShort')
  if (key.includes('look')) return t('online.types.looksShort')
  if (key.includes('cloth')) return t('online.types.clothingShort')
  if (key.includes('hair')) return t('online.types.hairShort')
  if (key.includes('plugin') || key.includes('script')) return t('online.types.pluginsShort')
  if (key.includes('morph')) return t('online.types.morphsShort')
  if (key.includes('asset') || key.includes('accessor')) return t('online.types.assetsShort')
  if (key.includes('toolkit') || key.includes('template')) return t('online.types.toolkitsShort')
  if (key.includes('audio')) return t('online.types.audioShort')
  return type
}
function hubCardType(type: string) { const key = getTypeBadgeClass(type); const types: Record<string, string> = { 'type-scenes': 'scene', 'type-looks': 'appearance', 'type-clothing': 'clothing', 'type-hair': 'hair', 'type-plugins': 'plugin', 'type-morphs': 'morph', 'type-assets': 'asset' }; return types[key] || 'other' }
</script>

<style scoped>
.online-hub-view {
  min-height: 0;
  display: flex;
  flex-direction: column;
  position: relative;
}

.header-title-group {
  display: flex;
  align-items: center;
  gap: var(--space-3);
  color: var(--text-primary);
}

.view-title {
  font-size: var(--text-lg);
  font-weight: var(--font-bold);
  margin: 0;
}

.hub-filters {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: var(--space-item);
}

@media (max-width: 600px) {
  .hub-filters {
    flex-direction: column;
    align-items: stretch;
  }
}



.hub-search-bar {
  display: flex;
  gap: var(--space-2);
  width: 320px;
}

@media (max-width: 600px) {
  .hub-search-bar {
    width: 100%;
  }
}

.hub-search-input {
  flex: 1;
  transition: all var(--duration-fast) var(--ease);
}

.hub-search-input:focus {
  outline: none;
}

.spinner {
  animation: rotate 1s linear infinite;
}

@keyframes rotate {
  100% {
    transform: rotate(360deg);
  }
}

/* ── Error Messages ── */
.hub-error-msg {
  display: flex;
  align-items: center;
  gap: var(--space-3);
  padding: var(--space-3) var(--space-4);
  background: var(--color-error-bg);
  border: none;
  border-radius: var(--radius-md);
  color: var(--color-error);
  font-size: var(--text-sm);
  flex-shrink: 0;
}

/* ── Main Content Scroll Area ── */
.hub-content-scroll {
  position: relative;
  flex: 1;
  overflow-y: auto;
  min-height: 0;
  display: flex;
  flex-direction: column;
  padding-right: var(--space-1);
}

.hub-results-section {
  position: relative;
  flex: 1;
  min-height: 320px;
  display: flex;
  flex-direction: column;
}

.hub-content-scroll.is-loading .hub-results-section > :not(.hub-inline-loading-overlay) {
  filter: blur(1.5px);
}

.hub-inline-loading-overlay {
  position: absolute;
  inset: 0;
  z-index: 5;
  display: flex;
  align-items: center;
  justify-content: center;
  padding: var(--space-6);
  background: color-mix(in srgb, var(--bg-surface) 55%, transparent);
  /* backdrop-filter removed */
}

.hub-inline-loading-card {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: var(--space-3);
  padding: var(--space-5) var(--space-6);
  border: none;
  border-radius: var(--radius-lg);
  background: var(--bg-overlay);
  box-shadow: none;
}

.hub-content-scroll::-webkit-scrollbar {
  width: 6px;
}
.hub-content-scroll::-webkit-scrollbar-track {
  background: transparent;
}
.hub-content-scroll::-webkit-scrollbar-thumb {
  background: var(--border-default);
  border-radius: var(--radius-full);
}

.hub-loading-state,
.hub-empty-state {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  padding: var(--space-5) * 2;
  text-align: center;
  flex: 1;
  gap: var(--space-3);
}

.loading-spinner {
  width: 32px;
  height: 32px;
  border: 3px solid var(--bg-active);
  border-top-color: var(--accent-primary);
  border-radius: var(--radius-full);
  animation: rotate 0.8s linear infinite;
}

.loading-spinner-sm {
  width: 14px;
  height: 14px;
  border: 2px solid var(--bg-active);
  border-top-color: var(--accent-primary);
  border-radius: var(--radius-full);
  animation: rotate 0.8s linear infinite;
}

.hub-empty-state svg {
  color: var(--text-tertiary);
  opacity: 0.4;
  margin-bottom: var(--space-2);
}

.hub-empty-state h3 {
  margin: 0;
  color: var(--text-primary);
  font-size: var(--text-lg);
}

/* ── Grid View ── */
.resources-grid {
  padding: var(--focus-room);
  margin-bottom: var(--space-grid);
}

.resource-grid-card {
  display: flex;
  flex-direction: column;
  background: var(--glass-bg);
  border: none;
  border-radius: var(--radius-lg);
  overflow: hidden;
  height: 330px;
  cursor: pointer;
}

.card-thumb-area {
  height: 180px;
  background: var(--bg-base);
  position: relative;
  display: flex;
  align-items: center;
  justify-content: center;
  overflow: hidden;
  border-bottom: 1px solid var(--border-subtle);
}

.card-img {
  width: 100%;
  height: 100%;
  object-fit: contain;
  transition: transform var(--transition-normal);
}

.resource-grid-card:hover .card-img {
  transform: scale(1.05);
}

.card-icon-img {
  width: 80px;
  height: 80px;
  object-fit: contain;
}

.card-placeholder {
  color: var(--text-tertiary);
  opacity: 0.3;
}

/* Resource Type Badge (top-left corner) */
.card-type-badge {
  position: absolute;
  top: var(--space-2);
  left: var(--space-2);
  padding: 2px var(--space-2);
  border-radius: var(--radius-sm);
  /* backdrop-filter removed */
  font-size: var(--text-xs);
  font-weight: var(--font-bold);
  letter-spacing: 0.3px;
  z-index: 2;
  text-shadow: 0 1px 2px var(--border-subtle);
}

.card-type-badge.type-scenes {
  background: var(--color-error-bg);
  border: none;
  color: var(--text-primary);
}

.card-type-badge.type-looks {
  background: var(--accent-muted);
  border: none;
  color: var(--text-primary);
}

.card-type-badge.type-clothing {
  background: var(--color-warning-bg);
  border: none;
  color: var(--text-disabled);
}

.card-type-badge.type-hair {
  background: var(--color-success-bg);
  border: none;
  color: var(--text-disabled);
}

.card-type-badge.type-plugins {
  background: var(--color-info-bg);
  border: none;
  color: var(--text-primary);
}

.card-type-badge.type-morphs {
  background: var(--color-success-bg);
  border: none;
  color: var(--text-disabled);
}

.card-type-badge.type-assets {
  background: var(--color-error-bg);
  border: none;
  color: var(--text-primary);
}

.card-type-badge.type-toolkits {
  background: var(--accent-muted);
  border: none;
  color: var(--text-primary);
}

.card-type-badge.type-audio {
  background: var(--color-warning-bg);
  border: none;
  color: var(--text-primary);
}

.card-type-badge.type-default {
  background: var(--bg-overlay);
  border: none;
  color: var(--text-primary);
}

/* Pricing Badge (top-right corner) */
.card-pricing-badge {
  position: absolute;
  top: var(--space-2);
  right: var(--space-2);
  padding: 2px var(--space-2);
  border-radius: var(--radius-sm);
  /* backdrop-filter removed */
  font-size: var(--text-xs);
  font-weight: var(--font-bold);
  letter-spacing: 0.3px;
  z-index: 2;
  text-shadow: 0 1px 2px var(--border-subtle);
}

.card-pricing-badge.pricing-free {
  background: var(--color-success-bg);
  border: none;
  color: var(--text-primary);
}

.card-pricing-badge.pricing-paid {
  background: var(--color-warning-bg);
  border: none;
  color: var(--text-disabled);
}

.card-info-area {
  padding: var(--space-3) var(--space-4);
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
  flex: 1;
  min-width: 0;
}

.card-title {
  margin: 0;
  font-size: var(--text-sm);
  font-weight: var(--font-bold);
  color: var(--text-primary);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.creator-name {
  color: var(--text-primary);
  font-weight: var(--font-medium);
}

.card-tagline {
  margin: 0;
  line-height: 1.4;
  height: 32px;
  overflow: hidden;
  display: -webkit-box;
  -webkit-line-clamp: 2;
  -webkit-box-orient: vertical;
  text-overflow: ellipsis;
}

.card-stats {
  margin-top: auto;
  display: flex;
  align-items: center;
  gap: var(--space-3);
  padding-top: var(--space-2);
  border-top: 1px solid var(--bg-subtle);
}

.card-stat {
  display: flex;
  align-items: center;
  gap: var(--space-1);
  font-size: var(--text-xs);
  color: var(--text-secondary);
}

.card-stat svg {
  color: var(--text-tertiary);
}

.card-stat.rating {
  margin-left: auto;
  color: var(--color-warning);
}

.card-stat.rating svg {
  color: var(--color-warning);
}

/* ── Pagination Bar ── */
.hub-pagination-bar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--space-2);
  padding: var(--space-1);
  flex-shrink: 0;
}

.pagination-info {
  font-size: var(--text-sm);
  color: var(--text-tertiary);
}

.pagination-controls {
  display: flex;
  align-items: center;
  gap: var(--space-1);
}

.page-indicator {
  min-width: 56px;
  text-align: center;
  font-size: var(--text-sm);
  color: var(--text-secondary);
  font-variant-numeric: tabular-nums;
}

/* ── Panel Overlay ── */

/* ── Slide-over Detail Panel ── */

.panel-category-tag {
  display: inline-flex;
  align-items: center;
  min-height: 22px;
  padding: 0 var(--space-2);
  background: var(--accent-muted);
  color: var(--accent-primary);
  border: none;
  border-radius: var(--radius-full);
  font-size: var(--text-xs);
  font-weight: var(--font-bold);
}

.panel-category-tag.pricing-free {
  background: var(--color-success-bg);
  color: var(--color-success);
  border-color: var(--color-success-bg);
}

.panel-category-tag.pricing-paid {
  background: var(--color-warning-bg);
  color: var(--color-warning);
  border-color: var(--color-warning-bg);
}

.panel-title {
  font-size: var(--text-3xl);
  font-weight: var(--font-heavy);
  overflow-wrap: anywhere;
}

.hub-workspace {
  position: relative;
  flex: 1;
  min-height: 0;
  display: flex;
  flex-direction: column;
  gap: var(--space-section);
}

.panel-loading {
  height: 100%;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: var(--space-4);
}

.panel-scroll-content {
  display: flex;
  flex-direction: column;
  gap: var(--space-section);
}

.panel-scroll-content::-webkit-scrollbar {
  width: 5px;
}
.panel-scroll-content::-webkit-scrollbar-track {
  background: transparent;
}
.panel-scroll-content::-webkit-scrollbar-thumb {
  background: var(--border-default);
  border-radius: var(--radius-full);
}

/* ── Panel summary ── */
.panel-hero-row {
  display: flex;
  flex-direction: column;
  gap: var(--space-stack);
}

.panel-icon-img {
  width: 64px;
  height: 64px;
  object-fit: contain;
}

.panel-img-placeholder {
  color: var(--text-tertiary);
  opacity: 0.3;
}

.panel-meta-info {
  flex: 1;
  min-width: 0;
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
}

.panel-version-row {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 6px;
}

.panel-type-badge {
  position: static;
  margin: 0;
  display: inline-flex;
  align-items: center;
  min-height: 22px;
  padding: 0 var(--space-2);
  border-radius: var(--radius-full);
  text-shadow: none;
}

.badge-version {
  display: inline-flex;
  align-items: center;
  min-height: 22px;
  font-size: var(--text-xs);
  font-weight: var(--font-bold);
  color: var(--text-primary);
  background: var(--bg-control);
  padding: 0 var(--space-2);
  border-radius: var(--radius-full);
}

.panel-creator {
  color: var(--text-tertiary);
  font-size: var(--text-sm);
}

.panel-creator .creator-name {
  color: var(--text-primary);
  font-weight: var(--font-bold);
}

.panel-stat-badges {
  display: flex;
  flex-wrap: wrap;
  gap: var(--space-2);
}

.badge-stat {
  display: inline-flex;
  align-items: center;
  min-height: 22px;
  font-size: var(--text-xs);
  padding: 0 var(--space-2);
  background: var(--bg-control);
  color: var(--text-secondary);
  border-radius: var(--radius-full);
}

.badge-stat.rating {
  color: var(--color-warning);
  background: var(--color-warning-bg);
}

.panel-tagline {
  color: var(--text-secondary);
  font-size: var(--text-base);
  line-height: 1.5;
}

/* ── Panel Section ── */
.panel-section {
  display: flex;
  flex-direction: column;
  gap: var(--space-3);
}

.section-title {
  margin: 0;
  font-size: var(--text-md);
  font-weight: var(--font-bold);
  color: var(--text-primary);
  display: flex;
  align-items: center;
  gap: var(--space-2);
}

.section-title svg {
  color: var(--accent-primary);
}

.desc-content {
  line-height: 1.6;
  max-height: 180px;
  overflow-y: auto;
  padding-right: var(--space-2);
}

.desc-content::-webkit-scrollbar {
  width: 4px;
}
.desc-content::-webkit-scrollbar-thumb {
  background: var(--border-default);
  border-radius: var(--radius-full);
}

/* ── Panel list items ── */
.panel-list {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
}

.list-item {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: var(--space-3) var(--space-4);
  background: var(--bg-subtle);
  border: none;
  border-radius: var(--radius-md);
  gap: var(--space-3);
}

.item-info {
  flex: 1;
  min-width: 0;
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.item-name {
  font-weight: var(--font-medium);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.item-meta {
  display: flex;
  align-items: center;
  gap: var(--space-3);
}

.license-type,
.dep-author,
.dep-tier {
  position: relative;
  padding-left: var(--space-2);
}

.license-type::before,
.dep-author::before,
.dep-tier::before {
  content: '•';
  position: absolute;
  left: -2px;
  opacity: 0.5;
}

.dep-tier.direct,
.dep-tier.sub {
  color: var(--text-tertiary);
}

.download-progress-container {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  width: 90px;
}

.download-progress-container.mini {
  width: 75px;
}

.progress-bar-wrapper {
  flex: 1;
  height: 4px;
  background: var(--bg-active);
  border-radius: var(--radius-full);
  overflow: hidden;
}

.progress-bar-fill {
  height: 100%;
  background: var(--accent-gradient);
  border-radius: var(--radius-full);
}

.progress-percentage {
  font-weight: var(--font-bold);
  min-width: 28px;
  text-align: right;
}

.download-status,
.dep-status {
  display: flex;
  align-items: center;
  gap: var(--space-1);
  font-weight: var(--font-bold);
}

.download-status.no-source {
  color: var(--color-warning);
  font-weight: var(--font-medium);
}

.badge {
  display: inline-flex;
  align-items: center;
  min-height: 22px;
  padding: 0 var(--space-2);
  border-radius: var(--radius-full);
  font-size: var(--text-xs);
  font-weight: var(--font-bold);
}

.badge.success {
  background: var(--color-success-bg);
  color: var(--color-success);
  border: none;
}

.badge.warning {
  background: var(--color-warning-bg);
  color: var(--color-warning);
  border: none;
}

.badge.upgrade {
  background: var(--color-warning-bg);
  color: var(--color-warning);
  border: none;
}

.empty-list-info {
  width: 100%;
  padding: var(--space-4);
  text-align: center;
  background: var(--bg-input);
  border: none;
  border-radius: var(--radius-md);
  color: var(--text-tertiary);
}

/* Animations */
.slide-panel-enter-active,
.slide-panel-leave-active {
  transition: transform 0.3s cubic-bezier(0.16, 1, 0.3, 1);
}
.slide-panel-enter-from,
.slide-panel-leave-to {
  transform: translateX(100%);
}

.fade-enter-active,
.fade-leave-active {
  transition: opacity 0.2s ease;
}
.fade-enter-from,
.fade-leave-to {
  opacity: 0;
}

/* ── Batch Download Button & Row ── */
.panel-actions-row {
  flex-shrink: 0;
  display: flex;
}
.panel-actions-row > .ui-button {
  flex: 1;
}

/* ── Batch Download Modal Backdrop & Container ── */

@keyframes modal-enter {
  from {
    opacity: 0;
    transform: scale(0.95) translateY(10px);
  }
  to {
    opacity: 1;
    transform: scale(1) translateY(0);
  }
}

.desc-loading {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  padding: var(--space-2) 0;
  color: var(--text-secondary);
}

/* ── 一键下载弹窗 ── */
.batch-loading {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: var(--space-3);
  padding: var(--space-8) 0 var(--space-4);
  text-align: center;
}

.batch-loading-title {
  color: var(--text-secondary);
  font-size: var(--text-base);
}

.resolve-progress {
  display: flex;
  align-items: center;
  gap: var(--space-3);
  width: min(360px, 100%);
}

.resolve-track {
  flex: 1;
  height: 4px;
  border-radius: var(--radius-full);
  background: var(--bg-active);
  overflow: hidden;
}

.resolve-fill {
  height: 100%;
  border-radius: var(--radius-full);
  background: var(--accent-primary);
  transition: width 0.3s ease-out;
}

.resolve-count {
  color: var(--text-tertiary);
  font-size: var(--text-xs);
  font-variant-numeric: tabular-nums;
}

.resolve-current {
  max-width: 100%;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  color: var(--text-tertiary);
  font-size: var(--text-xs);
}

.batch-content {
  display: flex;
  flex-direction: column;
  gap: var(--space-4);
}

.batch-summary {
  color: var(--text-secondary);
  font-size: var(--text-description);
}

.batch-group {
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
}

.batch-group-head {
  display: flex;
  align-items: center;
  gap: var(--space-3);
  min-height: 36px;
}

.batch-group-toggle {
  display: inline-flex;
  align-items: center;
  gap: var(--space-2);
  min-height: 32px;
  margin-left: calc(-1 * var(--space-2));
  padding: 0 var(--space-2);
  border-radius: var(--radius-control-sm);
  color: var(--text-primary);
}

.batch-group-toggle:hover {
  background: var(--bg-hover);
}

.batch-chevron {
  color: var(--text-tertiary);
  transition: transform var(--duration-fast) var(--ease);
}

.batch-chevron.collapsed {
  transform: rotate(-90deg);
}

.batch-group-title {
  font-size: var(--text-base);
  font-weight: var(--font-bold);
}

.batch-group-count {
  display: inline-flex;
  align-items: center;
  min-height: 20px;
  padding: 0 var(--space-2);
  border-radius: var(--radius-full);
  background: var(--bg-control);
  color: var(--text-secondary);
  font-size: var(--text-xs);
  font-weight: var(--font-bold);
}

.batch-group-hint {
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  color: var(--text-tertiary);
  font-size: var(--text-sm);
}

.batch-group-check {
  margin-left: auto;
  flex-shrink: 0;
  color: var(--text-secondary);
  font-size: var(--text-sm);
}

.batch-items {
  display: flex;
  flex-direction: column;
  border-radius: var(--radius-md);
  background: var(--bg-input);
}

.batch-item {
  display: flex;
  align-items: center;
  gap: var(--space-3);
  min-height: 40px;
  padding: var(--space-1) var(--space-4);
}

.batch-item + .batch-item {
  border-top: 1px solid var(--border-subtle);
}

.batch-item.selectable:hover {
  background: var(--bg-hover);
}

.batch-item:first-child {
  border-top-left-radius: var(--radius-md);
  border-top-right-radius: var(--radius-md);
}

.batch-item:last-child {
  border-bottom-left-radius: var(--radius-md);
  border-bottom-right-radius: var(--radius-md);
}

.batch-item-name {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  font-size: var(--text-sm);
}

.batch-item:not(.selectable) .batch-item-name {
  color: var(--text-secondary);
}

.batch-item-version {
  flex-shrink: 0;
  color: var(--color-success);
  font-size: var(--text-xs);
  font-weight: var(--font-semibold);
  font-variant-numeric: tabular-nums;
}

.batch-item-size {
  flex-shrink: 0;
  min-width: 64px;
  text-align: right;
  color: var(--text-tertiary);
  font-size: var(--text-xs);
  font-variant-numeric: tabular-nums;
}

.batch-footer-stats {
  margin-right: auto;
  align-self: center;
  color: var(--text-secondary);
  font-size: var(--text-sm);
  font-variant-numeric: tabular-nums;
}

/* 和左上角类型标签同一基线：同样 8px 偏移、22px 高 */
.hub-pricing {
  position: absolute;
  z-index: 2;
  top: var(--space-2);
  right: var(--space-2);
  display: inline-flex;
  align-items: center;
  min-height: 22px;
  padding: 0 var(--space-2);
  border-radius: var(--radius-full);
  background: var(--bg-on-image);
  color: var(--text-primary);
  font-size: var(--text-xs);
  font-weight: var(--font-bold);
  pointer-events: none;
}
.resources-grid :deep(.package-meta span:last-child) {
  display: flex;
  align-items: center;
  gap: var(--space-1);
}
</style>
