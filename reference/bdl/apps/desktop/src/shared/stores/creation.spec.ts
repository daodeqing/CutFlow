import { createPinia, setActivePinia } from 'pinia'
import { beforeEach, describe, expect, it, vi } from 'vitest'
import { emptyDraft, useCreationStore } from './creation'
import type { AiModel, CreationCatalog, CreationProject, CreationSession, CreationTurn, Job } from '../api/cutflow'

const invoke = vi.hoisted(() => vi.fn())
vi.mock('@tauri-apps/api/core', () => ({ invoke, convertFileSrc: (path: string) => path }))

const deferred = <T,>() => {
  let resolve!: (value: T) => void
  const promise = new Promise<T>(done => { resolve = done })
  return { promise, resolve }
}
const model = (id: string, kind = 'image'): AiModel => ({ id, kind, maxReferences: 3, description: '', duration: null, aspect: null })
const project = (id = 'p-a'): CreationProject => ({
  id, name: id, state: 'ready', flowProjectId: 'flow-' + id, instanceId: 'instance', createdAt: 1, updatedAt: 1,
})
const session = (id = 's-a', projectId = 'p-a'): CreationSession => ({
  id, projectId, title: id, createdAt: 1, updatedAt: 1, drafts: { image: emptyDraft(), video: emptyDraft() },
})
const job = (id = 'job-a', sid = 's-a', revision = 10, status = 'running'): Job => ({
  id, operationId: 'op-' + id, method: 'cutflow.generate', status, stage: status, revision, createdAt: 100,
  params: { creationProjectId: 'p-a', sessionId: sid, turnId: 'turn-' + id, kind: 'image', prompt: id },
})
const turn = (value = job()): CreationTurn => ({
  id: String(value.params.turnId), sessionId: String(value.params.sessionId), jobId: value.id,
  prompt: String(value.params.prompt), params: value.params, createdAt: value.createdAt!, job: value,
})
const catalog = (): CreationCatalog => ({
  connection: { baseUrl: 'http://127.0.0.1:8317/v1' }, models: [model('gflow-nano-banana-2-lite')],
  references: [], fault: null, workspaceId: 'workspace', instanceId: 'instance', projectBinding: true, revision: 1,
})
function seeded() {
  const store = useCreationStore()
  store.projects = [project(), project('p-b')]
  store.sessions = { 'p-a': [session(), session('s-a2')], 'p-b': [session('s-b', 'p-b')] }
  store.activeProjectId = 'p-a'; store.activeSessionId = 's-a'
  store.turns = { 's-a': [], 's-a2': [], 's-b': [] }
  store.catalog = catalog()
  return store
}
describe('project creation store consistency', () => {
  beforeEach(() => { setActivePinia(createPinia()); invoke.mockReset() })

  it('does not allow a delayed history job to overwrite a newer terminal revision', async () => {
    const store = seeded()
    const delayed = deferred<{ turns: CreationTurn[]; nextCursor: null }>()
    invoke.mockReturnValue(delayed.promise)
    const loading = store.loadTurns('s-a')
    const completed = { ...job('job-a', 's-a', 12, 'completed'), result: { assets: [{ path: 'D:/done.png' }] } }
    store.mergeJob(completed)
    delayed.resolve({ turns: [turn(job('job-a', 's-a', 10, 'running'))], nextCursor: null })
    await loading
    expect(store.jobs['job-a'].status).toBe('completed')
    expect(store.turns['s-a'][0].job.revision).toBe(12)
    expect(store.turns['s-a'][0].job.result?.assets?.[0].path).toBe('D:/done.png')
    expect(store.locked).toBe(false)
  })

  it('keeps authoritative completion when a late submit acknowledgement has no revision', () => {
    const store = seeded()
    const completed = { ...job('job-a', 's-a', 12, 'completed'), result: { assets: [{ path: 'D:/done.png' }] } }
    store.mergeJob(completed)
    store.turns['s-a'] = [turn(completed)]
    const acknowledgement = { ...job('job-a', 's-a', 0, 'queued'), revision: undefined }
    store.mergeJob(acknowledgement)
    expect(store.jobs['job-a']).toMatchObject({ status: 'completed', revision: 12 })
    expect(store.turns['s-a'][0].job.result?.assets?.[0].path).toBe('D:/done.png')
    expect(store.locked).toBe(false)
  })

  it('reconciles known running history omitted by the initial active-only snapshot', async () => {
    const store = seeded()
    const running = job('job-a', 's-a', 10, 'running')
    store.mergeJob(running)
    store.turns['s-a'] = [turn(running)]
    invoke.mockImplementation((_cmd, input) => {
      if (input.method === 'cutflow.jobs.list') return Promise.resolve({ jobs: [], revision: 12 })
      if (input.method === 'cutflow.jobs.get') return Promise.resolve({ ...running, status: 'completed', revision: 12 })
      throw new Error('Unexpected API: ' + input.method)
    })
    await store.poll()
    expect(invoke.mock.calls.map(([, input]) => input.method)).toEqual(['cutflow.jobs.list', 'cutflow.jobs.get'])
    expect(store.jobs['job-a'].status).toBe('completed')
    expect(store.locked).toBe(false)
    expect(store.revision).toBe(12)
  })

  it('preserves a newly received turn when an older in-flight history page finishes later', async () => {
    const store = seeded()
    const list = deferred<{ turns: CreationTurn[]; nextCursor: null }>()
    invoke.mockImplementation((_cmd, input) => input.method === 'cutflow.creation.turns.list'
      ? list.promise : Promise.resolve({ jobs: [job('new-job', 's-a', 20, 'completed')], revision: 20 }))
    const loading = store.loadTurns('s-a')
    await store.poll()
    expect(store.turns['s-a']).toHaveLength(1)
    list.resolve({ turns: [], nextCursor: null })
    await loading
    expect(store.turns['s-a'].map(item => item.jobId)).toEqual(['new-job'])
    expect(store.jobs['new-job'].revision).toBe(20)
  })

  it('coalesces concurrent polls and uses the returned revision for the next delta', async () => {
    const store = seeded()
    const receipt = deferred<{ jobs: Job[]; revision: number }>()
    invoke.mockReturnValue(receipt.promise)
    const first = store.poll(), duplicate = store.poll()
    expect(invoke).toHaveBeenCalledTimes(1)
    expect(invoke.mock.calls[0][1].params).toEqual({ activeOnly: true })
    receipt.resolve({ jobs: [job('job-a', 's-a', 41, 'completed')], revision: 41 })
    await Promise.all([first, duplicate])
    invoke.mockResolvedValue({ jobs: [], revision: 42 })
    await store.poll()
    expect(invoke).toHaveBeenCalledTimes(2)
    expect(invoke.mock.calls[1][1].params).toEqual({ sinceRevision: 41 })
    expect(store.revision).toBe(42)
    expect(store.jobs['job-a'].status).toBe('completed')
  })

  it('keeps late session history in the requested session after the user switches', async () => {
    const store = seeded()
    delete store.turns['s-a']; delete store.turns['s-a2']
    const first = deferred<{ turns: CreationTurn[] }>(), second = deferred<{ turns: CreationTurn[] }>()
    invoke.mockImplementation((_cmd, input) => input.params.sessionId === 's-a' ? first.promise : second.promise)
    const selectingFirst = store.selectSession(store.sessions['p-a'][0])
    const selectingSecond = store.selectSession(store.sessions['p-a'][1])
    second.resolve({ turns: [turn(job('second', 's-a2', 2, 'completed'))] }); await selectingSecond
    first.resolve({ turns: [turn(job('first', 's-a', 1, 'completed'))] }); await selectingFirst
    expect(store.activeSessionId).toBe('s-a2')
    expect(store.sessionTurns.map(item => item.jobId)).toEqual(['second'])
    expect(store.turns['s-a'].map(item => item.jobId)).toEqual(['first'])
  })

  it('does not let a slower project selection steal focus from a later project', async () => {
    const store = seeded()
    const first = deferred<{ sessions: CreationSession[] }>(), second = deferred<{ sessions: CreationSession[] }>()
    invoke.mockImplementation((_cmd, input) => input.method === 'cutflow.creation.sessions.list'
      ? input.params.projectId === 'p-a' ? first.promise : second.promise
      : Promise.resolve({ turns: [] }))
    const a = store.selectProject(store.projects[0]), b = store.selectProject(store.projects[1])
    second.resolve({ sessions: [session('s-b', 'p-b')] }); await b
    first.resolve({ sessions: [session('s-a', 'p-a')] }); await a
    expect(store.activeProjectId).toBe('p-b')
    expect(store.activeSessionId).toBe('s-b')
  })

  it('restores complete per-kind defaults while retaining a locally edited draft on refresh', async () => {
    const store = seeded()
    store.sessions['p-a'] = []
    invoke.mockResolvedValue({ sessions: [{
      id: 'partial', projectId: 'p-a', title: 'remote title', createdAt: 1, updatedAt: 2,
      drafts: { image: { prompt: 'saved image' }, video: {} },
    }] })
    await store.loadSessions('p-a')
    store.activeSessionId = 'partial'
    expect(store.draft).toMatchObject({ prompt: 'saved image', references: [], count: '1', imageSize: 'auto', independent: false })
    store.draft.prompt = 'typing locally'
    const oldImageDraft = store.session!.drafts.image
    invoke.mockResolvedValue({ sessions: [{
      ...session('partial'), title: 'generated first title', updatedAt: 100,
      drafts: { image: { ...emptyDraft(), prompt: 'stale server text' }, video: emptyDraft() },
    }] })
    await store.loadSessions('p-a')
    expect(store.session!.title).toBe('generated first title')
    expect(store.session!.updatedAt).toBe(100)
    expect(store.draft.prompt).toBe('typing locally')
    expect(store.draft.references).toEqual(oldImageDraft.references)
    store.kind = 'video'
    expect(store.draft.prompt).toBe('')
    expect(store.draft.references).toEqual([])
    expect(store.draft).not.toBe(oldImageDraft)
  })

  it('updates migrated legacy turns by job ID even without session fields in the legacy job', () => {
    const store = seeded()
    const original = job('legacy', 's-a', 1, 'failed')
    delete original.params.sessionId; delete original.params.turnId
    store.turns['s-a'] = [{ ...turn(original), id: 'legacy-turn', sessionId: 's-a' }]
    store.mergeJob({ ...original, revision: 2, status: 'completed' })
    expect(store.turns['s-a'][0].job.status).toBe('completed')
    expect(store.turns['s-a'][0].id).toBe('legacy-turn')
  })

  it('shares one catalog request and preserves the selected model when refresh removes it', async () => {
    const store = seeded()
    store.draft.model = 'gflow-nano-banana-2.1'; store.draft.modelSource = 'catalog'
    const response = deferred<CreationCatalog>()
    invoke.mockImplementation((_cmd, input) => input.method === 'cutflow.creation.catalog'
      ? response.promise : Promise.resolve(project()))
    const a = store.loadCatalog(true), b = store.loadCatalog(true)
    expect(invoke).toHaveBeenCalledTimes(1)
    response.resolve({ ...catalog(), models: [] })
    await Promise.all([a, b])
    expect(store.draft.model).toBe('gflow-nano-banana-2.1')
    expect(store.catalog.models).toEqual([])
  })

  it('refreshes the bound project capabilities without moving a later project selection', async () => {
    const store = seeded()
    store.projects[0].models = [{ ...model('gflow-nano-banana-2-lite'), imageSizes: ['1K'] }]
    const projectReceipt = deferred<CreationProject>()
    invoke.mockImplementation((_cmd, input) => input.method === 'cutflow.creation.catalog'
      ? Promise.resolve(catalog()) : projectReceipt.promise)
    const refreshing = store.loadCatalog(true)
    await Promise.resolve(); await Promise.resolve()
    expect(invoke.mock.calls.at(-1)?.[1]).toEqual({ method: 'cutflow.creation.projects.get', params: { projectId: 'p-a', refresh: true } })
    await store.selectSession(store.sessions['p-b'][0])
    projectReceipt.resolve({ ...project(), models: [{ ...model('gflow-nano-banana-2-lite'), imageSizes: ['1K', '2K', '4K'] }] })
    await refreshing
    expect(store.activeProjectId).toBe('p-b')
    expect(store.activeSessionId).toBe('s-b')
    expect(store.projects.find(item => item.id === 'p-a')?.models?.[0].imageSizes).toEqual(['1K', '2K', '4K'])
  })

  it('chooses a default only from the fixed project account capability list', () => {
    const store = seeded()
    store.projects[0].models = [model('gflow-nano-banana-2.1')]
    store.catalog.models = [model('gflow-nano-banana-2-lite')]
    store.defaultModel()
    expect(store.draft.model).toBe('gflow-nano-banana-2.1')
    store.projects[0].models = []
    store.draft.model = ''
    store.defaultModel()
    expect(store.draft.model).toBe('')
  })

  it('keeps a late new-chat receipt in its original project without stealing the active project', async () => {
    const store = seeded()
    const receipt = deferred<CreationSession>()
    invoke.mockImplementation((_cmd, input) => input.method === 'cutflow.creation.sessions.create'
      ? receipt.promise : Promise.resolve({ turns: [] }))
    const creating = store.newSession()
    await store.selectSession(store.sessions['p-b'][0])
    receipt.resolve(session('created-in-a', 'p-a'))
    await creating
    expect(store.activeProjectId).toBe('p-b')
    expect(store.activeSessionId).toBe('s-b')
    expect(store.sessions['p-a'].some(item => item.id === 'created-in-a')).toBe(true)
    expect(store.sessions['p-b'].some(item => item.id === 'created-in-a')).toBe(false)
    expect(invoke.mock.calls[0][1].params.projectId).toBe('p-a')
  })

  it('saves the explicitly requested session and kind while another session is selected', async () => {
    const store = seeded()
    store.sessions['p-a'][0].drafts.image.prompt = 'A image'
    store.sessions['p-a'][0].drafts.video.prompt = 'A video'
    store.activeSessionId = 's-a2'
    invoke.mockResolvedValue({ saved: true })
    await store.saveDraft('s-a', 'video')
    expect(invoke.mock.calls[0][1]).toMatchObject({
      method: 'cutflow.creation.drafts.save', params: { sessionId: 's-a', kind: 'video', draft: { prompt: 'A video' } },
    })
    const saved = invoke.mock.calls[0][1].params.draft
    store.sessions['p-a'][0].drafts.video.prompt = 'later edit'
    expect(saved.prompt).toBe('A video')
  })
})
