import { createRouter, createWebHistory } from 'vue-router'
import AppLayout from '@/components/layout/AppLayout.vue'

const router = createRouter({
  history: createWebHistory(),
  routes: [
    {
      path: '/',
      component: AppLayout,
      redirect: '/packages',
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
          meta: { title: 'Scenes' },
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
          name: 'on-demand',
          component: () => import('@/views/OnDemandLaunchView.vue'),
          meta: { title: 'On Demand Launch' },
        },
        {
          path: 'migration',
          name: 'migration',
          component: () => import('@/views/MigrationView.vue'),
          meta: { title: 'Migration' },
        },
        {
          path: 'unpack',
          name: 'unpack',
          component: () => import('@/views/UnpackView.vue'),
          meta: { title: 'Smart Unpack' },
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
