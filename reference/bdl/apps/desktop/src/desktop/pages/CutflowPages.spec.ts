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

  it('submits a custom generation model without requiring a project', async () => {
    invoke.mockImplementation((_command, input) => {
      if (input.method === 'cutflow.hub.snapshot') return Promise.resolve({ connection: { baseUrl: 'http://localhost:8000/v1' }, models: [], references: [], jobs: [], fault: null })
      return Promise.resolve({ jobId: 'image-job' })
    })
    const wrapper = mount(KeepAlive, { slots: { default: () => h(CreationPage) }, global })
    await flushPromises()
    await wrapper.get('#ai-model').setValue('my-custom-model')
    await wrapper.get('#ai-prompt').setValue('画一个安静的海边')
    await wrapper.get('form').trigger('submit')
    await flushPromises()
    const sent = invoke.mock.calls.find(([, input]) => input.method === 'cutflow.generate')?.[1].params
    expect(sent.model).toBe('my-custom-model')
    expect(sent.projectPath).toBeUndefined()
    expect(sent.maxGenerationCalls).toBe(1)
    expect(sent.operationId).toBeTruthy()
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
