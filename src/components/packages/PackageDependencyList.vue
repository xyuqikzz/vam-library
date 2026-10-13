<script setup lang="ts">
import { ref } from 'vue'
import { useI18n } from 'vue-i18n'
import Icon from '@/components/ui/Icon.vue'
import UiButton from '@/components/ui/UiButton.vue'

export interface DetailDependency {
  id: string
  installedId?: string
  status: 'owned' | 'missing' | 'lower-version' | 'referenced'
  statusText: string
  tier?: number
}
defineProps<{
  dependencies: DetailDependency[]
  thumbnails?: Record<string, string | null>
  busy?: boolean
  opening?: string
}>()
const emit = defineEmits<{
  copy: [id: string]
  details: [dependency: DetailDependency]
  complete: []
}>()
const { t } = useI18n()
const failedImages = ref(new Set<string>())
</script>

<template>
  <div class="dependency-list">
    <div
      v-for="dependency in dependencies"
      :key="dependency.id"
      class="dependency-row"
      :class="{ missing: dependency.status === 'missing' || dependency.status === 'lower-version' }"
    >
      <span class="dependency-thumb">
        <img
          v-if="thumbnails?.[dependency.id] && !failedImages.has(thumbnails[dependency.id]!)"
          data-resource-preview
          :src="thumbnails[dependency.id]!"
          :alt="dependency.id"
          @error="failedImages.add(thumbnails![dependency.id]!)"
        />
        <Icon v-else name="package" :size="20" />
      </span>
      <div class="dependency-name">
        <div class="dependency-title">
          <strong :title="dependency.id">{{ dependency.id }}</strong>
          <UiButton
            class="dependency-copy"
            variant="ghost"
            size="sm"
            icon="copy"
            icon-only
            :aria-label="t('ui.copyResourceName', { name: dependency.id })"
            @click="emit('copy', dependency.id)"
          />
        </div>
        <span class="dependency-status" :class="dependency.status">{{ dependency.statusText }}</span>
      </div>
      <div class="dependency-actions">
        <!-- 缺失的依赖本地没有文件，详情里什么都看不了，只给「补全」 -->
        <UiButton
          v-if="dependency.status !== 'missing'"
          class="dependency-details"
          size="sm"
          :data-dependency-id="dependency.id"
          :aria-label="t('ui.dependencyDetails', { name: dependency.id })"
          :disabled="busy"
          :loading="opening === dependency.id"
          @click="emit('details', dependency)"
        >{{ t('ui.details') }}</UiButton>
        <UiButton
          v-if="dependency.status === 'missing' || dependency.status === 'lower-version'"
          class="resolve-button"
          size="sm"
          :disabled="busy"
          @click="emit('complete')"
        >{{ t('ui.complete') }}</UiButton>
      </div>
    </div>
  </div>
</template>

<style scoped>
.dependency-list { display: flex; flex-direction: column; gap: var(--space-item); }
.dependency-row {
  display: flex;
  align-items: center;
  gap: var(--space-item);
  padding: var(--space-item);
  border-radius: var(--radius-sm);
  min-width: 0;
}
.dependency-row.missing { background: var(--color-warning-bg); }
.dependency-thumb {
  display: grid;
  place-items: center;
  flex-shrink: 0;
  width: var(--control-h-lg);
  height: var(--control-h-lg);
  overflow: hidden;
  border-radius: var(--radius-xs);
  background: var(--bg-control);
  color: var(--text-tertiary);
}
.dependency-thumb img { width: 100%; height: 100%; object-fit: contain; background: var(--bg-base); }
.dependency-name { flex: 1; min-width: 0; }
.dependency-title { display: flex; align-items: center; gap: var(--space-1); }
.dependency-title strong {
  flex: 1;
  min-width: 0;
  display: -webkit-box;
  -webkit-box-orient: vertical;
  -webkit-line-clamp: 2;
  overflow: hidden;
  overflow-wrap: anywhere;
  font-size: var(--text-sm);
}
.dependency-status { color: var(--text-tertiary); font-size: var(--text-xs); }
.dependency-status.owned { color: var(--color-success); }
.dependency-status.missing, .dependency-status.lower-version { color: var(--color-warning); }
.dependency-actions { display: flex; flex-direction: column; gap: var(--space-item); }
.resolve-button { background: var(--color-warning); color: var(--color-warning-ink); }
.resolve-button:hover:not(:disabled) { background: color-mix(in srgb, var(--color-warning), var(--bg-control-hover) 18%); }
</style>
