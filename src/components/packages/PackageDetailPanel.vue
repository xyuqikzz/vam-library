<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'
import type { PackageDisplayItem, PackageImageEntry } from '@/types/package'
import { formatSize } from '@/utils/bytes'
import { resourceColor, resourceThumbnail } from '@/utils/resourcePresentation'
import Icon from '@/components/ui/Icon.vue'
import UiButton from '@/components/ui/UiButton.vue'
export interface DetailDependency {
  id: string
  status: string
  statusText: string
  tier?: number
}
defineProps<{
  item: PackageDisplayItem
  image?: string | null
  images: PackageImageEntry[]
  imageUrls: Record<string, string>
  activeImage?: string
  expanded?: boolean
  busy?: boolean
  dependencies: DetailDependency[]
  missing: number
  loading?: boolean
  error?: string
}>()
const emit = defineEmits<{
  close: []
  image: [image: PackageImageEntry]
  details: []
  complete: []
}>()
const { t } = useI18n()
const date = computed(() => (value: string) => (value ? new Date(value).toLocaleDateString() : '—'))
</script>
<template>
  <aside class="detail-panel" :class="{ expanded }">
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
      <UiButton
        class="detail-close"
        size="sm"
        icon="close"
        icon-only
        :aria-label="t('ui.closeDetail')"
        :disabled="busy"
        @click="emit('close')"
      />
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
        <h2>{{ item.name }}</h2>
        <p class="detail-subtitle">
          {{ item.creator }} · v{{ item.version }} · {{ formatSize(item.size_bytes) }} ·
          {{ date(item.scan_time) }}
        </p>
      </div>
      <div v-if="item.tags.length" class="detail-tags">
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
          <div v-else-if="dependencies.length" class="dependency-list">
            <div
              v-for="dependency in dependencies"
              :key="dependency.id"
              class="dependency-row"
              :class="{
                missing: dependency.status === 'missing' || dependency.status === 'lower-version',
              }"
            >
              <span class="dependency-thumb"><Icon name="package" :size="16" /></span>
              <div class="dependency-name">
                <strong :title="dependency.id">{{ dependency.id }}</strong>
                <span>{{ dependency.id.split('.')[0] }}</span>
              </div>
              <UiButton
                v-if="dependency.status === 'missing' || dependency.status === 'lower-version'"
                size="sm"
                class="resolve-button"
                @click="emit('complete')"
              >
                {{ t('ui.complete') }}
              </UiButton>
              <span v-else class="dependency-owned">{{ t('ui.installed') }}</span>
            </div>
          </div>
          <p v-else class="detail-muted">{{ t('ui.noData') }}</p>
        </section>
      </slot>
    </div>
  </aside>
</template>
<style scoped>
.detail-panel {
  width: 320px;
  flex: 0 0 320px;
  align-self: flex-start;
  max-height: 100%;
  overflow: auto;
  border-radius: var(--radius-shell);
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
  width: 100%;
  height: 100%;
  object-fit: cover;
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
  gap: 6px;
  height: 22px;
  padding: 0 8px;
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
.detail-close {
  position: absolute;
  top: 10px;
  right: 10px;
  background: var(--bg-on-image);
}
.detail-thumbnails {
  position: absolute;
  bottom: 10px;
  left: 12px;
  right: 12px;
  display: flex;
  gap: 6px;
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
  gap: 16px;
  padding: 16px 18px 18px;
}
.expanded-type {
  display: inline-flex;
  align-items: center;
  align-self: flex-start;
  gap: 6px;
  min-height: 22px;
  padding: 0 8px;
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
.detail-body h2 {
  font-size: var(--text-xl);
  font-weight: var(--font-heavy);
  overflow-wrap: anywhere;
}
.expanded .detail-body h2 {
  font-size: var(--text-3xl);
}
.detail-subtitle {
  margin-top: 6px;
  color: var(--text-tertiary);
  font-size: var(--text-base);
  overflow-wrap: anywhere;
}
.detail-tags {
  display: flex;
  flex-wrap: wrap;
  gap: 6px;
}
.detail-tags span {
  display: inline-flex;
  align-items: center;
  min-height: 26px;
  padding: 0 10px;
  border-radius: var(--radius-full);
  background: var(--divider);
  color: var(--text-secondary);
  font-size: var(--text-xs);
}
.dependency-heading {
  display: flex;
  justify-content: space-between;
  align-items: center;
  gap: var(--space-2);
  margin-bottom: var(--space-2);
}
.dependency-heading h3 {
  font-size: var(--text-lg);
}
.dependency-heading > span {
  color: var(--color-warning);
  font-size: var(--text-xs);
}
.dependency-list {
  display: flex;
  flex-direction: column;
  gap: 4px;
}
.dependency-row {
  display: flex;
  align-items: center;
  gap: 8px;
  min-height: 44px;
  padding: 6px;
  border-radius: var(--radius-icon);
}
.dependency-row.missing {
  background: var(--color-warning-subtle);
}
.dependency-thumb {
  display: grid;
  place-items: center;
  width: 32px;
  height: 32px;
  flex-shrink: 0;
  border-radius: var(--radius-sm);
  background: var(--bg-control);
  color: var(--text-tertiary);
}
.dependency-name {
  display: flex;
  flex-direction: column;
  flex: 1;
  min-width: 0;
}
.dependency-name strong {
  font-size: var(--text-tooltip);
  font-weight: var(--font-bold);
  white-space: nowrap;
  text-overflow: ellipsis;
  overflow: hidden;
}
.dependency-name span {
  color: var(--text-tertiary);
  font-size: var(--text-xs);
}
.dependency-owned {
  color: var(--color-success);
  font-size: var(--text-xs);
  white-space: nowrap;
}
.resolve-button {
  background: var(--color-warning);
  color: var(--color-warning-ink);
}
.detail-muted {
  color: var(--text-tertiary);
  font-size: var(--text-sm);
}
.detail-error {
  color: var(--color-error);
  font-size: var(--text-sm);
}
@media (max-width: 1000px) {
  .detail-panel:not(.expanded) {
    width: 280px;
    flex-basis: 280px;
  }
}
@media (max-width: 760px) {
  .detail-panel:not(.expanded) {
    width: 100%;
    flex-basis: auto;
    max-height: 55vh;
  }
}
</style>
