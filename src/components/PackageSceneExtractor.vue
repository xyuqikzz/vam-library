<script setup lang="ts">
import { computed, onBeforeUnmount, ref, watch } from 'vue'
import { invoke } from '@tauri-apps/api/core'
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
const error = ref('')
const detailError = ref('')
const saveError = ref('')
const warnings = ref<string[]>([])
const savedPath = ref('')
let listEpoch = 0
let detailEpoch = 0
let alive = true

async function loadScenes() {
  if (saving.value) return
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
function toggle() {
  if (saving.value) return
  expanded.value = !expanded.value
  if (expanded.value && !loaded.value && !loading.value) void loadScenes()
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
    const result = await invoke<Detail>('get_game_content_detail', { vamRoot: root, source: scene.source })
    if (!alive || epoch !== detailEpoch || root !== app.vamRootPath) return
    detail.value = result
    characterIndex.value = result.characters.find(c => !c.error)?.index ?? result.characters[0]?.index ?? null
  } catch (e) { if (alive && epoch === detailEpoch) detailError.value = String(e) }
  finally { if (alive && epoch === detailEpoch) detailLoading.value = false }
}
watch(sceneKey, () => { void loadDetail() })
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
  if (!scene || !person || person.error || !revision || !root || saving.value) return
  const epoch = detailEpoch
  saving.value = true; emit('busyChange', true); saveError.value = ''; savedPath.value = ''
  try {
    const path = await invoke<string>('save_game_scene_appearance', { vamRoot: root, source: scene.source, revision, atomIndex: person.index, name: name.value })
    if (alive && epoch === detailEpoch && root === app.vamRootPath) savedPath.value = path
  } catch (e) { if (alive && epoch === detailEpoch && root === app.vamRootPath) saveError.value = String(e) }
  finally { saving.value = false; emit('busyChange', false) }
}
watch(() => app.vamRootPath, () => {
  ++listEpoch; ++detailEpoch
  scenes.value = []; sceneKey.value = ''; detail.value = null; loaded.value = false
  loading.value = false; detailLoading.value = false; error.value = ''; detailError.value = ''; savedPath.value = ''; saveError.value = ''; warnings.value = []
  expanded.value = false
})
onBeforeUnmount(() => { alive = false; ++listEpoch; ++detailEpoch; emit('busyChange', false) })
</script>

<template>
  <section class="scene-extractor" :aria-label="t('gameContent.extractCharacters')">
    <button class="extract-toggle" :aria-expanded="expanded" :disabled="saving" @click="toggle">
      <span>{{ t('gameContent.extractCharacters') }}</span><span aria-hidden="true">{{ expanded ? '−' : '+' }}</span>
    </button>
    <div v-if="expanded" class="extract-body" :aria-busy="loading || detailLoading || saving">
      <p v-if="loading" role="status">{{ t('gameContent.readingPackageScenes') }}</p>
      <div v-else-if="error"><p class="error" role="alert">{{ error }}</p><button @click="loadScenes">{{ t('gameContent.retry') }}</button></div>
      <p v-else-if="loaded && !scenes.length">{{ t('gameContent.noPackageScenes') }}</p>
      <template v-if="scenes.length">
        <label>{{ t('gameContent.selectPackageScene', { count: scenes.length }) }}
          <select v-model="sceneKey" :disabled="saving"><option v-for="scene in scenes" :key="scene.key" :value="scene.key">{{ scene.source.path.replace(/^Saves\/scene\//i, '') }}</option></select>
        </label>
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
            <select v-model="characterIndex" :disabled="saving"><option v-for="person in detail.characters" :key="person.index" :value="person.index">{{ person.name }} · {{ person.character || '—' }}</option></select>
          </label>
          <p v-if="character">{{ t('gameContent.characterContents', character) }}</p>
          <p v-if="character?.error" class="error" role="alert">{{ character.error }}</p>
          <template v-else>
            <label>{{ t('gameContent.filename') }}<input v-model="name" required maxlength="120" :disabled="saving" /></label>
            <button class="primary" :disabled="saving || !name.trim()" type="submit">{{ t(saving ? 'gameContent.working' : 'gameContent.saveAppearance') }}</button>
          </template>
        </form>
      </template>
      <p v-if="saveError" class="error" role="alert">{{ saveError }}</p>
      <div v-if="savedPath" class="saved" role="status"><p>{{ t('gameContent.appearanceSaved') }}</p><p class="path">{{ savedPath }}</p><RouterLink :to="{ path: '/appearances', query: { appearance: savedPath } }">{{ t('gameContent.viewAppearances') }}</RouterLink></div>
      <details v-if="warnings.length"><summary>{{ t('gameContent.warnings', { count: warnings.length }) }}</summary><p v-for="warning in warnings" :key="warning" class="error">{{ warning }}</p></details>
      <details v-if="scenes.length"><summary>{{ t('gameContent.extractScope') }}</summary><p>{{ t('gameContent.extractHelp') }}</p></details>
    </div>
  </section>
</template>

<style scoped>
.scene-extractor { flex-shrink: 0; min-width: 0; border: 1px solid var(--border-subtle); border-radius: var(--radius-md); background: var(--bg-surface); }
button, input, select { font: inherit; font-size: 12px; color: var(--text-primary); border: 1px solid var(--border-default); border-radius: var(--radius-sm); background: var(--bg-surface); min-height: 34px; padding: 7px 10px; }
button { cursor: pointer; }button:hover:not(:disabled) { background: var(--bg-hover); }button:disabled, input:disabled, select:disabled { opacity: .5; cursor: not-allowed; }
button:focus-visible, input:focus-visible, select:focus-visible, summary:focus-visible, a:focus-visible { outline: 2px solid var(--accent-primary); outline-offset: 2px; }
.extract-toggle { width: 100%; display: flex; align-items: center; justify-content: space-between; gap: 12px; border: 0; padding: 12px; font-weight: 600; text-align: left; }
.extract-body { display: flex; flex-direction: column; gap: 12px; padding: 0 12px 14px; }
.scene-preview { margin: 0; min-width: 0; }.scene-preview img { display: block; width: 100%; height: 120px; object-fit: contain; background: var(--bg-base); border-radius: var(--radius-sm); }
.scene-preview figcaption { margin-top: 6px; color: var(--text-tertiary); font-size: 11px; line-height: 1.5; }.preview-empty { padding: 8px 0; }
form { display: flex; flex-direction: column; gap: 10px; }
label { display: flex; flex-direction: column; gap: 6px; font-size: 12px; color: var(--text-secondary); }
select, input { width: 100%; min-width: 0; }select option { background: var(--bg-surface); }
p { font-size: 12px; color: var(--text-secondary); line-height: 1.6; overflow-wrap: anywhere; margin: 0; }
.primary { background: var(--accent-primary); color: white; border-color: transparent; }.primary:hover:not(:disabled) { background: var(--accent-hover, var(--accent-primary)); }
.error { color: var(--color-error); }.saved { display: flex; flex-direction: column; gap: 6px; }.path { user-select: text; }a { color: var(--accent-primary); font-size: 12px; }
summary { cursor: pointer; font-size: 12px; color: var(--text-secondary); }details p { margin-top: 8px; }
</style>
