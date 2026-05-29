<template>
  <div class="online-hub-view animate-fadeIn">
    <!-- Header Controls -->
    <div class="hub-header-bar glass-panel">
      <div class="header-title-group">
        <svg width="20" height="20" viewBox="0 0 24 24" fill="none">
          <circle cx="12" cy="12" r="10" stroke="currentColor" stroke-width="1.5" />
          <path d="M2 12h20M12 2a15.3 15.3 0 0 1 4 10 15.3 15.3 0 0 1-4 10 15.3 15.3 0 0 1-4-10 15.3 15.3 0 0 1 4-10z" stroke="currentColor" stroke-width="1.5" />
        </svg>
        <h2 class="view-title">在线资源库</h2>
      </div>

      <div class="hub-filters">
        <!-- Pricing Filter -->
        <div class="filter-group">
          <select v-model="selectedPricing" class="hub-select" @change="resetAndSearch">
            <option value="free">免费资源</option>
            <option value="all">全部资源</option>
            <option value="premium">付费资源</option>
          </select>
        </div>

        <!-- Category Select -->
        <div class="filter-group">
          <select v-model="selectedCategory" class="hub-select" @change="resetAndSearch">
            <option value="">所有分类</option>
            <option value="Scenes">场景 (Scenes)</option>
            <option value="Looks">外观 (Looks)</option>
            <option value="Clothing">服装 (Clothing)</option>
            <option value="Hairstyles">发型 (Hairstyles)</option>
            <option value="Plugins + Scripts">插件 (Plugins)</option>
            <option value="Morphs">变形 (Morphs)</option>
            <option value="Assets + Accessories">道具 (Assets)</option>
            <option value="Toolkits + Templates">工具包 (Toolkits)</option>
            <option value="Audio">音频 (Audio)</option>
          </select>
        </div>

        <!-- Sorting Select -->
        <div class="filter-group">
          <select v-model="selectedSort" class="hub-select" @change="resetAndSearch">
            <option value="last_update">最近更新</option>
            <option value="resource_date">提交日期</option>
            <option value="reaction_score">反应分数</option>
            <option value="download_count">下载量</option>
            <option value="rating_weighted">最爱</option>
            <option value="rating_count">喜欢</option>
            <option value="title">按标题</option>
            <option value="username">按用户名</option>
          </select>
        </div>

        <!-- Search Bar -->
        <div class="hub-search-bar">
          <input
            v-model="searchQuery"
            type="text"
            class="hub-search-input"
            placeholder="搜索关键字或包名 (例如 macgruber.Life.latest)"
            @keyup.enter="handleSearch"
          />
          <button
            class="hub-search-btn"
            :disabled="loading"
            @click="handleSearch"
          >
            <svg v-if="loading" class="spinner" width="14" height="14" viewBox="0 0 24 24" fill="none">
              <circle cx="12" cy="12" r="10" stroke="currentColor" stroke-width="3" stroke-dasharray="32" stroke-linecap="round" />
            </svg>
            <svg v-else width="14" height="14" viewBox="0 0 24 24" fill="none">
              <path d="M21 21l-6-6m2-5a7 7 0 1 1-14 0 7 7 0 0 1 14 0z" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" />
            </svg>
            <span>搜索</span>
          </button>
        </div>
      </div>
    </div>

    <!-- Error State -->
    <div v-if="error" class="hub-error-msg animate-fadeIn">
      <svg width="18" height="18" viewBox="0 0 24 24" fill="none">
        <path d="M12 9v4m0 4h.01m-6.938 4h13.856c1.54 0 2.502-1.667 1.732-3L13.732 4c-.77-1.333-2.694-1.333-3.464 0L3.34 16c-.77 1.333.192 3 1.732 3z" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" />
      </svg>
      <span>{{ error }}</span>
    </div>

    <!-- Main Content Area -->
    <div :class="['hub-content-scroll', { 'is-loading': loading && filteredAndSortedResources.length > 0 }]" ref="scrollContainer">
      <div class="hub-results-section">
        <!-- Loading Overlay -->
        <div v-if="loading && filteredAndSortedResources.length === 0" class="hub-loading-state">
          <div class="loading-spinner"></div>
          <p class="text-secondary text-sm">正在加载在线资源...</p>
        </div>

        <div v-if="loading && filteredAndSortedResources.length > 0" class="hub-inline-loading-overlay">
          <div class="hub-inline-loading-card">
            <div class="loading-spinner"></div>
            <p class="text-secondary text-sm">正在加载本页资源...</p>
          </div>
        </div>

        <!-- Resources Grid -->
        <div v-else-if="filteredAndSortedResources.length > 0" class="resources-grid">
          <div
            v-for="item in filteredAndSortedResources"
            :key="item.resource_id"
            class="resource-grid-card glass-card"
            @click="openPackageDetail(item)"
          >
            <!-- Thumbnail Area -->
            <div class="card-thumb-area">
              <img v-if="item.image_url" :src="item.image_url" class="card-img" alt="cover" loading="lazy" />
              <img v-else-if="item.icon_url" :src="item.icon_url" class="card-icon-img" alt="icon" loading="lazy" />
              <div v-else class="card-placeholder">
                <svg width="32" height="32" viewBox="0 0 24 24" fill="none">
                  <path d="M12 2L2 7l10 5 10-5-10-5zM2 17l10 5 10-5M2 12l10 5 10-5" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" />
                </svg>
              </div>
              <!-- Resource Type Badge (top-left) -->
              <span class="card-type-badge" v-if="item.type" :class="getTypeBadgeClass(item.type)">{{ translateType(item.type) }}</span>
              <!-- Pricing Badge (top-right) -->
              <span class="card-pricing-badge" v-if="item.category" :class="item.category === 'Free' ? 'pricing-free' : 'pricing-paid'">{{ item.category === 'Free' ? '免费' : '付费' }}</span>
            </div>

            <!-- Card Details -->
            <div class="card-info-area">
              <h3 class="card-title" :title="item.title">{{ item.title || '未命名资源' }}</h3>
              <div class="card-creator text-xs text-secondary">
                作者: <span class="creator-name">{{ item.username || '匿名' }}</span>
              </div>
              
              <p class="card-tagline text-xs text-secondary" v-if="item.tag_line" :title="item.tag_line">
                {{ item.tag_line }}
              </p>

              <div class="card-stats">
                <div class="card-stat" :title="'下载次数: ' + item.download_count">
                  <svg width="12" height="12" viewBox="0 0 24 24" fill="none">
                    <path d="M4 16v1a3 3 0 0 0 3 3h10a3 3 0 0 0 3-3v-1m-4-4l-4 4m0 0l-4-4m4 4V4" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" />
                  </svg>
                  <span>{{ formatCompactNumber(item.download_count) }}</span>
                </div>
                <div class="card-stat" :title="'浏览量: ' + item.view_count">
                  <svg width="12" height="12" viewBox="0 0 24 24" fill="none">
                    <path d="M1 12s4-8 11-8 11 8 11 8-4 8-11 8-11-8-11-8z" stroke="currentColor" stroke-width="1.5" stroke-linecap="round"/>
                    <circle cx="12" cy="12" r="3" stroke="currentColor" stroke-width="1.5" stroke-linecap="round"/>
                  </svg>
                  <span>{{ formatCompactNumber(item.view_count) }}</span>
                </div>
                <div class="card-stat rating" v-if="item.rating_avg" :title="'平均评分: ' + item.rating_avg">
                  <svg width="12" height="12" viewBox="0 0 24 24" fill="none">
                    <path d="M12 2l3.09 6.26L22 9.27l-5 4.87 1.18 6.88L12 17.77l-6.18 3.25L7 14.14 2 9.27l6.91-1.01L12 2z" fill="currentColor" />
                  </svg>
                  <span>{{ parseFloat(item.rating_avg).toFixed(1) }}</span>
                </div>
              </div>
            </div>
          </div>
        </div>

        <!-- Empty State -->
        <div v-else-if="!loading" class="hub-empty-state animate-fadeIn">
          <svg width="48" height="48" viewBox="0 0 24 24" fill="none">
            <path d="M21 21l-6-6m2-5a7 7 0 1 1-14 0 7 7 0 0 1 14 0z" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" />
          </svg>
          <h3>未找到任何在线包</h3>
          <p class="text-secondary text-sm">尝试在上方搜索其他关键字，或更改分类选项。</p>
        </div>
      </div>

      <!-- Pagination Bar -->
      <div v-if="filteredAndSortedResources.length > 0 && totalPages > 1" class="hub-pagination-bar glass-panel">
        <button
          class="page-btn"
          :disabled="currentPage <= 1 || loading"
          @click="changePage(currentPage - 1)"
        >
          <svg width="14" height="14" viewBox="0 0 24 24" fill="none">
            <path d="M15 19l-7-7 7-7" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" />
          </svg>
          <span>上一页</span>
        </button>

        <span class="page-indicator">第 {{ currentPage }} 页 / 共 {{ totalPages }} 页 (找到 {{ totalFound }} 项)</span>

        <button
          class="page-btn"
          :disabled="currentPage >= totalPages || loading"
          @click="changePage(currentPage + 1)"
        >
          <span>下一页</span>
          <svg width="14" height="14" viewBox="0 0 24 24" fill="none">
            <path d="M9 5l7 7-7 7" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" />
          </svg>
        </button>
      </div>
    </div>

    <!-- Background dim overlay for detail panel -->
    <Transition name="fade">
      <div v-if="detailPanelOpen" class="panel-overlay" @click="closeDetailPanel"></div>
    </Transition>

    <!-- Slide-over Detail Panel -->
    <Transition name="slide-panel">
      <div v-if="detailPanelOpen" class="detail-slide-panel glass-panel">
        <div class="panel-header">
          <button class="panel-close-btn" @click="closeDetailPanel">
            <svg width="18" height="18" viewBox="0 0 24 24" fill="none">
              <path d="M18 6L6 18M6 6l12 12" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" />
            </svg>
          </button>
          <div class="panel-title-wrap">
            <span class="panel-category-tag" v-if="detailPackage.category" :class="detailPackage.category === 'Free' ? 'pricing-free' : 'pricing-paid'">
              {{ detailPackage.category === 'Free' ? '免费资源' : '付费资源' }}
            </span>
            <h3 class="panel-title">{{ detailPackage.title || '包详情' }}</h3>
          </div>
        </div>

        <div class="panel-body">
          <div v-if="detailLoading && !detailPackage.title" class="panel-loading">
            <div class="loading-spinner"></div>
            <p class="text-secondary text-sm">正在获取该包的详细依赖信息...</p>
          </div>

          <div v-else class="panel-scroll-content">
            <!-- Summary Info Card -->
            <div class="panel-info-card glass-card">
              <div class="panel-hero-row">
                <div class="panel-image-container">
                  <img v-if="detailPackage.image_url" :src="detailPackage.image_url" class="panel-cover-img" alt="cover" />
                  <img v-else-if="detailPackage.icon_url" :src="detailPackage.icon_url" class="panel-icon-img" alt="icon" />
                  <div v-else class="panel-img-placeholder">
                    <svg width="40" height="40" viewBox="0 0 24 24" fill="none">
                      <path d="M12 2L2 7l10 5 10-5-10-5zM2 17l10 5 10-5M2 12l10 5 10-5" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" />
                    </svg>
                  </div>
                </div>

                <div class="panel-meta-info">
                  <div class="panel-version-row">
                    <span class="badge-version" v-if="detailPackage.version_string">v{{ detailPackage.version_string }}</span>
                    <span class="card-type-badge" v-if="detailPackage.type" :class="getTypeBadgeClass(detailPackage.type)" style="position: static; margin-left: var(--space-2); margin-top: 0; display: inline-flex;">{{ translateType(detailPackage.type) }}</span>
                  </div>
                  <div class="panel-creator text-sm text-secondary">
                    作者: <span class="creator-name">{{ detailPackage.username }}</span>
                  </div>
                  <div class="panel-stat-badges">
                    <span class="badge-stat">下载: {{ formatNumber(detailPackage.download_count) }}</span>
                    <span class="badge-stat">浏览: {{ formatNumber(detailPackage.view_count) }}</span>
                    <span class="badge-stat rating" v-if="detailPackage.rating_avg">
                      评分: {{ parseFloat(detailPackage.rating_avg).toFixed(1) }} ({{ detailPackage.rating_count || 0 }}人评)
                    </span>
                  </div>
                </div>
              </div>

              <div class="panel-tagline text-sm italic text-secondary" v-if="detailPackage.tag_line">
                "{{ detailPackage.tag_line }}"
              </div>

              <!-- Batch download action button -->
              <div class="panel-actions-row">
                <button
                  class="batch-download-btn"
                  :disabled="detailLoading"
                  @click="openBatchDownloadModal"
                >
                  <span>一键下载</span>
                </button>
              </div>
            </div>

            <!-- Description -->
            <div class="panel-section">
              <h4 class="section-title">详细描述</h4>
              <div v-if="descLoading" class="desc-loading text-secondary text-sm">
                <div class="loading-spinner-sm"></div>
                <span>正在加载描述...</span>
              </div>
              <div v-else-if="detailPackage.description" class="desc-content text-sm text-secondary" v-html="formatDescription(detailPackage.description)"></div>
              <div v-else class="text-secondary text-sm italic">无详细描述</div>
            </div>

            <!-- Files list -->
            <div class="panel-section">
              <h4 class="section-title">
                <svg width="14" height="14" viewBox="0 0 24 24" fill="none">
                  <path d="M7 18H17M7 14H17M7 10H12M4 22V2h16v20" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" />
                </svg>
                <span>包含的 .var 包 ({{ detailPackage.hubFiles?.length || 0 }})</span>
              </h4>

              <div v-if="descLoading" class="desc-loading text-secondary text-sm">
                <div class="loading-spinner-sm"></div>
                <span>正在加载文件列表...</span>
              </div>
              <div v-else-if="detailPackage.hubFiles?.length" class="panel-list files-list-container">
                <div v-for="file in detailPackage.hubFiles" :key="file.filename" class="list-item file-item glass-card">
                  <div class="item-info">
                    <div class="item-name text-sm text-primary" :title="file.filename">{{ file.filename }}</div>
                    <div class="item-meta text-xs text-secondary">
                      <span v-if="file.file_size">大小: {{ formatDisplaySize(file.file_size) }}</span>
                      <span v-if="file.licenseType" class="license-type">许可: {{ file.licenseType }}</span>
                    </div>
                  </div>

                  <div class="download-action">
                    <!-- If installed locally -->
                    <div v-if="isFileInstalled(file.filename) || (downloadStates[file.filename] && downloadStates[file.filename].status === 'completed')" class="download-status completed text-success text-xs">
                      <span>已下载</span>
                    </div>

                    <!-- If no download URL available -->
                    <div v-else-if="!file.urlHosted" class="download-status no-source text-xs">
                      <span>无下载链接</span>
                    </div>

                    <!-- If not downloading / completed / error -->
                    <button
                      v-else-if="!downloadStates[file.filename] || downloadStates[file.filename].status === 'idle'"
                      class="download-btn-sm"
                      @click="handleDownload(file)"
                    >
                      <span>下载</span>
                    </button>
                    
                    <!-- If downloading -->
                    <div v-else-if="downloadStates[file.filename].status === 'downloading'" class="download-progress-container">
                      <div class="progress-bar-wrapper">
                        <div class="progress-bar-fill" :style="{ width: downloadStates[file.filename].progress + '%' }"></div>
                      </div>
                      <span class="progress-percentage text-xs text-accent">{{ Math.round(downloadStates[file.filename].progress) }}%</span>
                    </div>
                    
                    <!-- If error -->
                    <button 
                      v-else-if="downloadStates[file.filename].status === 'error'"
                      class="download-btn-sm error"
                      :title="downloadStates[file.filename].errorMsg"
                      @click="handleDownload(file)"
                    >
                      <span>重试</span>
                    </button>
                  </div>
                </div>
              </div>
              <div v-else class="text-secondary text-sm italic">无包含的文件包</div>
            </div>

            <!-- Dependencies list -->
            <div class="panel-section">
              <h4 class="section-title">
                <svg width="14" height="14" viewBox="0 0 24 24" fill="none">
                  <path d="M19 11H5m14 0a2 2 0 012 2v6a2 2 0 01-2 2H5a2 2 0 01-2-2v-6a2 2 0 012-2m14 0V9a2 2 0 00-2-2M5 11V9a2 2 0 012-2m0 0V5a2 2 0 012-2h6a2 2 0 012 2v2M7 7h10" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" />
                </svg>
                <span>依赖关系分析 ({{ uniqueDependencies.length }}，直接 {{ directDependencies.length }} / 子依赖 {{ subDependencies.length }})</span>
              </h4>

              <div v-if="dependenciesLoading" class="desc-loading text-secondary text-sm">
                <div class="loading-spinner-sm"></div>
                <span>正在分析包依赖...</span>
              </div>
              <div v-else-if="uniqueDependencies.length" class="panel-list dependencies-list-container">
                <div v-for="dep in uniqueDependencies" :key="dep.filename || dep.packageName" class="list-item dep-item glass-card">
                  <div class="item-info">
                    <div class="item-name text-sm text-primary" :title="dep.packageName || dep.filename">
                      {{ dep.packageName || dep.filename }}
                    </div>
                    <div class="item-meta text-xs text-secondary">
                      <span v-if="dep.file_size">大小: {{ formatDisplaySize(dep.file_size) }}</span>
                      <span v-if="dep.username" class="dep-author">作者: {{ dep.username }}</span>
                      <span v-if="dep.dependencyType" class="dep-tier" :class="dep.dependencyType === 'sub' ? 'sub' : 'direct'">
                        {{ formatDependencyType(dep) }}
                      </span>
                    </div>
                  </div>

                  <div class="dep-action">
                    <!-- Installed Badge (version sufficient) -->
                    <div v-if="getDependencyInstallStatus(dep) === 'installed'" class="dep-status installed text-success text-xs">
                      <span class="badge success">已安装</span>
                    </div>

                    <!-- Upgrade Badge (installed but version too low) -->
                    <div v-else-if="getDependencyInstallStatus(dep) === 'upgrade'" class="dep-status-download">
                      <template v-if="dep.downloadUrl">
                        <button
                          v-if="!downloadStates[dep.filename || getFilenameFromUrl(dep.downloadUrl)] || downloadStates[dep.filename || getFilenameFromUrl(dep.downloadUrl)].status === 'idle'"
                          class="download-btn-sm upgrade"
                          @click="handleDownloadDependency(dep)"
                        >
                          <span>升级</span>
                        </button>

                        <div v-else-if="downloadStates[dep.filename || getFilenameFromUrl(dep.downloadUrl)].status === 'downloading'" class="download-progress-container mini">
                          <div class="progress-bar-wrapper">
                            <div class="progress-bar-fill" :style="{ width: downloadStates[dep.filename || getFilenameFromUrl(dep.downloadUrl)].progress + '%' }"></div>
                          </div>
                          <span class="progress-percentage text-xs text-accent">{{ Math.round(downloadStates[dep.filename || getFilenameFromUrl(dep.downloadUrl)].progress) }}%</span>
                        </div>

                        <div v-else-if="downloadStates[dep.filename || getFilenameFromUrl(dep.downloadUrl)].status === 'completed'" class="dep-status installed text-success text-xs">
                          <span class="badge success">已安装</span>
                        </div>

                        <button
                          v-else-if="downloadStates[dep.filename || getFilenameFromUrl(dep.downloadUrl)].status === 'error'"
                          class="download-btn-sm error"
                          @click="handleDownloadDependency(dep)"
                        >
                          <span>重试</span>
                        </button>
                      </template>
                      <span v-else class="badge upgrade">需升级</span>
                    </div>
                    
                    <!-- Not Installed / Download Action -->
                    <div v-else class="dep-status-download">
                      <template v-if="dep.downloadUrl">
                        <!-- Direct download if URL exists and we are not downloading it -->
                        <button
                          v-if="!downloadStates[dep.filename || getFilenameFromUrl(dep.downloadUrl)] || downloadStates[dep.filename || getFilenameFromUrl(dep.downloadUrl)].status === 'idle'"
                          class="download-btn-sm secondary"
                          @click="handleDownloadDependency(dep)"
                        >
                          <span>下载</span>
                        </button>
                        
                        <!-- Downloading state for dependency -->
                        <div v-else-if="downloadStates[dep.filename || getFilenameFromUrl(dep.downloadUrl)].status === 'downloading'" class="download-progress-container mini">
                          <div class="progress-bar-wrapper">
                            <div class="progress-bar-fill" :style="{ width: downloadStates[dep.filename || getFilenameFromUrl(dep.downloadUrl)].progress + '%' }"></div>
                          </div>
                          <span class="progress-percentage text-xs text-accent">{{ Math.round(downloadStates[dep.filename || getFilenameFromUrl(dep.downloadUrl)].progress) }}%</span>
                        </div>
                        
                        <!-- Completed state -->
                        <div v-else-if="downloadStates[dep.filename || getFilenameFromUrl(dep.downloadUrl)].status === 'completed'" class="dep-status installed text-success text-xs">
                          <span class="badge success">已安装</span>
                        </div>
                        
                        <!-- Error state -->
                        <button
                          v-else-if="downloadStates[dep.filename || getFilenameFromUrl(dep.downloadUrl)].status === 'error'"
                          class="download-btn-sm error"
                          @click="handleDownloadDependency(dep)"
                        >
                          <span>重试</span>
                        </button>
                      </template>
                      
                      <!-- No direct download URL -->
                      <span v-else class="badge warning">未安装</span>
                    </div>
                  </div>
                </div>
              </div>
              <div v-else class="empty-list-info text-xs text-secondary">
                该包无其它包依赖项 (纯净包)
              </div>
            </div>
          </div>
        </div>
      </div>
    </Transition>

    <!-- Batch Download Modal -->
    <Transition name="fade">
      <div v-if="batchModalOpen" class="batch-modal-backdrop" @click="closeBatchModal">
        <div class="batch-modal-container glass-panel" @click.stop>
          <div class="batch-modal-header">
            <h3 class="batch-modal-title">
              <svg width="20" height="20" viewBox="0 0 24 24" fill="none" style="color: var(--accent-primary);">
                <path d="M4 16v1a3 3 0 0 0 3 3h10a3 3 0 0 0 3-3v-1m-4-4l-4 4m0 0l-4-4m4 4V4" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" />
              </svg>
              <span>{{ $t('download.modal.title') }}</span>
            </h3>
            <button class="batch-modal-close-btn" @click="closeBatchModal">
              <svg width="18" height="18" viewBox="0 0 24 24" fill="none">
                <path d="M18 6L6 18M6 6l12 12" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" />
              </svg>
            </button>
          </div>

          <div class="batch-modal-body">
            <div v-if="batchResolving" class="batch-modal-loading">
              <div class="loading-spinner"></div>
              <p class="text-secondary text-sm">{{ $t('download.modal.loading') }}</p>
              
              <!-- Real-time Dependency Analysis Progress Bar -->
              <div class="resolve-progress-container">
                <div class="progress-bar-wrapper">
                  <div class="progress-bar-fill" :style="{ width: resolveProgress.percentage + '%' }"></div>
                </div>
                <div class="progress-info text-xs">
                  <span class="text-accent" style="color: var(--accent-primary); font-weight: var(--font-semibold);">
                    {{ resolveProgress.percentage }}%
                  </span>
                  <span class="text-secondary">
                    ({{ resolveProgress.resolved }} / {{ resolveProgress.total }})
                  </span>
                </div>
                <p class="resolve-current-package text-xs text-secondary truncate" v-if="resolveProgress.currentPackage" :title="resolveProgress.currentPackage">
                  正在解析: {{ resolveProgress.currentPackage }}
                </p>
              </div>
            </div>

            <div v-else class="batch-modal-content">
              <p class="batch-modal-summary text-sm text-secondary">
                {{ $t('download.modal.summary', { total: checklistItems.length, size: formattedTotalSize }) }}
              </p>

              <!-- Group sections -->
              <div class="groups-wrapper">
                <!-- Group: To Download -->
                <div v-if="needDownloadGroup.length" class="group-section">
                  <div class="group-header" @click="collapsedGroups.need_download = !collapsedGroups.need_download">
                    <input 
                      type="checkbox" 
                      :checked="isGroupAllChecked('need_download')" 
                      :indeterminate="isGroupSomeChecked('need_download')"
                      @click.stop="toggleGroupSelection('need_download')" 
                      class="group-checkbox"
                    />
                    <span class="group-dot to-download"></span>
                    <span class="group-title text-sm font-semibold">{{ $t('download.modal.groupToDownload', { count: needDownloadGroup.length }) }}</span>
                    <span class="expand-arrow" :class="{ 'collapsed': collapsedGroups.need_download }">
                      <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polyline points="6 9 12 15 18 9"></polyline></svg>
                    </span>
                  </div>
                  <div v-show="!collapsedGroups.need_download" class="group-items">
                      <label v-for="item in needDownloadGroup" :key="item.id" class="checklist-item-row">
                        <input type="checkbox" v-model="item.checked" class="item-checkbox" />
                        <span class="item-filename text-sm" :title="item.filename">{{ item.filename }}</span>
                      <span class="item-size text-xs text-secondary">{{ formatDisplaySize(item.file_size) }}</span>
                      </label>
                  </div>
                </div>

                <!-- Group: Upgraded -->
                <div v-if="upgradedGroup.length" class="group-section">
                  <div class="group-header" @click="collapsedGroups.upgraded = !collapsedGroups.upgraded">
                    <input 
                      type="checkbox" 
                      :checked="isGroupAllChecked('upgraded')" 
                      :indeterminate="isGroupSomeChecked('upgraded')"
                      @click.stop="toggleGroupSelection('upgraded')" 
                      class="group-checkbox"
                    />
                    <span class="group-dot upgraded"></span>
                    <span class="group-title text-sm font-semibold">{{ $t('download.modal.groupUpgraded', { count: upgradedGroup.length }) }}</span>
                    <span class="expand-arrow" :class="{ 'collapsed': collapsedGroups.upgraded }">
                      <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polyline points="6 9 12 15 18 9"></polyline></svg>
                    </span>
                  </div>
                  <div v-show="!collapsedGroups.upgraded" class="group-items">
                      <label v-for="item in upgradedGroup" :key="item.id" class="checklist-item-row">
                        <input type="checkbox" v-model="item.checked" class="item-checkbox" />
                        <span class="item-filename text-sm" :title="item.filename">{{ item.filename }} (v{{ item.queued_version }} -> v{{ item.version }})</span>
                      <span class="item-size text-xs text-secondary">{{ formatDisplaySize(item.file_size) }}</span>
                      </label>
                  </div>
                </div>

                <!-- Group: Queued/Skip -->
                <div v-if="queuedGroup.length" class="group-section">
                  <div class="group-header" @click="collapsedGroups.queued = !collapsedGroups.queued">
                    <input 
                      type="checkbox" 
                      :checked="isGroupAllChecked('queued')" 
                      :indeterminate="isGroupSomeChecked('queued')"
                      @click.stop="toggleGroupSelection('queued')" 
                      class="group-checkbox"
                    />
                    <span class="group-dot queued"></span>
                    <span class="group-title text-sm font-semibold">{{ $t('download.modal.groupInQueue', { count: queuedGroup.length }) }}</span>
                    <span class="expand-arrow" :class="{ 'collapsed': collapsedGroups.queued }">
                      <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polyline points="6 9 12 15 18 9"></polyline></svg>
                    </span>
                  </div>
                  <div v-show="!collapsedGroups.queued" class="group-items">
                      <label v-for="item in queuedGroup" :key="item.id" class="checklist-item-row text-disabled">
                        <input type="checkbox" v-model="item.checked" class="item-checkbox" />
                        <span class="item-filename text-sm" :title="item.filename">{{ item.filename }}</span>
                      <span class="item-size text-xs text-secondary">{{ formatDisplaySize(item.file_size) }}</span>
                      </label>
                  </div>
                </div>

                <!-- Group: Installed/Skip -->
                <div v-if="installedGroup.length" class="group-section">
                  <div class="group-header" @click="collapsedGroups.installed = !collapsedGroups.installed">
                    <input 
                      type="checkbox" 
                      :checked="isGroupAllChecked('installed')" 
                      :indeterminate="isGroupSomeChecked('installed')"
                      @click.stop="toggleGroupSelection('installed')" 
                      class="group-checkbox"
                    />
                    <span class="group-dot installed"></span>
                    <span class="group-title text-sm font-semibold">{{ $t('download.modal.groupInstalled', { count: installedGroup.length }) }}</span>
                    <span class="expand-arrow" :class="{ 'collapsed': collapsedGroups.installed }">
                      <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polyline points="6 9 12 15 18 9"></polyline></svg>
                    </span>
                  </div>
                  <div v-show="!collapsedGroups.installed" class="group-items">
                      <label v-for="item in installedGroup" :key="item.id" class="checklist-item-row text-disabled">
                        <input type="checkbox" v-model="item.checked" class="item-checkbox" />
                        <span class="item-filename text-sm" :title="item.filename">{{ item.filename }}</span>
                      <span class="item-size text-xs text-secondary">{{ formatDisplaySize(item.file_size) }}</span>
                      </label>
                  </div>
                </div>

                <!-- Group: No Source/Excluded -->
                <div v-if="noSourceGroup.length" class="group-section">
                  <div class="group-header" @click="collapsedGroups.no_source = !collapsedGroups.no_source">
                    <input type="checkbox" :checked="false" :disabled="true" class="group-checkbox" />
                    <span class="group-dot no-source"></span>
                    <span class="group-title text-sm font-semibold">{{ $t('download.modal.groupNoSource', { count: noSourceGroup.length }) }}</span>
                    <span class="expand-arrow" :class="{ 'collapsed': collapsedGroups.no_source }">
                      <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polyline points="6 9 12 15 18 9"></polyline></svg>
                    </span>
                  </div>
                  <div v-show="!collapsedGroups.no_source" class="group-items">
                    <label v-for="item in noSourceGroup" :key="item.id" class="checklist-item-row text-disabled no-source-row">
                      <input type="checkbox" :checked="false" :disabled="true" class="item-checkbox" />
                      <span class="item-filename text-sm" :title="item.filename">{{ item.filename }}</span>
                      <span class="item-badge text-xs">{{ $t('download.modal.actionNoDownloadable') }}</span>
                      <button class="manual-mark-btn" type="button" @click.stop.prevent="copyManualDependency(item)">
                        {{ $t('download.modal.manualMark') }}
                      </button>
                    </label>
                  </div>
                </div>
              </div>
            </div>
          </div>

          <div v-if="!batchResolving" class="batch-modal-footer">
            <div class="footer-stats text-sm">
              <span>已选择: <strong class="text-accent font-semibold" style="color: var(--accent-primary);">{{ checkedCount }}</strong> 个包</span>
              <span class="divider" style="margin: 0 var(--space-2); opacity: 0.5;">/</span>
              <span>预计下载大小: <strong class="text-success font-semibold" style="color: var(--color-success);">{{ formattedCheckedSize }}</strong></span>
            </div>
            <div class="footer-actions">
              <button class="action-btn-cancel" @click="closeBatchModal">{{ $t('common.cancel') }}</button>
              <button 
                class="action-btn-confirm" 
                :disabled="checkedCount === 0"
                @click="confirmBatchDownload"
              >
                <span>{{ $t('download.modal.action') }}</span>
              </button>
            </div>
          </div>
        </div>
      </div>
    </Transition>
  </div>
