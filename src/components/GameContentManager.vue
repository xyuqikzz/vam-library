<script setup lang="ts">
import { useResourceActions } from '@/composables/useResourceActions'
import UiMenu from '@/components/ui/UiMenu.vue'
import PackageCard from '@/components/packages/PackageCard.vue'
import PackageDetails from '@/components/packages/PackageDetails.vue'
import type { PackageDisplayItem, ResourceDetailItem } from '@/types/package'
import UiModal from '@/components/ui/UiModal.vue'
import UiDetailModal from '@/components/ui/UiDetailModal.vue'
import UiButton from '@/components/ui/UiButton.vue'
import UiInput from '@/components/ui/UiInput.vue'
import { computed, onMounted, onActivated, onDeactivated, onBeforeUnmount, ref, watch } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { useI18n } from 'vue-i18n'
import { useRoute } from 'vue-router'
import { useAppStore } from '@/stores/app'
import { useLocalLibraryStore } from '@/stores/localLibrary'
import type { GameContentKind, GameContentRef } from '@/types/presets'
import { previewUrl, resourceInvoke, resourceCacheRevision } from '@/utils/resourceCache'
import { compareDisplayNames } from '@/utils/nameSort'

interface ContentRef { packageId: string | null; path: string }
interface Item { source: ContentRef; key: string; name: string; alias: string | null; favorite: boolean; available: boolean }
interface Catalog { items: Item[]; warnings: string[]; directory: string }
interface Detail { json: Record<string, unknown>; image: string | null }
interface CopyReport { copied: number; skipped: number; errors: string[]; destination: string }
function record(value: unknown): Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value) ? value as Record<string, unknown> : {}
}
const props = defineProps<{ kind: GameContentKind }>()
const emit = defineEmits<{ loaded: [packageIds: string[]]; busyChange: [busy: boolean] }>()
const { t } = useI18n()
const app = useAppStore()
const library = useLocalLibraryStore()
const route = useRoute()
const items = ref<Item[]>([])
const warnings = ref<string[]>([])
const directory = ref('')
const copyDialog = ref<InstanceType<typeof UiModal> | null>(null)
const gallery = ref<HTMLElement | null>(null)
const pendingCopy = ref<{ root: string; source: ContentRef | null; kind: GameContentKind; count: number; directory: string } | null>(null)
const loading = ref(false)
const busy = ref(false)
const sceneBusy = ref(false)
const error = ref('')
const message = ref('')
const report = ref<CopyReport | null>(null)
const selected = ref<Item | null>(null)
const selectedResource = ref<ResourceDetailItem | null>(null)
const sceneExpanded = ref(false)
const packageDetails = ref<InstanceType<typeof PackageDetails> | null>(null)
const actionBusy = computed(() => busy.value || sceneBusy.value)
const { cardMenuOpen, cardMenuAnchor, cardMenuPoint, cardMenuItems, openDetails, openCardMenu, queueSceneAction, handleCardAction, packageShortcut } = useResourceActions({
  selected, details: packageDetails, busy: actionBusy, expanded: sceneExpanded,
  key: item => item.key, select, isScene: item => props.kind === 'scene' && item.available,
  isPackage: item => !!item.source.packageId,
  canDelete: item => !!item.source.packageId && item.available,
})
const packageById = computed(() => new Map((library.packages || []).map(pkg => [pkg.id.toLowerCase(), pkg])))
function cardItem(item: Item) {
  const pkg = item.source.packageId ? packageById.value.get(item.source.packageId.toLowerCase()) : undefined
  return { id: item.key, name: item.alias || item.name,
    creator: pkg?.creator || item.source.packageId?.split('.')[0] || t('gameContent.local'),
    version: pkg?.version, size_bytes: pkg?.size_bytes,
    resource_types: [props.kind === 'scene' ? 'scene' : ['appearance', 'clothing', 'hair'].includes(props.kind) ? props.kind : 'other'] }
}
const detail = ref<Detail | null>(null)
const detailLoading = ref(false)
const detailError = ref('')
const name = ref('')
const filter = ref('all')
const favoritesOnly = ref(props.kind === 'scene')
const query = ref('')
const page = ref(1)
const images = ref<Record<string, string | null>>({})
const imageErrors = ref<Record<string, string>>({})
const imageRequests = new Set<string>()
let imagePumpRunning = false
let listEpoch = 0
let detailEpoch = 0
let imageEpoch = 0
let active = false
const pageSize = 36
const isScene = computed(() => props.kind === 'scene')
const title = computed(() => t(isScene.value ? 'gameContent.scenes' : `gameContent.presetTypes.${props.kind}`))
const packagePresets = computed(() => items.value.filter(i => i.source.packageId && i.available).length)
const favoriteCount = computed(() => items.value.filter(i => i.favorite).length)
const filtered = computed(() => {
  const terms = `${query.value} ${app.searchQuery}`.trim().toLocaleLowerCase().split(/\s+/).filter(Boolean)
  return items.value.filter(i => (!favoritesOnly.value || i.favorite)
    && (filter.value === 'all' || (filter.value === 'local' ? !i.source.packageId : !!i.source.packageId))
    && terms.every(q => `${i.name} ${i.alias || ''} ${i.source.packageId || ''} ${i.source.path}`.toLocaleLowerCase().includes(q)))
    .sort((a, b) => isScene.value
      ? (a.alias || a.name).localeCompare(b.alias || b.name)
      : compareDisplayNames(a.alias || a.name, b.alias || b.name))
})
const pages = computed(() => Math.max(1, Math.ceil(filtered.value.length / pageSize)))
const visible = computed(() => filtered.value.slice((page.value - 1) * pageSize, page.value * pageSize))
const sections = computed(() => {
  const json = detail.value?.json
  if (!json) return []
  const storables = Array.isArray(json.storables) ? json.storables : []
  const geometry = record(storables.find(s => record(s).id === 'geometry'))
  const result: { name: string; value: string }[] = []
  if (geometry) {
    if (geometry.character) result.push({ name: t('gameContent.character'), value: String(geometry.character) })
    for (const key of ['clothing', 'hair', 'morphs']) {
      const entries = geometry[key]
      if (Array.isArray(entries)) result.push({ name: t(`gameContent.${key}`), value: entries.map(e => {
        if (typeof e !== 'object' || !e) return String(e)
        const entry = record(e)
        return `${entry.name || entry.id || entry.uid || '—'}${entry.enabled === false || entry.enabled === 'false' ? ` (${t('gameContent.disabled')})` : ''}${entry.value !== undefined ? `: ${entry.value}` : ''}`
      }).join('\n') || '—' })
    }
  }
  if (Array.isArray(json.atoms)) result.push({ name: t('gameContent.atoms'), value: json.atoms.map(a => `${record(a).id || '—'} · ${record(a).type || '—'}`).join('\n') })
  if (storables.length) result.push({ name: t('gameContent.sections'), value: storables.map(s => record(s).id || '—').join(', ') })
  return result
})

