import { flushPromises, shallowMount } from '@vue/test-utils'
import { createPinia, setActivePinia } from 'pinia'
import { beforeEach, describe, expect, it, vi } from 'vitest'
import ParseDownloadPlanner from './ParseDownloadPlanner.vue'
import UiSelect from '../../ui/Select.vue'
import { useSettingsStore } from '../../stores/settings'

const api = vi.hoisted(() => ({ selectionAvailableQualities: vi.fn(), mobilePickExportDirectory: vi.fn() }))
const stores = vi.hoisted(() => ({
  parse: {
    isBatch: false,
    activeSource: { source: { id: 'source-a', title: 'Source A' } },
    activeSelection: ['part-a', 'part-b'],
    selectionBySource: { 'source-a': ['part-a', 'part-b'] },
    loadingBySource: {},
    setNotice: vi.fn(),
    estimateDownloadSizeForSources: vi.fn(),
    createTasksForSources: vi.fn(),
  },
}))
vi.mock('../../api/tauri', () => api)
vi.mock('../../stores/parse', () => ({ useParseStore: () => stores.parse }))
vi.mock('@tauri-apps/plugin-dialog', () => ({ open: vi.fn() }))

beforeEach(() => {
  setActivePinia(createPinia())
  useSettingsStore().loaded = true
  stores.parse.estimateDownloadSizeForSources.mockResolvedValue({ estimated_bytes: 1, estimated_parts: 2, unknown_streams: 0 })
  stores.parse.createTasksForSources.mockResolvedValue({ created: [], failures: [], requires_confirmation: false })
})

async function openPlanner() {
  const wrapper = shallowMount(ParseDownloadPlanner, {
    global: {
      stubs: {
        UiDialog: { props: ['modelValue'], template: '<div v-if="modelValue"><slot /><slot name="footer" /></div>' },
        UiButton: { template: '<button><slot /></button>' },
      },
    },
  })
  await wrapper.vm.openDialog()
  await flushPromises()
  return wrapper
}

describe('source-backed download qualities', () => {
  it('keeps per-part SDR selection and passes it to the download request', async () => {
    api.selectionAvailableQualities.mockResolvedValue({ parts: [
      { partId: 'part-a', qualities: [126, 120, 80], sdr: true },
      { partId: 'part-b', qualities: [125, 64], sdr: true },
    ] })
    const wrapper = await openPlanner()
    try {
      const quality = wrapper.findAllComponents(UiSelect).find((select) => select.props('label') === '本次下载画质')!
      expect(quality.props('options')).toContainEqual({ value: 'sdr', label: '最高可用 SDR' })
      expect(quality.props('helper')).toContain('杜比视界 / 126')
      await quality.vm.$emit('update:modelValue', 'sdr')
      await flushPromises()
      await wrapper.findAll('button').find((button) => button.text() === '开始下载')!.trigger('click')
      await flushPromises()
      expect(stores.parse.createTasksForSources).toHaveBeenCalledWith(['source-a'], expect.objectContaining({ quality: 'sdr' }))
    } finally { wrapper.unmount() }
  })

  it('labels partial SDR coverage without offering absent numeric qualities', async () => {
    api.selectionAvailableQualities.mockResolvedValue({ parts: [
      { partId: 'part-a', qualities: [125], sdr: false },
      { partId: 'part-b', qualities: [80], sdr: true },
    ] })
    const wrapper = await openPlanner()
    try {
      const quality = wrapper.findAllComponents(UiSelect).find((select) => select.props('label') === '本次下载画质')!
      expect(quality.props('options')).toEqual([
        { value: 'best', label: '各视频的最高可用原画质' },
        { value: 'sdr', label: '最高可用 SDR · 1/2 个分集可用' },
        { value: '125', label: 'HDR / 125 · 1/2 个分集可用' },
        { value: '80', label: '1080P / 80 · 1/2 个分集可用' },
      ])
    } finally { wrapper.unmount() }
  })

  it('does not advertise SDR when the selected sources only provide HDR or Dolby', async () => {
    api.selectionAvailableQualities.mockResolvedValue({ parts: [
      { partId: 'part-a', qualities: [125], sdr: false },
      { partId: 'part-b', qualities: [126], sdr: false },
    ] })
    const wrapper = await openPlanner()
    try {
      const quality = wrapper.findAllComponents(UiSelect).find((select) => select.props('label') === '本次下载画质')!
      expect(quality.props('options').map((option) => option.value)).toEqual(['best', '126', '125'])
    } finally { wrapper.unmount() }
  })
})
