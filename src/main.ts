import { createApp } from 'vue'
import { createPinia } from 'pinia'
import i18n from './i18n'
import router from './router'
import App from './App.vue'

import './assets/styles/variables.css'
import './assets/styles/base.css'
import './assets/styles/typography.css'
import './assets/styles/glass.css'
import './assets/styles/animations.css'

function stringifyError(error: unknown): string {
  if (error instanceof Error) return error.stack || error.message
  if (typeof error === 'string') return error
  try {
    return JSON.stringify(error)
  } catch {
    return String(error)
  }
}

function showFatalError(error: unknown) {
  const message = stringifyError(error)
  const root = document.querySelector('#app')
  if (!root) return

  root.innerHTML = `
    <div style="display:flex;align-items:center;justify-content:center;width:100%;height:100%;padding:24px;background:#161618;color:#f5f5f7;font-family:-apple-system,BlinkMacSystemFont,'Segoe UI',sans-serif;">
      <div style="width:min(520px,100%);padding:20px;border:1px solid rgba(255,69,58,.28);border-radius:12px;background:#1c1c1e;box-shadow:0 8px 24px rgba(0,0,0,.35);">
        <strong style="display:block;margin-bottom:8px;color:#ff453a;font-size:16px;">应用启动失败</strong>
        <p style="margin:0 0 12px;color:#a1a1a6;font-size:13px;line-height:1.6;">请截图或复制以下错误信息反馈，应用不会再静默白屏。</p>
        <pre style="max-height:260px;overflow:auto;white-space:pre-wrap;user-select:text;margin:0;padding:12px;border-radius:8px;background:#111;color:#ddd;font-size:12px;line-height:1.5;">${message.replace(/[&<>"']/g, (char) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' }[char] || char))}</pre>
      </div>
    </div>
  `
}

async function bootstrap() {
  const app = createApp(App)

  app.config.errorHandler = (error) => {
    console.error('Vue runtime error:', error)
    showFatalError(error)
  }

  app.use(createPinia())
  app.use(i18n)
  app.use(router)

  app.mount('#app')
}

window.addEventListener('error', (event) => {
  showFatalError(event.error || event.message)
})

window.addEventListener('unhandledrejection', (event) => {
  showFatalError(event.reason)
})

bootstrap().catch((error) => {
  console.error('Bootstrap failed:', error)
  showFatalError(error)
})
