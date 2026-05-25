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

const app = createApp(App)

app.use(createPinia())
app.use(i18n)
app.use(router)

app.mount('#app')
