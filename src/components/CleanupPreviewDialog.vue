<template>
  <UiModal
    ref="dialog"
    class="cleanup-preview-modal"
    large
    :title="t('cleanup.dialogTitle', { count: requested.length })"
    :description="t('cleanup.scope')"
    :close-disabled="executing || confirming"
    @cancel.prevent="close"
    imperative
  >
    <div class="cleanup-dialog-body">
      <div v-if="busy" class="cleanup-progress">
        <span class="spinner-ring sm" aria-hidden="true" />
        <div class="cleanup-progress-copy"><p role="status">{{ t(progressLabel) }} {{ progressText }}</p><UiProgress v-if="progress.total" :value="progress.current" :max="progress.total" :label="t(progressLabel)" /></div>
      </div>
      <div v-if="error" class="cleanup-notice cleanup-error" role="alert">
        <Icon name="warning" />
        <div class="cleanup-notice-copy">
          <strong>{{ t('cleanup.errorTitle') }}</strong>
          <details class="cleanup-error-details">
            <summary class="cleanup-detail-title">{{ t('cleanup.errorDetails') }}</summary>
            <p class="error cleanup-diagnostic-entry">{{ error }}</p>
          </details>
        </div>
        <UiButton v-if="!result" size="sm" :disabled="busy || confirming" @click="analyze()">{{ t('cleanup.checkAgain') }}</UiButton>
      </div>

      <template v-if="preview && !result">
        <section class="preview-overview" :aria-label="t('cleanup.previewSummary', { packages: preview.items.length, files: preview.total_files, size: formatSize(preview.total_bytes) })">
          <dl class="preview-metrics">
            <div class="preview-metric">
              <dt>{{ t('cleanup.canRemove') }}</dt>
              <dd>{{ t('cleanup.packages', { count: preview.items.length }) }}</dd>
              <dd class="preview-metric-sub">{{ preview.items.length ? t('cleanup.sizeAndFiles', { size: formatSize(preview.total_bytes), files: preview.total_files, count: preview.total_files }) : t('cleanup.nothingYet') }}</dd>
            </div>
            <div v-if="decide.length" class="preview-metric">
              <dt>{{ t('cleanup.needsDecision') }}</dt>
              <dd>{{ t('cleanup.packages', { count: decide.length }) }}</dd>
              <dd class="preview-metric-sub">{{ t('cleanup.needsDecisionHint') }}</dd>
            </div>
          </dl>
        </section>

        <div v-if="preview.warnings.length && blockedByUnreadable" class="cleanup-notice warn unreadable-summary" role="alert">
          <Icon name="warning" />
          <div class="cleanup-notice-copy">
            <strong>{{ t('cleanup.unreadableTitle', { count: preview.warnings.length }) }}</strong>
            <p>{{ t('cleanup.unreadableHelp') }}</p>
            <details class="cleanup-list-section warning-details">
              <summary class="cleanup-detail-title">{{ t('cleanup.unreadableDetails') }}</summary>
              <nav v-if="pageCount('warnings', preview.warnings.length, 20) > 1" class="cleanup-pages" :aria-label="t('cleanup.unreadableDetails')">
                <UiButton size="sm" variant="ghost" :disabled="pageOf('warnings') <= 1" @click="turn('warnings', -1)">{{ t('cleanup.previousPage') }}</UiButton>
                <span>{{ pageOf('warnings') }} / {{ pageCount('warnings', preview.warnings.length, 20) }}</span>
                <UiButton size="sm" variant="ghost" :disabled="pageOf('warnings') >= pageCount('warnings', preview.warnings.length, 20)" @click="turn('warnings', 1)">{{ t('cleanup.nextPage') }}</UiButton>
              </nav>
              <ul class="cleanup-item-list">
                <li v-for="(warning, index) in slice('warnings', preview.warnings, 20)" :key="index" class="cleanup-diagnostic-entry">{{ warning }}</li>
              </ul>
            </details>
          </div>
          <UiButton size="sm" :disabled="busy || confirming" @click="analyze()">{{ t('cleanup.checkAgain') }}</UiButton>
        </div>

        <section v-if="decide.length" class="cleanup-group decide-group" :aria-label="t('cleanup.needsDecision')">
          <h3 class="cleanup-group-title">{{ t('cleanup.needsDecision') }} <span class="cleanup-count">{{ decide.length }}</span></h3>
          <nav v-if="pageCount('decide', decide.length) > 1" class="cleanup-pages" :aria-label="t('cleanup.needsDecision')">
            <UiButton size="sm" variant="ghost" :disabled="pageOf('decide') <= 1" @click="turn('decide', -1)">{{ t('cleanup.previousPage') }}</UiButton>
            <span>{{ pageOf('decide') }} / {{ pageCount('decide', decide.length) }}</span>
            <UiButton size="sm" variant="ghost" :disabled="pageOf('decide') >= pageCount('decide', decide.length)" @click="turn('decide', 1)">{{ t('cleanup.nextPage') }}</UiButton>
          </nav>
          <ul class="cleanup-item-list">
            <li v-for="item in slice('decide', decide)" :key="item.package_id" class="cleanup-item">
              <CleanupResourceItem :package-id="item.package_id" :thumbnail="thumbnails[item.package_id.toLowerCase()]" :disabled="busy || confirming" @details="openPackage(item.package_id)">
                <span v-if="item.invalid_name" class="cleanup-tags"><span class="cleanup-tag warning">{{ t('cleanup.issues.tag.invalidName') }}</span></span>
                <span v-if="hasBrokenInfo(item)" class="cleanup-tags"><span class="cleanup-tag error">{{ t('cleanup.issues.tag.brokenInfo') }}</span></span>
                <span v-if="item.size_bytes" class="cleanup-item-size">{{ formatSize(item.size_bytes) }}</span>
                <small v-for="line in reasonLines(item)" :key="line" class="cleanup-reason">{{ line }}</small>
                <template #actions><div class="cleanup-item-actions">
                  <UiButton size="sm" :disabled="busy || confirming" @click="keep(item.package_id)">{{ t('cleanup.keep') }}</UiButton>
                  <UiButton v-if="addableUsers(item).length" size="sm" :disabled="busy || confirming" @click="addUsers(item)">{{ t('cleanup.addUsers', { count: addableUsers(item).length }) }}</UiButton>
                  <UiButton v-if="canForce(item)" size="sm" variant="ghost" class="force-action" :disabled="busy || confirming" @click="force(item.package_id)">{{ t('cleanup.removeAnyway') }}</UiButton>
                </div></template>
              </CleanupResourceItem>
              <section v-if="users(item).length" class="cleanup-consumers" :aria-label="t('cleanup.usedBy', { count: users(item).length })">
                <h4>{{ t('cleanup.usedBy', { count: users(item).length }) }}</h4>
                <ul class="cleanup-consumer-list">
                  <li v-for="id in slice('users:' + item.package_id, users(item), 20)" :key="id">
                    <CleanupResourceItem :package-id="id" :thumbnail="thumbnails[id.toLowerCase()]" :disabled="busy || confirming" @details="openPackage(id)" />
                  </li>
                </ul>
                <nav v-if="users(item).length > 20" class="cleanup-pages" :aria-label="t('cleanup.usedBy', { count: users(item).length })">
                  <UiButton size="sm" variant="ghost" :disabled="pageOf('users:' + item.package_id) <= 1" @click="turn('users:' + item.package_id, -1)">{{ t('cleanup.previousPage') }}</UiButton>
                  <span>{{ pageOf('users:' + item.package_id) }} / {{ pageCount('', users(item).length, 20) }}</span>
                  <UiButton size="sm" variant="ghost" :disabled="pageOf('users:' + item.package_id) >= pageCount('', users(item).length, 20)" @click="turn('users:' + item.package_id, 1)">{{ t('cleanup.nextPage') }}</UiButton>
                </nav>
              </section>
            </li>
          </ul>
        </section>

        <section v-if="preview.items.length" class="cleanup-group remove-group" :aria-label="t('cleanup.canRemove')">
          <h3 class="cleanup-group-title">{{ t('cleanup.canRemove') }} <span class="cleanup-count">{{ preview.items.length }}</span></h3>
          <nav v-if="pageCount('items', preview.items.length) > 1" class="cleanup-pages" :aria-label="t('cleanup.canRemove')">
            <UiButton size="sm" variant="ghost" :disabled="pageOf('items') <= 1" @click="turn('items', -1)">{{ t('cleanup.previousPage') }}</UiButton>
            <span>{{ pageOf('items') }} / {{ pageCount('items', preview.items.length) }}</span>
            <UiButton size="sm" variant="ghost" :disabled="pageOf('items') >= pageCount('items', preview.items.length)" @click="turn('items', 1)">{{ t('cleanup.nextPage') }}</UiButton>
          </nav>
          <ul class="cleanup-item-list">
            <li v-for="item in slice('items', preview.items)" :key="item.package_id" class="cleanup-item">
              <CleanupResourceItem :package-id="item.package_id" :thumbnail="thumbnails[item.package_id.toLowerCase()]" :disabled="busy || confirming" @details="openPackage(item.package_id)">
                <span class="cleanup-tags">
                  <span v-if="item.issue" class="cleanup-tag error">{{ t(`cleanup.issues.tag.${issueTag(item.issue)}`) }}</span>
                  <span v-else-if="hasBrokenInfo(item)" class="cleanup-tag error">{{ t('cleanup.issues.tag.brokenInfo') }}</span>
                  <span v-if="item.invalid_name" class="cleanup-tag warning">{{ t('cleanup.issues.tag.invalidName') }}</span>
                  <span v-if="item.attached" class="cleanup-tag">{{ t('cleanup.attached') }}</span>
                  <span v-if="forcedIds.includes(item.package_id.toLowerCase())" class="cleanup-tag warning">{{ t('cleanup.forced') }}</span>
                </span>
                <small v-for="line in reasonLines(item)" :key="line" class="cleanup-reason">{{ line }}</small>
              <template #actions><div class="cleanup-item-side">
                <span class="cleanup-item-size">{{ formatSize(item.size_bytes) }} · {{ t('cleanup.fileCountShort', { count: item.file_count }) }}</span>
                <UiButton v-if="forcedIds.includes(item.package_id.toLowerCase())" size="sm" variant="ghost" :disabled="busy || confirming" @click="unforce(item.package_id)">{{ t('cleanup.undo') }}</UiButton>
                <UiButton v-else size="sm" variant="ghost" :disabled="busy || confirming" @click="keep(item.package_id)">{{ t('cleanup.keep') }}</UiButton>
              </div></template>
              </CleanupResourceItem>
              <section v-if="users(item).length" class="cleanup-consumers" :aria-label="t('cleanup.usedBy', { count: users(item).length })">
                <h4>{{ t('cleanup.usedBy', { count: users(item).length }) }}</h4>
                <ul class="cleanup-consumer-list">
                  <li v-for="id in slice('users:' + item.package_id, users(item), 20)" :key="id">
                    <CleanupResourceItem :package-id="id" :thumbnail="thumbnails[id.toLowerCase()]" :disabled="busy || confirming" @details="openPackage(id)" />
                  </li>
                </ul>
                <nav v-if="users(item).length > 20" class="cleanup-pages" :aria-label="t('cleanup.usedBy', { count: users(item).length })">
                  <UiButton size="sm" variant="ghost" :disabled="pageOf('users:' + item.package_id) <= 1" @click="turn('users:' + item.package_id, -1)">{{ t('cleanup.previousPage') }}</UiButton>
                  <span>{{ pageOf('users:' + item.package_id) }} / {{ pageCount('', users(item).length, 20) }}</span>
                  <UiButton size="sm" variant="ghost" :disabled="pageOf('users:' + item.package_id) >= pageCount('', users(item).length, 20)" @click="turn('users:' + item.package_id, 1)">{{ t('cleanup.nextPage') }}</UiButton>
                </nav>
              </section>
            </li>
          </ul>
        </section>

        <UiCheckbox v-if="includeDependencies || suggested.length" v-model="includeDependencies" class="choice-card dependency-choice" :disabled="busy || confirming">
          <span class="dependency-choice-copy">
            <span>{{ includeDependencies ? t('cleanup.includeDependencies') : t('cleanup.suggestDependencies', { count: suggested.length }) }}</span>
            <small>{{ includeDependencies ? t('cleanup.includeDependenciesHelp') : t('cleanup.suggestDependenciesHelp', { size: formatSize(preview.suggested_bytes ?? 0) }) }}</small>
          </span>
        </UiCheckbox>

        <details v-if="kept.length" class="cleanup-list-section kept-group">
          <summary class="cleanup-detail-title">{{ t('cleanup.keptThisTime') }} <span class="cleanup-count">{{ kept.length }}</span></summary>
          <ul class="cleanup-item-list">
            <li v-for="item in slice('kept', kept)" :key="item.package_id" class="cleanup-item">
              <CleanupResourceItem :package-id="item.package_id" :thumbnail="thumbnails[item.package_id.toLowerCase()]" :disabled="busy || confirming" @details="openPackage(item.package_id)">
                <template #actions><UiButton size="sm" variant="ghost" :disabled="busy || confirming" @click="reselect(item.package_id)">{{ t('cleanup.reselect') }}</UiButton></template>
              </CleanupResourceItem>
            </li>
          </ul>
        </details>

        <details v-if="retained.length" class="cleanup-list-section retained-group">
          <summary class="cleanup-detail-title">{{ t('cleanup.retained') }} <span class="cleanup-count">{{ retained.length }}</span></summary>
          <nav v-if="pageCount('retained', retained.length) > 1" class="cleanup-pages" :aria-label="t('cleanup.retained')">
            <UiButton size="sm" variant="ghost" :disabled="pageOf('retained') <= 1" @click="turn('retained', -1)">{{ t('cleanup.previousPage') }}</UiButton>
            <span>{{ pageOf('retained') }} / {{ pageCount('retained', retained.length) }}</span>
            <UiButton size="sm" variant="ghost" :disabled="pageOf('retained') >= pageCount('retained', retained.length)" @click="turn('retained', 1)">{{ t('cleanup.nextPage') }}</UiButton>
          </nav>
          <ul class="cleanup-item-list">
            <li v-for="item in slice('retained', retained)" :key="item.package_id" class="cleanup-item">
              <CleanupResourceItem :package-id="item.package_id" :thumbnail="thumbnails[item.package_id.toLowerCase()]" :disabled="busy || confirming" @details="openPackage(item.package_id)">
                <small v-for="line in reasonLines(item)" :key="line" class="cleanup-reason">{{ line }}</small>
              </CleanupResourceItem>
              <section v-if="users(item).length" class="cleanup-consumers" :aria-label="t('cleanup.usedBy', { count: users(item).length })">
                <h4>{{ t('cleanup.usedBy', { count: users(item).length }) }}</h4>
                <ul class="cleanup-consumer-list">
                  <li v-for="id in slice('users:' + item.package_id, users(item), 20)" :key="id">
                    <CleanupResourceItem :package-id="id" :thumbnail="thumbnails[id.toLowerCase()]" :disabled="busy || confirming" @details="openPackage(id)" />
                  </li>
                </ul>
                <nav v-if="users(item).length > 20" class="cleanup-pages" :aria-label="t('cleanup.usedBy', { count: users(item).length })">
                  <UiButton size="sm" variant="ghost" :disabled="pageOf('users:' + item.package_id) <= 1" @click="turn('users:' + item.package_id, -1)">{{ t('cleanup.previousPage') }}</UiButton>
                  <span>{{ pageOf('users:' + item.package_id) }} / {{ pageCount('', users(item).length, 20) }}</span>
                  <UiButton size="sm" variant="ghost" :disabled="pageOf('users:' + item.package_id) >= pageCount('', users(item).length, 20)" @click="turn('users:' + item.package_id, 1)">{{ t('cleanup.nextPage') }}</UiButton>
                </nav>
              </section>
            </li>
          </ul>
        </details>
      </template>

      <div v-if="result" class="cleanup-result">
        <p role="status" class="cleanup-group-title"><Icon name="check" />{{ t(`cleanup.status.${result.status}`) }}</p>
        <p v-if="result.status === 'completed'" class="cleanup-result-summary">{{ t('cleanup.resultSummary', { count: result.files.filter(f => f.status === 'trashed').length, size: formatSize(result.files.filter(f => f.status === 'trashed').reduce((total, f) => total + f.original.size, 0)) }) }}</p>
        <details v-if="result.errors.length" class="cleanup-list-section">
          <summary class="cleanup-detail-title">{{ t('cleanup.errorDetails') }}</summary>
          <p v-for="message in result.errors" :key="message" class="error cleanup-diagnostic-entry">{{ message }}</p>
        </details>
        <UiButton @click="viewBatch" variant="secondary">{{ t('cleanup.viewBatch') }}</UiButton>
      </div>
    </div>
    <template #footer>
      <span v-if="preview && !result && !busy" class="cleanup-footer-note">{{ t(preview.items.length ? 'cleanup.restoreHint' : decide.length ? 'cleanup.decideFirst' : 'cleanup.previewEmptyHelp') }}</span>
      <UiButton v-if="busy" @click="stop" variant="secondary">{{ t('cleanup.stop') }}</UiButton>
      <UiButton v-else-if="result" @click="close" variant="secondary">{{ t('common.close') }}</UiButton>
      <template v-else>
        <UiButton :disabled="confirming" @click="close" variant="ghost">{{ t('common.cancel') }}</UiButton>
        <UiButton ref="executeAction" :disabled="!preview?.items.length || confirming" @click="execute" variant="primary">
          {{ preview?.items.length ? t('cleanup.toTrashCount', { count: preview.items.length, size: formatSize(preview.total_bytes) }) : t('cleanup.toTrash') }}
        </UiButton>
      </template>
    </template>
  </UiModal>
  <PackageDetails v-if="detailItem" :key="detailItem.id" :item="detailItem" :image="thumbnails[detailItem.id.toLowerCase()]" :expanded="true" @close="detailItem = null" @update:expanded="value => { if (!value) detailItem = null }" />
