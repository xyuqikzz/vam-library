<script setup lang="ts">
/**
 * 统一的资源详情弹窗：左边大图（缩略图条 + 全屏查看），右边是调用方给的详情内容。
 * 弹窗高度固定，左右两栏各自独立，右栏内容再多也只在右栏里滚动。
 */
import { computed, nextTick, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import UiModal from './UiModal.vue'
import UiButton from './UiButton.vue'
import Icon from './Icon.vue'
import { focusInitial } from './overlay'
import { useImageZoom } from './imageZoom'

export interface DetailImage {
  /** 唯一标识（通常是包内路径） */
  key: string
  /** 显示在图片说明和全屏顶栏里的文字 */
  label: string
  /** 还没加载出来时为空，显示占位 */
  url?: string | null
}
const props = withDefaults(defineProps<{
  label: string
  images?: DetailImage[]
  /** 没有图片列表或当前图还没加载时显示的图 */
  fallbackImage?: string | null
  /** 换成别的资源时重置到第一张 */
  contentKey?: string | number
  busy?: boolean
  closeLabel?: string
}>(), { images: () => [] })
const emit = defineEmits<{ close: []; select: [index: number] }>()
const { t } = useI18n()
const fullscreenButton = ref<InstanceType<typeof UiButton> | null>(null)
const lightboxClose = ref<InstanceType<typeof UiButton> | null>(null)
const root = ref<HTMLElement | null>(null)
const lightboxStage = ref<HTMLElement | null>(null)
const index = ref(0),
  fullscreen = ref(false)
const zoom = useImageZoom(lightboxStage)
const current = computed<DetailImage | undefined>(() => props.images[index.value])
const url = computed(() => current.value?.url || props.fallbackImage || null)
const caption = computed(() => current.value?.label || props.label)
// 换图后先隐藏，等新图加载完再按「适应屏幕」显示
watch(url, () => { if (fullscreen.value) zoom.reset() })
function revealActiveThumbnail() {
  void nextTick(() => {
    root.value?.querySelectorAll<HTMLElement>('.thumbnail-strip .active').forEach((el) => {
      el.scrollIntoView?.({ block: 'nearest', inline: 'nearest' })
    })
  })
}
function select(next: number) {
  if (!props.images.length) return
  index.value = (next + props.images.length) % props.images.length
  emit('select', index.value)
  revealActiveThumbnail()
}
function openFullscreen() {
  if (!url.value) return
  fullscreen.value = true
  zoom.reset()
  revealActiveThumbnail()
  void nextTick(() => {
    if (lightboxClose.value?.element) focusInitial(lightboxClose.value.element)
  })
}
function closeFullscreen() {
  if (!fullscreen.value) return
  fullscreen.value = false
  void nextTick(() => {
    if (fullscreenButton.value?.element) focusInitial(fullscreenButton.value.element)
  })
}
watch(
  () => props.contentKey,
  () => {
    index.value = 0
    fullscreen.value = false
    if (props.images.length) emit('select', 0)
  },
  { immediate: true },
)
// 图片列表异步到达时，告诉调用方去加载第一张
watch(
  () => props.images.length,
  (length, previous) => {
    if (index.value >= length) index.value = 0
    if (length && !previous) emit('select', index.value)
  },
)
function keys(event: KeyboardEvent) {
  if ((event.target as HTMLElement).matches('input, textarea, select, [contenteditable="true"]'))
    return
  // 全屏时 Esc 只退出全屏，不能冒泡到 UiModal 把整个详情关掉
  if (fullscreen.value && event.key === 'Escape') {
    event.preventDefault()
    event.stopPropagation()
    closeFullscreen()
    return
  }
  if (fullscreen.value && !event.ctrlKey && !event.metaKey && !event.altKey) {
    const action = ({ '+': zoom.zoomIn, '=': zoom.zoomIn, '-': zoom.zoomOut, '_': zoom.zoomOut, '0': zoom.fit, '1': zoom.actualSize } as Record<string, () => void>)[event.key]
    if (action) {
      event.preventDefault()
      action()
      return
    }
  }
  if (event.key === 'ArrowLeft') {
    event.preventDefault()
    select(index.value - 1)
  }
  if (event.key === 'ArrowRight') {
    event.preventDefault()
    select(index.value + 1)
  }
}
defineExpose({ select, openFullscreen })
</script>
<template>
  <UiModal
    wide
    class="ui-detail-dialog"
    :aria-label="label"
    :show-close="false"
    :close-disabled="busy"
    @close="emit('close')"
  >
    <div ref="root" class="ui-detail-modal" @keydown="keys">
      <div class="gallery" :inert="fullscreen || undefined">
        <div v-if="$slots['gallery-overlay']" class="gallery-overlay"><slot name="gallery-overlay" /></div>
        <div class="gallery-image">
          <img
            v-if="url"
            data-resource-preview
            :src="url"
            :alt="caption"
            @click="openFullscreen"
          />
          <slot v-else name="placeholder"><Icon name="image" :size="34" /></slot>
          <UiButton
            v-if="images.length > 1"
            class="gallery-nav gallery-previous"
            icon="chevron-left"
            icon-only
            size="lg"
            :aria-label="t('ui.previousImage')"
            @click="select(index - 1)"
          />
          <UiButton
            v-if="images.length > 1"
            class="gallery-nav gallery-next"
            icon="chevron-right"
            icon-only
            size="lg"
            :aria-label="t('ui.nextImage')"
            @click="select(index + 1)"
          />
          <span v-if="images.length > 1" class="gallery-caption">
            {{ index + 1 }} / {{ images.length }} · {{ caption }}
          </span>
          <UiButton
            ref="fullscreenButton"
            class="gallery-fullscreen"
            icon="expand"
            icon-only
            :aria-label="t('ui.fullscreen')"
            :disabled="!url"
            @click="openFullscreen"
          />
        </div>
        <div v-if="images.length > 1" class="gallery-thumbnails thumbnail-strip">
          <button
            v-for="(image, i) in images"
            :key="image.key"
            type="button"
            :class="{ active: i === index }"
            :aria-label="image.label"
            :aria-current="i === index || undefined"
            :title="image.label"
            @click="select(i)"
          >
            <img v-if="image.url" data-resource-preview :src="image.url" :alt="image.label" />
            <Icon v-else name="image" :size="20" />
          </button>
        </div>
      </div>
      <div class="gallery-details" :inert="fullscreen || undefined">
        <div class="gallery-controls">
          <slot name="header-actions" />
          <UiButton
            class="gallery-close"
            variant="ghost"
            size="sm"
            icon="close"
            icon-only
            :aria-label="closeLabel || t('common.close')"
            :disabled="busy"
            @click="emit('close')"
          />
        </div>
        <div class="gallery-scroll">
          <slot />
        </div>
      </div>

      <!-- 全屏查看：留在弹窗 DOM 内（焦点陷阱仍然有效），用 fixed 铺满窗口 -->
      <Transition name="fade">
        <div
          v-if="fullscreen"
          class="gallery-lightbox"
          tabindex="-1"
          role="dialog"
          aria-modal="true"
          :aria-label="t('ui.fullscreen')"
        >
          <header class="lightbox-bar">
            <span v-if="images.length > 1" class="lightbox-counter">{{ index + 1 }} / {{ images.length }}</span>
            <span class="lightbox-path" :title="caption">{{ caption }}</span>
            <div class="lightbox-actions">
              <div class="lightbox-zoom" role="group" :aria-label="t('ui.zoom')">
                <UiButton
                  class="zoom-out"
                  variant="ghost"
                  icon="zoom-out"
                  icon-only
                  :aria-label="t('ui.zoomOut')"
                  :disabled="!zoom.canZoomOut.value"
                  @click="zoom.zoomOut()"
                />
                <UiButton
                  class="zoom-level"
                  variant="ghost"
                  :aria-label="t('ui.zoomLevel', { percent: zoom.percent.value })"
                  :tooltip="t('ui.fitToScreen')"
                  :disabled="!zoom.ready.value"
                  @click="zoom.fit()"
                >{{ zoom.percent.value }}%</UiButton>
                <UiButton
                  class="zoom-in"
                  variant="ghost"
                  icon="zoom-in"
                  icon-only
                  :aria-label="t('ui.zoomIn')"
                  :disabled="!zoom.canZoomIn.value"
                  @click="zoom.zoomIn()"
                />
              </div>
              <UiButton
                class="zoom-actual"
                variant="ghost"
                :aria-label="t('ui.actualSize')"
                :disabled="!zoom.ready.value"
                @click="zoom.actualSize()"
              >1:1</UiButton>
              <UiButton
                ref="lightboxClose"
                variant="ghost"
                icon="shrink"
                icon-only
                :aria-label="t('ui.exitFullscreen')"
                @click="closeFullscreen"
              />
            </div>
          </header>
          <div
            ref="lightboxStage"
            class="lightbox-stage"
            :class="{ pannable: zoom.canPan.value, dragging: zoom.dragging.value, zoomed: !zoom.fitted.value }"
            @click.self="closeFullscreen"
            @wheel="zoom.wheel"
          >
            <img
              v-if="url"
              :key="url"
              data-resource-preview
              :src="url"
              :alt="caption"
              :style="zoom.imageStyle.value"
              draggable="false"
              @load="zoom.loaded"
              @pointerdown="zoom.startDrag"
              @dblclick="zoom.toggle"
            />
            <Icon v-else name="image" :size="48" />
          </div>
          <UiButton
            v-if="images.length > 1"
            class="lightbox-nav lightbox-previous"
            icon="chevron-left"
            icon-only
            size="lg"
            :aria-label="t('ui.previousImage')"
            @click="select(index - 1)"
          />
          <UiButton
            v-if="images.length > 1"
            class="lightbox-nav lightbox-next"
            icon="chevron-right"
            icon-only
            size="lg"
            :aria-label="t('ui.nextImage')"
            @click="select(index + 1)"
          />
          <footer class="lightbox-footer">
            <div v-if="images.length > 1" class="lightbox-thumbnails thumbnail-strip">
              <button
                v-for="(image, i) in images"
                :key="image.key"
                type="button"
                :class="{ active: i === index }"
                :aria-label="image.label"
                :aria-current="i === index || undefined"
                :title="image.label"
                @click="select(i)"
              >
                <img v-if="image.url" data-resource-preview :src="image.url" :alt="image.label" />
                <Icon v-else name="image" :size="18" />
              </button>
            </div>
            <p class="lightbox-help">{{ t(images.length > 1 ? 'ui.lightboxHelp' : 'ui.lightboxHelpSingle') }}</p>
          </footer>
        </div>
      </Transition>
    </div>
  </UiModal>
</template>
<style scoped>
/*
 * 弹窗固定高度，左右两栏各自独立：
 * 左栏图片区高度只由弹窗决定，右栏内容再多也只在右栏内部滚动，不会把图片拉长或挤走。
 */
:global(.ui-detail-dialog.ui-modal) {
  display: flex;
  flex-direction: column;
  height: min(860px, calc(100dvh - 48px));
  overflow: hidden;
}
:global(.ui-detail-dialog .ui-modal-body) {
  display: flex;
  flex: 1;
  min-height: 0;
}
.ui-detail-modal {
  flex: 1;
  min-width: 0;
  min-height: 0;
  display: grid;
  grid-template-columns: minmax(0, 1.2fr) minmax(360px, 1fr);
  grid-template-rows: minmax(0, 1fr);
  gap: var(--space-8);
}
.gallery-overlay { position: absolute; top: var(--space-stack); left: var(--space-stack); z-index: 1; }
.gallery-overlay :deep(.ui-button) { background: var(--bg-on-image); }
.gallery {
  position: relative;
  display: flex;
  flex-direction: column;
  gap: var(--space-stack);
  min-width: 0;
  min-height: 0;
}
.gallery-image {
  position: relative;
  flex: 1;
  min-height: 0;
  display: grid;
  place-items: center;
  border-radius: var(--radius-card);
  background: var(--bg-base);
  color: var(--text-tertiary);
  overflow: hidden;
}
/* 绝对定位：图片的固有尺寸不参与布局，只在图片区内等比完整显示 */
.gallery-image > img {
  position: absolute;
  inset: 0;
  width: 100%;
  height: 100%;
  object-fit: contain;
  cursor: zoom-in;
}
.gallery-nav {
  position: absolute;
  top: calc(50% - var(--control-h-lg) / 2);
  background: var(--bg-on-image);
}
.gallery-previous { left: var(--space-stack); }
.gallery-next { right: var(--space-stack); }
.gallery-fullscreen {
  position: absolute;
  bottom: var(--space-stack);
  right: var(--space-stack);
  background: var(--bg-on-image);
}
.gallery-caption {
  position: absolute;
  bottom: var(--space-stack);
  left: var(--space-stack);
  max-width: calc(100% - var(--control-h-md) - var(--space-stack) * 3);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  padding: var(--space-1) var(--space-3);
  background: var(--bg-on-image);
  border-radius: var(--radius-full);
  color: var(--text-primary);
  font-size: var(--text-xs);
}

/* 缩略图单行横向滚动，数量再多也不会占用图片区的高度 */
.thumbnail-strip {
  display: flex;
  gap: var(--space-item);
  overflow-x: auto;
  overscroll-behavior-x: contain;
  scrollbar-width: thin;
}
.thumbnail-strip > button {
  flex: 0 0 auto;
  display: grid;
  place-items: center;
  overflow: hidden;
  border-radius: var(--radius-thumbnail);
  background: var(--bg-input);
  color: var(--text-tertiary);
  cursor: pointer;
}
.thumbnail-strip > button:hover:not(.active) { background: var(--bg-control); }
.thumbnail-strip img {
  width: 100%;
  height: 100%;
  object-fit: cover;
}
.thumbnail-strip .active { box-shadow: var(--ring-selected); }
.gallery-thumbnails {
  flex-shrink: 0;
  padding: var(--space-1);
  margin: calc(var(--space-1) * -1);
}
.gallery-thumbnails > button {
  width: 64px;
  height: 64px;
}

.gallery-details {
  position: relative;
  display: flex;
  flex-direction: column;
  min-width: 0;
  min-height: 0;
}
.gallery-controls {
  flex-shrink: 0;
  display: flex;
  justify-content: flex-end;
  gap: var(--space-item);
}
/* 右栏内容自己滚动 */
.gallery-scroll {
  flex: 1;
  min-height: 0;
  /* 滚动条放进弹窗右侧留白里，内容右缘和上面的关闭按钮对齐 */
  margin-right: calc(var(--space-3) * -1);
  overflow-y: auto;
  overscroll-behavior: contain;
  scrollbar-gutter: stable;
  padding: var(--space-2) var(--space-1) var(--space-panel);
}

/* ── 全屏查看 ─────────────────────────────── */
.gallery-lightbox {
  position: fixed;
  inset: 0;
  z-index: var(--z-overlay);
  display: grid;
  grid-template-rows: auto minmax(0, 1fr) auto;
  background: var(--bg-base);
  color: var(--text-primary);
}
/* 可聚焦：点图片后焦点留在全屏层里，键盘缩放、切图继续有效 */
.gallery-lightbox:focus { outline: none; }
.lightbox-bar {
  display: flex;
  align-items: center;
  gap: var(--space-stack);
  padding: var(--space-stack) var(--space-page-x);
  min-width: 0;
}
.lightbox-counter {
  flex-shrink: 0;
  display: inline-flex;
  align-items: center;
  min-height: 22px;
  padding: 0 var(--space-2);
  border-radius: var(--radius-full);
  background: var(--bg-input);
  font-size: var(--text-xs);
  font-weight: var(--font-bold);
}
.lightbox-path {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  color: var(--text-secondary);
  font-family: var(--font-mono);
  font-size: var(--text-sm);
}
.lightbox-actions {
  flex-shrink: 0;
  display: flex;
  gap: var(--space-item);
}
.lightbox-zoom {
  display: flex;
  align-items: center;
  border-radius: var(--radius-control);
  background: var(--bg-input);
}
/* 百分比宽度固定，数字变化时左右按钮不跳动 */
.lightbox-zoom .zoom-level {
  min-width: 64px;
  padding: 0 var(--space-2);
}
.zoom-actual {
  min-width: var(--control-h-md);
  padding: 0 var(--space-2);
}
/* 看图区：左右留出切换按钮的位置，「适应屏幕」按内容区计算；放大后图片可以铺满整个宽度 */
.lightbox-stage {
  position: relative;
  min-height: 0;
  display: grid;
  place-items: center;
  padding: var(--space-2) calc(var(--control-h-lg) + var(--space-page-x) * 2);
  overflow: hidden;
  color: var(--text-tertiary);
  touch-action: none;
}
.lightbox-stage > img {
  position: absolute;
  left: 50%;
  top: 50%;
  max-width: none;
  transform-origin: center;
  cursor: zoom-in;
  user-select: none;
  -webkit-user-drag: none;
}
.lightbox-stage.zoomed > img { cursor: zoom-out; }
.lightbox-stage.pannable > img { cursor: grab; }
.lightbox-stage.dragging > img { cursor: grabbing; }
.lightbox-nav {
  position: absolute;
  top: calc(50% - var(--control-h-lg) / 2);
  z-index: 1;
}
.lightbox-previous { left: var(--space-page-x); }
.lightbox-next { right: var(--space-page-x); }
.lightbox-footer {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: var(--space-item);
  padding: var(--space-stack) var(--space-page-x) var(--space-page-y);
  min-width: 0;
}
.lightbox-thumbnails {
  max-width: 100%;
  padding: var(--space-1);
}
.lightbox-thumbnails > button {
  width: 52px;
  height: 52px;
  background: var(--bg-elevated);
}
.lightbox-help {
  font-size: var(--text-sm);
  color: var(--text-tertiary);
}

@media (max-width: 850px) {
  .ui-detail-modal {
    grid-template-columns: minmax(0, 1fr);
    grid-template-rows: minmax(200px, 42%) minmax(0, 1fr);
    gap: var(--space-section);
  }
  .gallery-thumbnails { display: none; }
  .lightbox-stage { padding: var(--space-2) var(--space-page-x); }
}
</style>