async function refresh(reloadImages = false) {
  if (reloadImages) { ++imageEpoch; images.value = {}; imageErrors.value = {} }
  const epoch = ++listEpoch
  const root = app.vamRootPath
  if (!root) { items.value = []; loading.value = false; return }
  loading.value = true
  error.value = ''
  try {
    const result = await invoke<Catalog>('list_game_contents', { vamRoot: root, kind: props.kind })
    if (epoch !== listEpoch || root !== app.vamRootPath) return
    items.value = result.items
    warnings.value = result.warnings
    directory.value = result.directory
    emit('loaded', [...new Set(result.items.flatMap(i => i.source.packageId ? [i.source.packageId] : []))])
    page.value = Math.min(page.value, pages.value)
    if (selected.value) selected.value = items.value.find(i => i.key === selected.value?.key) || null
    const reveal = route.query.appearance
    if (props.kind === 'appearance' && route.path === '/appearances' && typeof reveal === 'string') {
      const target = items.value.find(i => !i.source.packageId && i.source.path === reveal)
      if (target && selected.value?.key !== target.key) {
        query.value = target.name; filter.value = 'local'; app.searchQuery = ''; page.value = 1
        void select(target)
      }
    }
  } catch (e) { if (epoch === listEpoch) error.value = String(e) }
  finally { if (epoch === listEpoch) loading.value = false }
}
async function select(item: Item) {
  if (busy.value || sceneBusy.value) return
  packageDetails.value?.resetNavigation()
  if (isScene.value && selected.value?.key === item.key && selectedResource.value && !detailError.value) return
  const epoch = ++detailEpoch
  const root = app.vamRootPath
  selected.value = item
  name.value = item.alias || item.name
  detail.value = null; detailError.value = ''; detailLoading.value = false
  sceneExpanded.value = false; selectedResource.value = null
  if (isScene.value) {
    selectedResource.value = { id: item.source.packageId || item.key, name: item.alias || item.name,
      creator: item.source.packageId?.split('.')[0] || t('gameContent.local'), resource_types: ['scene'] }
    if (!item.source.packageId || !item.available) return
    const cached = packageById.value.get(item.source.packageId.toLowerCase())
    if (cached) { selectedResource.value = cached; return }
    detailLoading.value = true
    try {
      const pkg = await invoke<PackageDisplayItem | null>('get_package_summary', { packageId: item.source.packageId })
      if (epoch === detailEpoch && root === app.vamRootPath && pkg) selectedResource.value = pkg
    } catch (e) { if (epoch === detailEpoch && root === app.vamRootPath) detailError.value = t('ui.loadFailed', { error: String(e) }) }
    finally { if (epoch === detailEpoch) detailLoading.value = false }
    return
  }
  if (!item.available) return
  detailLoading.value = true
  try {
    const result = await invoke<Detail>('get_game_content_detail', { vamRoot: root, source: item.source })
    if (epoch === detailEpoch && root === app.vamRootPath) detail.value = result
  } catch (e) { if (epoch === detailEpoch && root === app.vamRootPath) detailError.value = String(e) }
  finally { if (epoch === detailEpoch) detailLoading.value = false }
}
function closeDetail() { ++detailEpoch; selected.value = null; selectedResource.value = null; detail.value = null; detailLoading.value = false; sceneExpanded.value = false }
function onFavoriteChange(source: GameContentRef, favorite: boolean) {
  const item = items.value.find(item => item.source.packageId === source.packageId && item.source.path === source.path)
  if (item) item.favorite = favorite
  void refresh()
}
async function mutate(work: (root: string) => Promise<void>) {
  if (busy.value || !app.vamRootPath) return
  const root = app.vamRootPath
  busy.value = true
  error.value = ''; message.value = ''; report.value = null
  try { await work(root); if (root === app.vamRootPath) await refresh() }
  catch (e) { if (root === app.vamRootPath) error.value = String(e) }
  finally { busy.value = false }
}
function requestCopy(source: ContentRef | null = null) {
  if (busy.value || !app.vamRootPath || !directory.value) return
  pendingCopy.value = { root: app.vamRootPath, kind: props.kind, source, count: source ? 1 : packagePresets.value, directory: directory.value }
  copyDialog.value?.showModal()
}
function cancelCopy() {
  copyDialog.value?.close()
  pendingCopy.value = null
}
async function confirmCopy() {
  const request = pendingCopy.value
  if (!request) return
  cancelCopy()
  if (request.root !== app.vamRootPath || request.kind !== props.kind) return
  await mutate(async root => {
    const result = await invoke<CopyReport>('copy_game_presets', { vamRoot: root, source: request.source, kind: request.kind })
    if (root === app.vamRootPath) report.value = result
  })
}
async function saveName() {
  const item = selected.value
  if (!item) return
  const value = name.value
  await mutate(async root => {
    await invoke('rename_game_preset', { vamRoot: root, source: item.source, name: value })
    if (root === app.vamRootPath) closeDetail()
    if (root === app.vamRootPath) message.value = t('gameContent.saved')
  })
}
async function loadImages() {
  if (!active || imagePumpRunning) return
  const epoch = imageEpoch
  const root = app.vamRootPath
  imagePumpRunning = true
  try {
    await Promise.all(Array.from({ length: 4 }, async () => {
      while (active && epoch === imageEpoch) {
        const item = visible.value.find(i => needsImage(i) && !imageRequests.has(i.key))
        if (!item) break
        imageRequests.add(item.key)
        try {
          const image = previewUrl(await resourceInvoke<string | null>('get_game_content_image', { vamRoot: root, source: item.source }))
          // Cache null too: returning to the app must not repeatedly load missing thumbnails.
          if (epoch === imageEpoch) images.value[item.key] = image
        } catch (e) { if (epoch === imageEpoch) imageErrors.value[item.key] = String(e) }
        finally { imageRequests.delete(item.key) }
      }
    }))
  } finally {
    imagePumpRunning = false
    if (active && visible.value.some(needsImage)) void loadImages()
  }
}
function needsImage(item: Item) {
  return item.available && !Object.prototype.hasOwnProperty.call(images.value, item.key) && !imageErrors.value[item.key]
}
async function rescan() {
  if (!app.vamRootPath || app.isScanning || busy.value) return
  try { await app.startScan(app.vamRootPath); await refresh(true) } catch (e) { error.value = String(e) }
}
function onFocus() { if (active && !busy.value && !sceneBusy.value && !loading.value && !pendingCopy.value) void refresh() }
watch([query, filter, favoritesOnly, () => app.searchQuery], () => { page.value = 1 })
watch([page, query, filter, favoritesOnly, () => app.searchQuery], () => {
  if (gallery.value) gallery.value.scrollTop = 0
}, { flush: 'post' })
watch(visible, () => { void loadImages() })
watch(() => busy.value || sceneBusy.value, value => emit('busyChange', value), { flush: 'sync' })
watch(() => app.vamRootPath, () => {
  cancelCopy()
  ++listEpoch; ++imageEpoch; closeDetail(); items.value = []; images.value = {}; imageErrors.value = {}; report.value = null; message.value = ''; warnings.value = []; error.value = ''
  directory.value = ''; emit('loaded', [])
  if (active) void refresh()
})
watch([() => library.revision, resourceCacheRevision], () => { if (active && !busy.value && !sceneBusy.value) void refresh(true) })
function activate() { if (active) return; active = true; window.addEventListener('focus', onFocus); void refresh() }
onMounted(activate)
onActivated(activate)
onDeactivated(() => { active = false; cancelCopy(); window.removeEventListener('focus', onFocus) })
onBeforeUnmount(() => { active = false; cancelCopy(); ++listEpoch; ++detailEpoch; ++imageEpoch; window.removeEventListener('focus', onFocus) })
</script>