</template>

<script setup lang="ts">
import CleanupResourceItem from './CleanupResourceItem.vue'
import PackageDetails from '@/components/packages/PackageDetails.vue'
import UiProgress from '@/components/ui/UiProgress.vue'
import { loadPackageThumbnails } from '@/utils/resourceCache'
import { useCleanupStore } from '@/stores/cleanup'
import type { ResourceDetailItem } from '@/types/package'
import UiButton from '@/components/ui/UiButton.vue'
import UiModal from '@/components/ui/UiModal.vue'
import UiCheckbox from '@/components/ui/UiCheckbox.vue'
import Icon from '@/components/ui/Icon.vue'
import { computed, nextTick, onMounted, onUnmounted, ref, watch } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import { useI18n } from 'vue-i18n'
import { useRouter } from 'vue-router'
import { useAppStore } from '@/stores/app'
import { formatSize } from '@/utils/bytes'
import { useConfirm } from '@/composables/useConfirm'
import { focusInitial } from '@/components/ui/overlay'
import type { CleanupPreview, CleanupBatch, CleanupProgress, CleanupItem } from '@/types/cleanup'
const props = defineProps<{
  packageIds: string[]
  packages?: ResourceDetailItem[]
  /** Lower-case ids of packages whose info (meta.json) could not be read. */
  brokenInfoIds?: string[]
}>()
const emit = defineEmits<{ close: []; completed: [batch: CleanupBatch] }>()
const { t } = useI18n()
const confirm = useConfirm()
const router = useRouter()
const app = useAppStore()
const cleanup = useCleanupStore()
const detailItem = ref<ResourceDetailItem | null>(null)
const thumbnails = ref<Record<string, string | null>>({})
const summaryMap = computed(() => new Map((props.packages ?? []).map(item => [item.id.toLowerCase(), item])))
function summary(id: string): ResourceDetailItem { return summaryMap.value.get(id.toLowerCase()) ?? { id, name: id, creator: id.split('.')[0], resource_types: [] } }
function openPackage(id: string) { if (!busy.value && !confirming.value) detailItem.value = summary(id) }
const dialog = ref<InstanceType<typeof UiModal> | null>(null)
const lower = (id: string) => id.toLowerCase()
// The selection can grow inside the dialog ("also clean up the packages that use it").
const requested = ref<string[]>([...new Set(props.packageIds.map(lower))])
const includeDependencies = ref(false)
const excludedIds = ref<string[]>([])
// Selected packages the person chose to remove even though something uses them.
const forcedIds = ref<string[]>([])
const confirming = ref(false)
const executeAction = ref<InstanceType<typeof UiButton> | null>(null)
const busy = ref(false)
const executing = ref(false)
const preview = ref<CleanupPreview | null>(null)
const result = ref<CleanupBatch | null>(null)
const error = ref('')
const progress = ref({ stage: 'loading_index', current: 0, total: 0 })
const progressLabel = computed(() => {
  if (executing.value)
    return progress.value.stage === 'moving'
      ? 'cleanup.executing'
      : ['verifying', 'verifying_bytes'].includes(progress.value.stage)
        ? 'cleanup.verifying'
        : 'cleanup.rechecking'
  if (progress.value.stage === 'building_index') return 'cleanup.buildingIndex'
  if (progress.value.stage === 'updating_index') return 'cleanup.updatingIndex'
  if (progress.value.stage === 'loading_index') return 'cleanup.loadingIndex'
  if (progress.value.stage === 'checking_files') return 'cleanup.checkingFiles'
  return 'cleanup.analyzing'
})
let requestId = ''
let epoch = 0
let disposed = false
let unlisten: UnlistenFn | undefined

