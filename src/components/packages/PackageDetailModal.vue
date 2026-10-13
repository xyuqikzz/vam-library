<script setup lang="ts">
import { computed, ref } from 'vue'
import { useI18n } from 'vue-i18n'
import type { PackageImageEntry } from '@/types/package'
import UiDetailModal, { type DetailImage } from '@/components/ui/UiDetailModal.vue'
import UiButton from '@/components/ui/UiButton.vue'
import { focusInitial } from '@/components/ui/overlay'
const props = defineProps<{
  open: boolean
  title: string
  packageId: string
  images: PackageImageEntry[]
  imageUrls: Record<string, string>
  fallbackImage?: string | null
  busy?: boolean
  canGoBack?: boolean
}>()
const emit = defineEmits<{ close: []; back: []; image: [image: PackageImageEntry] }>()
const { t } = useI18n()
const backButton = ref<InstanceType<typeof UiButton> | null>(null)
defineExpose({ focusBack: () => { if (backButton.value?.element) focusInitial(backButton.value.element) } })
const galleryImages = computed<DetailImage[]>(() => props.images.map(image => ({
  key: image.path,
  label: image.path,
  url: props.imageUrls[image.path] || null,
})))
function selectImage(index: number) {
  const image = props.images[index]
  if (image) emit('image', image)
}
</script>
<template>
  <UiDetailModal
    v-if="open"
    class="package-detail-modal"
    :label="title"
    :images="galleryImages"
    :fallback-image="fallbackImage"
    :content-key="packageId"
    :busy="busy"
    @close="emit('close')"
    @select="selectImage"
  >
    <template v-if="canGoBack" #gallery-overlay>
      <UiButton
        ref="backButton"
        class="dependency-back"
        size="sm"
        icon="chevron-left"
        :disabled="busy"
        @click="emit('back')"
      >{{ t('ui.backToPreviousPackage') }}</UiButton>
    </template>
    <template #header-actions><slot name="header-actions" /></template>
    <slot />
  </UiDetailModal>
  <!--
    没打开详情时也要挂着内容：卡片右键菜单、快捷键、卡片上的启动按钮都通过这里的操作组件执行。
    只是不显示，不占位置，也不能被 Tab 到。
  -->
  <div v-else class="package-detail-headless" hidden>
    <slot name="header-actions" />
    <slot />
  </div>
</template>
<style scoped>
/* 详情弹窗自带左右留白，面板自己的内边距去掉 */
:global(.package-detail-modal .detail-body) {
  padding: 0;
}
</style>