<template>
  <section class="game-content" :aria-label="title">
    <p v-if="!app.vamRootPath" class="notice">
      {{ t('gameContent.noGame') }}
      <router-link to="/settings">{{ t('sidebar.settings') }}</router-link>
    </p>
    <div v-else class="content-toolbar">
      <UiInput
        v-model="query"
        type="search"
        :placeholder="t('gameContent.search')"
        :aria-label="t('gameContent.search')"
        search
      />
      <select v-model="filter" :aria-label="t('gameContent.source')">
        <option value="all">{{ t('gameContent.allSources') }}</option>
        <option value="package">{{ t('gameContent.package') }}</option>
        <option value="local">{{ t('gameContent.local') }}</option>
      </select>
      <UiButton
        v-if="isScene"
        :class="{ 'filter-active': favoritesOnly }"
        :aria-pressed="favoritesOnly"
        :title="t('gameContent.favoriteFilterHelp')"
        @click="favoritesOnly = !favoritesOnly"
        variant="secondary"
        icon="star"
      >
        {{ t('gameContent.favorites', { count: favoriteCount }) }}
      </UiButton>
      <UiButton :disabled="loading || busy" @click="refresh(true)" variant="secondary">
        {{ t('gameContent.refresh') }}
      </UiButton>
      <UiButton :disabled="app.isScanning || busy" @click="rescan" variant="secondary">
        {{ t(app.isScanning ? 'toolbar.scanning' : 'gameContent.scan') }}
      </UiButton>
      <UiButton
        v-if="!isScene"
        class="primary"
        :disabled="busy || loading || !packagePresets || app.isScanning"
        @click="requestCopy()"
        variant="primary"
      >
        {{ t(busy ? 'gameContent.working' : 'gameContent.copyLocal') }}
      </UiButton>
    </div>
    <p v-if="error" role="alert" class="error notice">{{ error }}</p>
    <p v-if="message" role="status" class="notice">{{ message }}</p>
    <div v-if="report" class="notice" role="status">
      <strong>{{ t('gameContent.copyResult', report) }}</strong>
      <p>{{ report.destination }}</p>
      <details v-if="report.errors.length">
        <summary>{{ t('gameContent.failures', { count: report.errors.length }) }}</summary>
        <ul>
          <li v-for="(issue, i) in report.errors" :key="i">{{ issue }}</li>
        </ul>
      </details>
    </div>
    <details v-if="warnings.length" class="notice">
      <summary>{{ t('gameContent.warnings', { count: warnings.length }) }}</summary>
      <ul>
        <li v-for="(warning, i) in warnings" :key="i">{{ warning }}</li>
      </ul>
    </details>
    <p class="count" role="status">
      {{ loading ? t('common.loading') : t(isScene ? (favoritesOnly ? 'gameContent.favoriteCount' : 'gameContent.sceneCount') : 'gameContent.count', { count: filtered.length }) }}
    </p>
    <div class="content-body" :aria-busy="loading || busy">
      <div class="content-list">
        <div v-if="visible.length" ref="gallery" class="content-grid resource-grid">
          <PackageCard
            v-for="item in visible"
            :key="item.key"
            :item="cardItem(item)"
            :image="images[item.key]"
            :selected="selected?.key === item.key"
            :disabled="actionBusy"
            :can-launch="isScene && item.available"
            @launch="queueSceneAction(item, 'launch')"
            @keydown="isScene && packageShortcut($event, item)"
            :favorite="item.favorite"
            @select="isScene ? openDetails(item) : select(item)"
            @details="isScene ? openDetails(item) : select(item)"
            @more="isScene ? openCardMenu(item, $event) : select(item)"
          >
            <template #badge>
              <span
                v-if="!item.available || imageErrors[item.key]"
                class="preview-status"
                :title="imageErrors[item.key]"
              >
                {{ t(!item.available ? 'gameContent.missing' : 'gameContent.imageFailed') }}
              </span>
            </template>
          </PackageCard>
        </div>
        <div v-else-if="!loading && app.vamRootPath" class="empty">
          <p>
            {{ t(isScene && favoritesOnly ? (favoriteCount ? 'gameContent.noMatchingFavorites' : 'gameContent.noFavorites') : 'gameContent.empty') }}
          </p>
          <UiButton
            v-if="isScene && favoritesOnly && !favoriteCount"
            class="browse-scenes"
            @click="favoritesOnly = false; query = ''; filter = 'all'; app.searchQuery = ''"
            variant="secondary"
          >
            {{ t('gameContent.browseScenes') }}
          </UiButton>
        </div>
        <nav v-if="pages > 1" class="pagination" :aria-label="t('gameContent.pagination')">
          <UiButton :disabled="page <= 1" @click="page--" variant="secondary">
            {{ t('gameContent.previous') }}
          </UiButton>
          <span>{{ page }} / {{ pages }}</span>
          <UiButton :disabled="page >= pages" @click="page++" variant="secondary">
            {{ t('gameContent.next') }}
          </UiButton>
        </nav>
      </div>
      <PackageDetails ref="packageDetails" :show-quick-delete="!!selected?.source.packageId && selected.available" @package-deleted="closeDetail(); refresh(true)" v-if="isScene && selected && selectedResource" :key="selected.key"
        :item="selectedResource" :scene="selected" :image="images[selected.key]" :load-error="detailError" :loading="detailLoading" :vam-root="app.vamRootPath"
        v-model:expanded="sceneExpanded" @busy-change="sceneBusy = $event" @close="closeDetail"
        @favorite-change="onFavoriteChange" />
      <UiDetailModal v-if="selected && !isScene" class="preset-detail-modal"
        :label="selected.alias || selected.name" :fallback-image="detail?.image || images[selected.key]"
        :content-key="selected.key" :close-label="t('gameContent.close')" :busy="busy" @close="closeDetail">
        <div class="inspector">
        <span class="preset-type">{{ t(`gameContent.presetTypes.${kind}`) }}</span>
        <h2 class="preset-title">{{ selected.alias || selected.name }}</h2>
        <p class="path">
          {{ selected.source.packageId ? `${selected.source.packageId}:/` : ''
          }}{{ selected.source.path }}
        </p>
        <p v-if="detailLoading" role="status">{{ t('common.loading') }}</p>
        <p v-if="detailError" role="alert" class="error">{{ detailError }}</p>
        <p v-if="!selected.available" class="notice">{{ t('gameContent.missingHelp') }}</p>
        <template v-if="!selected.source.packageId">
          <form @submit.prevent="saveName">
            <label for="content-name">
              {{ t('gameContent.filename') }}
            </label>
            <div class="rename-row">
              <UiInput
                id="content-name"
                v-model="name"
                maxlength="120"
                required
              />
              <UiButton
                :disabled="busy || !selected.available"
                type="submit"
                variant="secondary"
                size="lg"
              >
                {{ t('gameContent.save') }}
              </UiButton>
            </div>
          </form>
          <p class="help">{{ t('gameContent.renameHelp') }}</p>
        </template>
        <template v-else>
          <p class="help">{{ t('gameContent.packageRenameHelp') }}</p>
          <UiButton
            class="primary"
            :disabled="busy || !selected.available"
            @click="requestCopy(selected.source)"
            variant="primary"
            size="lg"
          >
            {{ t('gameContent.copyOne') }}
          </UiButton>
        </template>
        <dl class="sections">
          <template v-for="section in sections" :key="section.name">
            <dt>{{ section.name }}</dt>
            <dd>{{ section.value }}</dd>
          </template>
        </dl>
        <details v-if="detail">
          <summary>{{ t('gameContent.raw') }}</summary>
          <pre>{{ JSON.stringify(detail.json, null, 2) }}</pre>
        </details>
        </div>
      </UiDetailModal>
    </div>
    <UiMenu :label="t('ui.menu')" v-model:open="cardMenuOpen" :items="cardMenuItems" :anchor="cardMenuAnchor" :x="cardMenuPoint.x" :y="cardMenuPoint.y" placement="bottom-end" @select="handleCardAction" />
    <UiModal
      ref="copyDialog"
      :title="t('gameContent.copyLocal')"
      aria-describedby="copy-description"
      @close="pendingCopy = null"
      @cancel="cancelCopy"
      imperative
      :show-close="false"
    >
      <template v-if="pendingCopy">
        <p id="copy-description">
          {{ t(pendingCopy.source ? 'gameContent.copyOneConfirm' : 'gameContent.copyAllConfirm', { count: pendingCopy.count, type: title }) }}
        </p>
        <p class="copy-target">{{ pendingCopy.directory }}/VAM Library</p>
        <p class="copy-safety">{{ t('gameContent.copySafety') }}</p>
      </template>
      <template #footer>
        <UiButton autofocus @click="cancelCopy" variant="secondary">
          {{ t('gameContent.cancel') }}
        </UiButton>
        <UiButton @click="confirmCopy" variant="primary">
          {{ t('gameContent.confirmCopy') }}
        </UiButton>
      </template>
    </UiModal>
  </section>