// Codes that only say "something still uses it"; the person may override them.
const USAGE = new Set(['favorite', 'local', 'plan', 'referenced', 'referencedFile', 'protectedDependency', 'incomplete'])
const isExcluded = (item: CleanupItem) => excludedIds.value.includes(lower(item.package_id))
const decide = computed(() => preview.value?.blocked.filter(item => !isExcluded(item)) ?? [])
const kept = computed(() => [...(preview.value?.blocked ?? []), ...(preview.value?.retained ?? [])].filter(isExcluded))
const retained = computed(() => preview.value?.retained.filter(item => !isExcluded(item)) ?? [])
const suggested = computed(() => preview.value?.suggested ?? [])
const blockedByUnreadable = computed(() =>
  [...decide.value, ...retained.value].some(item => item.reasons.some(reason => reason.code === 'incomplete')),
)
function canForce(item: CleanupItem) {
  return !item.attached && item.reasons.length > 0 && item.reasons.every(reason => USAGE.has(reason.code))
}
const users = (item: CleanupItem) => [...new Set(item.reasons.filter(reason => reason.code === 'referenced').map(reason => reason.source))]
const addableUsers = (item: CleanupItem) => users(item).filter(id => !requested.value.includes(lower(id)))
function issueTag(issue: string) { return issue === 'damaged' ? 'damaged' : 'invalid' }
const brokenInfo = computed(() => new Set(props.brokenInfoIds ?? []))
const hasBrokenInfo = (item: CleanupItem) => !item.issue && !item.invalid_name && brokenInfo.value.has(lower(item.package_id))
/** One line per kind of reason; long lists of users are shortened. */
function reasonLines(item: CleanupItem) {
  const groups = new Map<string, string[]>()
  for (const reason of item.reasons) {
    if (reason.code === 'referenced') continue
    groups.set(reason.code, [...(groups.get(reason.code) ?? []), reason.source].filter(Boolean))
  }
  return [...groups].map(([code, sources]) => {
    const shown = sources.slice(0, 3).join(t('cleanup.listSeparator'))
    const source = sources.length > 3 ? t('cleanup.andMore', { names: shown, count: sources.length }) : shown
    return t(`cleanup.reasons.${code}`, { source, count: sources.length })
  })
}

