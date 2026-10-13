import '@fontsource-variable/manrope'
import { createApp } from 'vue'
import { createPinia } from 'pinia'
import i18n from './i18n'
import router from './router'
import App from './App.vue'
import { showFatalError } from './utils/startupError'

import './assets/styles/variables.css'
import './assets/styles/base.css'
import './assets/styles/typography.css'
import './assets/styles/glass.css'
import './assets/styles/animations.css'

async function bootstrap() {
  const app = createApp(App)

  app.config.errorHandler = (error) => {
    console.error('Vue runtime error:', error)
    showFatalError(error, i18n.global.locale.value)
  }

  app.use(createPinia())
  app.use(i18n)
  app.use(router)

  app.mount('#app')
}

window.addEventListener('error', (event) => {
  showFatalError(event.error || event.message, i18n.global.locale.value)
})

window.addEventListener('unhandledrejection', (event) => {
  showFatalError(event.reason, i18n.global.locale.value)
})

bootstrap().catch((error) => {
  console.error('Bootstrap failed:', error)
  showFatalError(error, i18n.global.locale.value)
})