</template>

<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref } from 'vue'
import { useRouter } from 'vue-router'
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
const batchResolving = ref(false)
const checklistItems = ref<any[]>([])
const collapsedGroups = ref({
  need_download: true,
  upgraded: true,
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
    error.value = err.toString() || '网络请求失败，请检查网络连接'
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
  detailPackage.value = { ...item, description: '', hubFiles: [], dependencies: {} }
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
      detailPackage.value = { title: pkgName, description: '未找到该包的详细信息，请确认包标识符正确。' }
    }
  } catch (err: any) {
    if (currentLoadingPkgName.value !== pkgName) return
    detailPackage.value = { title: pkgName, description: `加载失败: ${err.toString()}` }
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

const directDependencies = computed(() => {
  return uniqueDependencies.value.filter((dep: any) => dep.dependencyType !== 'sub')
})

const subDependencies = computed(() => {
  return uniqueDependencies.value.filter((dep: any) => dep.dependencyType === 'sub')
})

function formatDependencyType(dep: any): string {
  if (dep.dependencyType === 'sub') {
    return `子依赖 Tier ${dep.tier || 2}`
  }
  return '直接依赖'
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
    notify.error('该文件没有可用的下载链接')
    return
  }
  if (!vamRootPath.value) {
    notify.error('VAM 根目录未配置，请先在设置中配置')
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
      notify.info(`跳过添加: ${results[0].Skipped}`)
    } else {
      notify.success(t('download.toast.added', { count: 1 }))
    }
  } catch (error: any) {
    console.error('Failed to add single download:', error)
    notify.error('添加下载失败: ' + error.toString())
  }
}

