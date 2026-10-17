import { computed, ref } from 'vue';
import { defineStore } from 'pinia';
import {
  cutflow,
  operationId,
  type AiModel,
  type CreationCatalog,
  type CreationDraft,
  type CreationProject,
  type CreationSession,
  type CreationTurn,
  type Job,
} from '../api/cutflow';
export type { CreationDraft } from '../api/cutflow';
export type CreationKind = 'image' | 'video';
export const emptyDraft = (): CreationDraft => ({
  model: '',
  modelSource: 'catalog',
  prompt: '',
  aspect: 'auto',
  imageSize: 'auto',
  count: '1',
  duration: '8',
  references: [],
  stream: true,
  independent: false,
  selectedAssetPath: '',
});
const merge = <T extends { id: string }>(old: T[], incoming: T[]) => {
  const entries = new Map(old.map((value) => [value.id, value]));
  incoming.forEach((value) => entries.set(value.id, value));
  return [...entries.values()];
};
export const modelKind = (model: AiModel) =>
  model.kind === 'unknown'
    ? /^gflow-(veo|omni)-/.test(model.id)
      ? 'video'
      : /^gflow-nano-banana-/.test(model.id)
        ? 'image'
        : 'unknown'
    : model.kind;
export const modelName = (id: unknown) =>
  ({
    'gflow-nano-banana-pro': 'Nano Banana Pro',
    'gflow-nano-banana-2-lite': 'Nano Banana 2 Lite',
    'gflow-nano-banana-2.1': 'Nano Banana 2.1',
    'gflow-veo-3.1-lite': 'Veo 3.1 Lite',
    'gflow-veo-3.1-fast': 'Veo 3.1 Fast',
    'gflow-veo-3.1-quality': 'Veo 3.1 Quality',
    'gflow-omni-1.1-flash': 'Omni 1.1 Flash',
  })[String(id)] || String(id || '选择模型');
