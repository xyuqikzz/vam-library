import { createRouter, createWebHashHistory } from 'vue-router'
import AppLayout from '@/components/layout/AppLayout.vue'

const router = createRouter({
  history: createWebHashHistory(),
  routes: [
    {
      path: '/',
      component: AppLayout,
      redirect: '/dashboard',
      children: [
        {
          path: 'dashboard',
          name: 'dashboard',
          component: () => import('@/views/DashboardView.vue'),
          meta: { title: 'Dashboard' },
        },
        {
          path: 'packages',
          name: 'packages',
          component: () => import('@/views/PackagesView.vue'),
          meta: { title: 'Packages' },
        },
        {
          path: 'statistics',
          name: 'statistics',
          component: () => import('@/views/StatisticsView.vue'),
          meta: { title: 'Statistics' },
        },
        {
          path: 'online',
          name: 'online',
          component: () => import('@/views/OnlineHubView.vue'),
          meta: { title: 'Online Hub' },
        },
        {
          path: 'scenes',
          name: 'scenes',
          component: () => import('@/views/ScenesView.vue'),
          meta: { title: 'Favorites' },
        },
        {
          path: 'appearances',
          name: 'appearances',
          component: () => import('@/views/AppearancesView.vue'),
          meta: { title: 'Appearances' },
        },

        {
          path: 'dependency-completion',
          name: 'dependency-completion',
          component: () => import('@/views/DependencyCompletionView.vue'),
          meta: { title: 'Dependency Completion' },
        },
        {
          path: 'deduplication',
          name: 'deduplication',
          component: () => import('@/views/DeduplicationView.vue'),
          meta: { title: 'Deduplication' },
        },
        {
          path: 'on-demand',
          redirect: '/ingestion',
        },
        {
          path: 'migration',
          name: 'migration',
          component: () => import('@/views/MigrationView.vue'),
          meta: { title: 'Migration' },
        },
        {
          path: 'unpack',
          redirect: '/ingestion',
        },
        {
          path: 'ingestion',
          name: 'ingestion',
          component: () => import('@/views/IngestionView.vue'),
          meta: { title: 'Smart Import' },
        },
        {
          path: 'vam-prefs',
          name: 'vam-prefs',
          component: () => import('@/views/VamPrefsView.vue'),
          meta: { title: 'VAM Preferences' },
        },
        {
          path: 'trash',
          name: 'trash',
          component: () => import('@/views/TrashView.vue'),
          meta: { title: 'Recycle Bin' },
        },
        {
          path: 'settings',
          name: 'settings',
          component: () => import('@/views/SettingsView.vue'),
          meta: { title: 'Settings' },
        },
        {
          path: 'download',
          name: 'download',
          component: () => import('@/views/DownloadCenterView.vue'),
          meta: { title: 'Download Center' },
        },
      ],
    },
  ],
})

export default router
