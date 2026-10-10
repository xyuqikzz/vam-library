<script setup lang="ts">
import { onBeforeUnmount, ref } from 'vue'
import { useI18n } from 'vue-i18n'
import Icon from '@/components/ui/Icon.vue'
import UiButton from '@/components/ui/UiButton.vue'
export interface ToastItem { id: number; type: 'success' | 'error' | 'warning' | 'info'; title?: string; message: string; duration?: number; action?: { label: string; onClick: () => void | Promise<void> } }
const { t } = useI18n()
const toasts = ref<ToastItem[]>([])
const timers = new Map<number, ReturnType<typeof setTimeout>>()
let nextId = 0
function removeToast(id: number) { clearTimeout(timers.get(id)); timers.delete(id); toasts.value = toasts.value.filter(toast => toast.id !== id) }
function showToast(item: Omit<ToastItem, 'id'>) { const id = nextId++; toasts.value.push({ ...item, id }); const duration = item.type === 'error' ? 0 : item.duration ?? 4000; if (duration > 0) timers.set(id, setTimeout(() => removeToast(id), duration)) }
function success(message: string, title?: string) { showToast({ type: 'success', message, title }) }
function error(message: string, title?: string) { showToast({ type: 'error', message, title }) }
function warning(message: string, title?: string) { showToast({ type: 'warning', message, title }) }
function info(message: string, title?: string) { showToast({ type: 'info', message, title }) }
async function runAction(toast: ToastItem) { try { await toast.action?.onClick(); removeToast(toast.id) } catch (reason) { error(String(reason)) } }
onBeforeUnmount(() => { for (const timer of timers.values()) clearTimeout(timer); timers.clear() })
defineExpose({ success, error, warning, info, showToast, removeToast })
</script>
<template><Teleport to="body"><TransitionGroup name="toast-slide" tag="div" class="toast-container"><div v-for="toast in toasts" :key="toast.id" class="toast-item" :class="toast.type" :role="toast.type === 'error' ? 'alert' : 'status'"><span class="toast-icon"><Icon :name="toast.type === 'error' ? 'warning' : toast.type === 'warning' ? 'info' : 'check'" :size="14" /></span><div class="toast-body"><strong v-if="toast.title">{{ toast.title }}</strong><span>{{ toast.message }}</span></div><UiButton v-if="toast.action" size="sm" class="toast-action" @click="runAction(toast)">{{ toast.action.label }}</UiButton><UiButton variant="ghost" size="sm" icon="close" icon-only :aria-label="t('common.close')" class="toast-close" @click="removeToast(toast.id)" /></div></TransitionGroup></Teleport></template>
<style scoped>
.toast-container { position: fixed; right: var(--space-6); bottom: var(--space-6); z-index: var(--z-toast); display: flex; flex-direction: column-reverse; gap: var(--space-2); max-width: min(480px, calc(100vw - 48px)); pointer-events: none; }.toast-item { display: flex; align-items: center; gap: var(--space-3); min-height: 52px; padding: 10px 12px 10px 18px; border-radius: var(--radius-full); background: var(--text-primary); color: var(--text-inverse); box-shadow: var(--shadow-lg); pointer-events: auto; }.toast-item.error { background: var(--color-error-surface); color: var(--text-primary); box-shadow: var(--shadow-lg), inset 0 0 0 1px var(--color-error-border); border-radius: var(--radius-lg); }.toast-icon { width: 22px; height: 22px; display: grid; place-items: center; flex-shrink: 0; border-radius: var(--radius-full); background: var(--color-success); color: var(--text-inverse); }.error .toast-icon { background: var(--color-error-bg); color: var(--color-error); }.warning .toast-icon { background: var(--color-warning); color: var(--color-warning-ink); }.toast-body { display: flex; flex: 1; flex-direction: column; min-width: 0; overflow-wrap: anywhere; font-size: var(--text-base); }.toast-body strong { font-weight: var(--font-heavy); }.error .toast-body span { color: var(--text-secondary); font-size: var(--text-sm); }.toast-close { color: inherit; }.toast-action { background: var(--text-inverse); color: var(--text-primary); }.toast-slide-enter-active,.toast-slide-leave-active { transition: opacity var(--duration-base), transform var(--duration-base); }.toast-slide-enter-from,.toast-slide-leave-to { opacity: 0; transform: translateX(20px); }
</style>