// 依赖下载处理
async function handleDownloadDependency(dep: any) {
  if (!dep) return
  if (!dep.downloadUrl) {
    notify.error('该依赖没有可用的下载链接')
    return
  }
  if (!vamRootPath.value) {
    notify.error('VAM 根目录未配置，请先在设置中配置')
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
      notify.info(`跳过添加: ${results[0].Skipped}`)
    } else {
      notify.success(t('download.toast.added', { count: 1 }))
    }
  } catch (error: any) {
    console.error('Failed to add dependency download:', error)
    notify.error('添加下载失败: ' + error.toString())
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
    notify.error('VAM 根目录未配置，请先在设置中配置')
    return
  }

  if (!detailPackage.value.hubFiles || detailPackage.value.hubFiles.length === 0) {
    notify.warning('此资源没有可供下载的文件包 (可能是外部链接或付费资源)')
    return
  }
  
  batchModalOpen.value = true
  batchResolving.value = true
  checklistItems.value = []
  collapsedGroups.value = {
    need_download: true,
    upgraded: true,
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
          file_size: file.file_size ? (isNaN(Number(file.file_size)) ? file.file_size : formatBytes(Number(file.file_size))) : '未知',
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
      notify.error('依赖解析失败: ' + errMsg)
    }
    batchModalOpen.value = false
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
      notify.info(`跳过了 ${skippedCount} 个已存在或已安装的任务`)
    }
    
    batchModalOpen.value = false
    detailPanelOpen.value = false
    router.push('/download')
  } catch (err: any) {
    console.error('Failed to batch download:', err)
    notify.error('添加下载任务失败: ' + err.toString())
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

// Convert "12.3 MB" to bytes
function parseSizeStringToBytes(sizeStr: string): number {
  if (!sizeStr) return 0
  const trimmed = sizeStr.trim().toUpperCase()
  const match = trimmed.match(/^([\d.]+)\s*(B|KB|MB|GB|TB)?$/)
  if (!match) return 0
  const value = parseFloat(match[1])
  const unit = match[2] || 'B'
  const multipliers: Record<string, number> = {
    'B': 1,
    'KB': 1024,
    'MB': 1024 * 1024,
    'GB': 1024 * 1024 * 1024,
    'TB': 1024 * 1024 * 1024 * 1024
  }
  // 必须取整，否则 Rust 后端 u64 无法反序列化浮点数
  return Math.round(value * (multipliers[unit] || 1))
}

function formatBytes(bytes: number): string {
  if (!bytes || bytes === 0) return '0 B'
  const units = ['B', 'KB', 'MB', 'GB']
  const i = Math.min(Math.floor(Math.log(bytes) / Math.log(1024)), units.length - 1)
  return `${(bytes / Math.pow(1024, i)).toFixed(1)} ${units[i]}`
}

// 统一显示大小，兼容字节数和已格式化文本
function formatDisplaySize(size: string | number | null | undefined): string {
  if (size === null || size === undefined || size === '') return '未知'
  if (typeof size === 'number') return formatBytes(size)

  const text = String(size).trim()
  if (!text) return '未知'
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
function formatDescription(desc: string): string {
  if (!desc) return ''
  let formatted = desc.replace(/\r\n|\n/g, '<br/>')
  formatted = formatted.replace(/\[b\](.*?)\[\/b\]/gi, '<strong>$1</strong>')
  formatted = formatted.replace(/\[i\](.*?)\[\/i\]/gi, '<em>$1</em>')
  formatted = formatted.replace(/\[url=(.*?)\](.*?)\[\/url\]/gi, '<a href="$1" target="_blank" class="about-link">$2</a>')
  return formatted
}

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

function translateType(type: string): string {
  const t = (type || '').toLowerCase()
  if (t.includes('scene')) return '场景'
  if (t.includes('look')) return '外观'
  if (t.includes('cloth')) return '服装'
  if (t.includes('hair')) return '发型'
  if (t.includes('plugin') || t.includes('script')) return '插件'
  if (t.includes('morph')) return '变形'
  if (t.includes('asset') || t.includes('accessor')) return '道具'
  if (t.includes('toolkit') || t.includes('template')) return '工具包'
  if (t.includes('audio')) return '音频'
  return type
}
</script>

<style scoped>
.online-hub-view {
  height: 100%;
  display: flex;
  flex-direction: column;
  gap: var(--space-4);
  position: relative;
  overflow: hidden;
}

/* ── Header Controls ── */
.hub-header-bar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: var(--space-4) var(--space-5);
  background: var(--glass-bg);
  backdrop-filter: var(--glass-blur);
  border: 1px solid var(--border-default);
  border-radius: var(--radius-md);
  flex-shrink: 0;
  gap: var(--space-4);
}

@media (max-width: 900px) {
  .hub-header-bar {
    flex-direction: column;
    align-items: stretch;
  }
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
  align-items: center;
  gap: var(--space-3);
}

@media (max-width: 600px) {
  .hub-filters {
    flex-direction: column;
    align-items: stretch;
  }
}

.hub-select {
  height: 38px;
  padding: 0 var(--space-3);
  background: var(--bg-base);
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-md);
  color: var(--text-primary);
  font-size: var(--text-sm);
  cursor: pointer;
  outline: none;
  min-width: 140px;
  transition: border-color var(--duration-fast) var(--ease);
}

