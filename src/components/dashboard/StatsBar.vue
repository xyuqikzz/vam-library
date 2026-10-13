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
    appearance_count?: number
    plugin_count?: number
    morph_count?: number
  }
  loading: boolean
}>()

const { t } = useI18n()


const totalIssues = computed(() =>
  props.stats.missing_dependencies +
  props.stats.corrupted_packages +
  props.stats.duplicate_resources
)

const items = computed(() => {
  const s = props.stats
  const hasPackages = s.total_packages > 0
  return [
    {
      value: s.total_packages.toLocaleString(),
      label: t('dashboard.totalPackages'),
      cls: '',
      note: hasPackages
        ? t('ui.statPackagesNote', { appearance: s.appearance_count ?? 0, plugin: s.plugin_count ?? 0, morph: s.morph_count ?? 0 })
        : t('dashboard.totalPackagesSub'),
      noteCls: '',
    },
    {
      value: formatSize(s.total_size_bytes),
      label: t('dashboard.totalSize'),
      cls: '',
      note: hasPackages
        ? t('ui.statSizeNote', { size: formatSize(s.total_size_bytes / s.total_packages) })
        : t('dashboard.totalSizeSub'),
      noteCls: '',
    },
    { value: s.scene_count.toLocaleString(), label: t('dashboard.scenes'), cls: '', note: t('dashboard.scenesSub'), noteCls: '' },
    {
      value: totalIssues.value.toLocaleString(),
      label: t('dashboard.issues'),
      cls: totalIssues.value > 0 ? 'has-issues' : 'all-clear',
      note: totalIssues.value > 0
        ? t('ui.statIssuesNote', { missing: s.missing_dependencies, corrupted: s.corrupted_packages, duplicates: s.duplicate_resources })
        : t('dashboard.issuesSub'),
      noteCls: totalIssues.value > 0 ? '' : 'is-ok',
    },
  ]
})
</script><template>
  <div class="stats-bar">
    <div v-for="item in items" :key="item.label" class="stat-entry">
      <span class="stat-label">{{ item.label }}</span>
      <strong class="stat-value" :class="item.cls">{{ loading ? '—' : item.value }}</strong>
      <span class="stat-note" :class="item.noteCls" :title="item.note">{{ loading ? '' : item.note }}</span>
    </div>
  </div>
</template><style scoped>
.stats-bar {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(180px, 1fr));
  gap: var(--space-grid);
}
.stat-entry {
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
  padding: var(--space-4) var(--space-panel);
  border-radius: var(--radius-lg);
  background: var(--bg-elevated);
}
.stat-label {
  font-size: var(--text-base);
  color: var(--text-secondary);
}
.stat-value {
  font-size: var(--text-stat);
  font-weight: var(--font-heavy);
  line-height: 1.1;
  color: var(--text-primary);
}
.stat-value.has-issues {
  color: var(--color-warning);
}
.stat-note {
  min-height: 1.5em;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  font-size: var(--text-sm);
  color: var(--text-tertiary);
}
.stat-note.is-ok {
  color: var(--color-success);
}
</style>