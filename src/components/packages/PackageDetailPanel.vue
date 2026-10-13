<script setup lang="ts">
import { computed, ref } from 'vue'
import { useI18n } from 'vue-i18n'
import type { PackageImageEntry, ResourceDetailItem } from '@/types/package'
import { formatSize } from '@/utils/bytes'
import { resourceColor, resourceThumbnail, resourceVersion } from '@/utils/resourcePresentation'
import Icon from '@/components/ui/Icon.vue'
import { focusInitial } from '@/components/ui/overlay'
import UiButton from '@/components/ui/UiButton.vue'
import PackageDependencyList, { type DetailDependency } from './PackageDependencyList.vue'
withDefaults(defineProps<{
  item: ResourceDetailItem
  subtitle?: string
  image?: string | null
  images?: PackageImageEntry[]
  imageUrls?: Record<string, string>
  activeImage?: string
  expanded?: boolean
  busy?: boolean
  canGoBack?: boolean
  dependencyThumbnails?: Record<string, string | null>
  openingDependency?: string
  dependencies?: DetailDependency[]
  missing?: number
  loading?: boolean
  error?: string
}>(), { images: () => [], imageUrls: () => ({}), dependencies: () => [], missing: 0 })
const emit = defineEmits<{
  close: []
  image: [image: PackageImageEntry]
  details: []
  complete: []
  copy: []
  copyDependency: [id: string]
  dependencyDetails: [dependency: DetailDependency]
}>()
const titleCopy = ref<InstanceType<typeof UiButton> | null>(null)
defineExpose({ focusTitle: () => { if (titleCopy.value?.element) focusInitial(titleCopy.value.element) } })
const { t } = useI18n()
const date = computed(() => (value: string) => (value ? new Date(value).toLocaleDateString() : '—'))
</script>
<template>
  <aside class="detail-panel" :class="{ expanded, 'has-back': canGoBack }">
    <div
      v-if="!expanded"
      class="detail-preview"
      :style="{ background: resourceThumbnail(item.resource_types[0]) }"
    >
      <img v-if="image" data-resource-preview :src="image" :alt="item.name" />
      <Icon v-else name="image" :size="34" class="detail-placeholder" />
      <span class="detail-type">
        <i :style="{ background: resourceColor(item.resource_types[0]) }" />
        {{ t('resourceType.' + (item.resource_types[0] || 'other')) }}
      </span>
      <div v-if="images.length" class="detail-thumbnails">
        <button
          v-for="imageItem in images.slice(0, 5)"
          :key="imageItem.path"
          type="button"
          :class="{ active: activeImage === imageItem.path }"
          :aria-label="imageItem.path"
          :title="imageItem.path"
          @click="emit('image', imageItem)"
        >
          <img
            v-if="imageUrls[imageItem.path]"
            data-resource-preview
            :src="imageUrls[imageItem.path]"
            :alt="imageItem.path"
          />
          <Icon v-else name="image" :size="16" />
        </button>
        <UiButton
          v-if="images.length > 5"
          size="sm"
          class="more-images"
          :aria-label="t('ui.openDetails')"
          @click="emit('details')"
        >
          +{{ images.length - 5 }}
        </UiButton>
      </div>
    </div>
    <div class="detail-body">
      <span v-if="expanded" class="expanded-type">
        <i :style="{ background: resourceColor(item.resource_types[0]) }" />
        {{ t('resourceType.' + (item.resource_types[0] || 'other')) }}
      </span>
      <div>
        <div class="detail-title">
          <h2>{{ item.name }}</h2>
          <UiButton
            ref="titleCopy"
            class="copy-package-name"
            variant="ghost"
            size="sm"
            icon="copy"
            icon-only
            :aria-label="t('ui.copyResourceName', { name: item.name })"
            @click="emit('copy')"
          />
        </div>
        <p class="detail-subtitle">
          <template v-if="subtitle !== undefined">{{ subtitle }}</template>
          <template v-else>
            {{ item.creator }} · {{ resourceVersion(item) }} · {{ formatSize(item.size_bytes ?? 0) }} ·
            {{ date(item.scan_time || '') }}
          </template>
        </p>
      </div>
      <div v-if="item.tags?.length" class="detail-tags">
        <span v-for="tag in item.tags" :key="tag">{{ tag }}</span>
      </div>
      <slot name="actions" />
      <slot name="tabs" />
      <slot>
        <section class="detail-dependencies">
          <div class="dependency-heading">
            <h3>{{ t('ui.dependencies') }} {{ item.dependency_count }}</h3>
            <span v-if="missing">{{ t('ui.missing', { count: missing }) }}</span>
          </div>
          <p v-if="loading" class="detail-muted" role="status">{{ t('common.loading') }}</p>
          <p v-else-if="error" class="detail-error" role="alert">{{ error }}</p>
          <PackageDependencyList
            v-else-if="dependencies.length"
            :dependencies="dependencies"
            :thumbnails="dependencyThumbnails"
            :busy="busy"
            :opening="openingDependency"
            @copy="emit('copyDependency', $event)"
            @details="emit('dependencyDetails', $event)"
            @complete="emit('complete')"
          />
          <p v-else class="detail-muted">{{ t('ui.noData') }}</p>
        </section>
      </slot>
    </div>
  </aside>
