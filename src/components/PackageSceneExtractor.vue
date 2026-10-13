<script setup lang="ts">
import UiInput from '@/components/ui/UiInput.vue'
import UiButton from '@/components/ui/UiButton.vue'
import UiModal from '@/components/ui/UiModal.vue'
import { computed, onBeforeUnmount, onMounted, ref, useId, watch } from 'vue'
import { Channel, invoke } from '@tauri-apps/api/core'
import { open } from '@tauri-apps/plugin-dialog'
import { useI18n } from 'vue-i18n'
import { cachedResource, resourceInvoke, invalidateResourceCache } from '@/utils/resourceCache'
import { useFavoritesStore } from '@/stores/favorites'
import { useAppStore } from '@/stores/app'

interface ContentRef { packageId: string | null; path: string }
interface Scene { key: string; name: string; source: ContentRef; favorite: boolean; available: boolean }
interface Character { index: number; name: string; character: string; clothing: number; hair: number; morphs: number; error: string | null }
interface Detail { revision: string; characters: Character[]; image: string | null }
const props = withDefaults(defineProps<{ packageId?: string; source?: ContentRef; sceneName?: string; favorite?: boolean; available?: boolean; disabled?: boolean; cacheKey?: string; sceneEligible?: boolean }>(), { available: true, sceneEligible: true })
const emit = defineEmits<{ busyChange: [busy: boolean]; favoriteChange: [source: ContentRef, favorite: boolean] }>()
const { t } = useI18n()
const app = useAppStore()
const favoritesStore = useFavoritesStore()
const expanded = ref(false)
const formId = useId()
const scenes = ref<Scene[]>([])
const sceneKey = ref('')
const selectedScene = computed(() => scenes.value.find(s => s.key === sceneKey.value))
const detail = ref<Detail | null>(null)
const previewFailed = ref(false)
const characterIndex = ref<number | null>(null)
const character = computed(() => detail.value?.characters.find(c => c.index === characterIndex.value))
const name = ref('')
const loaded = ref(false)
const loading = ref(false)
const detailLoading = ref(false)
const saving = ref(false)
const favoriting = ref(false)
const favoriteError = ref('')
const launching = ref(false)
const launchMode = ref<'desktop' | 'vr'>('desktop')
const launchError = ref('')
const launchDetail = ref('')
const launchPhase = ref<'opening' | 'sending' | 'loading'>('opening')
const stillLoading = ref(false)
const launchSucceeded = ref(false)
const busy = computed(() => saving.value || launching.value || favoriting.value)
const error = ref('')
const detailError = ref('')
const saveError = ref('')
const warnings = ref<string[]>([])
const savedPath = ref('')
const defaultDirectory = computed(() => app.vamRootPath ? `${app.vamRootPath.replace(/[\\/]+$/, '')}\\Custom\\Atom\\Person\\Appearance` : '')
const outputDirectory = ref('')
const choosingDirectory = ref(false)
watch(() => busy.value || choosingDirectory.value || expanded.value, value => emit('busyChange', value), { flush: 'sync' })
const savedInLibrary = computed(() => savedPath.value.toLowerCase().startsWith('custom/atom/person/appearance/'))
const savedDisplayPath = computed(() => /^[a-z]:[\\/]|^[\\/]/i.test(savedPath.value)
  ? savedPath.value : `${app.vamRootPath?.replace(/[\\/]+$/, '')}\\${savedPath.value.replace(/\//g, '\\')}`)
function directoryPreferenceKey(root: string) { return `vamlibrary-appearance-output:${root.replace(/\\/g, '/').replace(/\/+$/, '').toLowerCase()}` }
watch(() => app.vamRootPath, root => {
  outputDirectory.value = defaultDirectory.value
  if (root) {
    try { outputDirectory.value = localStorage.getItem(directoryPreferenceKey(root)) || defaultDirectory.value }
    catch { /* Directory selection still works when local preferences are unavailable. */ }
  }
}, { immediate: true })
let listEpoch = 0
let detailEpoch = 0
let alive = true

async function browseOutputDirectory() {
  if (busy.value || choosingDirectory.value) return
  const root = app.vamRootPath
  const originalDirectory = outputDirectory.value
  choosingDirectory.value = true; saveError.value = ''
  try {
    const selected = await open({ directory: true, multiple: false, title: t('gameContent.chooseOutputDirectory'), defaultPath: originalDirectory || defaultDirectory.value })
    if (alive && root === app.vamRootPath && typeof selected === 'string') outputDirectory.value = selected
  } catch (e) { if (alive && root === app.vamRootPath) saveError.value = String(e) }
  finally { choosingDirectory.value = false }
}

