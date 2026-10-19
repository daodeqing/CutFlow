import { createPinia, setActivePinia } from 'pinia'
import { flushPromises, mount } from '@vue/test-utils'
import { h, KeepAlive, ref } from 'vue'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { open } from '@tauri-apps/plugin-dialog'
import CreationPage from './CreationPage.vue'
import { emptyDraft, useCreationStore } from '../../shared/stores/creation'
import { useCutflowStore } from '../../shared/stores/cutflow'
import { useUiStore } from '../../shared/stores/ui'
import type { AiModel, CreationCatalog, CreationProject, CreationSession, CreationTurn, Job } from '../../shared/api/cutflow'

const invoke = vi.hoisted(() => vi.fn())
vi.mock('@tauri-apps/api/core', () => ({ invoke, convertFileSrc: (path: string) => path }))
vi.mock('@tauri-apps/plugin-dialog', () => ({ open: vi.fn() }))

const global = {
  stubs: {
    UiButton: { template: '<button :disabled="$attrs.disabled" :type="$attrs.type || \'button\'"><slot /></button>' },
    UiDialog: { props: ['modelValue'], template: '<div v-if="modelValue" role="dialog"><slot /><slot name="footer" /></div>' },
    UPopover: {
      props: ['open', 'portal', 'content'], emits: ['update:open'],
      template: '<div class="popover-fixture" :data-portal="portal"><div @click="$emit(\'update:open\', !open)"><slot /></div><div v-if="open" data-popover-content><slot name="content" /></div></div>',
    },
    UIcon: true,
  },
}
const model = (id = 'gflow-nano-banana-2-lite', kind = 'image'): AiModel => ({
  id, kind, maxReferences: 3, description: '', duration: null, aspect: null,
  aspects: kind === 'image' ? ['1:1', '16:9', '9:16'] : ['16:9', '9:16'],
  imageSizes: kind === 'image' ? ['1K', '2K', '4K'] : undefined,
  durations: kind === 'video' ? [4, 6, 8] : undefined,
})
const clone = <T,>(value: T): T => JSON.parse(JSON.stringify(value))
const deferred = <T,>() => {
  let resolve!: (value: T) => void
  let reject!: (value: unknown) => void
  const promise = new Promise<T>((done, fail) => { resolve = done; reject = fail })
  return { promise, resolve, reject }
}
const calls = (method: string) => invoke.mock.calls.filter(([, input]) => input.method === method)
type CallParams = Record<string, unknown>
const job = (status = 'running', id = 'job-a', sid = 'session-a'): Job => ({
  id, status, method: 'cutflow.generate', operationId: id, stage: status,
  params: { creationProjectId: 'project-a', sessionId: sid, turnId: 'turn-' + id, model: 'gflow-nano-banana-2-lite', prompt: '苹果', kind: 'image' },
})
const asTurn = (value: Job, createdAt = 10): CreationTurn => ({
  id: String(value.params.turnId), sessionId: String(value.params.sessionId),
  jobId: value.id, prompt: String(value.params.prompt), params: value.params, job: value, createdAt,
})
const session = (id = 'session-a'): CreationSession => ({
  id, projectId: 'project-a', title: id === 'session-a' ? '新聊天' : '第二个聊天',
  createdAt: 1, updatedAt: 1, drafts: { image: emptyDraft(), video: emptyDraft() },
})
interface Fixture {
  catalog: CreationCatalog
  projects: CreationProject[]
  sessions: CreationSession[]
  turns: Record<string, CreationTurn[]>
  jobs: Job[]
  revision: number
  generate?: (params: CallParams, generated: Job) => Promise<{ jobId: string }>
  jobsList?: (params: CallParams) => Promise<{ jobs: Job[]; revision: number }>
}
function fixture(): Fixture {
  return {
    catalog: { connection: { baseUrl: 'http://localhost:8317/v1' }, models: [model(), model('gflow-veo-3.1-lite', 'video')], references: [], fault: null, workspaceId: 'workspace-a', instanceId: 'instance-a', projectBinding: true, revision: 1 },
    projects: [{ id: 'project-a', name: '猫咪项目', state: 'ready', flowProjectId: 'google-project-a', instanceId: 'instance-a', accountLabel: 'Google A', createdAt: 1, updatedAt: 1 }],
    sessions: [session(), session('session-b')], turns: { 'session-a': [], 'session-b': [] }, jobs: [], revision: 1,
  }
}
function install(state: Fixture) {
  invoke.mockImplementation(async (_command, input: { method: string; params: CallParams }) => {
    const p = input.params
    switch (input.method) {
      case 'cutflow.creation.catalog': return clone(state.catalog)
      case 'cutflow.creation.projects.list': return { projects: clone(state.projects), nextCursor: null }
      case 'cutflow.creation.projects.get': return clone(state.projects.find(item => item.id === p.projectId))
      case 'cutflow.creation.sessions.list': return { sessions: clone(state.sessions.filter(item => item.projectId === p.projectId)), nextCursor: null }
      case 'cutflow.creation.turns.list': return { turns: clone(state.turns[String(p.sessionId)] || []), nextCursor: null }
      case 'cutflow.creation.drafts.save': {
        const target = state.sessions.find(item => item.id === p.sessionId)
        if (target) target.drafts[p.kind as 'image' | 'video'] = clone(p.draft) as CreationSession['drafts']['image']
        return { saved: true }
      }
      case 'cutflow.jobs.list':
        if (state.jobsList) return state.jobsList(p)
        return { jobs: clone(state.jobs.filter(item => !p.activeOnly || ['queued', 'running'].includes(item.status))), revision: state.revision }
      case 'cutflow.generate': {
        const generated: Job = { ...job('running', 'generated-' + calls('cutflow.generate').length, String(p.sessionId)), operationId: String(p.operationId), params: clone(p) }
        state.jobs.push(generated); state.revision++
        state.turns[String(p.sessionId)] ||= []
        state.turns[String(p.sessionId)].push(asTurn(generated, Date.now()))
        return state.generate ? state.generate(p, generated) : { jobId: generated.id }
      }
      case 'cutflow.jobs.cancel': return { requested: true, message: '已请求停止；服务端可能仍在生成和计费' }
      case 'cutflow.jobs.retryDownload': return { jobId: p.jobId, status: 'running' }
      case 'cutflow.shell.open': return { opened: true }
      default: throw new Error('Unexpected creation API: ' + input.method)
    }
  })
}
const wrappers: { unmount(): void }[] = []
const view = () => {
  const wrapper = mount(KeepAlive, { slots: { default: () => h(CreationPage) }, global })
  wrappers.push(wrapper)
  return wrapper
}
const namedButton = (wrapper: ReturnType<typeof view>, name: string) => {
  const found = wrapper.findAll('button').find(button => button.text() === name)
  if (!found) throw new Error('Missing button: ' + name)
  return found
}
const composer = (wrapper: ReturnType<typeof view>) => wrapper.get('form.conversation-composer')
const submit = async (wrapper: ReturnType<typeof view>, prompt = '苹果') => {
  await wrapper.get('#ai-prompt').setValue(prompt)
  await composer(wrapper).trigger('submit')
  await flushPromises()
}
const tick = async (ms = 2000) => { await vi.advanceTimersByTimeAsync(ms); await flushPromises() }

