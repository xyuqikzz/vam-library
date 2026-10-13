<script setup lang="ts">
import { onBeforeUnmount, useId } from 'vue'
import { useI18n } from 'vue-i18n'
import { confirmation, dismissConfirmations, resolveConfirmation } from '@/composables/useConfirm'
import UiModal from './UiModal.vue'
import UiButton from './UiButton.vue'
import Icon from './Icon.vue'
const { t } = useI18n()
const titleId = useId()
const messageId = useId()
onBeforeUnmount(dismissConfirmations)
</script>
<template>
  <!-- 规范：可选 44px 圆形图标 → 标题 → 说明 → 右对齐按钮组 -->
  <UiModal
    v-if="confirmation"
    :aria-labelledby="titleId"
    :aria-describedby="messageId"
    :show-close="false"
    @close="resolveConfirmation(false)"
  >
    <div v-if="confirmation.danger" class="confirm-icon"><Icon name="warning" :size="22" /></div>
    <h2 :id="titleId" class="confirm-title">{{ confirmation.title }}</h2>
    <p :id="messageId" class="confirm-message">{{ confirmation.message }}</p>
    <template #footer>
      <UiButton autofocus @click="resolveConfirmation(false)">
        {{ confirmation.cancelText || t('common.cancel') }}
      </UiButton>
      <UiButton
        :variant="confirmation.danger ? 'danger' : 'primary'"
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
  margin-bottom: var(--space-4);
  border-radius: var(--radius-full);
  color: var(--color-error);
  background: var(--color-error-bg);
}
.confirm-title {
  font-size: var(--text-dialog);
  font-weight: var(--font-heavy);
  line-height: var(--leading-tight);
}
.confirm-message {
  margin-top: var(--space-2);
  color: var(--text-secondary);
  font-size: var(--text-description);
  line-height: var(--leading-relaxed);
  overflow-wrap: anywhere;
  white-space: pre-line;
}
</style>
