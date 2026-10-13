<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'
import Icon from '@/components/ui/Icon.vue'
import UiButton from '@/components/ui/UiButton.vue'
import { formatSize } from '@/utils/bytes'
import { resourceColor, resourceThumbnail, resourceVersion } from '@/utils/resourcePresentation'
export interface CardResource {
  id: string
  name: string
  creator?: string
  version?: number
  size_bytes?: number
  resource_types?: string[]
}
const props = defineProps<{
  item: CardResource
  image?: string | null
  selected?: boolean
  multiSelect?: boolean
  checked?: boolean
  disabled?: boolean
  selectionDisabled?: boolean
  favoriteDisabled?: boolean
  typeLabel?: string
  missing?: number
  favorite?: boolean
  favoriteTitle?: string
  canFavorite?: boolean
  canLaunch?: boolean
  loading?: boolean
  landscape?: boolean
  fresh?: boolean
}>()
const emit = defineEmits<{
  select: []
  details: []
  launch: []
  favorite: []
  toggle: [event: MouseEvent]
  more: [event: MouseEvent]
  imageError: []
}>()
const { t } = useI18n()
const type = computed(() => props.item.resource_types?.[0] || 'other')
</script>
<template>
  <article
    class="package-card"
    :class="{
      selected: selected || checked,
      landscape,
      loading,
      'large-card': !landscape,
      'small-card': landscape,
    }"
    :style="{ '--thumb': resourceThumbnail(type), '--type-color': resourceColor(type) }"
    @contextmenu.prevent="emit('more', $event)"
  >
    <div class="package-preview large-card-thumb small-card-thumb">
      <img
        v-if="image && !loading"
        data-resource-preview
        :src="image"
        :alt="item.name"
        loading="lazy"
        @error="emit('imageError')"
      />
      <Icon v-else-if="!loading" name="image" :size="34" class="image-placeholder" />
    </div>
    <div class="package-info">
      <template v-if="loading">
        <span class="skeleton" />
        <span class="skeleton short" />
      </template>
      <template v-else>
        <strong :title="item.name">{{ item.name }}</strong>
        <div class="package-meta">
          <slot name="meta">
            <span>
              {{ item.creator }}
              <template v-if="item.version !== undefined">· {{ resourceVersion(item) }}</template>
            </span>
            <span v-if="item.size_bytes !== undefined">{{ formatSize(item.size_bytes) }}</span>
          </slot>
        </div>
      </template>
    </div>
    <button
      type="button"
      class="card-hit-target"
      :aria-label="item.name"
      :aria-pressed="selected || checked"
      :disabled="disabled || loading"
      @click="emit('select')"
      @dblclick="emit('details')"
      @keydown.space.prevent="emit('details')"
      @keydown.enter.prevent="canLaunch ? emit('launch') : emit('select')"
    >
      <span class="sr-only">{{ item.name }}</span>
    </button>
    <span v-if="!loading" class="package-type">
      <span />
      {{ typeLabel || t('resourceType.' + type) }}
    </span>
    <label v-if="multiSelect" class="package-check cleanup-select" :class="{ checked }" @click.stop>
      <input
        type="checkbox"
        :checked="checked"
        :aria-label="item.id"
        :disabled="disabled || selectionDisabled"
        @click.stop="emit('toggle', $event)"
      />
    </label>
    <div v-else class="package-badges">
      <span v-if="fresh" class="package-new">{{ t('ui.new') }}</span>
      <span v-if="missing" class="package-missing">{{ t('ui.missing', { count: missing }) }}</span>
      <span v-if="favorite" class="package-favorite" :title="favoriteTitle || t('ui.favorite')" :aria-label="favoriteTitle || t('ui.favorite')"><Icon name="star" :size="14" /></span>
    </div>
    <slot name="badge" />
    <div v-if="!loading && !multiSelect" class="package-hover">
      <UiButton
        v-if="canLaunch"
        variant="primary"
        icon="play"
        icon-only
        size="lg"
        class="package-play"
        :aria-label="t('gameContent.launchScene')"
        :disabled="disabled"
        @click.stop="emit('launch')"
      />
      <UiButton
        v-if="canFavorite"
        icon="star"
        icon-only
        class="image-action"
        :aria-label="t('ui.favorite')"
        :disabled="disabled || favoriteDisabled"
        @click.stop="emit('favorite')"
      />
      <UiButton
        icon="more"
        icon-only
        class="image-action"
        :aria-label="t('ui.more')"
        :disabled="disabled"
        @click.stop="emit('more', $event)"
      />
    </div>
  </article>