.hub-select:focus {
  border-color: var(--accent-primary);
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
  height: 38px;
  padding: 0 var(--space-4);
  background: var(--bg-base);
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-md);
  color: var(--text-primary);
  font-size: var(--text-sm);
  transition: all var(--duration-fast) var(--ease);
}

.hub-search-input:focus {
  border-color: var(--accent-primary);
  box-shadow: 0 0 0 2px rgba(124, 92, 252, 0.15);
  outline: none;
}

.hub-search-btn {
  display: inline-flex;
  align-items: center;
  gap: var(--space-2);
  padding: 0 var(--space-4);
  height: 38px;
  background: var(--accent-gradient);
  color: white;
  font-size: var(--text-sm);
  font-weight: var(--font-semibold);
  border-radius: var(--radius-md);
  cursor: pointer;
  border: none;
  transition: opacity var(--duration-fast) var(--ease);
}

.hub-search-btn:hover {
  opacity: 0.9;
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
  background: rgba(248, 113, 113, 0.15);
  border: 1px solid rgba(248, 113, 113, 0.35);
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
  padding-right: 4px;
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
  background: rgba(13, 16, 28, 0.28);
  backdrop-filter: blur(3px);
  -webkit-backdrop-filter: blur(3px);
}

.hub-inline-loading-card {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: var(--space-3);
  padding: var(--space-5) var(--space-6);
  border: 1px solid rgba(255, 255, 255, 0.08);
  border-radius: var(--radius-lg);
  background: rgba(22, 24, 40, 0.88);
  box-shadow: 0 18px 48px rgba(0, 0, 0, 0.28);
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

.hub-loading-state, .hub-empty-state {
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
  border: 3px solid rgba(255, 255, 255, 0.1);
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
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(220px, 1fr));
  gap: var(--space-4);
  padding: 2px;
  margin-bottom: var(--space-4);
}

