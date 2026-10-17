import { createPinia, setActivePinia } from 'pinia'
import { flushPromises, mount } from '@vue/test-utils'
import { nextTick } from 'vue'
import { expect, it, vi } from 'vitest'
import AppWorkspace from './AppWorkspace.vue'
import { useUiStore } from '../shared/stores/ui'

const coldPage = vi.hoisted(() => {
  let ready!: () => void
  const pending = new Promise<void>(resolve => { ready = resolve })
  return { pending, ready }
})
vi.mock('../shared/platform/environment', () => ({ isMobilePlatform: () => false }))
vi.mock('../shared/ui/BackgroundParseStatus.vue', () => ({ default: { template: '<div />' } }))
vi.mock('../desktop/pages/BilibiliPage.vue', async () => {
  const { ref } = await import('vue')
  return { __esModule: true, default: { setup: () => ({ count: ref(0) }), template: '<button class="cached-page" @click="count++">{{ count }}</button>' } }
})
vi.mock('../desktop/pages/CreationPage.vue', async () => {
  await coldPage.pending
  return { __esModule: true, default: { template: '<div class="cold-page">创作</div>' } }
})

it('shows cold-page loading immediately and preserves prior page state without a leave transition', async () => {
  setActivePinia(createPinia())
  const ui = useUiStore()
  const wrapper = mount(AppWorkspace)
  await flushPromises()
  await wrapper.get('.cached-page').trigger('click')
  expect(wrapper.get('.cached-page').text()).toBe('1')
  ui.setTab('creation')
  await nextTick()
  expect(wrapper.find('.cached-page').exists()).toBe(false)
  expect(wrapper.get('[role="status"]').text()).toBe('正在准备工作区')
  coldPage.ready()
  await flushPromises()
  expect(wrapper.get('.cold-page').text()).toBe('创作')
  ui.setTab('bilibili')
  await nextTick()
  await flushPromises()
  expect(wrapper.get('.cached-page').text()).toBe('1')
  wrapper.unmount()
})
