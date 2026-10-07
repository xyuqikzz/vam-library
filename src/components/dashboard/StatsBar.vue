<template>
  <div class="stats-bar">
    <div v-for="(item, i) in items" :key="item.label" class="stat-entry">
      <div v-if="i > 0" class="stat-sep" />
      <span class="stat-value" :class="item.cls">
        {{ loading ? '—' : item.value }}
      </span>
      <span class="stat-label">{{ item.label }}</span>
    </div>
  </div>
</template>

<script setup lang="ts">
import { formatSize } from '@/utils/bytes'
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'

const props = defineProps<{
  stats: {
    total_packages: number
    total_size_bytes: number
    scene_count: number
    missing_dependencies: number
    corrupted_packages: number
    duplicate_resources: number
  }
  loading: boolean
}>()

const { t } = useI18n()


const totalIssues = computed(() =>
  props.stats.missing_dependencies +
  props.stats.corrupted_packages +
  props.stats.duplicate_resources
)

const items = computed(() => [
  { value: props.stats.total_packages, label: t('dashboard.totalPackages'), cls: '' },
  { value: formatSize(props.stats.total_size_bytes), label: t('dashboard.totalSize') || '总大小', cls: '' },
  { value: props.stats.scene_count, label: t('dashboard.scenes'), cls: '' },
  {
    value: totalIssues.value,
    label: t('dashboard.issues'),
    cls: totalIssues.value > 0 ? 'has-issues' : 'all-clear',
  },
])
</script>

<style scoped>
.stats-bar {
  display: flex;
  align-items: baseline;
  padding: var(--space-3) 0;
}

.stat-entry {
  display: flex;
  align-items: baseline;
  gap: 4px;
}

.stat-sep {
  width: 1px;
  height: 16px;
  background: var(--border-default);
  margin: 0 var(--space-4);
  align-self: center;
}

.stat-value {
  font-size: var(--text-xl);
  font-weight: var(--font-semibold);
  color: var(--text-primary);
  font-variant-numeric: tabular-nums;
}

.stat-value.has-issues {
  color: var(--color-error);
}

.stat-value.all-clear {
  color: var(--color-success);
}

.stat-label {
  font-size: var(--text-xs);
  color: var(--text-tertiary);
}
</style>
