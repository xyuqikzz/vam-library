<script setup lang="ts">
import { useRouter } from 'vue-router'
import UiButton from '@/components/ui/UiButton.vue'
import Icon from '@/components/ui/Icon.vue'

import { computed, ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { useNotification } from '@/composables/useNotification'

const props = defineProps<{
  missing: number
  corrupted: number
  duplicates: number
  missingList: Array<{ depends_on_id: string; package_id: string }>
  corruptedList: Array<{ package_id: string; error: string }>
}>()

const { t } = useI18n()
const notify = useNotification()

const showMissing = ref(false)
const showCorrupted = ref(false)

const totalIssues = computed(() => props.missing + props.corrupted + props.duplicates)

async function copyId(text: string) {
  try {
    await navigator.clipboard.writeText(text)
    notify.success(t('dashboard.idCopied'))
  } catch (e) {
    notify.error(String(e), t('common.error'))
  }
}

const router = useRouter()
const mostMissing = computed(() => { const counts = new Map<string, number>(); for (const dependency of props.missingList) counts.set(dependency.depends_on_id, (counts.get(dependency.depends_on_id) || 0) + 1); return [...counts].sort((a,b) => b[1] - a[1]).slice(0,5) })
</script><template>
  <div class="health-status">
    <p v-if="!totalIssues" class="health-ok">
      <Icon name="check" :size="16" />
      {{ t('dashboard.noHealthData') }}
    </p>
    <div class="health-row">
      <span class="health-icon warning"><Icon name="link" /></span>
      <button
        class="health-label"
        type="button"
        :aria-expanded="showMissing"
        @click="showMissing = !showMissing"
      >
        <strong>{{ t('ui.missingDependencies') }}</strong>
        <small>{{ t('dashboard.missingDependency', { count: missing }) }}</small>
      </button>
      <UiButton
        variant="primary"
        size="sm"
        :disabled="!missing"
        @click="router.push('/dependency-completion')"
      >
        {{ t('ui.allDependencies') }}
      </UiButton>
    </div>
    <div v-if="showMissing" class="health-detail">
      <div v-for="(dep, i) in missingList" :key="i" class="dep-row">
        <span :title="dep.depends_on_id">{{ dep.depends_on_id }}</span>
        <UiButton
          icon="copy"
          icon-only
          variant="ghost"
          size="sm"
          :aria-label="t('dashboard.copyId')"
          @click="copyId(dep.depends_on_id)"
        />
      </div>
    </div>
    <div class="health-row">
      <span class="health-icon error"><Icon name="warning" /></span>
      <div class="health-label">
        <strong>{{ t('ui.corruption') }}</strong>
        <small>{{ t('dashboard.corruptedPackageCount', { count: corrupted }) }}</small>
      </div>
      <UiButton
        variant="ghost"
        size="sm"
        :aria-expanded="showCorrupted"
        @click="showCorrupted = !showCorrupted"
      >
        {{ t('ui.inspect') }}
      </UiButton>
    </div>
    <div v-if="showCorrupted" class="health-detail">
      <div v-for="pkg in corruptedList" :key="pkg.package_id" class="corrupted-row">
        <strong>{{ pkg.package_id }}</strong>
        <p>{{ pkg.error }}</p>
      </div>
      <p v-if="!corruptedList.length">{{ t('ui.noData') }}</p>
    </div>
    <div class="health-row">
      <span class="health-icon info"><Icon name="copy" /></span>
      <div class="health-label">
        <strong>{{ t('ui.duplicates') }}</strong>
        <small>{{ t('dashboard.duplicateResource', { count: duplicates }) }}</small>
      </div>
      <UiButton variant="ghost" size="sm" @click="router.push('/deduplication')">
        {{ t('sidebar.deduplication') }}
      </UiButton>
    </div>
    <section v-if="mostMissing.length" class="missing-list">
      <h3>{{ t('ui.missingMany') }}</h3>
      <div v-for="[id, count] in mostMissing" :key="id" class="dep-row">
        <span :title="id">{{ id }}</span>
        <small>{{ count }}</small>
        <UiButton
          icon="copy"
          icon-only
          variant="ghost"
          size="sm"
          :aria-label="t('dashboard.copyId')"
          @click="copyId(id)"
        />
      </div>
    </section>
  </div>
</template><style scoped>
.health-status {
  display: flex;
  flex-direction: column;
  gap: 12px;
}
.health-ok {
  display: flex;
  align-items: center;
  gap: 6px;
  color: var(--color-success);
  font-size: var(--text-sm);
}
.health-row {
  display: flex;
  align-items: center;
  gap: 10px;
}
.health-icon {
  display: grid;
  place-items: center;
  width: 36px;
  height: 36px;
  flex-shrink: 0;
  border-radius: var(--radius-full);
}
.warning {
  background: var(--color-warning-bg);
  color: var(--color-warning);
}
.error {
  background: var(--color-error-bg);
  color: var(--color-error);
}
.info {
  background: var(--color-info-bg);
  color: var(--color-info);
}
.health-label {
  display: flex;
  flex-direction: column;
  flex: 1;
  min-width: 0;
  gap: 3px;
  text-align: left;
  border-radius: var(--radius-xs);
}
.health-label strong {
  font-size: var(--text-md);
  font-weight: var(--font-heavy);
}
.health-label small {
  color: var(--text-tertiary);
  font-size: var(--text-xs);
}
.health-detail {
  padding: 8px;
  border-radius: var(--radius-sm);
  background: var(--bg-input);
  font-size: var(--text-sm);
}
.corrupted-row {
  margin-bottom: 8px;
  overflow-wrap: anywhere;
}
.corrupted-row p {
  color: var(--color-error);
}
.missing-list {
  margin-top: 8px;
  padding-top: 14px;
  border-top: 1px solid var(--divider);
}
.missing-list h3 {
  font-size: var(--text-base);
  margin-bottom: 8px;
}
.dep-row {
  display: flex;
  align-items: center;
  gap: 8px;
  min-width: 0;
  min-height: 32px;
  font-size: var(--text-xs);
  color: var(--text-secondary);
}
.dep-row > span {
  flex: 1;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.dep-row small {
  color: var(--color-warning);
}
</style>