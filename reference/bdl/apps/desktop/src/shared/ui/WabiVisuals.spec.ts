import { mount } from '@vue/test-utils'
import { expect, it } from 'vitest'
import WabiVisuals from './WabiVisuals.vue'

it('mounts live refraction only while the specification panel is open', async () => {
  const wrapper = mount(WabiVisuals, {
    props: { modelValue: false },
    global: { stubs: { UiDialog: { template: '<div><slot /></div>' }, UiProgressBar: true } },
  })
  expect(wrapper.find('.wabi-filter-defs').exists()).toBe(false)
  expect(wrapper.find('.wabi-lens').exists()).toBe(false)
  expect(wrapper.find('.wabi-ambient').exists()).toBe(false)
  await wrapper.setProps({ modelValue: true })
  expect(wrapper.find('#wabi-liquid-refract').exists()).toBe(true)
  expect(wrapper.find('.wabi-lens').exists()).toBe(true)
  await wrapper.setProps({ modelValue: false })
  expect(wrapper.find('#wabi-liquid-refract').exists()).toBe(false)
  expect(wrapper.find('.wabi-lens').exists()).toBe(false)
  wrapper.unmount()
})