</template>
<style scoped>
.package-card {
  position: relative;
  display: flex;
  flex-direction: column;
  min-width: 0;
  aspect-ratio: var(--resource-card-ratio);
  overflow: hidden;
  border-radius: var(--radius-card);
  background: var(--bg-elevated);
  transition:
    background var(--duration-fast),
    box-shadow var(--duration-fast);
}
/* 悬停保持原位，避免把选中或焦点环移出滚动区。 */
.package-card:not(.selected):not(.loading):hover {
  background: var(--bg-hover);
}
.package-card.selected {
  box-shadow: var(--ring-selected);
}
.package-card:has(.card-hit-target:focus-visible) {
  box-shadow: var(--focus-ring);
}
.package-preview {
  position: relative;
  flex: 1;
  min-width: 0;
  min-height: 0;
  display: grid;
  place-items: center;
  overflow: hidden;
  background: var(--thumb);
}
.package-preview:has(img) {
  background: var(--bg-base);
}
.package-preview img {
  /* 脱离网格内在尺寸，避免方图将已缩小的预览区撑高。 */
  position: absolute;
  inset: 0;
  width: 100%;
  height: 100%;
  object-fit: contain;
  object-position: center;
}
.image-placeholder {
  color: var(--text-image-placeholder);
}
.package-info {
  height: var(--resource-card-info-h);
  padding: var(--space-2) var(--space-3);
  flex-shrink: 0;
  background: inherit;
}
.package-info strong {
  display: block;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  font-size: var(--text-base);
  font-weight: var(--font-bold);
  color: var(--text-primary);
}
.package-meta {
  display: flex;
  justify-content: space-between;
  gap: var(--space-2);
  font-size: var(--text-caption);
  color: var(--text-tertiary);
  margin-top: 2px;
}
.package-meta span:first-child {
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.package-meta span:last-child {
  flex-shrink: 0;
}
.card-hit-target {
  position: absolute;
  inset: 0;
  z-index: 1;
  width: 100%;
  height: 100%;
  border-radius: inherit;
}
.package-type,
.package-missing,
.package-favorite,
.package-new {
  display: inline-flex;
  align-items: center;
  gap: var(--space-1);
  min-height: 22px;
  padding: 0 var(--space-2);
  border-radius: var(--radius-full);
  font-size: var(--text-xs);
  font-weight: var(--font-bold);
  pointer-events: none;
}
.package-badges {
  position: absolute;
  top: var(--space-2);
  right: var(--space-2);
  z-index: 3;
  display: flex;
  align-items: center;
  gap: var(--space-1);
}
.package-type {
  position: absolute;
  top: var(--space-2);
  z-index: 2;
  left: var(--space-2);
  background: var(--bg-on-image);
  color: var(--text-primary);
}
.package-type span {
  width: 6px;
  height: 6px;
  border-radius: var(--radius-full);
  background: var(--type-color);
}
.package-missing {
  background: var(--color-warning);
  color: var(--color-warning-ink);
}
.package-favorite {
  pointer-events: auto;
  background: var(--bg-on-image);
  color: var(--accent-primary);
  padding: var(--space-1);
}
.package-favorite :deep(path) {
  fill: currentColor;
}
.package-new {
  background: var(--accent-primary);
  color: var(--accent-ink);
}
.package-check {
  position: absolute;
  z-index: 3;
  top: var(--space-2);
  right: var(--space-2);
  display: grid;
  place-items: center;
  width: 22px;
  height: 22px;
  border-radius: var(--radius-full);
  background: var(--bg-on-image);
  box-shadow: inset 0 0 0 2px var(--text-primary);
}
.package-check input {
  width: 22px;
  height: 22px;
  border-radius: var(--radius-full);
  border: 2px solid var(--text-primary);
  background: var(--bg-on-image);
  margin: 0;
  animation: none;
}
.package-check input:checked {
  background: var(--accent-primary);
  border-color: var(--accent-primary);
}
.package-check input::after {
  left: 6px;
  top: 3px;
}
.package-check.checked {
  background: var(--accent-primary);
  color: var(--accent-ink);
  box-shadow: none;
}
.package-hover {
  position: absolute;
  inset: 0 0 var(--resource-card-info-h);
  z-index: 2;
  display: flex;
  justify-content: center;
  align-items: center;
  gap: var(--space-2);
  background: var(--bg-image-hover);
  opacity: 0;
  visibility: hidden;
  pointer-events: none;
  transition: opacity var(--duration-fast);
}
.package-card:hover .package-hover,
.package-card:focus-within .package-hover {
  opacity: 1;
  visibility: visible;
}
.package-hover :deep(button) {
  pointer-events: auto;
}
.package-hover .package-play {
  width: 48px;
  height: 48px;
}
.image-action {
  background: var(--text-primary);
  color: var(--text-inverse);
}
/* 白色圆按钮悬停时稍微压暗，而不是被 UiButton 的灰色悬停底整个替换掉 */
.image-action:hover:not(:disabled) {
  background: var(--text-muted-strong);
}
.skeleton {
  display: block;
  height: 10px;
  margin-bottom: var(--space-2);
  width: 80%;
  border-radius: var(--radius-full);
  background: var(--bg-active);
}
.skeleton.short {
  width: 50%;
}
.loading .package-preview {
  background: var(--bg-elevated);
}
</style>