</template>

<style scoped>
.game-content {
  display: flex;
  flex-direction: column;
  flex: 1;
  gap: var(--space-3);
  min-width: 0;
  min-height: 0;
  overflow: hidden;
}
.game-content > :not(.content-body):not(dialog) {
  flex-shrink: 0;
}
.content-toolbar,
.pagination,
.rename-row {
  display: flex;
  align-items: center;
  gap: var(--space-3);
}
p,
.help {
  color: var(--text-secondary);
  font-size: var(--text-sm);
  line-height: 1.7;
}
.content-toolbar {
  flex-wrap: wrap;
  gap: var(--space-item);
}
.content-toolbar .ui-input-field {
  flex: 0 1 280px;
  min-width: 180px;
}
.content-toolbar select {
  flex: 0 0 140px;
}
.content-toolbar button {
  flex: 0 0 auto;
  white-space: nowrap;
}
.content-toolbar > .primary {
  margin-left: auto;
}
.content-toolbar .filter-active {
  color: var(--accent-primary);
  background: var(--accent-muted);
}
.content-toolbar .filter-active:hover {
  background: color-mix(in srgb, var(--accent-primary) 24%, transparent);
}
.notice {
  border: none;
  background: var(--bg-elevated);
  border-radius: var(--radius-thumbnail);
  padding: var(--space-3);
  font-size: var(--text-sm);
  overflow-wrap: anywhere;
  max-height: 140px;
  overflow-y: auto;
}
.notice li {
  margin-top: var(--space-2);
}
.error {
  color: var(--color-error);
}
/* Keep page controls outside the scrolling panes; their height must never be compressed by the gallery. */
.content-body {
  position: relative;
  flex: 1;
  min-height: 0;
  display: grid;
  grid-template-columns: minmax(0, 1fr);
  grid-template-rows: minmax(0, 1fr);
  gap: var(--space-4);
  overflow: hidden;
}
.count {
  flex-shrink: 0;
  margin-bottom: calc(-1 * var(--space-1));
}
.content-list {
  display: flex;
  flex-direction: column;
  min-width: 0;
  min-height: 0;
}
/* The floating inspector leaves the grid dimensions unchanged. */
.content-grid {
  flex: 1;
  min-height: 0;
  overflow-y: auto;
  overflow-x: hidden;
  overscroll-behavior: contain;
  scrollbar-gutter: stable;
  grid-auto-rows: max-content;
  align-content: start;
  align-items: start;
  padding: var(--focus-room);
}
.pagination {
  flex-shrink: 0;
  justify-content: center;
  padding-top: var(--space-3);
  font-size: var(--text-sm);
}
.empty {
  flex: 1;
  min-height: 0;
  overflow: auto;
  padding: var(--space-10) var(--space-3);
  text-align: center;
}
.browse-scenes {
  margin-top: var(--space-3);
}
.inspector {
  display: flex;
  flex-direction: column;
  gap: var(--space-stack);
  min-width: 0;
}
.inspector > * {
  flex-shrink: 0;
}
.path {
  color: var(--text-tertiary);
  font-family: var(--font-mono);
  font-size: var(--text-xs);
  line-height: 1.6;
  overflow-wrap: anywhere;
  user-select: text;
}
.preset-type {
  align-self: flex-start;
  display: inline-flex;
  align-items: center;
  min-height: 22px;
  padding: 0 var(--space-2);
  border-radius: var(--radius-full);
  background: var(--bg-input);
  font-size: var(--text-xs);
  font-weight: var(--font-bold);
}
.preset-title {
  font-size: var(--text-3xl);
  font-weight: var(--font-heavy);
  overflow-wrap: anywhere;
}
label {
  display: block;
  font-size: var(--text-sm);
  margin-bottom: var(--space-2);
}
.rename-row .ui-input-field {
  min-width: 0;
  flex: 1;
}
.sections dt {
  margin: var(--space-3) 0 var(--space-2);
  font-size: var(--text-sm);
  font-weight: 600;
}
.sections dd {
  white-space: pre-wrap;
  overflow-wrap: anywhere;
  font-size: var(--text-xs);
  color: var(--text-secondary);
  max-height: 240px;
  overflow: auto;
  user-select: text;
}
summary {
  cursor: pointer;
  font-size: var(--text-sm);
}
pre {
  margin-top: var(--space-3);
  max-height: 350px;
  overflow: auto;
  font-size: var(--text-xs);
  user-select: text;
}
.copy-target {
  margin-top: var(--space-3);
  padding: var(--space-2) var(--space-3);
  border-radius: var(--radius-md);
  background: var(--bg-input);
  color: var(--text-primary);
  font-family: var(--font-mono);
  font-size: var(--text-xs);
  overflow-wrap: anywhere;
  user-select: text;
}
.copy-safety {
  margin-top: var(--space-3);
  color: var(--text-tertiary);
  font-size: var(--text-xs);
}
.preview-status {
  position: absolute;
  top: 38px;
  left: 10px;
  z-index: 2;
  background: var(--bg-on-image);
  border-radius: var(--radius-full);
  color: var(--color-warning);
  font-size: var(--text-xs);
  padding: var(--space-1) var(--space-2);
}
</style>