// Pagination per list; long selections stay one page of rows at a time.
const pages = ref<Record<string, number>>({})
const pageCount = (_key: string, length: number, size = 100) => Math.max(1, Math.ceil(length / size))
const pageOf = (key: string) => pages.value[key] ?? 1
function turn(key: string, delta: number) { pages.value = { ...pages.value, [key]: Math.max(1, pageOf(key) + delta) } }
function slice<T>(key: string, list: T[], size = 100) {
  const page = Math.min(pageOf(key), pageCount(key, list.length, size))
  return list.slice((page - 1) * size, page * size)
}

async function stop() {
  if (requestId)
    await invoke('cancel_resource_cleanup', { requestId }).catch((e) => {
      error.value = String(e)
    })
}
function close() {
  if (executing.value) return
  detailItem.value = null
  void stop()
  dialog.value?.close()
  emit('close')
}
// reuseIndex: re-plan against the check this dialog already ran (keep, remove
// anyway, add users, dependency choice). Opening and "check again" always run a
// fresh check; executing re-verifies everything regardless.
async function analyze(reuseIndex = false) {
  if (busy.value || confirming.value || !app.vamRootPath) return
  await stop()
  const token = ++epoch
  requestId = crypto.randomUUID()
  busy.value = true
  error.value = ''
  progress.value = { stage: 'loading_index', current: 0, total: 0 }
  try {
    const value = await invoke<CleanupPreview>('preview_resource_cleanup', {
      vamRoot: app.vamRootPath,
      requestId,
      packageIds: requested.value,
      includeDependencies: includeDependencies.value,
      excludedPackageIds: excludedIds.value,
      reuseIndex,
      quick: false,
      forcePackageIds: forcedIds.value,
    })
    if (!disposed && token === epoch) preview.value = value
  } catch (e) {
    if (!disposed && token === epoch) {
      error.value = String(e)
      preview.value = null
    }
  } finally {
    if (!disposed && token === epoch) busy.value = false
  }
}
function keep(id: string) {
  excludedIds.value = [...new Set([...excludedIds.value, lower(id)])]
  forcedIds.value = forcedIds.value.filter(key => key !== lower(id))
  void analyze(true)
}
function reselect(id: string) {
  excludedIds.value = excludedIds.value.filter((key) => key !== lower(id))
  void analyze(true)
}
function force(id: string) {
  forcedIds.value = [...new Set([...forcedIds.value, lower(id)])]
  void analyze(true)
}
function unforce(id: string) {
  forcedIds.value = forcedIds.value.filter(key => key !== lower(id))
  void analyze(true)
}
function addUsers(item: CleanupItem) {
  requested.value = [...new Set([...requested.value, ...addableUsers(item).map(lower)])]
  void analyze(true)
}
async function execute() {
  if (!preview.value?.items.length || busy.value || confirming.value) return
  const candidate = preview.value
  const root = app.vamRootPath
  if (candidate.requires_confirmation) {
    const risky = candidate.items.filter(item => item.reasons.length > 0).map(item => item.package_id)
    confirming.value = true
    let accepted = false
    try {
      accepted = await confirm({
        title: t('cleanup.removalConfirmTitle'),
        message: t('cleanup.removalConfirm', {
          count: risky.length,
          names: risky.length > 3 ? t('cleanup.andMore', { names: risky.slice(0, 3).join(t('cleanup.listSeparator')), count: risky.length }) : risky.join(t('cleanup.listSeparator')),
        }),
        danger: true,
        confirmText: t('cleanup.toTrash'),
      })
    } finally {
      confirming.value = false
    }
    if (disposed || preview.value !== candidate || app.vamRootPath !== root) return
    if (!accepted) {
      await nextTick()
      const button = executeAction.value?.element
      if (button) focusInitial(button)
      return
    }
  }
  detailItem.value = null
  busy.value = true
  executing.value = true
  error.value = ''
  progress.value = { stage: 'rechecking', current: 0, total: 0 }
  try {
    const completed = await invoke<CleanupBatch>('execute_resource_cleanup', {
      planId: candidate.plan_id,
      confirmRisks: candidate.requires_confirmation ?? false,
    })
    if (!disposed && app.vamRootPath === root) {
      result.value = completed
      cleanup.recordBatch(completed)
      emit('completed', completed)
    }
    preview.value = null
  } catch (e) {
    error.value = String(e)
    preview.value = null
  } finally {
    busy.value = false
    executing.value = false
    if (!disposed && app.vamRootPath !== root) close()
  }
}
function viewBatch() {
  if (!result.value) return
  cleanup.focusBatch(result.value.id)
  close()
  void router.push('/trash')
}
let thumbnailEpoch = 0
const visiblePackageIds = computed(() => [...new Set([
  ...slice('decide', decide.value), ...slice('items', preview.value?.items ?? []),
  ...slice('kept', kept.value), ...slice('retained', retained.value),
].flatMap(item => [item.package_id, ...slice('users:' + item.package_id, users(item), 20)]))])
watch([visiblePackageIds, busy], () => {
  const generation = ++thumbnailEpoch
  if (busy.value || !visiblePackageIds.value.length) return
  void loadPackageThumbnails(visiblePackageIds.value.map(summary), (id, url) => {
    thumbnails.value = { ...thumbnails.value, [id.toLowerCase()]: url }
  }, () => !disposed && !busy.value && generation === thumbnailEpoch)
}, { immediate: true })
const byteProgress = computed(() => progress.value.stage.endsWith('_bytes'))
const progressText = computed(() => byteProgress.value
  ? t('cleanup.verifiedBytes', { current: formatSize(progress.value.current), total: formatSize(progress.value.total) })
  : progress.value.total ? progress.value.current + ' / ' + progress.value.total : '')
