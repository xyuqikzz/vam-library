<template>
  <div class="json-node" :class="{ nested: level > 0 }">
    <template v-if="isObjectNode">
      <div
        v-for="[key, value] in objectEntries"
        :key="key"
        class="field-row"
        :class="{ group: isObjectValue(value) }"
      >
        <div class="field-label">
          <span class="field-name">{{ fieldLabel(key) }}</span>
          <span class="field-meta">
            <span v-if="fieldLabel(key) !== key" class="field-key">{{ key }}</span>
            <span class="field-type">{{ valueType(value) }}</span>
          </span>
        </div>

        <JsonVisualEditor
          v-if="isObjectValue(value)"
          :model-value="value"
          :level="level + 1"
          @update:model-value="updateObjectField(key, $event)"
          @invalid="$emit('invalid', $event)"
        />

        <button
          v-else-if="typeof value === 'boolean'"
          type="button"
          :class="['toggle-switch', { on: value }]"
          @click="updateObjectField(key, !value)"
        >
          <span class="toggle-thumb" />
        </button>

        <input
          v-else-if="typeof value === 'number'"
          class="field-input number-input"
          type="number"
          :value="value"
          @input="updateObjectField(key, parseNumber(($event.target as HTMLInputElement).value, value))"
        />

        <textarea
          v-else-if="Array.isArray(value)"
          class="field-textarea"
          :value="formatArray(value)"
          spellcheck="false"
          @change="updateArrayField(key, ($event.target as HTMLTextAreaElement).value)"
        />

        <input
          v-else
          class="field-input"
          type="text"
          :value="stringValue(value)"
          @input="updateObjectField(key, ($event.target as HTMLInputElement).value)"
        />
      </div>
    </template>

    <textarea
      v-else-if="Array.isArray(modelValue)"
      class="field-textarea root-array"
      :value="formatArray(modelValue)"
      spellcheck="false"
      @change="updateRootArray(($event.target as HTMLTextAreaElement).value)"
    />

    <input
      v-else
      class="field-input"
      type="text"
      :value="stringValue(modelValue)"
      @input="$emit('update:modelValue', ($event.target as HTMLInputElement).value)"
    />
  </div>
</template>

<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'

defineOptions({ name: 'JsonVisualEditor' })

type JsonValue = null | boolean | number | string | JsonValue[] | { [key: string]: JsonValue }

const props = withDefaults(defineProps<{
  modelValue: JsonValue
  level?: number
}>(), {
  level: 0,
})

const emit = defineEmits<{
  'update:modelValue': [value: JsonValue]
  invalid: [message: string]
}>()

const { locale } = useI18n()

const isObjectNode = computed(() => isPlainObject(props.modelValue))
const objectEntries = computed(() => {
  if (!isPlainObject(props.modelValue)) return []
  return Object.entries(props.modelValue)
})

function updateObjectField(key: string, value: JsonValue) {
  if (!isPlainObject(props.modelValue)) return
  emit('update:modelValue', {
    ...props.modelValue,
    [key]: value,
  })
}

function updateArrayField(key: string, value: string) {
  try {
    const parsed = JSON.parse(value)
    if (!Array.isArray(parsed)) {
      emit('invalid', '数组字段必须填写 JSON 数组。')
      return
    }
    updateObjectField(key, parsed)
  } catch (e) {
    emit('invalid', String(e))
  }
}

function updateRootArray(value: string) {
  try {
    const parsed = JSON.parse(value)
    if (!Array.isArray(parsed)) {
      emit('invalid', '根节点必须填写 JSON 数组。')
      return
    }
    emit('update:modelValue', parsed)
  } catch (e) {
    emit('invalid', String(e))
  }
}

function isPlainObject(value: JsonValue): value is { [key: string]: JsonValue } {
  return typeof value === 'object' && value !== null && !Array.isArray(value)
}

function isObjectValue(value: JsonValue) {
  return isPlainObject(value)
}