.resource-grid-card {
  display: flex;
  flex-direction: column;
  background: var(--glass-bg);
  border: 1px solid var(--border-default);
  border-radius: var(--radius-lg);
  overflow: hidden;
  height: 330px;
  cursor: pointer;
}

.card-thumb-area {
  height: 180px;
  background: #000;
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
  padding: 2px 8px;
  border-radius: var(--radius-sm);
  backdrop-filter: blur(8px);
  font-size: 10px;
  font-weight: var(--font-bold);
  letter-spacing: 0.3px;
  z-index: 2;
  text-shadow: 0 1px 2px rgba(0, 0, 0, 0.3);
}

.card-type-badge.type-scenes {
  background: rgba(244, 114, 182, 0.8);
  border: 1px solid rgba(244, 114, 182, 0.5);
  color: #fff;
}

.card-type-badge.type-looks {
  background: rgba(167, 139, 250, 0.8);
  border: 1px solid rgba(167, 139, 250, 0.5);
  color: #fff;
}

.card-type-badge.type-clothing {
  background: rgba(251, 191, 36, 0.8);
  border: 1px solid rgba(251, 191, 36, 0.5);
  color: #1a1a2e;
}

.card-type-badge.type-hair {
  background: rgba(45, 212, 191, 0.8);
  border: 1px solid rgba(45, 212, 191, 0.5);
  color: #1a1a2e;
}