</template>
<style scoped>
.detail-panel {
  width: 100%;
  min-width: 0;
  background: var(--bg-elevated);
}
.detail-panel.expanded {
  width: 100%;
  max-height: none;
}
.detail-preview {
  position: relative;
  aspect-ratio: 4 / 3;
  display: grid;
  place-items: center;
  overflow: hidden;
}
.detail-preview > img {
  position: absolute;
  inset: 0;
  width: 100%;
  height: 100%;
  object-fit: contain;
  background: var(--bg-base);
}
.detail-placeholder {
  color: var(--text-image-placeholder);
}
.detail-type {
  position: absolute;
  left: 12px;
  top: 12px;
  display: flex;
  align-items: center;
  gap: var(--space-2);
  height: 22px;
  padding: 0 var(--space-2);
  background: var(--bg-on-image);
  border-radius: var(--radius-full);
  font-size: var(--text-xs);
  font-weight: var(--font-bold);
}
.detail-type i {
  width: 6px;
  height: 6px;
  border-radius: var(--radius-full);
}
.detail-thumbnails {
  position: absolute;
  bottom: 10px;
  left: 12px;
  right: 12px;
  display: flex;
  gap: var(--space-2);
}
.detail-thumbnails > button:not(.more-images) {
  width: 36px;
  height: 36px;
  border-radius: var(--radius-thumbnail);
  background: var(--bg-on-image);
  display: grid;
  place-items: center;
  overflow: hidden;
}
.detail-thumbnails button.active {
  box-shadow: 0 0 0 2px var(--text-primary);
}
.detail-thumbnails img {
  width: 100%;
  height: 100%;
  object-fit: cover;
}
.more-images {
  margin-left: auto;
}
.detail-body {
  display: flex;
  flex-direction: column;
  gap: var(--space-section);
  padding: var(--space-panel);
}
.expanded-type {
  display: inline-flex;
  align-items: center;
  align-self: flex-start;
  gap: var(--space-2);
  min-height: 22px;
  padding: 0 var(--space-2);
  border-radius: var(--radius-full);
  background: var(--bg-input);
  font-size: var(--text-xs);
  font-weight: var(--font-bold);
}
.expanded-type i {
  width: 6px;
  height: 6px;
  border-radius: var(--radius-full);
}
.detail-title {
  display: flex;
  align-items: center;
  gap: var(--space-item);
}
.detail-title h2 { flex: 1; min-width: 0; }
.has-back .detail-type { top: calc(var(--space-stack) + var(--control-h-sm) + var(--space-item)); }
.detail-body h2 {
  font-size: var(--text-xl);
  font-weight: var(--font-heavy);
  overflow-wrap: anywhere;
}
.expanded .detail-body h2 {
  font-size: var(--text-3xl);
}
.detail-subtitle {
  margin-top: var(--space-2);
  color: var(--text-tertiary);
  font-size: var(--text-base);
  overflow-wrap: anywhere;
}
.detail-tags {
  display: flex;
  flex-wrap: wrap;
  gap: var(--space-2);
}
.detail-tags span {
  display: inline-flex;
  align-items: center;
  min-height: 26px;
  padding: 0 var(--space-3);
  border-radius: var(--radius-full);
  background: var(--divider);
  color: var(--text-secondary);
  font-size: var(--text-xs);
}
.dependency-heading {
  display: flex;
  justify-content: space-between;
  align-items: center;
  gap: var(--space-item);
  margin-bottom: var(--space-stack);
}
.dependency-heading h3 {
  font-size: var(--text-lg);
}
.dependency-heading > span {
  color: var(--color-warning);
  font-size: var(--text-xs);
}
.detail-muted {
  color: var(--text-tertiary);
  font-size: var(--text-sm);
}
.detail-error {
  color: var(--color-error);
  font-size: var(--text-sm);
}
</style>