function valueType(value: JsonValue) {
  if (Array.isArray(value)) return 'array'
  if (value === null) return 'null'
  return typeof value
}

function fieldLabel(key: string) {
  if (locale.value !== 'zh-CN') {
    return key
  }
  return zhFieldLabels[key] || humanizeFieldKey(key)
}

function stringValue(value: JsonValue) {
  if (value === null) return ''
  if (typeof value === 'string') return value
  return String(value)
}

function parseNumber(value: string, fallback: number) {
  const parsed = Number(value)
  return Number.isFinite(parsed) ? parsed : fallback
}

function formatArray(value: JsonValue[]) {
  return JSON.stringify(value, null, 2)
}

const zhFieldLabels: Record<string, string> = {
  alwaysAllowPluginsDownloadedFromHub: '始终允许从 Hub 下载的插件',
  alwaysAllowPluginsFromTrustedCreators: '始终允许受信任作者的插件',
  alwaysCheckForMissingPackages: '始终检查缺失包',
  alwaysRebuildUidCacheOnStart: '启动时始终重建 UID 缓存',
  alwaysUseMonitorCamera: '始终使用显示器相机',
  allowPluginsNetworkAccess: '允许插件访问网络',
  allowPluginsReadAccess: '允许插件读取文件',
  allowPluginsWriteAccess: '允许插件写入文件',
  allowPossessGrab: '允许附身抓取',
  allowPossessSelect: '允许附身选择',
  allowScenePlugins: '允许场景插件',
  allowScriptPluginLoading: '允许加载脚本插件',
  allowScriptsToAccessOtherPeople: '允许脚本访问其他人物',
  audioInputDevice: '音频输入设备',
  audioInputLevel: '音频输入音量',
  audioOutputDevice: '音频输出设备',
  autoLoadDefaultScene: '自动加载默认场景',
  autoLoadLastScene: '自动加载上次场景',
  autoSaveScene: '自动保存场景',
  browserCameraEnabled: '启用浏览器相机',
  cacheFolder: '缓存文件夹',
  cachePath: '缓存路径',
  creatorName: '作者名称',
  custom: '自定义',
  debug: '调试',
  disableAllPlugins: '禁用所有插件',
  disablePhysics: '禁用物理',
  displayIndex: '显示器编号',
  enableCaching: '启用缓存',
  enabled: '启用',
  enableDepthTexture: '生成深度纹理',
  enableMirrorReflections: '启用镜面反射',
  enablePlugins: '启用插件',
  enableSoftPhysics: '软体物理',
  enableSound: '启用声音',
  enableWebBrowser: '启用内置浏览器',
  forceDesktopMode: '强制桌面模式',
  freeMoveMultiplier: '自由移动倍率',
  fullscreen: '全屏',
  gamepadEnabled: '启用手柄',
  graphicsQuality: '图形质量',
  hairPhysicsQuality: '头发物理质量',
  highQualityPhysics: '高质量物理',
  language: '语言',
  lastBrowseDir: '上次浏览目录',
  lastLoadDir: '上次加载目录',
  lastSaveDir: '上次保存目录',
  launchInVR: '以 VR 模式启动',
  logLevel: '日志级别',
  maxFileBrowserCacheSize: '文件浏览器最大缓存',
  maxTextureSize: '最大贴图尺寸',
  mirrorReflection: '镜面反射',
  monitorHeight: '显示器高度',
  monitorWidth: '显示器宽度',
  msaa: 'MSAA 抗锯齿',
  msaaLevel: 'MSAA 等级',
  physicsRate: '物理更新补偿',
  physicsUpdateCap: '物理更新上限',
  pixelLightCount: '像素灯光数',
  pluginsAlwaysEnabled: '插件始终启用',
  preloadMorphs: '预加载变形',
  processPriority: '进程优先级',
  qualityLevel: '质量等级',
  realtimeReflectionProbes: '实时反射探测点',
  refreshRate: '刷新率',
  renderScale: '渲染比例',
  renderScaleDesktop: '桌面渲染比例',
  renderScaleVR: 'VR 渲染比例',
  resetCachePath: '重置缓存路径',
  safeMode: '安全模式',
  screenshotDirectory: '截图目录',
  shadowQuality: '阴影质量',
  softPhysics: '软体物理',
  softPhysicsUpdateCap: '软体物理更新上限',
  targetFrameRate: '目标帧率',
  textureQuality: '贴图质量',
  uiScale: '界面缩放',
  useCache: '使用缓存',
  useMonitorView: '使用显示器视图',
  usePhysics: '使用物理',
  useRealtimeReflectionProbes: '实时反射探测点',
  useSoftPhysics: '使用软体物理',
  userName: '用户名',
  verticalSync: '垂直同步',
  vr: 'VR',
  vrHandChoice: 'VR 手柄选择',
  vrRenderScale: 'VR 渲染比例',
  windowHeight: '窗口高度',
  windowWidth: '窗口宽度',
}