.card-type-badge.type-plugins {
  background: rgba(96, 165, 250, 0.8);
  border: 1px solid rgba(96, 165, 250, 0.5);
  color: #fff;
}

.card-type-badge.type-morphs {
  background: rgba(52, 211, 153, 0.8);
  border: 1px solid rgba(52, 211, 153, 0.5);
  color: #1a1a2e;
}

.card-type-badge.type-assets {
  background: rgba(248, 113, 113, 0.8);
  border: 1px solid rgba(248, 113, 113, 0.5);
  color: #fff;
}

.card-type-badge.type-toolkits {
  background: rgba(139, 92, 246, 0.8);
  border: 1px solid rgba(139, 92, 246, 0.5);
  color: #fff;
}

.card-type-badge.type-audio {
  background: rgba(251, 146, 60, 0.8);
  border: 1px solid rgba(251, 146, 60, 0.5);
  color: #fff;
}

.card-type-badge.type-default {
  background: rgba(13, 13, 20, 0.7);
  border: 1px solid rgba(255, 255, 255, 0.08);
  color: var(--text-primary);
}

/* Pricing Badge (top-right corner) */
.card-pricing-badge {
  position: absolute;
  top: var(--space-2);
  right: var(--space-2);
  padding: 2px 8px;
  border-radius: var(--radius-sm);
  backdrop-filter: blur(8px);
  font-size: 10px;
  font-weight: var(--font-bold);
  letter-spacing: 0.3px;
  z-index: 2;
  text-shadow: 0 1px 2px rgba(0, 0, 0, 0.3);
}

.card-pricing-badge.pricing-free {
  background: rgba(52, 211, 153, 0.8);
  border: 1px solid rgba(52, 211, 153, 0.5);
  color: #fff;
}

.card-pricing-badge.pricing-paid {
  background: rgba(251, 191, 36, 0.85);
  border: 1px solid rgba(251, 191, 36, 0.5);
  color: #1a1a2e;
}

.card-info-area {
  padding: var(--space-3) var(--space-4);
  display: flex;
  flex-direction: column;
  gap: 6px;
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
  border-top: 1px solid rgba(255, 255, 255, 0.03);
}

.card-stat {
  display: flex;
  align-items: center;
  gap: 3px;
  font-size: 11px;
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
  padding: var(--space-3) var(--space-5);
  background: var(--glass-bg);
  border: 1px solid var(--border-default);
  border-radius: var(--radius-md);
  margin-top: auto;
  flex-shrink: 0;
}

.page-btn {
  display: inline-flex;
  align-items: center;
  gap: var(--space-2);
  padding: 6px var(--space-4);
  height: 32px;
  background: var(--bg-hover);
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-sm);
  color: var(--text-primary);
  font-size: var(--text-sm);
  font-weight: var(--font-medium);
  cursor: pointer;
  transition: all var(--duration-fast) var(--ease);
}

.page-btn:hover:not(:disabled) {
  background: var(--bg-surface);
  border-color: var(--accent-primary);
}

.page-btn:disabled {
  opacity: 0.4;
  cursor: not-allowed;
}

.page-indicator {
  font-size: var(--text-sm);
  color: var(--text-secondary);
  font-weight: var(--font-medium);
}

/* ── Panel Overlay ── */
.panel-overlay {
  position: absolute;
  top: 0;
  left: 0;
  right: 0;
  bottom: 0;
  background: rgba(0, 0, 0, 0.55);
  backdrop-filter: blur(4px);
  z-index: 100;
}

/* ── Slide-over Detail Panel ── */
.detail-slide-panel {
  position: absolute;
  top: 0;
  right: 0;
  width: 580px;
  max-width: 100%;
  height: 100%;
  background: var(--bg-surface);
  backdrop-filter: var(--glass-blur-heavy);
  border-left: 1px solid var(--border-default);
  box-shadow: -10px 0 40px rgba(0, 0, 0, 0.5);
  z-index: 101;
  display: flex;
  flex-direction: column;
  overflow: hidden;
}

.panel-header {
  display: flex;
  align-items: flex-start;
  gap: var(--space-3);
  padding: var(--space-5);
  border-bottom: 1px solid var(--border-default);
  flex-shrink: 0;
}

.panel-close-btn {
  width: 32px;
  height: 32px;
  display: flex;
  align-items: center;
  justify-content: center;
  border-radius: var(--radius-full);
  color: var(--text-secondary);
  background: var(--bg-hover);
  cursor: pointer;
  border: none;
  transition: all var(--duration-fast) var(--ease);
}

