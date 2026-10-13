<script setup lang="ts">
import { useI18n } from 'vue-i18n'
import UiButton from '@/components/ui/UiButton.vue'

defineProps<{ favorite: boolean; disabled?: boolean; loading?: boolean }>()
const emit = defineEmits<{ toggle: [] }>()
const { t } = useI18n()
</script>

<template>
  <UiButton
    class="scene-favorite-action"
    :class="{ 'is-favorite': favorite }"
    variant="ghost"
    size="sm"
    icon="star"
    icon-only
    :aria-label="t(favorite ? 'gameContent.unfavorite' : 'gameContent.favorite')"
    :aria-pressed="favorite"
    :disabled="disabled"
    :loading="loading"
    @click="emit('toggle')"
  />
</template>

<style scoped>
.ui-button.scene-favorite-action {
  background: var(--bg-on-image);
}
.scene-favorite-action.is-favorite {
  color: var(--accent-primary);
}
.is-favorite :deep(svg) {
  fill: currentColor;
}
</style>
