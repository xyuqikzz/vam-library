import { ref, type Ref } from 'vue'

interface ToastItem {
  id: number
  type: 'success' | 'error' | 'warning' | 'info'
  title?: string
  message: string
  duration?: number
}

type ToastExpose = {
  success(message: string, title?: string): void
  error(message: string, title?: string): void
  warning(message: string, title?: string): void
  info(message: string, title?: string): void
  showToast(item: Omit<ToastItem, 'id'>): void
  removeToast(id: number): void
}

/**
 * Singleton reference to the Toast component instance.
 * Populated when AppLayout mounts the <Toast> element.
 */
export const toastRef: Ref<ToastExpose | null> = ref(null)

/**
 * Convenience composable for showing toast notifications.
 * Usage: const notify = useNotification(); notify.success('Done!');
 */
export function useNotification() {
  function send(type: ToastItem['type'], message: string, title?: string) {
    const t = toastRef.value
    if (!t) {
      console.warn(`[Toast] ${type}: ${title ?? ''} ${message}`)
      return
    }
    t.showToast({ type, message, title })
  }

  return {
    success: (msg: string, title?: string) => send('success', msg, title),
    error: (msg: string, title?: string) => send('error', msg, title),
    warning: (msg: string, title?: string) => send('warning', msg, title),
    info: (msg: string, title?: string) => send('info', msg, title),
  }
}
