<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { open } from '@tauri-apps/plugin-dialog'
import { useI18n } from 'vue-i18n'
import { useAppStore } from '@/stores/app'

interface ContentRef { packageId: string | null; path: string }
interface Scene { key: string; name: string; source: ContentRef }
interface Character { index: number; name: string; character: string; clothing: number; hair: number; morphs: number; error: string | null }
interface Detail { revision: string; characters: Character[]; image: string | null }
const props = defineProps<{ packageId: string }>()
const emit = defineEmits<{ busyChange: [busy: boolean] }>()
const { t } = useI18n()
const app = useAppStore()
const expanded = ref(false)
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
const launching = ref(false)
const launchMode = ref<'desktop' | 'vr'>('desktop')
const launchError = ref('')
const launchSucceeded = ref(false)
const busy = computed(() => saving.value || launching.value)
watch(busy, value => emit('busyChange', value), { flush: 'sync' })
const error = ref('')
const detailError = ref('')
const saveError = ref('')
const warnings = ref<string[]>([])
const savedPath = ref('')
const defaultDirectory = computed(() => app.vamRootPath ? `${app.vamRootPath.replace(/[\\/]+$/, '')}\\Custom\\Atom\\Person\\Appearance` : '')
const outputDirectory = ref('')
const choosingDirectory = ref(false)
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
  const epoch = ++listEpoch
  ++detailEpoch
  const root = app.vamRootPath
  scenes.value = []; sceneKey.value = ''; detail.value = null
  loading.value = true; loaded.value = false; error.value = ''; warnings.value = []
  detailLoading.value = false; detailError.value = ''; savedPath.value = ''; saveError.value = ''
  if (!root) { error.value = t('gameContent.noGame'); loading.value = false; return }
  try {
    const result = await invoke<{ items: Scene[]; warnings: string[] }>('list_package_scene_contents', { vamRoot: root, packageId: props.packageId })
    if (!alive || epoch !== listEpoch || root !== app.vamRootPath) return
    scenes.value = result.items; warnings.value = result.warnings; loaded.value = true
    sceneKey.value = result.items[0]?.key || ''
  } catch (e) { if (alive && epoch === listEpoch) error.value = String(e) }
  finally { if (alive && epoch === listEpoch) loading.value = false }
}
function toggleExtraction() {
  if (busy.value || choosingDirectory.value || !selectedScene.value) return
  expanded.value = !expanded.value
  if (expanded.value && !detail.value && !detailLoading.value) void loadDetail()
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
  launchError.value = ''; launchSucceeded.value = false
  if (expanded.value) void loadDetail()
})
async function launchScene() {
  const scene = selectedScene.value
  const root = app.vamRootPath
  const epoch = listEpoch
  if (!scene || !root || busy.value) return
  launching.value = true; launchError.value = ''; launchSucceeded.value = false
  try {
    await invoke('launch_game_scene', { vamRoot: root, source: scene.source, mode: launchMode.value })
    if (alive && epoch === listEpoch && root === app.vamRootPath) launchSucceeded.value = true
  } catch (e) {
    if (alive && epoch === listEpoch && root === app.vamRootPath) launchError.value = String(e)
  } finally { launching.value = false }
}
watch(characterIndex, () => {
  saveError.value = ''; savedPath.value = ''
  name.value = `${selectedScene.value?.name || ''} ${character.value?.name || ''}`
    .replace(/[<>:"/\\|?*\u0000-\u001f]/g, '_').trim().replace(/[. ]+$/, '').slice(0, 50)
})
async function save() {
  const scene = selectedScene.value
  const person = character.value
  const revision = detail.value?.revision
  const root = app.vamRootPath
  if (!scene || !person || person.error || !revision || !root || busy.value || choosingDirectory.value || !outputDirectory.value.trim()) return
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
  launchError.value = ''; launchSucceeded.value = false
  if (!busy.value) void loadScenes()
})
watch(busy, value => { if (!value && alive && !loaded.value && !loading.value && !error.value) void loadScenes() })
onMounted(() => { void loadScenes() })
onBeforeUnmount(() => { alive = false; ++listEpoch; ++detailEpoch; emit('busyChange', false) })
</script>

<template>
  <div class="package-scene-actions">
    <slot name="actions" :launch="launchScene" :extract="toggleExtraction" :disabled="busy || choosingDirectory || loading || !selectedScene" :expanded="expanded" :launching="launching" :panel="`package-extraction-${packageId}`" />
    <section class="scene-extractor" :aria-label="t('gameContent.sceneActions')">
      <div class="extract-body" :aria-busy="loading || detailLoading || busy">
        <p v-if="loading" role="status">{{ t('gameContent.readingPackageScenes') }}</p>
        <div v-else-if="error"><p class="error" role="alert">{{ error }}</p><button @click="loadScenes">{{ t('gameContent.retry') }}</button></div>
        <p v-else-if="loaded && !scenes.length">{{ t('gameContent.noPackageScenes') }}</p>
        <template v-if="scenes.length">
          <label>{{ t('gameContent.selectPackageScene', { count: scenes.length }) }}
            <select v-model="sceneKey" :disabled="busy"><option v-for="scene in scenes" :key="scene.key" :value="scene.key">{{ scene.source.path.replace(/^Saves\/scene\//i, '') }}</option></select>
          </label>
          <label>{{ t('gameContent.launchMode') }}<select v-model="launchMode" :disabled="busy"><option value="desktop">{{ t('gameContent.desktopMode') }}</option><option value="vr">{{ t('gameContent.vrMode') }}</option></select></label>
          <p>{{ t('gameContent.launchHelp') }} <RouterLink to="/settings">{{ t('gameContent.setupSceneMod') }}</RouterLink></p>
          <p v-if="launching" role="status">{{ t('gameContent.launchWaiting') }}</p>
          <p v-if="launchSucceeded" role="status">{{ t('gameContent.sceneLoaded') }}</p>
          <p v-if="launchError" class="error" role="alert">{{ launchError }}</p>
          <div v-show="expanded" :id="`package-extraction-${packageId}`" class="extraction-panel">
            <p v-if="detailLoading" role="status">{{ t('common.loading') }}</p>
            <figure v-if="detail" class="scene-preview">
              <img v-if="detail.image && !previewFailed" data-resource-preview :src="detail.image" :alt="t('gameContent.scenePreview')" @error="previewFailed = true" />
              <p v-else class="preview-empty">{{ t(previewFailed ? 'gameContent.imageFailed' : 'gameContent.noScenePreview') }}</p>
              <figcaption v-if="detail.image && !previewFailed">{{ t('gameContent.scenePreviewHelp') }}</figcaption>
            </figure>
            <div v-if="detailError"><p class="error" role="alert">{{ detailError }}</p><button @click="loadDetail">{{ t('gameContent.retry') }}</button></div>
            <p v-else-if="detail && !detail.characters.length">{{ t('gameContent.noCharacters') }}</p>
            <form v-else-if="detail?.characters.length" @submit.prevent="save">
              <label>{{ t('gameContent.selectCharacter') }}
                <select v-model="characterIndex" :disabled="busy"><option v-for="person in detail.characters" :key="person.index" :value="person.index">{{ person.name }} · {{ person.character || '—' }}</option></select>
              </label>
              <p v-if="character">{{ t('gameContent.characterContents', character) }}</p>
              <p v-if="character?.error" class="error" role="alert">{{ character.error }}</p>
              <template v-else>
                <label>{{ t('gameContent.filename') }}<input v-model="name" required maxlength="120" :disabled="busy" /></label>
                <label>{{ t('gameContent.outputDirectory') }}<input v-model="outputDirectory" required :disabled="busy || choosingDirectory" :placeholder="defaultDirectory" spellcheck="false" /></label>
                <div class="directory-actions"><button type="button" :disabled="busy || choosingDirectory" @click="browseOutputDirectory">{{ t('gameContent.chooseOutputDirectory') }}</button><button type="button" :disabled="busy || choosingDirectory || outputDirectory === defaultDirectory" @click="outputDirectory = defaultDirectory">{{ t('gameContent.resetOutputDirectory') }}</button></div>
                <p>{{ t('gameContent.outputDirectoryHelp') }}</p>
                <button class="primary" :disabled="busy || choosingDirectory || !name.trim() || !outputDirectory.trim()" type="submit">{{ t(saving ? 'gameContent.working' : 'gameContent.saveAppearance') }}</button>
              </template>
            </form>
            <p v-if="saveError" class="error" role="alert">{{ saveError }}</p>
            <div v-if="savedPath" class="saved" role="status"><p>{{ t('gameContent.appearanceSaved') }}</p><p class="path">{{ savedDisplayPath }}</p><RouterLink v-if="savedInLibrary" :to="{ path: '/appearances', query: { appearance: savedPath } }">{{ t('gameContent.viewAppearances') }}</RouterLink></div>
            <details><summary>{{ t('gameContent.extractScope') }}</summary><p>{{ t('gameContent.extractHelp') }}</p></details>
          </div>
        </template>
        <details v-if="warnings.length"><summary>{{ t('gameContent.warnings', { count: warnings.length }) }}</summary><p v-for="warning in warnings" :key="warning" class="error">{{ warning }}</p></details>
      </div>
    </section>
  </div>
</template>

<style scoped>
.scene-extractor { flex-shrink: 0; min-width: 0; border: 1px solid var(--border-subtle); border-radius: var(--radius-md); background: var(--bg-surface); }
button, input, select { font: inherit; font-size: 12px; color: var(--text-primary); border: 1px solid var(--border-default); border-radius: var(--radius-sm); background: var(--bg-surface); min-height: 34px; padding: 7px 10px; }
button { cursor: pointer; }button:hover:not(:disabled) { background: var(--bg-hover); }button:disabled, input:disabled, select:disabled { opacity: .5; cursor: not-allowed; }
button:focus-visible, input:focus-visible, select:focus-visible, summary:focus-visible, a:focus-visible { outline: 2px solid var(--accent-primary); outline-offset: 2px; }
.package-scene-actions { flex-shrink: 0; min-width: 0; }
.extract-body { display: flex; flex-direction: column; gap: 12px; padding: 12px; }
.extraction-panel { display: flex; flex-direction: column; gap: 12px; border-top: 1px solid var(--border-subtle); padding-top: 12px; }
.scene-preview { margin: 0; min-width: 0; }.scene-preview img { display: block; width: 100%; height: 120px; object-fit: contain; background: var(--bg-base); border-radius: var(--radius-sm); }
.scene-preview figcaption { margin-top: 6px; color: var(--text-tertiary); font-size: 11px; line-height: 1.5; }.preview-empty { padding: 8px 0; }
form { display: flex; flex-direction: column; gap: 10px; }
.directory-actions { display: flex; flex-wrap: wrap; gap: 8px; }
label { display: flex; flex-direction: column; gap: 6px; font-size: 12px; color: var(--text-secondary); }
select, input { width: 100%; min-width: 0; }select option { background: var(--bg-surface); }
p { font-size: 12px; color: var(--text-secondary); line-height: 1.6; overflow-wrap: anywhere; margin: 0; }
.primary { background: var(--accent-primary); color: white; border-color: transparent; }.primary:hover:not(:disabled) { background: var(--accent-hover, var(--accent-primary)); }
.error { color: var(--color-error); }.saved { display: flex; flex-direction: column; gap: 6px; }.path { user-select: text; }a { color: var(--accent-primary); font-size: 12px; }
summary { cursor: pointer; font-size: 12px; color: var(--text-secondary); }details p { margin-top: 8px; }
</style>
