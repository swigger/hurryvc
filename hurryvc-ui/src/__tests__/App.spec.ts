import { beforeEach, describe, expect, it } from 'vitest'

import { createPinia } from 'pinia'
import { mount } from '@vue/test-utils'

import App from '../App.vue'
import router from '../router'

describe('App', () => {
  beforeEach(async () => {
    router.push('/')
    await router.isReady()
  })

  it('renders the auth shell on default route', async () => {
    const wrapper = mount(App, {
      global: {
        plugins: [createPinia(), router],
      },
    })
    expect(wrapper.text()).toContain('接管正在运行的终端')
  })
})