describe('CutFlow project creation lifecycle', () => {
  beforeEach(() => {
    localStorage.clear()
    setActivePinia(createPinia())
    invoke.mockReset(); vi.mocked(open).mockReset()
    vi.useFakeTimers()
  })
  afterEach(() => {
    wrappers.splice(0).forEach(wrapper => wrapper.unmount())
    vi.clearAllTimers()
    vi.useRealTimers()
  })

  it('locks rapid submissions, the pending receipt, and the running turn within one session', async () => {
    const state = fixture(), receipt = deferred<{ jobId: string }>()
    state.generate = () => receipt.promise
    install(state)
    const wrapper = view(); await flushPromises()
    await wrapper.get('#ai-prompt').setValue('苹果')
    await Promise.all([composer(wrapper).trigger('submit'), composer(wrapper).trigger('submit')])
    await flushPromises()
    expect(calls('cutflow.generate')).toHaveLength(1)
    expect(useCreationStore().locked).toBe(true)
    await composer(wrapper).trigger('submit'); await flushPromises()
    expect(calls('cutflow.generate')).toHaveLength(1)
    receipt.resolve({ jobId: state.jobs[0].id }); await flushPromises()
    await submit(wrapper, '第二张苹果')
    expect(calls('cutflow.generate')).toHaveLength(1)
    state.jobs[0].status = 'completed'; state.revision++
    await tick()
    expect(useCreationStore().locked).toBe(false)
    await submit(wrapper, '第三张苹果')
    expect(calls('cutflow.generate')).toHaveLength(2)
  })

  it('keeps cancellation pending and preserves partial assets in the original turn', async () => {
    const state = fixture()
    const running = { ...job(), result: { assets: [{ path: 'D:/partial.jpg', thumbnail: 'D:/partial-thumb.jpg', width: 1024, height: 1024 }] } }
    state.jobs = [running]; state.turns['session-a'] = [asTurn(running)]
    install(state)
    const wrapper = view(); await flushPromises()
    await namedButton(wrapper, '停止生成').trigger('click'); await flushPromises()
    expect(namedButton(wrapper, '正在停止…').attributes('disabled')).toBeDefined()
    await submit(wrapper)
    expect(calls('cutflow.jobs.cancel')).toHaveLength(1)
    expect(calls('cutflow.generate')).toHaveLength(0)
    expect(wrapper.get('.turn-assets img').attributes('src')).toBe('D:/partial.jpg')
    expect(wrapper.get('.turn-assets img').attributes('loading')).toBe('lazy')
    state.jobs[0].status = 'cancelled'; state.revision++
    await tick()
    expect(wrapper.text()).not.toContain('正在停止…')
    expect(wrapper.get('.creation-turn').attributes('data-status')).toBe('cancelled')
    expect(wrapper.get('.turn-assets img').attributes('src')).toBe('D:/partial.jpg')
  })

  it('retains independent image and video drafts and cannot bypass a running image by switching kind', async () => {
    const state = fixture()
    state.jobs = [job()]; state.turns['session-a'] = [asTurn(state.jobs[0])]
    install(state)
    const wrapper = view(); await flushPromises()
    await wrapper.get('#ai-prompt').setValue('图片草稿')
    await namedButton(wrapper, '视频').trigger('click')
    await wrapper.get('#ai-prompt').setValue('视频草稿')
    await composer(wrapper).trigger('submit'); await flushPromises()
    expect(calls('cutflow.generate')).toHaveLength(0)
    await namedButton(wrapper, '图片').trigger('click')
    expect((wrapper.get('#ai-prompt').element as HTMLTextAreaElement).value).toBe('图片草稿')
    await namedButton(wrapper, '视频').trigger('click')
    expect((wrapper.get('#ai-prompt').element as HTMLTextAreaElement).value).toBe('视频草稿')
    await tick(500)
    expect(calls('cutflow.creation.drafts.save').some(([, input]) => input.params.kind === 'video' && input.params.draft.prompt === '视频草稿')).toBe(true)
  })

  it('counts the automatic continuation image when validating local and library references', async () => {
    const state = fixture()
    const previous = { ...job('completed'), result: { assets: [{ path: 'D:/previous.jpg', thumbnail: 'D:/previous-thumb.jpg' }] } }
    state.jobs = [previous]; state.turns['session-a'] = [asTurn(previous)]
    state.catalog.references = [{ id: 'r', kind: 'character', name: '角色', path: 'D:/library.jpg', prompt: '参考草稿' }]
    install(state)
    const wrapper = view(); await flushPromises()
    vi.mocked(open).mockResolvedValue(['D:/1.jpg', 'D:/2.jpg', 'D:/3.jpg'])
    await wrapper.get('[aria-label="添加参考图"]').trigger('click'); await flushPromises()
    expect(useCreationStore().draft.references).toHaveLength(0)
    expect(wrapper.text()).toContain('最多支持 3 张')
    useCreationStore().draft.references = ['D:/1.jpg', 'D:/2.jpg']
    await wrapper.get('[aria-label="打开角色与场景参考库"]').trigger('click'); await flushPromises()
    await namedButton(wrapper, '角色 · 角色').trigger('click'); await flushPromises()
    expect(useCreationStore().draft.references).toHaveLength(2)
    useCreationStore().draft.references.push('D:/extra.jpg')
    await submit(wrapper)
    expect(calls('cutflow.generate')).toHaveLength(0)
    // Removing the automatic reference frees exactly one slot.
    useCreationStore().draft.independent = true
    await submit(wrapper)
    expect(calls('cutflow.generate')).toHaveLength(1)
  })

  it('requires reselection when a catalog model disappears and rejects wrong media categories', async () => {
    const state = fixture(); install(state)
    const wrapper = view(); await flushPromises()
    await wrapper.get('#ai-prompt').setValue('苹果')
    state.catalog.models = [model('gflow-veo-3.1-lite', 'video')]
    await useCreationStore().loadCatalog(true)
    await composer(wrapper).trigger('submit'); await flushPromises()
    expect(calls('cutflow.generate')).toHaveLength(0)
    expect(wrapper.text()).toContain('模型已失效')
    useCreationStore().draft.model = 'gflow-veo-3.1-lite'
    await composer(wrapper).trigger('submit'); await flushPromises()
    expect(calls('cutflow.generate')).toHaveLength(0)
    expect(wrapper.text()).toContain('类别不匹配')
    useCreationStore().draft.model = 'unknown-provider-model'
    useCreationStore().draft.modelSource = 'custom'
    await composer(wrapper).trigger('submit'); await flushPromises()
    expect(calls('cutflow.generate')).toHaveLength(0)
  })

  it('keeps jobs polling single-flight and stops polling when the cached page is inactive', async () => {
    const state = fixture()
    state.jobs = [job()]; state.turns['session-a'] = [asTurn(state.jobs[0])]
    install(state)
    const enabled = ref(true)
    const wrapper = mount({ render: () => h(KeepAlive, null, { default: () => enabled.value ? h(CreationPage) : h('div', 'other page') }) }, { global })
    wrappers.push(wrapper)
    await flushPromises()
    const baseline = calls('cutflow.jobs.list').length
    const waiting = deferred<{ jobs: Job[]; revision: number }>()
    state.jobsList = () => waiting.promise
    await tick(2000)
    await tick(6000)
    expect(calls('cutflow.jobs.list')).toHaveLength(baseline + 1)
    enabled.value = false; await flushPromises()
    waiting.resolve({ jobs: clone(state.jobs), revision: ++state.revision }); await flushPromises()
    await tick(6000)
    expect(calls('cutflow.jobs.list')).toHaveLength(baseline + 1)
    state.jobsList = undefined
    enabled.value = true; await flushPromises()
    expect(calls('cutflow.jobs.list')).toHaveLength(baseline + 2)
    expect(calls('cutflow.hub.snapshot')).toHaveLength(0)
  })

  it('clears a recovered catalog fault without fetching the full historical snapshot', async () => {
    const state = fixture()
    state.catalog.fault = '项目服务暂时不可用'
    state.catalog.projectBinding = false
    install(state)
    const wrapper = view(); await flushPromises()
    expect(wrapper.text()).toContain('项目服务暂时不可用')
    state.catalog.fault = null
    state.catalog.projectBinding = true
    const baseline = calls('cutflow.creation.catalog').length
    await namedButton(wrapper, '重新检查').trigger('click'); await flushPromises()
    expect(calls('cutflow.creation.catalog')).toHaveLength(baseline + 1)
    expect(wrapper.text()).not.toContain('项目服务暂时不可用')
    expect(calls('cutflow.hub.snapshot')).toHaveLength(0)
    expect(calls('cutflow.generate')).toHaveLength(0)
  })

  it('stops local polling while the document is hidden and resumes with an immediate delta', async () => {
    const state = fixture()
    state.jobs = [job()]; state.turns['session-a'] = [asTurn(state.jobs[0])]
    install(state)
    view(); await flushPromises()
    const baseline = calls('cutflow.jobs.list').length
    const hidden = vi.spyOn(document, 'hidden', 'get').mockReturnValue(true)
    document.dispatchEvent(new Event('visibilitychange')); await tick(6000)
    expect(calls('cutflow.jobs.list')).toHaveLength(baseline)
    hidden.mockReturnValue(false)
    document.dispatchEvent(new Event('visibilitychange')); await flushPromises()
    expect(calls('cutflow.jobs.list')).toHaveLength(baseline + 1)
  })

  it('routes a late submission receipt to its original session while another session stays usable', async () => {
    const state = fixture(), receipt = deferred<{ jobId: string }>()
    state.generate = () => receipt.promise
    install(state)
    const wrapper = view(); await flushPromises()
    await submit(wrapper, '会话 A 的猫')
    const store = useCreationStore()
    const second = store.sessions['project-a'].find(item => item.id === 'session-b')!
    await store.selectSession(second); await flushPromises()
    expect(store.locked).toBe(false)
    await wrapper.get('#ai-prompt').setValue('会话 B 的独立草稿')
    receipt.resolve({ jobId: state.jobs[0].id }); await flushPromises()
    expect(store.activeSessionId).toBe('session-b')
    expect(store.turns['session-b']).toHaveLength(0)
    expect(store.turns['session-a']).toHaveLength(1)
    expect(store.turns['session-a'][0].prompt).toBe('会话 A 的猫')
    expect((wrapper.get('#ai-prompt').element as HTMLTextAreaElement).value).toBe('会话 B 的独立草稿')
    expect(calls('cutflow.generate')[0][1].params.sessionId).toBe('session-a')
  })

  it('keeps an explicitly selected history image when newer results arrive and snapshots its reference', async () => {
    const state = fixture()
    const first = { ...job('completed', 'first'), result: { assets: [{ path: 'D:/first.png', thumbnail: 'D:/first-thumb.jpg' }] } }
    const newer = { ...job('completed', 'newer'), result: { assets: [{ path: 'D:/newer.png', thumbnail: 'D:/newer-thumb.jpg' }] } }
    state.jobs = [first]; state.turns['session-a'] = [asTurn(first)]
    install(state)
    const wrapper = view(); await flushPromises()
    await namedButton(wrapper, '继续改图').trigger('click')
    state.jobs.push(newer); state.turns['session-a'].push(asTurn(newer, 20)); state.revision++
    await useCreationStore().poll(); await flushPromises()
    expect(wrapper.get('.composer-reference img').attributes('src')).toBe('D:/first-thumb.jpg')
    await submit(wrapper, '改为水彩')
    const submitted = calls('cutflow.generate')[0][1].params
    expect(submitted.continuationAssetPath).toBe('D:/first.png')
    expect(submitted.references).toEqual(['D:/first.png'])
    expect(submitted.creationProjectId).toBe('project-a')
  })

  it('shows lazy original image bytes directly in visible cards while keeping reference thumbnails', async () => {
    const state = fixture()
    const saved = { ...job('completed'), result: { assets: [{ path: 'D:/4k-original.png', thumbnail: 'D:/240px-thumb.jpg' }] } }
    state.jobs = [saved]; state.turns['session-a'] = [asTurn(saved)]
    install(state)
    const wrapper = view(); await flushPromises()
    expect(wrapper.get('.turn-assets img').attributes('src')).toBe('D:/4k-original.png')
    expect(wrapper.get('.turn-assets img').attributes('loading')).toBe('lazy')
    expect(wrapper.get('.turn-assets img').attributes('decoding')).toBe('async')
    expect(wrapper.get('.composer-reference img').attributes('src')).toBe('D:/240px-thumb.jpg')
    await wrapper.get('.asset-preview').trigger('click'); await flushPromises()
    expect(wrapper.get('.original-image').attributes('src')).toBe('D:/4k-original.png')
  })

  it('keeps video cards on thumbnails instead of loading the video into an image', async () => {
    const state = fixture()
    const saved = { ...job('completed'), params: { ...job('completed').params, kind: 'video' }, result: { assets: [{ path: 'D:/movie.mp4', thumbnail: 'D:/movie-thumb.jpg' }] } }
    state.jobs = [saved]; state.turns['session-a'] = [asTurn(saved)]
    install(state)
    const wrapper = view(); await flushPromises()
    expect(wrapper.get('.turn-assets img').attributes('src')).toBe('D:/movie-thumb.jpg')
  })

  it('starts with a collapsed model popover, closes on selection, and supports Escape', async () => {
    const state = fixture()
    state.catalog.models.push(model('gflow-nano-banana-pro'))
    install(state)
    const wrapper = view(); await flushPromises()
    const trigger = wrapper.get('[aria-label="选择生成模型"]')
    expect(trigger.attributes('aria-expanded')).toBe('false')
    expect(wrapper.find('.creation-model-popover').exists()).toBe(false)
    expect(wrapper.find('details.model-picker').exists()).toBe(false)
    await trigger.trigger('click')
    expect(wrapper.get('.popover-fixture').attributes('data-portal')).toBe('true')
    expect(wrapper.find('.creation-model-popover').exists()).toBe(true)
    await wrapper.findAll('.model-options button').find(button => button.text().includes('Nano Banana Pro'))!.trigger('click')
    expect(useCreationStore().draft.model).toBe('gflow-nano-banana-pro')
    expect(trigger.attributes('aria-expanded')).toBe('false')
    await trigger.trigger('click')
    document.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true }))
    await flushPromises()
    expect(trigger.attributes('aria-expanded')).toBe('false')
    expect(wrapper.find('.creation-model-popover').exists()).toBe(false)
  })

  it('always shows all quality choices but enables only confirmed project model capabilities and refreshes them', async () => {
    const state = fixture()
    state.projects[0].models = [{ ...model(), imageSizes: ['1K', '2K'] }]
    install(state)
    const wrapper = view(); await flushPromises()
    const quality = wrapper.get('#ai-image-size')
    expect(quality.findAll('option').map(option => option.attributes('value'))).toEqual(['auto', '1K', '2K', '4K'])
    expect(quality.get('option[value="4K"]').attributes('disabled')).toBeDefined()
    expect(quality.get('option[value="2K"]').attributes('disabled')).toBeUndefined()
    expect(wrapper.text()).toContain('模型 / 账号 / 服务未开放')
    state.projects[0].models[0].imageSizes = ['1K', '2K', '4K']
    await useCreationStore().loadCatalog(true); await flushPromises()
    expect(calls('cutflow.creation.projects.get').at(-1)?.[1].params).toEqual({ projectId: 'project-a', refresh: true })
    expect(quality.get('option[value="4K"]').attributes('disabled')).toBeUndefined()
    await quality.setValue('4K')
    state.projects[0].models[0].imageSizes = []
    await useCreationStore().loadCatalog(true); await flushPromises()
    expect(useCreationStore().draft.imageSize).toBe('auto')
    expect(quality.get('option[value="1K"]').attributes('disabled')).toBeDefined()
    expect(quality.get('option[value="4K"]').attributes('disabled')).toBeDefined()
    expect(wrapper.text()).toContain('确认前仅可使用默认画质')
  })

  it('preserves the confirmed import project and refuses additions during import without navigation', () => {
    const store = useCutflowStore(), ui = useUiStore()
    store.savePending({ files: ['D:/one.png'], operationId: 'first', target: { projectPath: 'D:/project-a', revision: 7 } })
    expect(store.sendToEditor(['D:/two.png'])).toBe(true)
    expect(store.pending?.target?.projectPath).toBe('D:/project-a')
    store.savePending({ ...store.pending!, jobId: 'importing' }); ui.setTab('creation')
    expect(store.sendToEditor(['D:/three.png'])).toBe(false)
    expect(store.pending?.files).toEqual(['D:/one.png', 'D:/two.png'])
    expect(ui.activeTab).toBe('creation')
  })
})