const normalizeSession = (item: CreationSession): CreationSession => ({
  ...item,
  drafts: { image: { ...emptyDraft(), ...item.drafts?.image }, video: { ...emptyDraft(), ...item.drafts?.video } },
});
const active = (job: Job) => ['running', 'queued'].includes(job.status);
export const useCreationStore = defineStore('creation', () => {
  const kind = ref<CreationKind>('image');
  const catalog = ref<CreationCatalog>({
    connection: { baseUrl: '' },
    models: [],
    references: [],
    fault: null,
    workspaceId: '',
    projectBinding: false,
    revision: 0,
  });
  const projects = ref<CreationProject[]>([]);
  const sessions = ref<Record<string, CreationSession[]>>({});
  const turns = ref<Record<string, CreationTurn[]>>({});
  const projectCursor = ref<string | null>(null);
  const sessionCursors = ref<Record<string, string | null>>({});
  const turnCursors = ref<Record<string, string | null>>({});
  const activeProjectId = ref('');
  const activeSessionId = ref('');
  const loading = ref(false);
  const error = ref('');
  const revision = ref(0);
  const pending = ref<Record<string, string>>({});
  const jobs = ref<Record<string, Job>>({});
  const stoppingIds = ref<string[]>([]);
  const selection = ref<Record<string, string>>({});
  const detached = ref<Record<string, { image: CreationDraft; video: CreationDraft }>>({});
  const project = computed(() => projects.value.find((item) => item.id === activeProjectId.value));
  const session = computed(() =>
    (sessions.value[activeProjectId.value] || []).find((item) => item.id === activeSessionId.value),
  );
  const sessionTurns = computed(() => turns.value[activeSessionId.value] || []);
  const draft = computed(() => session.value?.drafts[kind.value] || detached.value.none?.[kind.value] || emptyDraft());
  const activeJob = computed(() =>
    Object.values(jobs.value).find((item) => item.params.sessionId === activeSessionId.value && active(item)),
  );
  const locked = computed(() => !!pending.value[activeSessionId.value] || !!activeJob.value);
  let catalogFlight: Promise<void> | undefined;
  let polling: Promise<void> | undefined;
  let sessionTicket = 0;
  function defaultModel() {
    if (draft.value.model) return;
    const available =
      project.value?.state === 'ready' && project.value.models ? project.value.models : catalog.value.models;
    const models = available.filter(
      (item) => item.id.startsWith('gflow-') && item.id !== 'gflow-project-control' && modelKind(item) === kind.value,
    );
    const model = models.find((item) => item.id === 'gflow-nano-banana-2-lite') || models[0];
    if (model) {
      draft.value.model = model.id;
      draft.value.modelSource = 'catalog';
    }
  }
  async function loadCatalog(refresh = false) {
    if (catalogFlight) return catalogFlight;
    catalogFlight = (async () => {
      try {
        const response = await cutflow<CreationCatalog>('cutflow.creation.catalog', { refresh });
        catalog.value = response;
        defaultModel();
      } catch (reason) {
        error.value = String((reason as Error).message || reason);
      }
    })().finally(() => {
      catalogFlight = undefined;
    });
    return catalogFlight;
  }
  async function loadProjects(more = false) {
    const response = await cutflow<{ projects: CreationProject[]; nextCursor?: string }>(
      'cutflow.creation.projects.list',
      { limit: 50, ...(more && projectCursor.value ? { cursor: projectCursor.value } : {}) },
    );
    projects.value = more ? merge(projects.value, response.projects || []) : response.projects || [];
    projectCursor.value = response.nextCursor || null;
  }
  async function loadSessions(projectId: string, more = false) {
    const response = await cutflow<{ sessions: CreationSession[]; nextCursor?: string }>(
      'cutflow.creation.sessions.list',
      {
        projectId,
        limit: 50,
        ...(more && sessionCursors.value[projectId] ? { cursor: sessionCursors.value[projectId] } : {}),
      },
    );
    // 已加载草稿保持同一对象，异步列表回包不能覆盖用户正在输入的内容。
    const old = sessions.value[projectId] || [];
    const incoming = response.sessions.map((item) => {
      const existing = old.find((value) => value.id === item.id);
      return normalizeSession({ ...item, drafts: existing?.drafts || item.drafts });
    });
    sessions.value[projectId] = more ? merge(old, incoming) : incoming;
    sessionCursors.value[projectId] = response.nextCursor || null;
  }
  function mergeJob(job: Job) {
    const previous = jobs.value[job.id];
    if (previous?.revision && job.revision && previous.revision > job.revision) return;
    jobs.value[job.id] = job;
    for (const list of Object.values(turns.value)) {
      const turn = list.find((item) => item.jobId === job.id);
      if (turn) turn.job = job;
    }
    if (!active(job)) stoppingIds.value = stoppingIds.value.filter((id) => id !== job.id);
  }
  async function loadTurns(sessionId: string, more = false) {
    const response = await cutflow<{ turns: CreationTurn[]; nextCursor?: string }>('cutflow.creation.turns.list', {
      sessionId,
      limit: 50,
      ...(more && turnCursors.value[sessionId] ? { cursor: turnCursors.value[sessionId] } : {}),
    });
    const incoming = response.turns || [];
    incoming.forEach((turn) => {
      mergeJob(turn.job);
      turn.job = jobs.value[turn.jobId] || turn.job;
    });
    turns.value[sessionId] = merge(turns.value[sessionId] || [], incoming).sort(
      (a, b) => a.createdAt - b.createdAt || a.id.localeCompare(b.id),
    );
    turnCursors.value[sessionId] = response.nextCursor || null;
  }
  async function selectSession(item: CreationSession) {
    sessionTicket++;
    activeProjectId.value = item.projectId;
    activeSessionId.value = item.id;
    defaultModel();
    if (!turns.value[item.id]) await loadTurns(item.id);
  }
  async function selectProject(item: CreationProject) {
    const ticket = ++sessionTicket;
    activeProjectId.value = item.id;
    activeSessionId.value = '';
    await loadSessions(item.id);
    if (ticket !== sessionTicket) return;
    const first = sessions.value[item.id]?.[0];
    if (first) await selectSession(first);
  }
  async function createProject(name: string, preferredModel: string, creationOperation: string = operationId()) {
    const item = await cutflow<CreationProject>('cutflow.creation.projects.create', {
      name,
      preferredModel,
      operationId: creationOperation,
    });
    projects.value = merge(projects.value, [item]).sort((a, b) => b.updatedAt - a.updatedAt);
    await selectProject(item);
    if (activeProjectId.value === item.id && item.state === 'ready' && !sessions.value[item.id]?.length)
      await newSession();
    return item;
  }
  async function checkProject() {
    if (!project.value) return;
    const item = await cutflow<CreationProject>('cutflow.creation.projects.get', { projectId: project.value.id });
    projects.value = merge(projects.value, [item]);
    if (activeProjectId.value === item.id && item.state === 'ready' && !sessions.value[item.id]?.length)
      await newSession();
  }
  async function newSession() {
    if (!project.value || project.value.state !== 'ready') return;
    const requestedProject = project.value.id;
    const ticket = ++sessionTicket;
    const item = normalizeSession(
      await cutflow<CreationSession>('cutflow.creation.sessions.create', {
        projectId: requestedProject,
        title: '新聊天',
        operationId: operationId(),
      }),
    );
    sessions.value[item.projectId] = [item, ...(sessions.value[item.projectId] || [])];
    if (ticket === sessionTicket && activeProjectId.value === requestedProject) await selectSession(item);
  }
  async function saveDraft(sessionId = activeSessionId.value, mediaKind = kind.value) {
    const item = Object.values(sessions.value)
      .flat()
      .find((value) => value.id === sessionId);
    if (!item || item.projectId === 'legacy') return;
    await cutflow('cutflow.creation.drafts.save', {
      sessionId,
      kind: mediaKind,
      draft: JSON.parse(JSON.stringify(item.drafts[mediaKind])),
    });
  }
  async function poll() {
    if (polling) return polling;
    polling = (async () => {
      const response = await cutflow<{ jobs: Job[]; revision: number }>(
        'cutflow.jobs.list',
        revision.value ? { sinceRevision: revision.value } : { activeOnly: true },
      );
      response.jobs.forEach((job) => {
        mergeJob(job);
        const sid = String(job.params.sessionId || '');
        const id = String(job.params.turnId || '');
        if (sid && turns.value[sid] && id && !turns.value[sid].some((turn) => turn.id === id)) {
          turns.value[sid].push({
            id,
            sessionId: sid,
            jobId: job.id,
            prompt: String(job.params.prompt || ''),
            params: job.params,
            createdAt: Number((job as Job & { createdAt?: number }).createdAt || Date.now()),
            job,
          });
        }
      });
      revision.value = response.revision;
    })().finally(() => {
      polling = undefined;
    });
    return polling;
  }
  const hasActive = computed(() => Object.values(jobs.value).some(active) || Object.keys(pending.value).length > 0);
  async function initialize() {
    loading.value = true;
    try {
      await Promise.all([loadCatalog(), loadProjects()]);
      if (!activeProjectId.value && projects.value[0]) await selectProject(projects.value[0]);
      await poll();
      void loadCatalog(true);
    } catch (reason) {
      error.value = String((reason as Error).message || reason);
    } finally {
      loading.value = false;
    }
  }
  return {
    kind,
    catalog,
    projects,
    sessions,
    turns,
    projectCursor,
    sessionCursors,
    turnCursors,
    activeProjectId,
    activeSessionId,
    loading,
    error,
    revision,
    pending,
    jobs,
    stoppingIds,
    selection,
    project,
    session,
    sessionTurns,
    draft,
    activeJob,
    locked,
    hasActive,
    defaultModel,
    loadCatalog,
    loadProjects,
    loadSessions,
    loadTurns,
    selectSession,
    selectProject,
    createProject,
    checkProject,
    newSession,
    saveDraft,
    poll,
    mergeJob,
    initialize,
  };
});