watch(includeDependencies, (include) => {
  if (!include) {
    const roots = new Set(requested.value)
    excludedIds.value = excludedIds.value.filter((id) => roots.has(id))
  }
  void analyze(true)
})
watch(() => app.vamRootPath, close)
onMounted(async () => {
  dialog.value?.showModal()
  try {
    const off = await listen<CleanupProgress>('cleanup-progress', (event) => {
      if (event.payload.request_id === requestId) progress.value = event.payload
    })
    if (disposed) {
      off()
      return
    }
    unlisten = off
    await analyze()
  } catch (e) {
    error.value = String(e)
  }
})
onUnmounted(() => {
  disposed = true
  epoch++
  unlisten?.()
  void stop()
})
</script>

<style scoped>
/* 长清单只在正文中滚动，保留共用弹窗的页头和底部操作区。 */
:global(.cleanup-preview-modal.ui-modal) {
  display: flex;
  flex-direction: column;
  overflow: hidden;
}
:global(.cleanup-preview-modal .ui-modal-header),
:global(.cleanup-preview-modal .ui-modal-description),
:global(.cleanup-preview-modal .ui-modal-footer) {
  flex-shrink: 0;
}
:global(.cleanup-preview-modal .ui-modal-body) {
  min-height: 0;
  overflow: auto;
  scrollbar-gutter: stable;
}
.cleanup-dialog-body {
  display: flex;
  flex-direction: column;
  gap: var(--space-panel);
  min-width: 0;
}
.preview-metrics {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(var(--resource-card-width), 1fr));
  gap: var(--space-stack);
}
.preview-metric { padding: var(--space-item) 0; }
.preview-metric dt {
  font-size: var(--text-sm);
  color: var(--text-secondary);
}
.preview-metric dd {
  margin-top: var(--space-1);
  font-size: var(--text-2xl);
  font-weight: var(--font-semibold);
  line-height: var(--leading-tight);
  overflow-wrap: anywhere;
}
.preview-metric .preview-metric-sub {
  font-size: var(--text-sm);
  font-weight: var(--font-normal);
  color: var(--text-secondary);
}
.cleanup-group {
  display: flex;
  flex-direction: column;
}
.cleanup-group-title {
  font-size: var(--text-md);
  font-weight: var(--font-semibold);
}
.cleanup-count {
  margin-left: var(--space-item);
  color: var(--text-tertiary);
  font-weight: var(--font-normal);
}
.cleanup-notice {
  display: flex;
  align-items: flex-start;
  gap: var(--space-stack);
  padding: var(--space-stack);
  border-radius: var(--radius-sm);
  font-size: var(--text-base);
  line-height: var(--leading-relaxed);
  overflow-wrap: anywhere;
}
.cleanup-notice > :deep(svg) {
  flex-shrink: 0;
  margin-top: var(--space-1);
}
.cleanup-notice-copy {
  flex: 1;
  min-width: 0;
}
.cleanup-notice-copy strong {
  display: block;
  margin-bottom: var(--space-1);
  font-weight: var(--font-semibold);
}
.warn {
  color: var(--color-warning);
  background: var(--color-warning-bg);
}
.warn .cleanup-list-section {
  margin-top: var(--space-item);
}
.cleanup-error {
  color: var(--color-error);
  background: var(--color-error-bg);
}
.cleanup-progress {
  display: flex;
  align-items: center;
  gap: var(--space-stack);
  padding: var(--space-panel);
  background: var(--bg-input);
  border-radius: var(--radius-lg);
  font-size: var(--text-md);
}
.cleanup-detail-title {
  font-size: var(--text-base);
  font-weight: var(--font-semibold);
  color: var(--text-secondary);
}
.cleanup-list-section + .cleanup-list-section {
  border-top: 1px solid var(--border-subtle);
  padding-top: var(--space-item);
}
.cleanup-item-list {
  margin-top: var(--space-item);
}
.cleanup-item {
  display: block;
  padding: var(--space-stack) 0;
  font-size: var(--text-base);
}
.cleanup-item + .cleanup-item {
  border-top: 1px solid var(--border-subtle);
}
.cleanup-item-copy {
  flex: 1;
  min-width: 0;
}
.cleanup-package-name {
  font-family: var(--font-mono);
  overflow-wrap: anywhere;
}
.cleanup-item-copy .cleanup-item-size {
  margin-left: var(--space-item);
}
.cleanup-reason {
  display: block;
  margin-top: var(--space-1);
  color: var(--text-secondary);
  font-size: var(--text-sm);
  overflow-wrap: anywhere;
}
.cleanup-tags {
  display: inline-flex;
  flex-wrap: wrap;
  gap: var(--space-1);
  margin-left: var(--space-item);
  vertical-align: middle;
}
.cleanup-tag {
  display: inline-flex;
  align-items: center;
  min-height: 22px;
  padding: 0 var(--space-2);
  border-radius: var(--radius-full);
  font-size: var(--text-xs);
  font-weight: var(--font-bold);
  background: var(--bg-control);
  color: var(--text-secondary);
}
.cleanup-tag.error {
  background: var(--color-error-bg);
  color: var(--color-error);
}
.cleanup-tag.warning {
  background: var(--color-warning-bg);
  color: var(--color-warning);
}
.cleanup-item-actions {
  display: flex;
  flex-wrap: wrap;
  gap: var(--space-item);
  margin-top: var(--space-item);
}
.force-action:hover:not(:disabled) {
  color: var(--color-error);
}
.cleanup-item-side {
  display: flex;
  align-items: center;
  flex-wrap: wrap;
  justify-content: flex-end;
  gap: var(--space-item);
}
.cleanup-item-size {
  color: var(--text-secondary);
  font-size: var(--text-sm);
}
.dependency-choice {
  display: flex;
  align-items: flex-start;
  padding: var(--space-stack) var(--space-panel);
  border-radius: var(--radius-lg);
  background: var(--bg-input);
}
.dependency-choice :deep(input) {
  flex-shrink: 0;
  margin-top: 2px;
}
.dependency-choice-copy {
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
  font-size: var(--text-base);
}
.dependency-choice-copy small {
  color: var(--text-secondary);
  font-size: var(--text-sm);
}
.cleanup-diagnostic-entry {
  padding: var(--space-item) 0;
  font-family: var(--font-mono);
  font-size: var(--text-sm);
  line-height: var(--leading-relaxed);
  color: var(--text-secondary);
  overflow-wrap: anywhere;
}
.error {
  color: var(--color-error);
}
.cleanup-pages {
  display: flex;
  justify-content: flex-end;
  align-items: center;
  flex-wrap: wrap;
  gap: var(--space-item);
  font-size: var(--text-sm);
  margin-top: var(--space-item);
}
.cleanup-result {
  display: flex;
  flex-direction: column;
  align-items: flex-start;
  gap: var(--space-stack);
}
.cleanup-footer-note {
  margin-right: auto;
  font-size: var(--text-sm);
  color: var(--text-tertiary);
}
.cleanup-consumers { margin: var(--space-item) 0 var(--space-stack); padding-left: var(--space-item); }
.cleanup-consumers h4 { margin: 0 0 var(--space-item); color: var(--text-secondary); font-size: var(--text-sm); font-weight: var(--font-bold); }
.cleanup-consumer-list { display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); gap: var(--space-1) var(--space-item); list-style: none; margin: 0; padding: 0; }
@media (max-width: 600px) { .cleanup-consumer-list { grid-template-columns: minmax(0, 1fr); } }
.cleanup-progress-copy { display: flex; flex: 1; flex-direction: column; gap: var(--space-item); min-width: 0; }
.cleanup-result-summary { color: var(--text-secondary); font-size: var(--text-sm); }
</style>
