import { createPinia, setActivePinia } from 'pinia'
import { flushPromises, mount } from '@vue/test-utils'
import { h, KeepAlive } from 'vue'
import { beforeEach, describe, expect, it, vi } from 'vitest'
import CreationPage from './CreationPage.vue'
import EditorPage from './EditorPage.vue'
import { useCutflowStore } from '../../shared/stores/cutflow'
import { emptyDraft, useCreationStore } from '../../shared/stores/creation'
import type { AiModel, CreationProject, CreationSession, Job } from '../../shared/api/cutflow'

const invoke = vi.hoisted(() => vi.fn())
vi.mock('@tauri-apps/api/core', () => ({ invoke, convertFileSrc: (path: string) => path }))
vi.mock('@tauri-apps/plugin-dialog', () => ({ open: vi.fn() }))
const global = { stubs: {
  UiButton: { template: '<button :disabled="$attrs.disabled" :type="$attrs.type || \'button\'"><slot /></button>' },
  UiDialog: { props: ['modelValue'], template: '<div v-if="modelValue" role="dialog"><slot /><slot name="footer" /></div>' },
  UiTabs: true,
  UIcon: true,
} }


const aiModel = (id = 'gflow-nano-banana-2-lite', kind = 'image'): AiModel => ({
  id, kind, maxReferences: 3, description: '', duration: null, aspect: null,
  aspects: kind === 'image' ? ['1:1', '16:9', '9:16', '4:3', '3:4'] : ['16:9', '9:16'],
  imageSizes: kind === 'image' ? ['1K', '2K', '4K'] : [],
})
const boundProject = (): CreationProject => ({
  id: 'project-a', name: '海边项目', state: 'ready', flowProjectId: 'google-flow-project',
  instanceId: 'service-a', accountLabel: 'Google A', createdAt: 1, updatedAt: 1,
})
const creationSession = (): CreationSession => ({
  id: 'session-a', projectId: 'project-a', title: '新聊天', createdAt: 1, updatedAt: 1,
  drafts: { image: emptyDraft(), video: emptyDraft() },
})
const generatedJob = (id: string, status: string, overrides: Partial<Job> = {}): Job => ({
  id, operationId: id, method: 'cutflow.generate', status, stage: status,
  params: { creationProjectId: 'project-a', sessionId: 'session-a', turnId: 'turn-' + id, model: 'gflow-nano-banana-2-lite', kind: 'image', prompt: id },
  ...overrides,
})
const workspaceFixture = () => ({
  projects: [boundProject()], sessions: [creationSession()], models: [aiModel()] as AiModel[],
  jobs: [] as Job[], rejectGeneration: false,
})
function mockCreation(state: ReturnType<typeof workspaceFixture>) {
  const copy = <T,>(value: T): T => JSON.parse(JSON.stringify(value))
  invoke.mockImplementation(async (_command, input) => {
    const p = input.params
    switch (input.method) {
      case 'cutflow.creation.catalog': return { connection: { baseUrl: 'http://localhost:8317/v1' }, models: copy(state.models), references: [], fault: null, workspaceId: 'workspace-a', instanceId: 'service-a', projectBinding: true, revision: 1 }
      case 'cutflow.creation.projects.list': return { projects: copy(state.projects), nextCursor: null }
      case 'cutflow.creation.projects.create': {
        const project = { ...boundProject(), name: p.name }; state.projects.push(project); return copy(project)
      }
      case 'cutflow.creation.projects.get': return copy(state.projects.find(item => item.id === p.projectId))
      case 'cutflow.creation.sessions.list': return { sessions: copy(state.sessions.filter(item => item.projectId === p.projectId)), nextCursor: null }
      case 'cutflow.creation.sessions.create': { const item = creationSession(); state.sessions.push(item); return copy(item) }
      case 'cutflow.creation.turns.list': return {
        turns: copy(state.jobs.filter(job => job.params.sessionId === p.sessionId).map((job, index) => ({
          id: job.params.turnId, jobId: job.id, sessionId: p.sessionId, prompt: job.params.prompt,
          params: job.params, createdAt: index + 1, job,
        }))), nextCursor: null,
      }
      case 'cutflow.creation.drafts.save': return { saved: true }
      case 'cutflow.jobs.list': return { jobs: copy(state.jobs.filter(job => !p.activeOnly || ['queued', 'running'].includes(job.status))), revision: 1 }
      case 'cutflow.generate': if (state.rejectGeneration) throw { message: 'provider rejected' }; return { jobId: 'image-job' }
      case 'cutflow.jobs.cancel': return { requested: true, message: '已请求停止；服务端可能仍在生成' }
      case 'cutflow.jobs.retryDownload': return { jobId: p.jobId, status: 'running' }
      default: throw new Error('Unexpected API in creation test: ' + input.method)
    }
  })
}
const creationView = () => mount(KeepAlive, { slots: { default: () => h(CreationPage) }, global })
const generationCalls = () => invoke.mock.calls.filter(([, input]) => input.method === 'cutflow.generate')

