<script setup lang="ts">
import UiButton from '@/components/ui/UiButton.vue'
import Icon from '@/components/ui/Icon.vue'

import { useI18n } from 'vue-i18n'

defineProps<{
  vamRoot: string | null
  screenshotPath: string | null
  appearancePresetPath: string | null
  clothingPresetPath: string | null
}>()

defineEmits<{
  open: [path: string]
  gameConfig: []
}>()

useI18n()
</script><template><div class="quick-actions"><UiButton size="lg" :disabled="!vamRoot" @click="vamRoot && $emit('open', vamRoot)"><span class="quick-icon"><Icon name="folder" :size="16" /></span>{{ $t('dashboard.gameRoot') }}</UiButton><UiButton size="lg" :disabled="!screenshotPath" @click="screenshotPath && $emit('open', screenshotPath)"><span class="quick-icon"><Icon name="image" :size="16" /></span>{{ $t('dashboard.screenshotDirectory') }}</UiButton><UiButton size="lg" :disabled="!appearancePresetPath" @click="appearancePresetPath && $emit('open', appearancePresetPath)"><span class="quick-icon"><Icon name="user" :size="16" /></span>{{ $t('dashboard.appearancePresetDirectory') }}</UiButton><UiButton size="lg" :disabled="!clothingPresetPath" @click="clothingPresetPath && $emit('open', clothingPresetPath)"><span class="quick-icon"><Icon name="package" :size="16" /></span>{{ $t('dashboard.clothingPresetDirectory') }}</UiButton><UiButton v-if="vamRoot" size="lg" @click="$emit('gameConfig')"><span class="quick-icon"><Icon name="settings" :size="16" /></span>{{ $t('dashboard.gameConfig') }}</UiButton></div></template><style scoped>.quick-actions { display: flex; flex-wrap: wrap; gap: 8px; }.quick-actions :deep(button) { justify-content: flex-start; padding: 0 14px 0 6px; }.quick-icon { display: grid; place-items: center; width: 32px; height: 32px; border-radius: var(--radius-full); background: var(--bg-active); color: var(--text-secondary); }</style>