async function loadScenes() {
  if (busy.value) return
  const previousScene = sceneKey.value
  const epoch = ++listEpoch
  ++detailEpoch
  const root = app.vamRootPath
  scenes.value = []; sceneKey.value = ''; detail.value = null
  loading.value = true; loaded.value = false; error.value = ''; warnings.value = []
  detailLoading.value = false; detailError.value = ''; savedPath.value = ''; saveError.value = ''
  favoriteError.value = ''
  if (!props.sceneEligible) { loaded.value = true; loading.value = false; return }
  if (!root) { error.value = t('gameContent.noGame'); loading.value = false; return }
  try {
    const result = props.source
      ? { items: [{ key: props.source.path, name: props.sceneName || props.source.path, source: props.source, favorite: !!props.favorite, available: props.available }], warnings: [] }
      : await cachedResource(`scenes:${root}:${props.cacheKey || props.packageId}`, () => resourceInvoke<{ items: Scene[]; warnings: string[] }>('list_package_scene_contents', { vamRoot: root, packageId: props.packageId }))
    if (!alive || epoch !== listEpoch || root !== app.vamRootPath) return
    scenes.value = result.items.map(item => ({ ...item })); warnings.value = result.warnings; loaded.value = true
    sceneKey.value = result.items.some(item => item.key === previousScene) ? previousScene : result.items[0]?.key || ''
  } catch (e) { if (alive && epoch === listEpoch) error.value = String(e) }
  finally { if (alive && epoch === listEpoch) loading.value = false }
}
function toggleExtraction() {
  if (busy.value || props.disabled || choosingDirectory.value || !selectedScene.value?.available) return
  expanded.value = true
  savedPath.value = ''; saveError.value = ''
  if (!detail.value && !detailLoading.value) void loadDetail()
}
function closeExtraction() {
  if (!busy.value && !choosingDirectory.value) expanded.value = false
}
async function toggleFavorite() {
  const scene = selectedScene.value
  const root = app.vamRootPath
  const epoch = listEpoch
  if (!scene || !root || busy.value || props.disabled || expanded.value || choosingDirectory.value || (!scene.available && !scene.favorite)) return
  favoriting.value = true; favoriteError.value = ''
  try {
    await invoke('set_game_scene_favorite', { vamRoot: root, source: scene.source, favorite: !scene.favorite })
    invalidateResourceCache('scenes:')
    void favoritesStore.refresh()
    if (alive && epoch === listEpoch && root === app.vamRootPath) {
      scene.favorite = !scene.favorite
      emit('favoriteChange', scene.source, scene.favorite)
    }
  } catch (e) {
    if (alive && epoch === listEpoch && root === app.vamRootPath) favoriteError.value = String(e)
  } finally { favoriting.value = false }
}
async function loadDetail() {
  const epoch = ++detailEpoch
  const scene = selectedScene.value
  const root = app.vamRootPath
  detail.value = null; characterIndex.value = null; detailError.value = ''; saveError.value = ''; savedPath.value = ''
  previewFailed.value = false
  detailLoading.value = false
  if (!scene || !root) return
  detailLoading.value = true
  try {
    const result = await invoke<Detail>('get_game_content_detail', { vamRoot: root, source: scene.source, summaryOnly: true })
    if (!alive || epoch !== detailEpoch || root !== app.vamRootPath) return
    detail.value = result
    characterIndex.value = result.characters.find(c => !c.error)?.index ?? result.characters[0]?.index ?? null
  } catch (e) { if (alive && epoch === detailEpoch) detailError.value = String(e) }
  finally { if (alive && epoch === detailEpoch) detailLoading.value = false }
}
watch(sceneKey, () => {
  ++detailEpoch
  detail.value = null; characterIndex.value = null; detailLoading.value = false
  detailError.value = ''; saveError.value = ''; savedPath.value = ''
  launchError.value = ''; launchDetail.value = ''; stillLoading.value = false; launchSucceeded.value = false
  favoriteError.value = ''
  if (expanded.value) void loadDetail()
})
watch(() => [props.favorite, props.available], () => {
  if (props.source && selectedScene.value) {
    selectedScene.value.favorite = !!props.favorite
    selectedScene.value.available = props.available
  }
})
async function launchScene() {
  const scene = selectedScene.value
  const root = app.vamRootPath
  const epoch = listEpoch
  if (!scene?.available || !root || busy.value || props.disabled || choosingDirectory.value) return
  launching.value = true; launchError.value = ''; launchDetail.value = ''; stillLoading.value = false; launchSucceeded.value = false
  try {
    launchPhase.value = 'opening'
    const onProgress = new Channel<string>()
    onProgress.onmessage = phase => {
      if (alive && epoch === listEpoch && root === app.vamRootPath && (phase === 'opening' || phase === 'sending' || phase === 'loading')) launchPhase.value = phase
    }
    const outcome = await invoke<'loaded' | 'still_loading'>('launch_game_scene', { vamRoot: root, source: scene.source, mode: launchMode.value, onProgress })
    if (alive && epoch === listEpoch && root === app.vamRootPath) {
      stillLoading.value = outcome === 'still_loading'
      launchSucceeded.value = outcome === 'loaded'
    }
  } catch (e) {
    if (alive && epoch === listEpoch && root === app.vamRootPath) {
      const failure = e as { code?: string; detail?: string }
      const codes = ['invalid_request', 'scene_unavailable', 'package_disabled', 'load_not_started', 'load_failed', 'internal', 'game_exited', 'accept_timeout', 'invalid_response', 'busy', 'launch_unavailable']
      launchError.value = codes.includes(failure?.code || '') ? failure.code! : 'internal'
      launchDetail.value = failure?.detail || (failure?.code ? '' : String(e))
    }
  } finally { launching.value = false }
}
watch(characterIndex, () => {
  saveError.value = ''; savedPath.value = ''
  name.value = `${selectedScene.value?.name || ''} ${character.value?.name || ''}`
    .replace(/[<>:"/\\|?*\u0000-\u001f]/g, '_').trim().replace(/[. ]+$/, '').slice(0, 50)
})
watch([name, outputDirectory], () => { savedPath.value = ''; saveError.value = '' })
async function save() {
  const scene = selectedScene.value
  const person = character.value
  const revision = detail.value?.revision
  const root = app.vamRootPath
  if (!scene || !person || person.error || !revision || !root || busy.value || props.disabled || choosingDirectory.value || !name.value.trim() || !outputDirectory.value.trim()) return
  const directory = outputDirectory.value.trim()
  const epoch = detailEpoch
  saving.value = true; saveError.value = ''; savedPath.value = ''
  try {
    const path = await invoke<string>('save_game_scene_appearance', { vamRoot: root, source: scene.source, revision, atomIndex: person.index, name: name.value, outputDir: directory })
    if (alive && epoch === detailEpoch && root === app.vamRootPath) {
      savedPath.value = path
      try { localStorage.setItem(directoryPreferenceKey(root), directory) }
      catch { /* A preference failure must not report a successfully saved preset as failed. */ }
    }
  } catch (e) { if (alive && epoch === detailEpoch && root === app.vamRootPath) saveError.value = String(e) }
  finally { saving.value = false }
}
watch(() => app.vamRootPath, () => {
  ++listEpoch; ++detailEpoch
  scenes.value = []; sceneKey.value = ''; detail.value = null; loaded.value = false
  loading.value = false; detailLoading.value = false; error.value = ''; detailError.value = ''; savedPath.value = ''; saveError.value = ''; warnings.value = []
  expanded.value = false
  favoriteError.value = ''
  launchError.value = ''; launchDetail.value = ''; stillLoading.value = false; launchSucceeded.value = false
  if (!busy.value) void loadScenes()
})
watch(busy, value => { if (!value && alive && !loaded.value && !loading.value && !error.value) void loadScenes() })
watch(() => [props.cacheKey, props.sceneEligible], () => {
  loaded.value = false
  if (!busy.value) void loadScenes()
})
onMounted(() => { void loadScenes() })
onBeforeUnmount(() => { alive = false; ++listEpoch; ++detailEpoch; emit('busyChange', false) })
defineExpose({ launch: launchScene, extract: toggleExtraction, toggleFavorite, selectedScene, favoriting, ready: computed(() => !busy.value && !props.disabled && !choosingDirectory.value && !loading.value && !!selectedScene.value?.available), loading, error })
</script>

<template>
  <div class="package-scene-actions">
    <slot name="actions" :launch="launchScene" :extract="toggleExtraction"
      :disabled="busy || disabled || choosingDirectory || loading || !selectedScene?.available" :launching="launching" />
    <p v-if="favoriteError" class="error" role="alert">{{ favoriteError }}</p>
    <details class="scene-extractor" :open="launching || launchSucceeded || stillLoading || !!launchError || !!error">
      <summary>{{ t('gameContent.sceneActions') }}</summary>
      <div class="extract-body" :aria-busy="loading || busy">
        <p v-if="loading" role="status">{{ t('gameContent.readingPackageScenes') }}</p>
        <div v-else-if="error">
          <p class="error" role="alert">{{ error }}</p>
          <UiButton @click="loadScenes">{{ t('gameContent.retry') }}</UiButton>
        </div>
        <p v-else-if="loaded && !scenes.length">{{ t('gameContent.noPackageScenes') }}</p>
        <template v-if="scenes.length">
          <label v-if="scenes.length > 1" class="extract-field">
            {{ t('gameContent.selectPackageScene', { count: scenes.length }) }}
            <select v-model="sceneKey" class="extract-select" :disabled="busy || disabled || expanded">
              <option v-for="scene in scenes" :key="scene.key" :value="scene.key">{{ scene.source.path.replace(/^Saves\/scene\//i, '') }}</option>
            </select>
          </label>
          <label class="extract-field">
            {{ t('gameContent.launchMode') }}
            <select v-model="launchMode" class="extract-select" :disabled="busy || disabled">
              <option value="desktop">{{ t('gameContent.desktopMode') }}</option>
              <option value="vr">{{ t('gameContent.vrMode') }}</option>
            </select>
          </label>
          <p>{{ t('gameContent.launchHelp') }} <RouterLink to="/settings">{{ t('gameContent.setupSceneMod') }}</RouterLink></p>
          <p v-if="launching" role="status">{{ t(`sceneLaunch.${launchPhase}`) }}</p>
          <p v-if="launchSucceeded" role="status">{{ t('gameContent.sceneLoaded') }}</p>
          <p v-if="stillLoading" role="status">{{ t('sceneLaunch.stillLoading') }}</p>
          <p v-if="launchError" class="error" role="alert">{{ t(`sceneLaunch.errors.${launchError}`) }}</p>
          <details v-if="launchDetail">
            <summary>{{ t('sceneLaunch.details') }}</summary>
            <p class="error">{{ launchDetail }}</p>
          </details>
        </template>
        <details v-if="warnings.length">
          <summary>{{ t('gameContent.warnings', { count: warnings.length }) }}</summary>
          <p v-for="warning in warnings" :key="warning" class="error">{{ warning }}</p>
        </details>
      </div>
    </details>
    <UiModal v-if="expanded" class="scene-extraction-modal" :title="t('gameContent.extractPreset')" :description="t('gameContent.extractHelp')"
      :close-disabled="busy || choosingDirectory" @close="closeExtraction">
      <div class="extraction-panel" :aria-busy="detailLoading || busy">
        <label v-if="scenes.length > 1" class="extract-field">
          {{ t('gameContent.selectPackageScene', { count: scenes.length }) }}
          <select v-model="sceneKey" class="extract-select" :disabled="busy || choosingDirectory">
            <option v-for="scene in scenes" :key="scene.key" :value="scene.key">{{ scene.source.path.replace(/^Saves\/scene\//i, '') }}</option>
          </select>
        </label>
        <p v-if="detailLoading" role="status">{{ t('common.loading') }}</p>
        <figure v-if="detail" class="scene-preview">
          <img v-if="detail.image && !previewFailed" data-resource-preview :src="detail.image" :alt="t('gameContent.scenePreview')" @error="previewFailed = true" />
          <p v-else class="preview-empty">{{ t(previewFailed ? 'gameContent.imageFailed' : 'gameContent.noScenePreview') }}</p>
          <figcaption v-if="detail.image && !previewFailed">{{ t('gameContent.scenePreviewHelp') }}</figcaption>
        </figure>
        <div v-if="detailError">
          <p class="error" role="alert">{{ detailError }}</p>
          <UiButton @click="loadDetail">{{ t('gameContent.retry') }}</UiButton>
        </div>
        <p v-else-if="detail && !detail.characters.length">{{ t('gameContent.noCharacters') }}</p>
        <form v-else-if="detail && detail.characters.length" :id="formId" class="extraction-form" @submit.prevent="save">
          <label class="extract-field">
            {{ t('gameContent.selectCharacter') }}
            <select v-model="characterIndex" class="extract-select" :disabled="busy || choosingDirectory">
              <option v-for="person in detail.characters" :key="person.index" :value="person.index">{{ person.name }}</option>
            </select>
          </label>
          <p v-if="character">{{ t('gameContent.characterContents', character) }}</p>
          <p v-if="character?.error" class="error" role="alert">{{ character.error }}</p>
          <template v-else>
            <label class="extract-field">{{ t('gameContent.filename') }}
              <UiInput v-model="name" required maxlength="120" :disabled="busy || choosingDirectory" />
            </label>
            <label class="extract-field">{{ t('gameContent.outputDirectory') }}
              <UiInput v-model="outputDirectory" required :disabled="busy || choosingDirectory" :placeholder="defaultDirectory" spellcheck="false" />
            </label>
            <div class="directory-actions">
              <UiButton type="button" :disabled="busy || choosingDirectory" @click="browseOutputDirectory">{{ t('gameContent.chooseOutputDirectory') }}</UiButton>
              <UiButton type="button" :disabled="busy || choosingDirectory || outputDirectory === defaultDirectory" @click="outputDirectory = defaultDirectory">{{ t('gameContent.resetOutputDirectory') }}</UiButton>
            </div>
            <p>{{ t('gameContent.outputDirectoryHelp') }}</p>
          </template>
        </form>
        <p v-if="saveError" class="error" role="alert">{{ saveError }}</p>
        <div v-if="savedPath" class="saved" role="status">
          <p class="path">{{ savedDisplayPath }}</p>
          <RouterLink v-if="savedInLibrary" :to="{ path: '/appearances', query: { appearance: savedPath } }">{{ t('gameContent.viewAppearances') }}</RouterLink>
        </div>
      </div>
      <template #footer>
        <p v-if="savedPath" class="extraction-status" role="status">{{ t('gameContent.appearanceSaved') }}</p>
        <UiButton class="extraction-cancel" :disabled="busy || choosingDirectory" @click="closeExtraction">{{ t(savedPath ? 'common.close' : 'gameContent.cancel') }}</UiButton>
        <UiButton class="extraction-save" type="submit" :form="formId" variant="primary" :loading="saving"
          :disabled="busy || choosingDirectory || detailLoading || !character || !!character.error || !name.trim() || !outputDirectory.trim() || !!savedPath">
          {{ t('gameContent.addAppearance') }}
        </UiButton>
      </template>
    </UiModal>
  </div>
</template>

<style scoped>
:global(.ui-modal.scene-extraction-modal[open]) {
  display: flex;
  flex-direction: column;
  overflow: hidden;
}
:global(.scene-extraction-modal .ui-modal-header),
:global(.scene-extraction-modal .ui-modal-description),
:global(.scene-extraction-modal .ui-modal-footer) {
  flex-shrink: 0;
}
:global(.scene-extraction-modal .ui-modal-body) {
  min-height: 0;
  overflow: auto;
  overscroll-behavior: contain;
  scrollbar-gutter: stable;
}
.scene-extractor {
  flex-shrink: 0;
  min-width: 0;
  border: none;
  border-radius: var(--radius-md);
  background: var(--bg-surface);
}
.extract-select {
  width: 100%;
}
.package-scene-actions {
  display: flex;
  flex-direction: column;
  gap: var(--space-stack);
  flex-shrink: 0;
  min-width: 0;
}
.extract-body {
  display: flex;
  flex-direction: column;
  gap: var(--space-3);
  padding: var(--space-3);
}
.extraction-panel {
  display: flex;
  flex-direction: column;
  gap: var(--space-3);
}
.extraction-status {
  flex: 1;
  min-width: 0;
  align-self: center;
  color: var(--color-success);
}
.scene-preview {
  margin: 0;
  min-width: 0;
}
.scene-preview img {
  display: block;
  width: 100%;
  height: 120px;
  object-fit: contain;
  background: var(--bg-base);
  border-radius: var(--radius-sm);
}
.scene-preview figcaption {
  margin-top: var(--space-2);
  color: var(--text-tertiary);
  font-size: var(--text-xs);
  line-height: 1.5;
}
.preview-empty {
  padding: var(--space-2) 0;
}
.extraction-form {
  display: flex;
  flex-direction: column;
  gap: var(--space-3);
}
.directory-actions {
  display: flex;
  flex-wrap: wrap;
  gap: var(--space-2);
}
.extract-field {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
  font-size: var(--text-sm);
  color: var(--text-secondary);
}
.extract-select {
  width: 100%;
  min-width: 0;
}
p {
  font-size: var(--text-sm);
  color: var(--text-secondary);
  line-height: 1.6;
  overflow-wrap: anywhere;
  margin: 0;
}
.error {
  color: var(--color-error);
}
.saved {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
}
.path {
  user-select: text;
}
a {
  color: var(--accent-primary);
  font-size: var(--text-sm);
}
a:hover {
  color: var(--accent-primary-hover);
  text-decoration: underline;
}
summary {
  cursor: pointer;
  font-size: var(--text-sm);
  color: var(--text-secondary);
}
details p {
  margin-top: var(--space-2);
}
.scene-extractor {
  background: var(--bg-input);
}
.scene-extractor > summary {
  padding: var(--space-3) var(--space-3);
  cursor: pointer;
  color: var(--text-secondary);
  font-size: var(--text-sm);
  border-radius: var(--radius-md);
}
</style>