.panel-close-btn:hover {
  color: var(--text-primary);
  background: rgba(255, 255, 255, 0.1);
}

.panel-title-wrap {
  flex: 1;
  min-width: 0;
  display: flex;
  flex-direction: column;
  gap: 4px;
}

.panel-category-tag {
  align-self: flex-start;
  padding: 1px 6px;
  background: rgba(124, 92, 252, 0.15);
  color: var(--accent-primary);
  border: 1px solid rgba(124, 92, 252, 0.25);
  border-radius: var(--radius-sm);
  font-size: 10px;
  font-weight: var(--font-bold);
  text-transform: uppercase;
}

.panel-category-tag.pricing-free {
  background: rgba(52, 211, 153, 0.12);
  color: var(--color-success);
  border-color: rgba(52, 211, 153, 0.25);
}

.panel-category-tag.pricing-paid {
  background: rgba(251, 191, 36, 0.12);
  color: var(--color-warning);
  border-color: rgba(251, 191, 36, 0.25);
}

.panel-title {
  margin: 0;
  font-size: var(--text-lg);
  font-weight: var(--font-bold);
  color: var(--text-primary);
  line-height: 1.3;
}

.panel-body {
  flex: 1;
  overflow: hidden;
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
  height: 100%;
  overflow-y: auto;
  padding: var(--space-5);
  display: flex;
  flex-direction: column;
  gap: var(--space-5);
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

/* ── Panel Info Card ── */
.panel-info-card {
  padding: var(--space-4) var(--space-5);
  display: flex;
  flex-direction: column;
  gap: var(--space-3);
}

.panel-hero-row {
  display: flex;
  gap: var(--space-4);
}

.panel-image-container {
  width: 90px;
  height: 90px;
  border-radius: var(--radius-md);
  overflow: hidden;
  background: #000;
  display: flex;
  align-items: center;
  justify-content: center;
  flex-shrink: 0;
  border: 1px solid var(--border-subtle);
}

.panel-cover-img {
  width: 100%;
  height: 100%;
  object-fit: contain;
}

.panel-icon-img {
  width: 50px;
  height: 50px;
  object-fit: contain;
}

.panel-img-placeholder {
  color: var(--text-tertiary);
  opacity: 0.3;
}

.panel-meta-info {
  flex: 1;
  display: flex;
  flex-direction: column;
  gap: 6px;
  justify-content: center;
}

.badge-version {
  display: inline-block;
  font-size: var(--text-xs);
  font-weight: var(--font-bold);
  color: var(--accent-primary);
  background: rgba(124, 92, 252, 0.12);
  padding: 2px 8px;
  border-radius: var(--radius-full);
}

.panel-creator {
  font-weight: var(--font-medium);
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
  font-size: 10px;
  padding: 2px 6px;
  background: var(--bg-hover);
  color: var(--text-secondary);
  border-radius: var(--radius-sm);
}

.badge-stat.rating {
  color: var(--color-warning);
  background: rgba(251, 191, 36, 0.08);
}

.panel-tagline {
  border-left: 2px solid var(--border-strong);
  padding-left: var(--space-3);
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
  padding-right: 6px;
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
  background: rgba(255, 255, 255, 0.02);
  border: 1px solid rgba(255, 255, 255, 0.05);
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

.license-type, .dep-author, .dep-tier {
  position: relative;
  padding-left: var(--space-2);
}

.license-type::before, .dep-author::before, .dep-tier::before {
  content: "•";
  position: absolute;
  left: -2px;
  opacity: 0.5;
}

.dep-tier.direct {
  color: var(--color-info);
}

.dep-tier.sub {
  color: var(--color-warning);
}

.download-btn-sm {
  display: inline-flex;
  align-items: center;
  gap: var(--space-1);
  padding: 5px 12px;
  height: 28px;
  background: var(--accent-gradient);
  color: white;
  font-size: var(--text-xs);
  font-weight: var(--font-semibold);
  border-radius: var(--radius-sm);
  border: none;
  cursor: pointer;
  transition: all var(--duration-fast) var(--ease);
}

.download-btn-sm:hover:not(:disabled) {
  opacity: 0.95;
  transform: translateY(-1px);
}

.download-btn-sm:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}

.download-btn-sm.secondary {
  background: rgba(124, 92, 252, 0.1);
  color: var(--accent-primary);
  border: 1px solid rgba(124, 92, 252, 0.3);
}

.download-btn-sm.secondary:hover {
  background: rgba(124, 92, 252, 0.2);
}

.download-btn-sm.error {
  background: rgba(248, 113, 113, 0.12);
  color: var(--color-error);
  border: 1px solid rgba(248, 113, 113, 0.3);
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
  background: rgba(255, 255, 255, 0.08);
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

.download-status, .dep-status {
  display: flex;
  align-items: center;
  gap: 3px;
  font-weight: var(--font-bold);
}

.download-status.no-source {
  color: var(--color-warning);
  font-weight: var(--font-medium);
}

.badge {
  display: inline-block;
  padding: 2px 6px;
  border-radius: var(--radius-sm);
  font-size: var(--text-xs);
  font-weight: var(--font-bold);
}

.badge.success {
  background: rgba(52, 211, 153, 0.15);
  color: var(--color-success);
  border: 1px solid rgba(52, 211, 153, 0.25);
}

.badge.warning {
  background: rgba(251, 191, 36, 0.15);
  color: var(--color-warning);
  border: 1px solid rgba(251, 191, 36, 0.25);
}

.badge.upgrade {
  background: rgba(251, 146, 60, 0.15);
  color: #fb923c;
  border: 1px solid rgba(251, 146, 60, 0.3);
}

.download-btn-sm.upgrade {
  background: rgba(251, 146, 60, 0.15);
  color: #fb923c;
  border: 1px solid rgba(251, 146, 60, 0.35);
}

.download-btn-sm.upgrade:hover {
  background: rgba(251, 146, 60, 0.25);
}

.empty-list-info {
  width: 100%;
  padding: var(--space-4);
  text-align: center;
  background: rgba(255, 255, 255, 0.01);
  border: 1px dashed rgba(255, 255, 255, 0.05);
  border-radius: var(--radius-md);
  font-style: italic;
}

/* Animations */
.slide-panel-enter-active, .slide-panel-leave-active {
  transition: transform 0.3s cubic-bezier(0.16, 1, 0.3, 1);
}
.slide-panel-enter-from, .slide-panel-leave-to {
  transform: translateX(100%);
}

.fade-enter-active, .fade-leave-active {
  transition: opacity 0.2s ease;
}
.fade-enter-from, .fade-leave-to {
  opacity: 0;
}

/* ── Batch Download Button & Row ── */
.panel-actions-row {
  margin-top: var(--space-4);
  display: flex;
  justify-content: stretch;
}

.batch-download-btn {
  flex: 1;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  gap: var(--space-2);
  height: 40px;
  background: var(--accent-gradient);
  color: white;
  font-size: var(--text-sm);
  font-weight: var(--font-bold);
  border-radius: var(--radius-md);
  border: none;
  cursor: pointer;
  box-shadow: 0 4px 15px rgba(124, 92, 252, 0.25);
  transition: all var(--transition-normal);
}

.batch-download-btn:hover:not(:disabled) {
  opacity: 0.95;
  transform: translateY(-1px);
  box-shadow: 0 6px 20px rgba(124, 92, 252, 0.35);
}

.batch-download-btn:disabled {
  opacity: 0.5;
  cursor: not-allowed;
  box-shadow: none;
}

/* ── Batch Download Modal Backdrop & Container ── */
.batch-modal-backdrop {
  position: absolute;
  top: 0;
  left: 0;
  right: 0;
  bottom: 0;
  background: rgba(13, 13, 20, 0.65);
  backdrop-filter: blur(12px);
  -webkit-backdrop-filter: blur(12px);
  z-index: 200;
  display: flex;
  align-items: center;
  justify-content: center;
  padding: var(--space-6);
}

.batch-modal-container {
  width: 500px;
  max-width: 100%;
  max-height: 90%;
  background: var(--bg-surface);
  backdrop-filter: var(--glass-blur-heavy);
  -webkit-backdrop-filter: var(--glass-blur-heavy);
  border: 1px solid var(--border-default);
  border-radius: var(--radius-lg);
  box-shadow: 0 20px 50px rgba(0, 0, 0, 0.5);
  display: flex;
  flex-direction: column;
  overflow: hidden;
  animation: modal-enter 0.3s cubic-bezier(0.34, 1.56, 0.64, 1);
}

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

.batch-modal-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: var(--space-4) var(--space-5);
  border-bottom: 1px solid var(--border-subtle);
  flex-shrink: 0;
}

.batch-modal-title {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  margin: 0;
  font-size: var(--text-md);
  font-weight: var(--font-bold);
  color: var(--text-primary);
}

.batch-modal-close-btn {
  width: 28px;
  height: 28px;
  display: flex;
  align-items: center;
  justify-content: center;
  border-radius: var(--radius-full);
  color: var(--text-secondary);
  background: transparent;
  cursor: pointer;
  border: none;
  transition: all var(--transition-fast);
}

.batch-modal-close-btn:hover {
  color: var(--text-primary);
  background: var(--bg-hover);
}

.batch-modal-body {
  flex: 1;
  overflow-y: auto;
  padding: var(--space-5);
}

.batch-modal-loading {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  padding: var(--space-6) 0;
  gap: var(--space-4);
}

.batch-modal-content {
  display: flex;
  flex-direction: column;
  gap: var(--space-4);
}

.batch-modal-summary {
  margin: 0;
  line-height: 1.5;
}

.groups-wrapper {
  display: flex;
  flex-direction: column;
  gap: var(--space-4);
}

.group-section {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
}

.group-header {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  cursor: pointer;
  user-select: none;
  padding: var(--space-1) 0;
}

.group-checkbox {
  width: 14px;
  height: 14px;
  cursor: pointer;
  accent-color: var(--accent-primary);
}

.group-dot {
  width: 8px;
  height: 8px;
  border-radius: var(--radius-full);
}

.group-dot.to-download {
  background: var(--color-info);
  box-shadow: 0 0 8px var(--color-info);
}

.group-dot.upgraded {
  background: var(--color-success);
  box-shadow: 0 0 8px var(--color-success);
}

.group-dot.queued {
  background: var(--type-appearance);
}

.group-dot.installed {
  background: var(--text-tertiary);
}

.group-dot.no-source {
  background: var(--color-error);
}

.group-title {
  color: var(--text-primary);
}

.group-items {
  display: flex;
  flex-direction: column;
  gap: 2px;
  padding-left: 22px;
}

.checklist-item-row {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  padding: var(--space-2) var(--space-3);
  background: rgba(255, 255, 255, 0.02);
  border: 1px solid rgba(255, 255, 255, 0.04);
  border-radius: var(--radius-sm);
  cursor: pointer;
  transition: all var(--transition-fast);
}

.checklist-item-row:hover:not(.text-disabled) {
  background: rgba(255, 255, 255, 0.04);
  border-color: rgba(124, 92, 252, 0.15);
}

.checklist-item-row.text-disabled {
  opacity: 0.6;
  cursor: default;
}

.item-checkbox {
  width: 12px;
  height: 12px;
  cursor: pointer;
  accent-color: var(--accent-primary);
}

.checklist-item-row.text-disabled .item-checkbox {
  cursor: not-allowed;
}

.item-filename {
  flex: 1;
  color: var(--text-primary);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.checklist-item-row.text-disabled .item-filename {
  color: var(--text-secondary);
}

.item-size {
  font-variant-numeric: tabular-nums;
  flex-shrink: 0;
}

.item-badge {
  font-size: 10px;
  padding: 1px 6px;
  background: rgba(248, 113, 113, 0.12);
  color: var(--color-error);
  border: 1px solid rgba(248, 113, 113, 0.25);
  border-radius: var(--radius-sm);
  font-weight: var(--font-bold);
}

.manual-mark-btn {
  flex-shrink: 0;
  height: 24px;
  padding: 0 var(--space-2);
  border: 1px solid rgba(251, 191, 36, 0.25);
  border-radius: var(--radius-sm);
  background: rgba(251, 191, 36, 0.1);
  color: var(--color-warning);
  font-size: 10px;
  font-weight: var(--font-bold);
}

.manual-mark-btn:hover {
  background: rgba(251, 191, 36, 0.18);
}

/* ── Batch Download Modal Footer ── */
.batch-modal-footer {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: var(--space-4) var(--space-5);
  border-top: 1px solid var(--border-subtle);
  background: rgba(13, 13, 20, 0.25);
  flex-shrink: 0;
}

.footer-stats {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  color: var(--text-secondary);
}

.footer-stats .divider {
  color: var(--text-tertiary);
  opacity: 0.5;
}

.footer-actions {
  display: flex;
  gap: var(--space-2);
}

.action-btn-cancel,
.action-btn-confirm {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  padding: 0 var(--space-4);
  height: 32px;
  font-size: var(--text-sm);
  font-weight: var(--font-medium);
  border-radius: var(--radius-md);
  cursor: pointer;
  transition: all var(--transition-fast);
}

.action-btn-cancel {
  color: var(--text-primary);
  background: var(--bg-hover);
  border: 1px solid var(--border-subtle);
}

.action-btn-cancel:hover {
  background: var(--bg-surface);
  border-color: var(--border-default);
}

.action-btn-confirm {
  background: var(--accent-gradient);
  color: white;
  border: none;
  font-weight: var(--font-semibold);
  box-shadow: 0 2px 8px rgba(124, 92, 252, 0.2);
}

.action-btn-confirm:hover:not(:disabled) {
  opacity: 0.95;
  box-shadow: 0 4px 12px rgba(124, 92, 252, 0.3);
}

.action-btn-confirm:disabled {
  opacity: 0.5;
  cursor: not-allowed;
  box-shadow: none;
}

/* ── Expand/Collapse Chevron and Loaders ── */
.expand-arrow {
  margin-left: auto;
  color: var(--text-secondary);
  display: flex;
  align-items: center;
  transition: transform var(--transition-normal);
}

.expand-arrow.collapsed {
  transform: rotate(-90deg);
}

.desc-loading {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  padding: var(--space-2) 0;
  color: var(--text-secondary);
}

.loading-spinner-sm {
  width: 14px;
  height: 14px;
  border: 2px solid rgba(255, 255, 255, 0.1);
  border-top-color: var(--accent-primary);
  border-radius: var(--radius-full);
  animation: rotate 0.8s linear infinite;
}

/* Resolve Dependency Progress */
.resolve-progress-container {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: var(--space-2);
  width: 80%;
  max-width: 320px;
  margin-top: var(--space-2);
}

.resolve-progress-container .progress-bar-wrapper {
  width: 100%;
  height: 6px;
  background: rgba(255, 255, 255, 0.08);
  border-radius: var(--radius-full);
  overflow: hidden;
  box-shadow: inset 0 1px 2px rgba(0, 0, 0, 0.2);
}

.resolve-progress-container .progress-bar-fill {
  height: 100%;
  background: var(--accent-gradient);
  border-radius: var(--radius-full);
  transition: width 0.3s ease-out;
}

.progress-info {
  display: flex;
  gap: var(--space-2);
  font-variant-numeric: tabular-nums;
}

.resolve-current-package {
  max-width: 100%;
  font-style: italic;
  opacity: 0.85;
}
</style>