describe('CutFlow workspaces', () => {
  beforeEach(() => {
    localStorage.clear()
    setActivePinia(createPinia())
    invoke.mockReset()
  })

  it('requires a bound project and then submits a catalog model within its own session', async () => {
    const state = workspaceFixture(); state.projects = []; state.sessions = []
    mockCreation(state)
    const wrapper = creationView(); await flushPromises()
    await wrapper.get('form.conversation-composer').trigger('submit'); await flushPromises()
    expect(generationCalls()).toHaveLength(0)
    expect(wrapper.get('#ai-prompt').attributes('disabled')).toBeDefined()
    await wrapper.findAll('button').find(button => button.text() === '新建项目')!.trigger('click')
    await wrapper.get('#new-project-name').setValue('安静的海边')
    await wrapper.get('form.project-create-form').trigger('submit'); await flushPromises()
    expect(invoke.mock.calls.some(([, input]) => input.method === 'cutflow.creation.projects.create')).toBe(true)
    expect(invoke.mock.calls.some(([, input]) => input.method === 'cutflow.creation.sessions.create')).toBe(true)
    expect(wrapper.find('.custom-model').exists()).toBe(false)
    expect(wrapper.find('.model-picker').attributes('open')).toBeUndefined()
    await wrapper.get('.model-summary').trigger('click')
    expect(wrapper.get('.model-options').text()).not.toContain('gflow-')
    await wrapper.get('.model-options button').trigger('click')
    await wrapper.get('#ai-prompt').setValue('画一个安静的海边')
    await wrapper.get('form.conversation-composer').trigger('submit'); await flushPromises()
    const sent = generationCalls()[0][1].params
    expect(sent).toMatchObject({ model: 'gflow-nano-banana-2-lite', creationProjectId: 'project-a', sessionId: 'session-a', maxGenerationCalls: 1 })
    expect(sent.projectPath).toBeUndefined()
    expect(sent.operationId).toBeTruthy()
    expect(sent.turnId).toBeTruthy()
    wrapper.unmount()
  })

  it('keeps the right-hand model list scrollable and snapshots the chosen image settings', async () => {
    const state = workspaceFixture(); state.models = [aiModel('gflow-nano-banana-2.1')]
    mockCreation(state)
    const wrapper = creationView(); await flushPromises()
    await wrapper.get('.model-options').trigger('wheel', { deltaY: 150 })
    expect(wrapper.find('.model-options').exists()).toBe(true)
    await wrapper.get('.model-options button').trigger('click')
    await wrapper.get('#ai-count').setValue('4')
    await wrapper.get('#ai-aspect').setValue('3:4')
    await wrapper.get('#ai-image-size').setValue('4K')
    await wrapper.get('#ai-prompt').setValue('海边')
    await wrapper.get('form.conversation-composer').trigger('submit'); await flushPromises()
    const sent = generationCalls()[0][1].params
    expect(sent).toMatchObject({ model: 'gflow-nano-banana-2.1', n: 4, maxGenerationCalls: 4, aspect_ratio: '3:4', image_size: '4K', creationProjectId: 'project-a', sessionId: 'session-a' })
    await wrapper.get('#ai-image-size').setValue('1K')
    expect(sent.image_size).toBe('4K')
    wrapper.unmount()
  })

  it('lists only GFlow image models, preserves explicit selection, and never retries failure silently', async () => {
    const state = workspaceFixture()
    state.models = [
      aiModel('gflow-nano-banana-2.1'), aiModel(), aiModel('gpt-image-1.5'),
      aiModel('gpt-6-sol', 'unknown'), aiModel('gflow-omni-1.1-flash', 'unknown'),
      aiModel('gflow-veo-3.1-fast', 'video'), aiModel('gflow-project-control', 'unknown'),
    ]
    state.rejectGeneration = true
    mockCreation(state)
    const wrapper = creationView(); await flushPromises()
    expect(wrapper.get('.model-options button[aria-pressed="true"]').text()).toContain('Nano Banana 2 Lite')
    const choices = wrapper.findAll('.model-options button strong').map(item => item.text())
    expect(choices).toEqual(['Nano Banana 2.1', 'Nano Banana 2 Lite'])
    expect(wrapper.get('.model-options').text()).not.toContain('gflow-')
    await wrapper.get('.model-options button').trigger('click')
    await wrapper.get('#ai-prompt').setValue('人物')
    await wrapper.get('form.conversation-composer').trigger('submit'); await flushPromises()
    expect(generationCalls()).toHaveLength(1)
    expect(wrapper.text()).toContain('provider rejected')
    expect(wrapper.get('.model-options button[aria-pressed="true"]').text()).toContain('Nano Banana 2.1')
    expect(generationCalls()[0][1].params.model).toBe('gflow-nano-banana-2.1')
    wrapper.unmount()
  })

  it('rejects a non-GFlow model before submitting a project generation', async () => {
    const state = workspaceFixture(); mockCreation(state)
    const wrapper = creationView(); await flushPromises()
    useCreationStore().draft.model = 'gpt-image-1.5'
    await wrapper.get('#ai-prompt').setValue('人物')
    await wrapper.get('form.conversation-composer').trigger('submit'); await flushPromises()
    expect(generationCalls()).toHaveLength(0)
    expect(wrapper.text()).toContain('模型已失效')
    expect(wrapper.get('[aria-label="发送生成请求"]').attributes('disabled')).toBeDefined()
    wrapper.unmount()
  })

  it('exposes cancellation for the running turn in its own project session', async () => {
    const state = workspaceFixture(); state.jobs = [generatedJob('running-job', 'running')]
    mockCreation(state)
    const wrapper = creationView(); await flushPromises()
    await wrapper.findAll('button').find(button => button.text() === '停止生成')!.trigger('click')
    await flushPromises()
    expect(invoke.mock.calls.some(([, input]) => input.method === 'cutflow.jobs.cancel' && input.params.jobId === 'running-job')).toBe(true)
    expect(wrapper.findAll('button').find(button => button.text() === '正在停止…')!.attributes('disabled')).toBeDefined()
    expect(generationCalls()).toHaveLength(0)
    wrapper.unmount()
  })

  it('offers download-only retry solely for a task with an unsaved captured result', async () => {
    const state = workspaceFixture()
    state.jobs = [
      generatedJob('captcha-failed', 'failed', { canRetryDownload: false }),
      generatedJob('download-failed', 'failed', { canRetryDownload: true }),
    ]
    mockCreation(state)
    const wrapper = creationView(); await flushPromises()
    const retries = wrapper.findAll('button').filter(button => button.text() === '只重试下载结果')
    expect(retries).toHaveLength(1)
    await retries[0].trigger('click'); await flushPromises()
    const retryCalls = invoke.mock.calls.filter(([, input]) => input.method === 'cutflow.jobs.retryDownload')
    expect(retryCalls).toHaveLength(1)
    expect(retryCalls[0][1].params.jobId).toBe('download-failed')
    expect(generationCalls()).toHaveLength(0)
    wrapper.unmount()
  })

  it('labels an output aspect mismatch while preserving the original file without regeneration', async () => {
    const state = workspaceFixture()
    state.jobs = [generatedJob('saved', 'completed', {
      params: { creationProjectId: 'project-a', sessionId: 'session-a', turnId: 'turn-saved', kind: 'image', prompt: '人物', aspect_ratio: '1:1' },
      result: { assets: [{ path: 'D:/saved.png', width: 1145, height: 1374 }] },
    })]
    mockCreation(state)
    const wrapper = creationView(); await flushPromises()
    expect(wrapper.get('.ratio-warning').text()).toContain('实际比例与所选不同')
    expect(wrapper.get('figcaption').text()).toContain('1145 × 1374')
    expect(wrapper.text()).toContain('已保留原图')
    expect(generationCalls()).toHaveLength(0)
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
