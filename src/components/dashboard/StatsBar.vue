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
  { value: formatSize(props.stats.total_size_bytes), label: t('dashboard.totalSize'), cls: '' },
  { value: props.stats.scene_count, label: t('dashboard.scenes'), cls: '' },
  {
    value: totalIssues.value,
    label: t('dashboard.issues'),
    cls: totalIssues.value > 0 ? 'has-issues' : 'all-clear',
  },
])
</script><template><div class="stats-bar"><div v-for="(item, i) in items" :key="item.label" class="stat-entry"><span class="stat-label">{{ item.label }}</span><strong class="stat-value" :class="item.cls">{{ loading ? '—' : item.value }}</strong><span class="stat-note">{{ i === 3 ? t('ui.health') : i === 2 ? t('resourceType.scene') : t('sidebar.packages') }}</span></div></div></template><style scoped>.stats-bar { display: grid; grid-template-columns: repeat(auto-fit, minmax(180px, 1fr)); gap: 14px; }.stat-entry { display: flex; flex-direction: column; gap: 8px; padding: 18px; border-radius: var(--radius-lg); background: var(--bg-elevated); }.stat-label { font-size: var(--text-base); color: var(--text-secondary); }.stat-value { font-size: var(--text-stat); font-weight: var(--font-heavy); line-height: 1.1; color: var(--text-primary); }.stat-value.has-issues { color: var(--color-warning); }.stat-note { font-size: var(--text-sm); color: var(--text-tertiary); }</style>