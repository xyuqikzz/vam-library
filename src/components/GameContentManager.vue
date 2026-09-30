<script setup lang="ts">
import { computed, onMounted, onActivated, onDeactivated, onBeforeUnmount, ref, watch } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { useI18n } from 'vue-i18n'
import { useRoute } from 'vue-router'
import { useAppStore } from '@/stores/app'
import { useLocalLibraryStore } from '@/stores/localLibrary'
import type { GameContentKind } from '@/types/presets'

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
const copyDialog = ref<HTMLDialogElement | null>(null)
const gallery = ref<HTMLElement | null>(null)
const inspectorPane = ref<HTMLElement | null>(null)
const pendingCopy = ref<{ root: string; source: ContentRef | null; kind: GameContentKind; count: number; directory: string } | null>(null)
const loading = ref(false)
const busy = ref(false)
const error = ref('')
const message = ref('')
const report = ref<CopyReport | null>(null)
const selected = ref<Item | null>(null)
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
    .sort((a, b) => (a.alias || a.name).localeCompare(b.alias || b.name))
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
  if (busy.value) return
  const epoch = ++detailEpoch
  selected.value = item
  name.value = item.alias || item.name
  detail.value = null
  detailError.value = ''
  detailLoading.value = false
  if (!item.available) return
  detailLoading.value = true
  try {
    const result = await invoke<Detail>('get_game_content_detail', { vamRoot: app.vamRootPath, source: item.source })
    if (epoch === detailEpoch) detail.value = result
  } catch (e) { if (epoch === detailEpoch) detailError.value = String(e) }
  finally { if (epoch === detailEpoch) detailLoading.value = false }
}
function closeDetail() { ++detailEpoch; selected.value = null; detail.value = null; detailLoading.value = false }
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
async function favorite(item: Item) {
  await mutate(async root => {
    await invoke('set_game_scene_favorite', { vamRoot: root, source: item.source, favorite: !item.favorite })
    if (root === app.vamRootPath) message.value = t(item.favorite ? 'gameContent.removed' : 'gameContent.added')
  })
}
async function saveName() {
  const item = selected.value
  if (!item) return
  const value = name.value
  await mutate(async root => {
    if (isScene.value) {
      await invoke('set_game_scene_name', { vamRoot: root, source: item.source, name: value })
    } else {
      await invoke('rename_game_preset', { vamRoot: root, source: item.source, name: value })
      if (root === app.vamRootPath) closeDetail()
    }
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
          const image = await invoke<string | null>('get_game_content_image', { vamRoot: root, source: item.source })
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
function onFocus() { if (active && !busy.value && !loading.value && !pendingCopy.value) void refresh() }
watch([query, filter, favoritesOnly, () => app.searchQuery], () => { page.value = 1 })
watch([page, query, filter, favoritesOnly, () => app.searchQuery], () => {
  if (gallery.value) gallery.value.scrollTop = 0
}, { flush: 'post' })
watch(() => selected.value?.key, () => {
  if (inspectorPane.value) inspectorPane.value.scrollTop = 0
}, { flush: 'post' })
watch(visible, () => { void loadImages() })
watch(busy, value => emit('busyChange', value), { flush: 'sync' })
watch(() => app.vamRootPath, () => {
  cancelCopy()
  ++listEpoch; ++imageEpoch; closeDetail(); items.value = []; images.value = {}; imageErrors.value = {}; report.value = null; message.value = ''; warnings.value = []; error.value = ''
  directory.value = ''; emit('loaded', [])
  if (active) void refresh()
})
watch(() => library.revision, () => { if (active && !busy.value) void refresh(true) })
function activate() { if (active) return; active = true; window.addEventListener('focus', onFocus); void refresh() }
onMounted(activate)
onActivated(activate)
onDeactivated(() => { active = false; cancelCopy(); window.removeEventListener('focus', onFocus) })
onBeforeUnmount(() => { active = false; cancelCopy(); ++listEpoch; ++detailEpoch; ++imageEpoch; window.removeEventListener('focus', onFocus) })
</script>

<template>
  <section class="game-content" :aria-label="title">
    <p v-if="!app.vamRootPath" class="notice">{{ t('gameContent.noGame') }} <router-link to="/settings">{{ t('sidebar.settings') }}</router-link></p>
    <div v-else class="content-toolbar">
      <input v-model="query" type="search" :placeholder="t('gameContent.search')" :aria-label="t('gameContent.search')" />
      <select v-model="filter" :aria-label="t('gameContent.source')">
        <option value="all">{{ t('gameContent.allSources') }}</option>
        <option value="package">{{ t('gameContent.package') }}</option>
        <option value="local">{{ t('gameContent.local') }}</option>
      </select>
      <button v-if="isScene" :class="{ 'filter-active': favoritesOnly }" :aria-pressed="favoritesOnly" :title="t('gameContent.favoriteFilterHelp')" @click="favoritesOnly = !favoritesOnly">★ {{ t('gameContent.favorites', { count: favoriteCount }) }}</button>
      <button :disabled="loading || busy" @click="refresh(true)">{{ t('gameContent.refresh') }}</button>
      <button :disabled="app.isScanning || busy" @click="rescan">{{ t(app.isScanning ? 'toolbar.scanning' : 'gameContent.scan') }}</button>
      <button v-if="!isScene" class="primary" :disabled="busy || loading || !packagePresets || app.isScanning" @click="requestCopy()">
        {{ t(busy ? 'gameContent.working' : 'gameContent.copyLocal') }}
      </button>
    </div>
    <p v-if="error" role="alert" class="error notice">{{ error }}</p>
    <p v-if="message" role="status" class="notice">{{ message }}</p>
    <div v-if="report" class="notice" role="status">
      <strong>{{ t('gameContent.copyResult', report) }}</strong><p>{{ report.destination }}</p>
      <details v-if="report.errors.length"><summary>{{ t('gameContent.failures', { count: report.errors.length }) }}</summary><ul><li v-for="(issue, i) in report.errors" :key="i">{{ issue }}</li></ul></details>
    </div>
    <details v-if="warnings.length" class="notice"><summary>{{ t('gameContent.warnings', { count: warnings.length }) }}</summary><ul><li v-for="(warning, i) in warnings" :key="i">{{ warning }}</li></ul></details>
    <div class="content-body" :class="{ inspecting: selected }" :aria-busy="loading || busy">
      <div class="content-list">
        <p class="count" role="status">{{ loading ? t('common.loading') : t(isScene ? (favoritesOnly ? 'gameContent.favoriteCount' : 'gameContent.sceneCount') : 'gameContent.count', { count: filtered.length }) }}</p>
        <div v-if="visible.length" ref="gallery" class="content-grid">
          <article v-for="item in visible" :key="item.key" :class="['content-card', { selected: selected?.key === item.key }]">
            <button class="card-main" :disabled="busy" @click="select(item)">
              <div class="preview">
                <img v-if="images[item.key]" data-resource-preview :src="images[item.key] || undefined" :alt="item.alias || item.name" loading="lazy" />
                <span v-else>{{ t('packages.noThumbnail') }}</span>
                <span v-if="!item.available" class="preview-status">{{ t('gameContent.missing') }}</span>
                <span v-else-if="imageErrors[item.key]" class="preview-status" :title="imageErrors[item.key]">{{ t('gameContent.imageFailed') }}</span>
              </div>
              <div class="card-text"><strong :title="item.alias || item.name">{{ item.alias || item.name }}</strong><span :title="item.source.packageId || t('gameContent.local')">{{ item.source.packageId || t('gameContent.local') }}</span></div>
            </button>
            <button v-if="isScene" class="favorite" :aria-pressed="item.favorite" :title="t(item.favorite ? 'gameContent.unfavoriteHelp' : 'gameContent.favorite')" :disabled="busy || (!item.available && !item.favorite)" @click="favorite(item)">{{ item.favorite ? '★' : '☆' }} {{ t(item.favorite ? 'gameContent.unfavorite' : 'gameContent.favorite') }}</button>
          </article>
        </div>
        <div v-else-if="!loading && app.vamRootPath" class="empty">
          <p>{{ t(isScene && favoritesOnly ? (favoriteCount ? 'gameContent.noMatchingFavorites' : 'gameContent.noFavorites') : 'gameContent.empty') }}</p>
          <button v-if="isScene && favoritesOnly && !favoriteCount" class="browse-scenes" @click="favoritesOnly = false; query = ''; filter = 'all'; app.searchQuery = ''">{{ t('gameContent.browseScenes') }}</button>
        </div>
        <nav v-if="pages > 1" class="pagination" :aria-label="t('gameContent.pagination')"><button :disabled="page <= 1" @click="page--">{{ t('gameContent.previous') }}</button><span>{{ page }} / {{ pages }}</span><button :disabled="page >= pages" @click="page++">{{ t('gameContent.next') }}</button></nav>
      </div>
      <aside v-if="selected" ref="inspectorPane" class="inspector" :aria-label="t('gameContent.details')">
        <div class="inspector-heading"><h3>{{ selected.alias || selected.name }}</h3><button :disabled="busy" :aria-label="t('gameContent.close')" @click="closeDetail">×</button></div>
        <p class="path">{{ selected.source.packageId ? `${selected.source.packageId}:/` : '' }}{{ selected.source.path }}</p>
        <img v-if="detail?.image" data-resource-preview class="detail-image" :src="detail.image" :alt="selected.name" />
        <p v-if="detailLoading" role="status">{{ t('common.loading') }}</p>
        <p v-if="detailError" role="alert" class="error">{{ detailError }}</p>
        <p v-if="!selected.available" class="notice">{{ t('gameContent.missingHelp') }}</p>
        <template v-if="isScene || !selected.source.packageId">
          <form @submit.prevent="saveName"><label for="content-name">{{ t(isScene ? 'gameContent.displayName' : 'gameContent.filename') }}</label><div class="rename-row"><input id="content-name" v-model="name" :maxlength="isScene ? 120 : 120" :required="!isScene" /><button :disabled="busy || (!isScene && !selected.available)" type="submit">{{ t('gameContent.save') }}</button></div></form>
          <p class="help">{{ t(isScene ? 'gameContent.aliasHelp' : 'gameContent.renameHelp') }}</p>
        </template>
        <template v-else><p class="help">{{ t('gameContent.packageRenameHelp') }}</p><button class="primary" :disabled="busy || !selected.available" @click="requestCopy(selected.source)">{{ t('gameContent.copyOne') }}</button></template>
        <button v-if="isScene" :disabled="busy || (!selected.available && !selected.favorite)" @click="favorite(selected)">{{ t(selected.favorite ? 'gameContent.unfavorite' : 'gameContent.favorite') }}</button>
        <dl class="sections"><template v-for="section in sections" :key="section.name"><dt>{{ section.name }}</dt><dd>{{ section.value }}</dd></template></dl>
        <details v-if="detail"><summary>{{ t('gameContent.raw') }}</summary><pre>{{ JSON.stringify(detail.json, null, 2) }}</pre></details>
      </aside>
    </div>
    <dialog ref="copyDialog" class="copy-dialog" aria-labelledby="copy-title" aria-describedby="copy-description" @close="pendingCopy = null" @cancel="cancelCopy">
      <template v-if="pendingCopy">
        <h3 id="copy-title">{{ t('gameContent.copyLocal') }}</h3>
        <p id="copy-description">{{ t(pendingCopy.source ? 'gameContent.copyOneConfirm' : 'gameContent.copyAllConfirm', { count: pendingCopy.count, type: title }) }}</p>
        <p class="copy-target">{{ pendingCopy.directory }}/VAM Library</p>
        <p>{{ t('gameContent.copySafety') }}</p>
        <div class="dialog-actions">
          <button autofocus @click="cancelCopy">{{ t('gameContent.cancel') }}</button>
          <button class="primary" @click="confirmCopy">{{ t('gameContent.confirmCopy') }}</button>
        </div>
      </template>
    </dialog>
  </section>
</template>

<style scoped>
.game-content { display: flex; flex-direction: column; gap: 12px; min-width: 0; min-height: 0; overflow: hidden; }
.game-content > :not(.content-body):not(dialog) { flex-shrink: 0; }
.content-toolbar, .pagination, .inspector-heading, .rename-row { display: flex; align-items: center; gap: 10px; }
p, .help { color: var(--text-secondary); font-size: 12px; line-height: 1.7; }
button, input, select { border: 1px solid var(--border-default, #3a3a3c); border-radius: 7px; padding: 8px 12px; font-size: 12px; background: var(--bg-surface); }
button:hover:not(:disabled) { background: var(--bg-hover); } button:disabled { opacity: .45; cursor: not-allowed; }
button:focus-visible, input:focus-visible, select:focus-visible, summary:focus-visible { outline: 2px solid var(--accent-primary); outline-offset: 2px; }
button.primary { background: var(--accent-primary); color: white; border-color: transparent; white-space: nowrap; }
.content-toolbar { flex-wrap: wrap; gap: 8px; }
.content-toolbar > input, .content-toolbar > select, .content-toolbar > button { height: 34px; padding: 0 12px; }
.content-toolbar input { flex: 1 1 180px; min-width: 140px; }
.content-toolbar select { flex: 0 0 140px; }
.content-toolbar button { flex: 0 0 auto; white-space: nowrap; }
.content-toolbar > .primary { margin-left: auto; }
.content-toolbar .filter-active { color: var(--text-primary); background: var(--accent-subtle); border-color: var(--accent-primary); }
select option { background: var(--bg-surface); }
.notice { border: 1px solid var(--border-default, #3a3a3c); background: var(--bg-surface); border-radius: 8px; padding: 12px; font-size: 12px; overflow-wrap: anywhere; max-height: 140px; overflow-y: auto; }
.notice li { margin-top: 6px; }.error { color: var(--color-error); }
/* Keep page controls outside the scrolling panes; their height must never be compressed by the gallery. */
.content-body { flex: 1; min-height: 0; display: grid; grid-template-columns: minmax(0, 1fr); grid-template-rows: minmax(0, 1fr); gap: 16px; overflow: hidden; }
.content-body.inspecting { grid-template-columns: minmax(0, 1fr) 360px; }
.count { flex-shrink: 0; margin-bottom: 8px; }
.content-list { display: flex; flex-direction: column; min-width: 0; min-height: 0; }
/* Opening details may change the number of columns, never the card dimensions. */
.content-grid { flex: 1; min-height: 0; overflow-y: auto; overflow-x: hidden; overscroll-behavior: contain; scrollbar-gutter: stable; display: grid; grid-template-columns: repeat(auto-fill, minmax(0, 190px)); grid-auto-rows: max-content; gap: 12px; align-content: start; align-items: start; padding: 2px 4px 4px 2px; }
.content-card { border: 1px solid var(--border-default, #3a3a3c); border-radius: 10px; overflow: hidden; background: var(--bg-surface); min-width: 0; }
.content-card.selected { border-color: var(--accent-primary); }.card-main { padding: 0; border: 0; border-radius: 0; display: block; width: 100%; text-align: left; }
.preview { position: relative; aspect-ratio: 4 / 3; overflow: hidden; display: grid; place-items: center; background: var(--bg-subtle); color: var(--text-tertiary); }.preview img { width: 100%; height: 100%; object-fit: cover; }
.preview-status { position: absolute; left: 6px; right: 6px; bottom: 6px; padding: 4px 6px; border-radius: 4px; background: var(--bg-base); color: var(--color-warning); font-size: 11px; line-height: 1.4; }
.card-text { display: flex; flex-direction: column; gap: 6px; padding: 12px; }.card-text strong, .card-text span { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }.card-text span { font-size: 11px; color: var(--text-secondary); }
.card-text strong { line-height: 18px; }.card-text span { line-height: 16px; }
.favorite { width: 100%; border: 0; border-top: 1px solid var(--border-default, #3a3a3c); border-radius: 0; text-align: left; }.favorite[aria-pressed="true"] { color: var(--color-warning); }
.pagination { flex-shrink: 0; justify-content: center; padding-top: 10px; font-size: 12px; }.empty { flex: 1; min-height: 0; overflow: auto; padding: 40px 10px; text-align: center; }
.browse-scenes { margin-top: 12px; }
.inspector { height: 100%; min-height: 0; overflow-y: auto; overflow-x: hidden; overscroll-behavior: contain; scrollbar-gutter: stable; border: 1px solid var(--border-default, #3a3a3c); border-radius: 10px; padding: 16px; background: var(--bg-surface); display: flex; flex-direction: column; gap: 14px; min-width: 0; }
.inspector > * { flex-shrink: 0; }
.inspector-heading { justify-content: space-between; position: sticky; top: -16px; z-index: 1; background: var(--bg-surface); padding: 8px 0; }.inspector-heading button { flex-shrink: 0; }.inspector h3 { font-size: 16px; overflow-wrap: anywhere; }.path { overflow-wrap: anywhere; user-select: text; }.detail-image { height: 160px; width: 100%; object-fit: contain; border-radius: 6px; background: var(--bg-subtle); }
label { display: block; font-size: 12px; margin-bottom: 8px; }.rename-row input { min-width: 0; flex: 1; }.sections dt { margin: 12px 0 6px; font-size: 12px; font-weight: 600; }.sections dd { white-space: pre-wrap; overflow-wrap: anywhere; font-size: 11px; color: var(--text-secondary); max-height: 240px; overflow: auto; user-select: text; }
summary { cursor: pointer; font-size: 12px; }pre { margin-top: 10px; max-height: 350px; overflow: auto; font-size: 11px; user-select: text; }
.copy-dialog { margin: auto; width: min(520px, calc(100vw - 40px)); max-height: calc(100vh - 40px); overflow-y: auto; padding: 24px; color: var(--text-primary); background: var(--bg-surface); border: 1px solid var(--border-default, #3a3a3c); border-radius: 12px; }
.copy-dialog::backdrop { background: rgba(0, 0, 0, .55); }
.copy-dialog h3 { margin-bottom: 14px; font-size: 16px; }.copy-dialog p + p { margin-top: 12px; }.copy-target { overflow-wrap: anywhere; user-select: text; }
.dialog-actions { display: flex; justify-content: flex-end; gap: 10px; margin-top: 24px; }
@media (max-width: 1100px) { .content-body.inspecting { grid-template-columns: minmax(0, 1fr) 300px; }.content-grid { grid-template-columns: repeat(auto-fill, minmax(0, 160px)); } }
@media (max-width: 900px) { .content-body.inspecting { grid-template-columns: minmax(0, 1fr); }.content-body.inspecting .content-list { display: none; } }
</style>
