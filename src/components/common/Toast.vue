<template>
  <Teleport to="body">
    <TransitionGroup name="toast-slide" tag="div" class="toast-container">
      <div
        v-for="toast in toasts"
        :key="toast.id"
        :class="['toast-item', toast.type]"
        @click="removeToast(toast.id)"
      >
        <!-- Icon -->
        <div class="toast-icon">
          <!-- Success -->
          <svg v-if="toast.type === 'success'" width="18" height="18" viewBox="0 0 24 24" fill="none">
            <path d="M22 11.08V12a10 10 0 1 1-5.93-9.14" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" />
            <path d="M22 4L12 14.01l-3-3" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" />
          </svg>
          <!-- Error -->
          <svg v-else-if="toast.type === 'error'" width="18" height="18" viewBox="0 0 24 24" fill="none">
            <path d="M12 22c5.523 0 10-4.477 10-10S17.523 2 12 2 2 6.477 2 12s4.477 10 10 10Z" stroke="currentColor" stroke-width="1.5" />
            <path d="M15 9l-6 6M9 9l6 6" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" />
          </svg>
          <!-- Warning -->
          <svg v-else-if="toast.type === 'warning'" width="18" height="18" viewBox="0 0 24 24" fill="none">
            <path d="M12 22c5.523 0 10-4.477 10-10S17.523 2 12 2 2 6.477 2 12s4.477 10 10 10Z" stroke="currentColor" stroke-width="1.5" />
            <path d="M12 8v4M12 16h.01" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" />
          </svg>
          <!-- Info -->
          <svg v-else width="18" height="18" viewBox="0 0 24 24" fill="none">
            <path d="M12 22c5.523 0 10-4.477 10-10S17.523 2 12 2 2 6.477 2 12s4.477 10 10 10Z" stroke="currentColor" stroke-width="1.5" />
            <path d="M12 16v-4M12 8h.01" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" />
          </svg>
        </div>

        <!-- Message -->
        <div class="toast-body">
          <span v-if="toast.title" class="toast-title">{{ toast.title }}</span>
          <span class="toast-message">{{ toast.message }}</span>
        </div>

        <!-- Close -->
        <button class="toast-close" @click.stop="removeToast(toast.id)">
          <svg width="14" height="14" viewBox="0 0 14 14" fill="none">
            <path d="M4 4l6 6M10 4l-6 6" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" />
          </svg>
        </button>
      </div>
    </TransitionGroup>
  </Teleport>
</template>

<script setup lang="ts">
import { ref } from 'vue'

export interface ToastItem {
  id: number
  type: 'success' | 'error' | 'warning' | 'info'
  title?: string
  message: string
  duration?: number
}

const toasts = ref<ToastItem[]>([])
let nextId = 0

function showToast(item: Omit<ToastItem, 'id'>) {
  const id = nextId++
  const duration = item.duration ?? 4000
  const toast: ToastItem = { ...item, id }

  toasts.value.push(toast)

  if (duration > 0) {
    setTimeout(() => {
      removeToast(id)
    }, duration)
  }
}

function removeToast(id: number) {
  const idx = toasts.value.findIndex(t => t.id === id)
  if (idx !== -1) {
    toasts.value.splice(idx, 1)
  }
}

// Convenience methods
function success(message: string, title?: string) {
  showToast({ type: 'success', message, title })
}

function error(message: string, title?: string) {
  showToast({ type: 'error', message, title, duration: 6000 })
}

function warning(message: string, title?: string) {
  showToast({ type: 'warning', message, title })
}

function info(message: string, title?: string) {
  showToast({ type: 'info', message, title, duration: 3000 })
}

defineExpose({ success, error, warning, info, showToast, removeToast })
</script>

<style scoped>
.toast-container {
  position: fixed;
  bottom: 24px;
  right: 24px;
  z-index: 9999;
  display: flex;
  flex-direction: column-reverse;
  gap: var(--space-2);
  pointer-events: none;
  max-width: 380px;
}

.toast-item {
  display: flex;
  align-items: flex-start;
  gap: var(--space-3);
  padding: var(--space-3) var(--space-4);
  border-radius: var(--radius-lg);
  background: var(--bg-elevated);
  border: 1px solid var(--border-default);
  box-shadow: var(--glass-shadow-lg);
  pointer-events: all;
  cursor: pointer;
  transition: background var(--duration-fast) var(--ease);
}

.toast-item:hover {
  background: var(--bg-hover);
}

.toast-icon {
  flex-shrink: 0;
  margin-top: 1px;
}

.toast-item.success .toast-icon { color: var(--color-success); }
.toast-item.error .toast-icon { color: var(--color-error); }
.toast-item.warning .toast-icon { color: var(--color-warning); }
.toast-item.info .toast-icon { color: var(--color-info); }

.toast-body {
  flex: 1;
  min-width: 0;
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.toast-title {
  font-size: var(--text-sm);
  font-weight: var(--font-semibold);
  color: var(--text-primary);
}

.toast-message {
  font-size: var(--text-xs);
  color: var(--text-secondary);
  line-height: 1.4;
}

.toast-close {
  flex-shrink: 0;
  padding: 2px;
  border-radius: var(--radius-sm);
  color: var(--text-tertiary);
  transition: color var(--duration-fast) var(--ease);
  margin-top: 1px;
}

.toast-close:hover {
  color: var(--text-primary);
}

/* ── Transitions ──────────────────────────────────────────── */
.toast-slide-enter-active {
  transition: all 300ms var(--ease);
}

.toast-slide-leave-active {
  transition: all 200ms var(--ease);
}

.toast-slide-enter-from {
  opacity: 0;
  transform: translateX(40px) scale(0.95);
}

.toast-slide-leave-to {
  opacity: 0;
  transform: translateX(20px) scale(0.95);
}

.toast-slide-move {
  transition: transform 200ms var(--ease);
}
</style>
