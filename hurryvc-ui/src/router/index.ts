import { createRouter, createWebHashHistory } from 'vue-router'

import AuthView from '@/views/AuthView.vue'
import SessionsView from '@/views/SessionsView.vue'
import TerminalView from '@/views/TerminalView.vue'

const router = createRouter({
  history: createWebHashHistory(),
  routes: [
    {
      path: '/',
      name: 'auth',
      component: AuthView,
    },
    {
      path: '/sessions',
      name: 'sessions',
      component: SessionsView,
    },
    {
      path: '/sessions/:producerId',
      name: 'terminal',
      component: TerminalView,
      props: true,
    },
  ],
})

export default router
