<script setup lang="ts">
import { useRouter, useRoute } from 'vue-router'
import { useI18n } from 'vue-i18n'
import Icon from '@/components/ui/Icon.vue'
import UiButton from '@/components/ui/UiButton.vue'
defineProps<{ icon?: string; title: string; description?: string }>()
const router = useRouter()
const route = useRoute()
const { t } = useI18n()
function nextStep() { router.push(route.path === '/packages' ? '/settings' : '/packages') }
</script>
<template><div class="empty-state"><div class="empty-icon-wrapper"><Icon :path="icon" :size="28" /></div><h3 class="empty-title">{{ title }}</h3><p v-if="description" class="empty-description">{{ description }}</p><div class="empty-action"><slot name="action"><UiButton variant="primary" size="lg" @click="nextStep">{{ route.path === '/packages' ? t('packages.selectDirectory') : t('ui.browsePackages') }}</UiButton></slot></div></div></template>
<style scoped>
.empty-state { display: flex; flex-direction: column; align-items: center; justify-content: center; padding: var(--space-8) var(--space-6); text-align: center; background: var(--bg-elevated); border-radius: var(--radius-xl); }.empty-icon-wrapper { display: grid; place-items: center; width: 56px; height: 56px; border-radius: var(--radius-full); background: var(--bg-control); color: var(--text-tertiary); margin-bottom: var(--space-4); }.empty-title { font-size: var(--text-lg); font-weight: var(--font-heavy); color: var(--text-primary); margin-bottom: var(--space-2); }.empty-description { font-size: var(--text-base); color: var(--text-secondary); max-width: 360px; line-height: var(--leading-relaxed); }.empty-action { margin-top: var(--space-5); }
</style>
