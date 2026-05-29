// @ts-ignore
import { createI18n } from 'vue-i18n'
import zhCN from './locales/zh-CN'
import enUS from './locales/en-US'

const savedLocale = typeof window !== 'undefined'
  ? window.localStorage.getItem('vamlibrary-locale')
  : null

const locale = savedLocale === 'en-US' || savedLocale === 'zh-CN'
  ? savedLocale
  : 'zh-CN'

const i18n = createI18n({
  legacy: false,
  locale,
  fallbackLocale: 'en-US',
  messages: {
    'zh-CN': zhCN,
    'en-US': enUS,
  },
})

export default i18n
