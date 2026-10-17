import { createPinia, setActivePinia } from 'pinia'
import { flushPromises, mount } from '@vue/test-utils'
import { h, KeepAlive } from 'vue'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { open } from '@tauri-apps/plugin-dialog'
import CreationPage from './CreationPage.vue'
import { useCreationStore } from '../../shared/stores/creation'
import { useCutflowStore } from '../../shared/stores/cutflow'
import { useUiStore } from '../../shared/stores/ui'
import type { HubSnapshot, Job } from '../../shared/api/cutflow'
const invoke = vi.hoisted(() => vi.fn())
vi.mock('@tauri-apps/api/core', () => ({ invoke, convertFileSrc: (path: string) => path }))
vi.mock('@tauri-apps/plugin-dialog', () => ({ open: vi.fn() }))
const global = { stubs: { UiButton: { template: '<button :disabled="$attrs.disabled" :type="$attrs.type || \'button\'"><slot /></button>' }, UIcon: true } }
const model = (id = 'gflow-nano-banana-2-lite', kind = 'image') => ({ id, kind, maxReferences: 3, description: '', duration: null, aspect: null })
const job = (status = 'running', id = 'new-job'): Job => ({ id, status, method: 'cutflow.generate', operationId: id, stage: status, params: { model: 'gflow-nano-banana-2-lite', prompt: '苹果', kind: 'image' } })
const snapshot = (): HubSnapshot => ({ connection: { baseUrl: 'http://localhost:8317/v1' }, models: [model(), model('gflow-veo-3.1-lite', 'video')], jobs: [], references: [], fault: null })
const view = () => mount(KeepAlive, { slots: { default: () => h(CreationPage) }, global })
const namedButton = (wrapper: ReturnType<typeof view>, name: string) => wrapper.findAll('button').find(button => button.text() === name)!
const calls = (method: string) => invoke.mock.calls.filter(([, input]) => input.method === method)
describe('CutFlow 0.5 creation lifecycle', () => {
  beforeEach(() => { localStorage.clear(); setActivePinia(createPinia()); invoke.mockReset(); vi.mocked(open).mockReset(); vi.useFakeTimers() })
  afterEach(() => { vi.useRealTimers() })
  it('locks rapid and subsequent submissions until the returned task reaches a terminal state', async () => {
    const state = snapshot()
    invoke.mockImplementation((_cmd, input) => Promise.resolve(input.method === 'cutflow.hub.snapshot' ? state : { jobId: 'new-job' }))
    const wrapper = view(); await flushPromises()
    await wrapper.get('#ai-prompt').setValue('苹果')
    await Promise.all([wrapper.get('form').trigger('submit'), wrapper.get('form').trigger('submit')]); await flushPromises()
    await wrapper.get('form').trigger('submit'); await flushPromises()
    expect(calls('cutflow.generate')).toHaveLength(1)
    state.jobs = [job()]; await vi.advanceTimersByTimeAsync(1800); await flushPromises()
    expect(namedButton(wrapper, '停止生成')).toBeTruthy()
    state.jobs = [job('completed')]; await vi.advanceTimersByTimeAsync(1800); await flushPromises()
    expect(wrapper.get('button[type=submit]').attributes('disabled')).toBeUndefined()
    wrapper.unmount()
  })
  it('keeps a cancellation pending and the saved partial output until terminal receipt', async () => {
    const state = snapshot()
    state.jobs = [{ ...job(), result: { assets: [{ path: 'D:/partial.jpg', width: 1024, height: 1024 }] } }]
    invoke.mockImplementation((_cmd, input) => Promise.resolve(input.method === 'cutflow.hub.snapshot' ? state : { requested: true, message: '服务端可能仍在计费' }))
    const wrapper = view(); await flushPromises()
    await namedButton(wrapper, '停止生成').trigger('click'); await flushPromises()
    expect(namedButton(wrapper, '正在停止…').attributes('disabled')).toBeDefined()
    await wrapper.get('form').trigger('submit'); await flushPromises()
    expect(calls('cutflow.jobs.cancel')).toHaveLength(1); expect(calls('cutflow.generate')).toHaveLength(0)
    expect(wrapper.get('.preview-stage img').attributes('src')).toBe('D:/partial.jpg')
    state.jobs = [{ ...state.jobs[0], status: 'cancelled' }]
    await vi.advanceTimersByTimeAsync(1800); await flushPromises()
    expect(wrapper.text()).not.toContain('等待任务停止')
    expect(wrapper.get('.preview-stage img').attributes('src')).toBe('D:/partial.jpg')
    wrapper.unmount()
  })
  it('retains separate drafts and cannot bypass a running image by switching to video', async () => {
    const state = snapshot(); state.jobs = [job()]
    invoke.mockResolvedValue(state)
    const wrapper = view(); await flushPromises()
    await wrapper.get('#ai-prompt').setValue('图片草稿')
    await namedButton(wrapper, '视频').trigger('click')
    await wrapper.get('#ai-prompt').setValue('视频草稿')
    await wrapper.get('form').trigger('submit'); await flushPromises()
    expect(calls('cutflow.generate')).toHaveLength(0)
    await namedButton(wrapper, '图片').trigger('click')
    expect((wrapper.get('#ai-prompt').element as HTMLTextAreaElement).value).toBe('图片草稿')
    wrapper.unmount()
  })
  it('validates reference limits before creating a job, including reference library additions', async () => {
    const state = snapshot(); state.references = [{ id: 'r', kind: 'character', name: '角色', path: 'D:/four.jpg', prompt: '参考草稿' }]
    invoke.mockResolvedValue(state)
    const wrapper = view(); await flushPromises()
    vi.mocked(open).mockResolvedValue(['D:/1.jpg', 'D:/2.jpg', 'D:/3.jpg', 'D:/four.jpg'])
    await namedButton(wrapper, '参考图').trigger('click'); await flushPromises()
    expect(useCreationStore().drafts.image.references).toHaveLength(0)
    useCreationStore().drafts.image.references = ['D:/1.jpg', 'D:/2.jpg', 'D:/3.jpg']
    await namedButton(wrapper, '更多设置').trigger('click'); await flushPromises()
    await namedButton(wrapper, '角色 · 角色').trigger('click')
    expect(useCreationStore().drafts.image.references).toHaveLength(3)
    expect(wrapper.text()).toContain('最多支持 3 张')
    useCreationStore().drafts.image.references.push('D:/extra.jpg'); useCreationStore().drafts.image.prompt = '苹果'
    await wrapper.get('form').trigger('submit'); await flushPromises()
    expect(calls('cutflow.generate')).toHaveLength(0)
    wrapper.unmount()
  })
  it('requires reselection when a catalog model disappears and rejects the wrong media kind', async () => {
    const state = snapshot(); invoke.mockResolvedValue(state)
    const wrapper = view(); await flushPromises()
    await wrapper.get('#ai-prompt').setValue('苹果')
    state.models = [model('gflow-veo-3.1-lite', 'video')]
    await vi.advanceTimersByTimeAsync(1800); await flushPromises()
    expect(wrapper.get('.model-trigger').attributes('title')).toBe('gflow-nano-banana-2-lite')
    await wrapper.get('form').trigger('submit'); await flushPromises()
    expect(calls('cutflow.generate')).toHaveLength(0)
    useCreationStore().drafts.image.model = 'gflow-veo-3.1-lite'
    await wrapper.get('form').trigger('submit'); await flushPromises()
    expect(wrapper.text()).toContain('类别不匹配'); expect(calls('cutflow.generate')).toHaveLength(0)
    wrapper.unmount()
  })
  it('does not overlap polling and clears recovered snapshot faults', async () => {
    let resolve!: (value: HubSnapshot) => void
    invoke.mockReturnValue(new Promise<HubSnapshot>(done => { resolve = done }))
    const wrapper = view()
    await vi.advanceTimersByTimeAsync(6000)
    expect(calls('cutflow.hub.snapshot')).toHaveLength(1)
    resolve({ ...snapshot(), fault: '快照故障' }); await flushPromises()
    expect(wrapper.text()).toContain('快照故障')
    invoke.mockResolvedValue(snapshot())
    await vi.advanceTimersByTimeAsync(1800); await flushPromises()
    expect(wrapper.text()).not.toContain('快照故障')
    wrapper.unmount()
  })
  it('keeps a selected history result while new snapshots and jobs arrive', async () => {
    const state = snapshot(); state.jobs = [job('completed', 'old')]
    invoke.mockResolvedValue(state)
    const wrapper = view(); await flushPromises()
    state.jobs.push({ ...job('failed', 'recent'), params: { prompt: '其他任务', model: 'gflow-nano-banana-pro' } })
    await vi.advanceTimersByTimeAsync(1800); await flushPromises()
    expect(wrapper.get('.preview-description').text()).toContain('Nano Banana 2 Lite')
    wrapper.unmount()
  })
  it('preserves the confirmed import project on append and refuses additions during import without navigation', () => {
    const store = useCutflowStore(); const ui = useUiStore()
    store.savePending({ files: ['D:/one.png'], operationId: 'first', target: { projectPath: 'D:/project-a', revision: 7 } })
    expect(store.sendToEditor(['D:/two.png'])).toBe(true)
    expect(store.pending?.target?.projectPath).toBe('D:/project-a')
    store.savePending({ ...store.pending!, jobId: 'importing' }); ui.setTab('creation')
    expect(store.sendToEditor(['D:/three.png'])).toBe(false)
    expect(store.pending?.files).toEqual(['D:/one.png', 'D:/two.png'])
    expect(ui.activeTab).toBe('creation')
  })
})
