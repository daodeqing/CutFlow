import { createPinia, setActivePinia } from 'pinia'
import { flushPromises, mount } from '@vue/test-utils'
import { h, KeepAlive } from 'vue'
import { beforeEach, describe, expect, it, vi } from 'vitest'
import CreationPage from './CreationPage.vue'
import EditorPage from './EditorPage.vue'
import { useCutflowStore } from '../../shared/stores/cutflow'

const invoke = vi.hoisted(() => vi.fn())
vi.mock('@tauri-apps/api/core', () => ({ invoke, convertFileSrc: (path: string) => path }))
vi.mock('@tauri-apps/plugin-dialog', () => ({ open: vi.fn() }))
const global = { stubs: {
  UiButton: { template: '<button :disabled="$attrs.disabled" :type="$attrs.type || \'button\'"><slot /></button>' },
  UiTabs: true,
  UIcon: true,
} }

describe('CutFlow workspaces', () => {
  beforeEach(() => {
    localStorage.clear()
    setActivePinia(createPinia())
    invoke.mockReset()
  })

  it('submits a custom GFlow generation model without requiring a project', async () => {
    invoke.mockImplementation((_command, input) => {
      if (input.method === 'cutflow.hub.snapshot') return Promise.resolve({ connection: { baseUrl: 'http://localhost:8000/v1' }, models: [], references: [], jobs: [], fault: null })
      return Promise.resolve({ jobId: 'image-job' })
    })
    const wrapper = mount(KeepAlive, { slots: { default: () => h(CreationPage) }, global })
    await flushPromises()
    await wrapper.get('#ai-model').setValue('gflow-custom-image')
    await wrapper.get('#ai-prompt').setValue('画一个安静的海边')
    await wrapper.get('form').trigger('submit')
    await flushPromises()
    const sent = invoke.mock.calls.find(([, input]) => input.method === 'cutflow.generate')?.[1].params
    expect(sent.model).toBe('gflow-custom-image')
    expect(sent.projectPath).toBeUndefined()
    expect(sent.maxGenerationCalls).toBe(1)
    expect(sent.operationId).toBeTruthy()
    wrapper.unmount()
  })

  it('keeps a scrollable model picker open and submits image settings', async () => {
    invoke.mockImplementation((_command, input) => {
      if (input.method === 'cutflow.hub.snapshot') return Promise.resolve({ connection: { baseUrl: 'http://localhost/v1' }, models: [{ id: 'gflow-nano-banana-2.1', kind: 'image', maxReferences: 3 }], references: [], jobs: [], fault: null })
      return Promise.resolve({ jobId: 'image-job' })
    })
    const wrapper = mount(KeepAlive, { slots: { default: () => h(CreationPage) }, global })
    await flushPromises()
    await wrapper.findAll('button').find(button => button.text() === '选择')!.trigger('click')
    await wrapper.get('.model-options').trigger('wheel', { deltaY: 150 })
    expect(wrapper.find('.model-options').exists()).toBe(true)
    await wrapper.get('.model-options button').trigger('click')
    await wrapper.get('#ai-count').setValue('4')
    await wrapper.get('#ai-aspect').setValue('3:4')
    await wrapper.get('#ai-image-size').setValue('4K')
    await wrapper.get('#ai-prompt').setValue('海边')
    await wrapper.get('form').trigger('submit')
    await flushPromises()
    const sent = invoke.mock.calls.find(([, input]) => input.method === 'cutflow.generate')?.[1].params
    expect(sent).toMatchObject({ n: 4, maxGenerationCalls: 4, aspect_ratio: '3:4', image_size: '4K' })
    wrapper.unmount()
  })

  it('lists only GFlow images, preserves explicit selection and never retries failures silently', async () => {
    invoke.mockImplementation((_command, input) => {
      if (input.method === 'cutflow.hub.snapshot') return Promise.resolve({
        connection: { baseUrl: 'http://localhost/v1' }, references: [], jobs: [], fault: null,
        models: [
          { id: 'gflow-nano-banana-2.1', kind: 'image' }, { id: 'gflow-nano-banana-2-lite', kind: 'image' },
          { id: 'gpt-image-1.5', kind: 'image' }, { id: 'gpt-6-sol', kind: 'unknown' },
          { id: 'gflow-omni-1.1-flash', kind: 'unknown' }, { id: 'gflow-veo-3.1-fast', kind: 'video' },
        ],
      })
      return Promise.reject({ message: 'provider rejected' })
    })
    const wrapper = mount(KeepAlive, { slots: { default: () => h(CreationPage) }, global })
    await flushPromises()
    expect((wrapper.get('#ai-model').element as HTMLInputElement).value).toBe('gflow-nano-banana-2-lite')
    await wrapper.findAll('button').find(button => button.text() === '选择')!.trigger('click')
    const choices = wrapper.findAll('.model-options button').map(button => button.text())
    expect(choices).toEqual(['gflow-nano-banana-2.1', 'gflow-nano-banana-2-lite'])
    expect(wrapper.text()).not.toContain('使用兼容图片模型')
    await wrapper.get('.model-options button').trigger('click')
    await wrapper.get('#ai-prompt').setValue('人物')
    await wrapper.get('form').trigger('submit')
    await flushPromises()
    expect(invoke.mock.calls.filter(([, input]) => input.method === 'cutflow.generate')).toHaveLength(1)
    expect((wrapper.get('#ai-model').element as HTMLInputElement).value).toBe('gflow-nano-banana-2.1')
    wrapper.unmount()
  })

  it('rejects a non-GFlow model before submitting generation', async () => {
    invoke.mockResolvedValue({ connection: { baseUrl: 'http://localhost/v1' }, models: [], references: [], jobs: [], fault: null })
    const wrapper = mount(KeepAlive, { slots: { default: () => h(CreationPage) }, global })
    await flushPromises()
    await wrapper.get('#ai-model').setValue('gpt-image-1.5')
    await wrapper.get('#ai-prompt').setValue('人物')
    await wrapper.get('form').trigger('submit')
    await flushPromises()
    expect(invoke.mock.calls.some(([, input]) => input.method === 'cutflow.generate')).toBe(false)
    expect(wrapper.text()).toContain('请选择 GFlow')
    wrapper.unmount()
  })

  it('exposes stop generation while a job is running', async () => {
    invoke.mockImplementation((_command, input) => {
      if (input.method === 'cutflow.hub.snapshot') return Promise.resolve({ connection: { baseUrl: 'http://localhost/v1' }, models: [], references: [], jobs: [{ id: 'running-job', method: 'cutflow.generate', status: 'running', params: { prompt: '海边' } }], fault: null })
      return Promise.resolve({ requested: true })
    })
    const wrapper = mount(KeepAlive, { slots: { default: () => h(CreationPage) }, global })
    await flushPromises()
    await wrapper.findAll('button').find(button => button.text() === '停止生成')!.trigger('click')
    await flushPromises()
    expect(invoke.mock.calls.some(([, input]) => input.method === 'cutflow.jobs.cancel' && input.params.jobId === 'running-job')).toBe(true)
    wrapper.unmount()
  })

  it('offers download retry only when the backend has an unsaved result', async () => {
    invoke.mockImplementation((_command, input) => {
      if (input.method === 'cutflow.hub.snapshot') return Promise.resolve({
        connection: { baseUrl: 'http://localhost/v1' }, models: [], references: [], fault: null,
        jobs: [
          { id: 'captcha-failed', method: 'cutflow.generate', status: 'failed', params: { prompt: 'captcha' }, canRetryDownload: false },
          { id: 'download-failed', method: 'cutflow.generate', status: 'failed', params: { prompt: 'download' }, canRetryDownload: true },
        ],
      })
      return Promise.resolve({})
    })
    const wrapper = mount(KeepAlive, { slots: { default: () => h(CreationPage) }, global })
    await flushPromises()
    const retries = wrapper.findAll('button').filter(button => button.text() === '只重试下载结果')
    expect(retries).toHaveLength(1)
    await retries[0].trigger('click')
    await flushPromises()
    expect(invoke.mock.calls.some(([, input]) => input.method === 'cutflow.jobs.retryDownload' && input.params.jobId === 'download-failed')).toBe(true)
    wrapper.unmount()
  })

  it('shows when provider output ignores the requested aspect without retrying', async () => {
    invoke.mockImplementation((_command, input) => Promise.resolve({
      connection: { baseUrl: 'http://localhost/v1' }, models: [], references: [], fault: null,
      jobs: [{ id: 'saved', method: 'cutflow.generate', status: 'completed',
        params: { kind: 'image', prompt: '人物', aspect_ratio: '1:1' },
        result: { assets: [{ path: 'D:/saved.png', width: 1145, height: 1374 }] } }],
    }))
    const wrapper = mount(KeepAlive, { slots: { default: () => h(CreationPage) }, global })
    await flushPromises()
    expect(wrapper.text()).toContain('服务返回的比例与所选 1:1 不一致')
    expect(invoke.mock.calls.some(([, input]) => input.method === 'cutflow.generate')).toBe(false)
    wrapper.unmount()
  })

  it('keeps the confirmed import target when the visible project changes', async () => {
    let current = { projectPath: 'D:/project-a', revision: 7 }
    invoke.mockImplementation((_command, input) => {
      if (input.method === 'cutflow.editor.status') return Promise.resolve({ running: true, current })
      if (input.method === 'cutflow.editor.recents') return Promise.resolve([])
      if (input.method === 'cutflow.assets.import') {
        current = { projectPath: 'D:/project-b', revision: 8 }
        return Promise.reject({ code: 'project_changed', message: '工程已切换' })
      }
      return Promise.resolve({})
    })
    const store = useCutflowStore()
    store.sendToEditor(['D:/asset.png'])
    const wrapper = mount(KeepAlive, { slots: { default: () => h(EditorPage) }, global })
    await flushPromises()
    const confirm = wrapper.findAll('button').find(button => button.text() === '确认导入此工程')!
    await confirm.trigger('click')
    await flushPromises()
    const sent = invoke.mock.calls.find(([, input]) => input.method === 'cutflow.assets.import')?.[1].params
    expect(sent.projectPath).toBe('D:/project-a')
    expect(sent.expectedRevision).toBe(7)
    expect(sent.place).toBe(false)
    expect(store.pending?.files).toEqual(['D:/asset.png'])
    expect(store.pending?.target?.projectPath).toBe('D:/project-a')
    expect(wrapper.text()).toContain('工程已切换')
    wrapper.unmount()
  })
})