function humanizeFieldKey(key: string) {
  return key
    .replace(/([a-z0-9])([A-Z])/g, '$1 $2')
    .replace(/[_-]+/g, ' ')
    .trim()
}
</script>

<style scoped>
.json-node {
  display: flex;
  flex-direction: column;
  gap: var(--space-3);
}

.json-node.nested {
  width: 100%;
  padding: var(--space-3);
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-sm);
  background: rgba(255, 255, 255, 0.02);
}

.field-row {
  display: grid;
  grid-template-columns: minmax(180px, 260px) minmax(0, 1fr);
  align-items: center;
  gap: var(--space-4);
  padding: var(--space-3);
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-sm);
  background: var(--bg-subtle);
}

.field-row.group {
  align-items: flex-start;
}

.field-label {
  display: flex;
  flex-direction: column;
  gap: 3px;
  min-width: 0;
}

.field-name {
  overflow: hidden;
  color: var(--text-primary);
  font-size: var(--text-sm);
  font-weight: var(--font-medium);
  text-overflow: ellipsis;
  white-space: nowrap;
}

.field-meta {
  display: flex;
  flex-wrap: wrap;
  gap: var(--space-2);
  align-items: center;
}

.field-key,
.field-type {
  color: var(--text-tertiary);
  font-family: var(--font-mono);
  font-size: 10px;
}

.field-input,
.field-textarea {
  width: 100%;
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-sm);
  background: var(--bg-input);
  color: var(--text-primary);
  font-size: var(--text-sm);
  outline: none;
}

.field-input {
  height: 32px;
  padding: 0 var(--space-3);
}

.number-input {
  max-width: 180px;
}

.field-textarea {
  min-height: 92px;
  padding: var(--space-3);
  resize: vertical;
  font-family: var(--font-mono);
  font-size: 11px;
  line-height: 1.5;
}

.root-array {
  min-height: 280px;
}

.field-input:focus,
.field-textarea:focus {
  border-color: var(--accent-primary);
}

.toggle-switch {
  position: relative;
  width: 44px;
  height: 24px;
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-full);
  background: var(--bg-hover);
  cursor: pointer;
  transition:
    background var(--duration-base) var(--ease),
    border-color var(--duration-base) var(--ease);
}

.toggle-switch.on {
  border-color: var(--accent-primary);
  background: var(--accent-primary);
}

.toggle-thumb {
  position: absolute;
  top: 2px;
  left: 2px;
  width: 18px;
  height: 18px;
  border-radius: 50%;
  background: white;
  box-shadow: 0 1px 3px rgba(0, 0, 0, 0.3);
  transition: transform var(--duration-base) var(--ease);
}

.toggle-switch.on .toggle-thumb {
  transform: translateX(20px);
}

@media (max-width: 760px) {
  .field-row {
    grid-template-columns: 1fr;
  }
}
</style>
