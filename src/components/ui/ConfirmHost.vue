<script setup lang="ts">
import { onBeforeUnmount } from 'vue'
import { useI18n } from 'vue-i18n'
import { confirmation, dismissConfirmations, resolveConfirmation } from '@/composables/useConfirm'
import UiModal from './UiModal.vue'
import UiButton from './UiButton.vue'
import Icon from './Icon.vue'
const { t } = useI18n()
onBeforeUnmount(dismissConfirmations)
</script>
<template>
  <UiModal
    v-if="confirmation"
    :title="confirmation.title"
    :description="confirmation.message"
    :show-close="false"
    @close="resolveConfirmation(false)"
  >
    <div v-if="confirmation.danger" class="confirm-icon"><Icon name="warning" :size="22" /></div>
    <template #footer>
      <UiButton @click="resolveConfirmation(false)">
        {{ confirmation.cancelText || t('common.cancel') }}
      </UiButton>
      <UiButton
        :variant="confirmation.danger ? 'danger' : 'primary'"
        :class="{ 'confirm-danger': confirmation.danger }"
        @click="resolveConfirmation(true)"
      >
        {{ confirmation.confirmText || t('common.confirm') }}
      </UiButton>
    </template>
  </UiModal>
</template>
<style scoped>
.confirm-icon {
  display: grid;
  place-items: center;
  width: 44px;
  height: 44px;
  border-radius: var(--radius-full);
  color: var(--color-error);
  background: var(--color-error-bg);
}
.confirm-danger {
  background: var(--color-error);
  color: var(--color-error-ink);
}
</style>
