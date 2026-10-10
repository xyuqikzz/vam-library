import '@fontsource-variable/manrope'
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
    return JSON.stringify(error) ?? String(error)
  } catch {
    return String(error)
  }
}

function showFatalError(error: unknown) {
  const message = stringifyError(error)
  const root = document.querySelector('#app')
  if (!root) return

  root.innerHTML = `
    <div class="startup-fatal">
      <div class="startup-fatal-card">
        <strong class="startup-fatal-title">应用启动失败</strong>
        <p class="startup-fatal-description">请截图或复制以下错误信息反馈，应用不会再静默白屏。</p>
        <pre class="startup-fatal-details">${message.replace(/[&<>"']/g, (char) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' }[char] || char))}</pre>
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
