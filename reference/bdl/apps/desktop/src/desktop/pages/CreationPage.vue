<script setup lang="ts">
import { computed, nextTick, onActivated, onDeactivated, onMounted, onUnmounted, ref, watch } from 'vue';
import { open } from '@tauri-apps/plugin-dialog';
import UiButton from '../../shared/ui/Button.vue';
import UiDialog from '../../shared/ui/Dialog.vue';
import {
  cutflow,
  errorMessage,
  mediaSource,
  operationId,
  type Asset,
  type CreationSession,
  type CreationTurn,
  type Job,
} from '../../shared/api/cutflow';
import { useCreationStore, modelKind, modelName } from '../../shared/stores/creation';
import { useCutflowStore } from '../../shared/stores/cutflow';
import { useUiStore } from '../../shared/stores/ui';
import { calculateVirtualWindow } from '../../shared/utils/virtualWindow';

const creation = useCreationStore();
const editor = useCutflowStore();
const ui = useUiStore();
const projectDialog = ref(false),
  projectName = ref(''),
  creatingProject = ref(false),
  projectError = ref(''),
  projectOperation = ref('');
const projectExpanded = ref(true);
const panels = ref<'projects' | 'parameters' | null>(null);
const originalAsset = ref<Asset | null>(null),
  details = ref<Job | null>(null);
const modelSearch = ref(''),
  sessionSearch = ref(''),
  referencesOpen = ref(false);
const referenceName = ref(''),
  referenceKind = ref('character'),
  notice = ref('');
const timeline = ref<HTMLElement | null>(null),
  sessionList = ref<HTMLElement | null>(null);
const followBottom = ref(true),
  newResults = ref(false),
  sessionOffset = ref(0),
  sessionHeight = ref(600);
// 历史数据分页保存，DOM仅渲染可见轮次；真实高度改变时校正滚动锚点。
const turnOffset = ref(0);
const turnViewportHeight = ref(600);
const turnHeights = ref<Record<string, number>>({});
let turnObserver: ResizeObserver | undefined;
let bottomPending = false;
let restoringViewport = false;
const turnPositions = computed(() => {
  const prefix = [0];
  for (const turn of creation.sessionTurns) {
    const estimated = turn.job.result?.assets?.length ? 520 : 210;
    prefix.push(prefix[prefix.length - 1] + (turnHeights.value[turn.id] || estimated));
  }
  return prefix;
});
const turnWindow = computed(() => {
  const prefix = turnPositions.value,
    count = creation.sessionTurns.length;
  if (count <= 20) return { start: 0, end: count };
  if (followBottom.value) return { start: Math.max(0, count - 6), end: count };
  let first = 0,
    end = count;
  while (first < count && prefix[first + 1] < turnOffset.value) first++;
  end = first;
  while (end < count && prefix[end] < turnOffset.value + turnViewportHeight.value) end++;
  return { start: Math.max(0, first - 2), end: Math.min(count, end + 3) };
});
const visibleTurns = computed(() => creation.sessionTurns.slice(turnWindow.value.start, turnWindow.value.end));
async function observeTurns() {
  await nextTick();
  if (!live || typeof ResizeObserver === 'undefined') return;
  turnObserver?.disconnect();
  turnObserver = new ResizeObserver((entries) => {
    let changed = false;
    for (const entry of entries) {
      const element = entry.target as HTMLElement;
      const id = element.dataset.turnId;
      if (!id) continue;
      const height =
        element.getBoundingClientRect().height + Number.parseFloat(getComputedStyle(element).marginBottom || '0');
      if (Math.abs((turnHeights.value[id] || 0) - height) > 1) {
        turnHeights.value[id] = height;
        changed = true;
      }
    }
    if (changed && followBottom.value) void bottom();
  });
  timeline.value?.querySelectorAll('.creation-turn').forEach((element) => turnObserver!.observe(element));
}
let live = false;
let timer: ReturnType<typeof setTimeout> | undefined;
let draftTimer: ReturnType<typeof setTimeout> | undefined;
let observer: ResizeObserver | undefined;
let starting: Promise<void> | undefined;

const draft = computed(() => creation.draft);
const projectModels = computed(() =>
  creation.project?.state === 'ready' && creation.project.models ? creation.project.models : creation.catalog.models,
);
const models = computed(() =>
  projectModels.value.filter(
    (item) =>
      item.id.startsWith('gflow-') &&
      item.id !== 'gflow-project-control' &&
      modelKind(item) === creation.kind &&
      (item.id + modelName(item.id)).toLowerCase().includes(modelSearch.value.toLowerCase()),
  ),
);
const capability = computed(() => projectModels.value.find((item) => item.id === draft.value.model));
const invalidModel = computed(
  () =>
    !draft.value.model.startsWith('gflow-') ||
    draft.value.model === 'gflow-project-control' ||
    (draft.value.modelSource !== 'custom' && !capability.value) ||
    (!!capability.value && modelKind(capability.value) !== creation.kind),
);
const aspectOptions = computed(() =>
  capability.value?.aspects?.length
    ? capability.value.aspects
    : creation.kind === 'image'
      ? ['1:1', '16:9', '9:16', '4:3', '3:4']
      : ['16:9', '9:16'],
);
const sizeOptions = computed(
  () =>
    capability.value?.imageSizes || (capability.value?.id.startsWith('gflow-nano-banana-') ? ['1K', '2K', '4K'] : []),
);
const durationOptions = computed(
  () => capability.value?.durations || (draft.value.model.includes('omni') ? [4, 6, 8, 10] : [4, 6, 8]),
);
const selectedReference = computed(() => {
  const selected = creation.selection[creation.activeSessionId] || draft.value.selectedAssetPath;
  if (selected) {
    for (const turn of creation.sessionTurns) {
      if (turn.job.params.kind === 'image') {
        const image = turn.job.result?.assets?.find((item) => item.path === selected);
        if (image) return image;
      }
    }
    // 选中过的旧作品可能尚未翻页载入；保留明确引用，不改指向另一张图。
    return { path: selected };
  }
  for (const turn of [...creation.sessionTurns].reverse()) {
    if (turn.job.params.kind === 'image' && turn.job.result?.assets?.length) return turn.job.result.assets[0];
  }
  return undefined;
});
const automaticReference = computed(() =>
  creation.kind === 'image' && !draft.value.independent ? selectedReference.value : undefined,
);
const effectiveReferences = computed(() => [
  ...new Set([...(automaticReference.value ? [automaticReference.value.path] : []), ...draft.value.references]),
]);
const referenceLimit = computed(() => capability.value?.maxReferences ?? 3);
const canGenerate = computed(
  () =>
    creation.project?.state === 'ready' &&
    !!creation.session &&
    !creation.locked &&
    !invalidModel.value &&
    !!draft.value.prompt.trim() &&
    effectiveReferences.value.length <= referenceLimit.value,
);
const sessions = computed(() =>
  [...(creation.sessions[creation.activeProjectId] || [])]
    .sort((a, b) => b.updatedAt - a.updatedAt)
    .filter((item) => item.title.toLowerCase().includes(sessionSearch.value.toLowerCase())),
);
const sessionWindow = computed(() =>
  calculateVirtualWindow(sessions.value.length, sessionOffset.value, sessionHeight.value, 60),
);
const visibleSessions = computed(() => sessions.value.slice(sessionWindow.value.start, sessionWindow.value.end));
const statusLabel = (status: string) =>
  ({
    queued: '等待中',
    running: '生成中',
    completed: '已保存',
    failed: '生成失败',
    cancelled: '已停止',
    interrupted: '已中断',
    result_unknown: '结果待核查',
  })[status] || status;
const projectStatus = (state: string) =>
  ({
    ready: '已连接 Flow',
    creating: '正在创建',
    failed: '创建失败',
    creation_unknown: '创建结果待核查',
    legacy: '本地历史',
  })[state] || state;
const isRunning = (job: Job) => ['running', 'queued'].includes(job.status);
const isStopping = (job: Job) => creation.stoppingIds.includes(job.id);
const timeLabel = (value: number) =>
  new Date(value).toLocaleString('zh-CN', { month: 'numeric', day: 'numeric', hour: '2-digit', minute: '2-digit' });
const thumb = (turn: CreationTurn, asset: Asset, index: number) =>
  asset.thumbnail || (index === 0 ? turn.job.result?.thumbnail : '');
const assetSelected = (asset: Asset) => automaticReference.value?.path === asset.path;
const ratioMismatch = (turn: CreationTurn, asset: Asset) => {
  if (turn.params.kind !== 'image' || !asset.width || !asset.height) return false;
  const ratio = String(turn.params.aspect_ratio || 'auto');
  if (ratio === 'auto') return false;
  const [w, h] = ratio.split(':').map(Number);
  return w > 0 && h > 0 && Math.abs(asset.width / asset.height / (w / h) - 1) > 0.03;
};
function updateViewport() {
  if (sessionList.value) {
    sessionOffset.value = sessionList.value.scrollTop;
    sessionHeight.value = sessionList.value.clientHeight;
  }
}
function observeScroll() {
  if (!live || restoringViewport) return;
  const element = timeline.value;
  if (!element) return;
  turnOffset.value = element.scrollTop;
  turnViewportHeight.value = element.clientHeight;
  if (bottomPending) return;
  followBottom.value = element.scrollHeight - element.clientHeight - element.scrollTop < 80;
  if (followBottom.value) newResults.value = false;
}
async function bottom() {
  followBottom.value = true;
  newResults.value = false;
  if (bottomPending) return;
  bottomPending = true;
  await nextTick();
  const element = timeline.value;
  if (!element) {
    bottomPending = false;
    return;
  }
  element.scrollTop = element.scrollHeight;
  turnOffset.value = element.scrollTop;
  turnViewportHeight.value = element.clientHeight;
  requestAnimationFrame(() => {
    if (followBottom.value && timeline.value) {
      timeline.value.scrollTop = timeline.value.scrollHeight;
      turnOffset.value = timeline.value.scrollTop;
    }
    bottomPending = false;
  });
}
async function olderTurns() {
  followBottom.value = false;
  const element = timeline.value,
    previous = element?.scrollHeight || 0;
  await creation.loadTurns(creation.activeSessionId, true);
  await nextTick();
  if (element) {
    element.scrollTop += element.scrollHeight - previous;
    turnOffset.value = element.scrollTop;
  }
}
async function selectSession(item: CreationSession) {
  try {
    await creation.saveDraft();
    await creation.selectSession(item);
    panels.value = null;
    notice.value = '';
    await bottom();
  } catch (reason) {
    creation.error = errorMessage(reason);
  }
}
async function selectProject(id: string) {
  projectExpanded.value = true;
  const item = creation.projects.find((value) => value.id === id);
  if (!item) return;
  try {
    await creation.saveDraft();
    await creation.selectProject(item);
    sessionOffset.value = 0;
    panels.value = null;
    await bottom();
  } catch (reason) {
    creation.error = errorMessage(reason);
  }
}
async function newChat() {
  projectExpanded.value = true;
  if (creation.project?.state !== 'ready') {
    projectDialog.value = true;
    return;
  }
  try {
    await creation.saveDraft();
    await creation.newSession();
    panels.value = null;
    await bottom();
  } catch (reason) {
    creation.error = errorMessage(reason);
  }
}
async function newProject() {
  if (creatingProject.value || !projectName.value.trim()) return;
  creatingProject.value = true;
  projectError.value = '';
  projectOperation.value ||= operationId();
  try {
    const preferred =
      draft.value.model ||
      creation.catalog.models.find((item) => modelKind(item) === 'image')?.id ||
      'gflow-nano-banana-2-lite';
    const project = await creation.createProject(projectName.value.trim(), preferred, projectOperation.value);
    projectExpanded.value = true;
    projectDialog.value = false;
    if (project.state === 'ready') {
      projectName.value = '';
      projectOperation.value = '';
      panels.value = null;
      await bottom();
    } else {
      notice.value = project.error || projectStatus(project.state);
      if (project.state === 'failed') projectOperation.value = '';
    }
  } catch (reason) {
    projectError.value = errorMessage(reason);
    await creation.loadProjects();
  } finally {
    creatingProject.value = false;
  }
}
async function checkProject() {
  try {
    await creation.checkProject();
    notice.value = '';
    await bottom();
  } catch (reason) {
    notice.value = errorMessage(reason);
  }
}
async function chooseReferences() {
  try {
    const paths = await open({
      multiple: true,
      filters: [{ name: '参考图片', extensions: ['png', 'jpg', 'jpeg', 'webp'] }],
    });
    const incoming = Array.isArray(paths) ? paths : typeof paths === 'string' ? [paths] : [];
    const merged = [...new Set([...draft.value.references, ...incoming])];
    if (
      merged.length + (automaticReference.value && !merged.includes(automaticReference.value.path) ? 1 : 0) >
      referenceLimit.value
    ) {
      notice.value = '当前模型最多支持 ' + referenceLimit.value + ' 张参考图，请先移除部分图片。';
      return;
    }
    draft.value.references = merged;
  } catch (reason) {
    notice.value = errorMessage(reason);
  }
}
function useAsReference(asset: Asset) {
  if (creation.kind !== 'image') creation.kind = 'image';
  creation.selection[creation.activeSessionId] = asset.path;
  draft.value.selectedAssetPath = asset.path;
  draft.value.independent = false;
}
async function saveReference() {
  const path = effectiveReferences.value[0];
  if (!path || !referenceName.value.trim()) return;
  try {
    await cutflow('cutflow.references.save', {
      path,
      name: referenceName.value.trim(),
      kind: referenceKind.value,
      prompt: draft.value.prompt,
    });
    referenceName.value = '';
    await creation.loadCatalog();
  } catch (reason) {
    notice.value = errorMessage(reason);
  }
}
function addSavedReference(path: string, prompt: string) {
  const paths = [...new Set([...draft.value.references, path])];
  if (
    paths.length + (automaticReference.value && !paths.includes(automaticReference.value.path) ? 1 : 0) >
    referenceLimit.value
  ) {
    notice.value = '当前模型最多支持 ' + referenceLimit.value + ' 张参考图。';
    return;
  }
  draft.value.references = paths;
  if (!draft.value.prompt) draft.value.prompt = prompt;
}
async function submit() {
  if (creation.locked) return;
  notice.value = '';
  if (!creation.session || creation.project?.state !== 'ready') {
    notice.value = '请先新建或选择已连接 Flow 的项目。';
    return;
  }
  if (invalidModel.value) {
    notice.value = '模型已失效或与当前图片／视频类别不匹配，请重新选择。';
    return;
  }
  if (effectiveReferences.value.length > referenceLimit.value) {
    notice.value = '当前模型最多支持 ' + referenceLimit.value + ' 张参考图。';
    return;
  }
  if (!canGenerate.value) return;
  const sid = creation.activeSessionId,
    pid = creation.activeProjectId;
  const turnId = operationId(),
    receipt = operationId(),
    prompt = draft.value.prompt.trim(),
    mediaKind = creation.kind;
  const parameters = {
    creationProjectId: pid,
    sessionId: sid,
    turnId,
    operationId: receipt,
    model: draft.value.model,
    prompt,
    kind: mediaKind,
    references: effectiveReferences.value,
    ...(automaticReference.value ? { continuationAssetPath: automaticReference.value.path } : {}),
    maxGenerationCalls: mediaKind === 'image' ? Number(draft.value.count) : 1,
    n: mediaKind === 'image' ? Number(draft.value.count) : 1,
    aspect_ratio: draft.value.aspect,
    image_size: draft.value.imageSize,
    duration: Number(draft.value.duration),
    stream: draft.value.stream,
  };
  creation.pending[sid] = receipt;
  // 发送时取固定快照，切换会话不会改变任务归属或参数。
  try {
    await creation.saveDraft(sid, mediaKind);
    const response = await cutflow<{ jobId: string }>('cutflow.generate', parameters);
    const job: Job = {
      id: response.jobId,
      operationId: receipt,
      method: 'cutflow.generate',
      params: parameters,
      status: 'queued',
      stage: '正在等待生成',
    };
    creation.mergeJob(job);
    creation.turns[sid] ||= [];
    if (!creation.turns[sid].some((turn) => turn.id === turnId))
      creation.turns[sid].push({
        id: turnId,
        sessionId: sid,
        jobId: job.id,
        prompt,
        params: parameters,
        createdAt: Date.now(),
        job,
      });
    const session = (creation.sessions[pid] || []).find((item) => item.id === sid);
    if (session) {
      if (session.drafts[mediaKind].prompt.trim() === prompt) session.drafts[mediaKind].prompt = '';
      if (session.title === '新聊天') session.title = prompt.slice(0, 24);
      session.updatedAt = Date.now();
      await creation.saveDraft(sid, mediaKind);
    }
    if (creation.activeSessionId === sid) await bottom();
    schedulePoll();
  } catch (reason) {
    notice.value = errorMessage(reason);
  } finally {
    delete creation.pending[sid];
  }
}
async function stopJob(job: Job) {
  if (isStopping(job)) return;
  creation.stoppingIds.push(job.id);
  try {
    const result = await cutflow<{ message: string }>('cutflow.jobs.cancel', { jobId: job.id });
    notice.value = result.message;
    schedulePoll();
  } catch (reason) {
    creation.stoppingIds = creation.stoppingIds.filter((id) => id !== job.id);
    notice.value = errorMessage(reason);
  }
}
async function retryDownload(job: Job) {
  if (creation.locked) return;
  try {
    await cutflow('cutflow.jobs.retryDownload', { jobId: job.id });
    creation.mergeJob({ ...job, status: 'running', error: null, stage: '正在重新下载已生成结果' });
    schedulePoll();
  } catch (reason) {
    notice.value = errorMessage(reason);
  }
}
async function openFile(path: string) {
  try {
    await cutflow('cutflow.shell.open', { path });
  } catch (reason) {
    notice.value = errorMessage(reason);
  }
}
function schedulePoll() {
  clearTimeout(timer);
  if (!live || document.hidden || !creation.hasActive) return;
  timer = setTimeout(async () => {
    try {
      await creation.poll();
    } catch (reason) {
      if (live) creation.error = errorMessage(reason);
    } finally {
      schedulePoll();
    }
  }, 2000);
}
async function activate() {
  const restoreOffset = turnOffset.value;
  restoringViewport = true;
  live = true;
  if (starting) return starting;
  starting = (async () => {
    if (!creation.catalog.workspaceId) await creation.initialize();
    else {
      await creation.poll();
      await creation.loadCatalog();
    }
    if (live) {
      schedulePoll();
      if (followBottom.value) await bottom();
      else {
        await nextTick();
        if (timeline.value) timeline.value.scrollTop = restoreOffset;
        turnOffset.value = restoreOffset;
      }
      await observeTurns();
      requestAnimationFrame(() => {
        restoringViewport = false;
      });
    }
  })().finally(() => {
    starting = undefined;
  });
  return starting;
}
function deactivate() {
  live = false;
  restoringViewport = false;
  clearTimeout(timer);
  clearTimeout(draftTimer);
  turnObserver?.disconnect();
  void creation.saveDraft().catch(() => undefined);
}
function escapePanel(event: KeyboardEvent) {
  if (event.key === 'Escape') panels.value = null;
}
function visibility() {
  clearTimeout(timer);
  if (document.hidden || !live) return;
  void creation
    .poll()
    .catch((reason) => {
      creation.error = errorMessage(reason);
    })
    .finally(schedulePoll);
}
watch(
  () => [creation.kind, draft.value.model],
  () => {
    creation.defaultModel();
    if (!aspectOptions.value.includes(draft.value.aspect) && draft.value.aspect !== 'auto') draft.value.aspect = 'auto';
    if (!sizeOptions.value.includes(draft.value.imageSize) && draft.value.imageSize !== 'auto')
      draft.value.imageSize = 'auto';
  },
);
watch(
  () => JSON.stringify(draft.value),
  () => {
    clearTimeout(draftTimer);
    const sid = creation.activeSessionId,
      mediaKind = creation.kind;
    if (sid)
      draftTimer = setTimeout(() => {
        void creation.saveDraft(sid, mediaKind).catch((reason) => {
          creation.error = errorMessage(reason);
        });
      }, 450);
  },
);
watch(
  () =>
    creation.sessionTurns
      .map((turn) => turn.job.id + ':' + turn.job.status + ':' + (turn.job.result?.assets?.length || 0))
      .join('|'),
  () => {
    if (followBottom.value) void bottom();
    else newResults.value = true;
  },
);
watch(
  () => visibleTurns.value.map((turn) => turn.id).join('|'),
  () => {
    void observeTurns();
  },
  { flush: 'post' },
);
watch(() => creation.hasActive, schedulePoll);
onMounted(() => {
  document.addEventListener('visibilitychange', visibility);
  document.addEventListener('keydown', escapePanel);
  if (typeof ResizeObserver !== 'undefined' && sessionList.value) {
    observer = new ResizeObserver(updateViewport);
    observer.observe(sessionList.value);
  }
});
onActivated(() => {
  void activate().catch((reason) => {
    creation.error = errorMessage(reason);
  });
});
onDeactivated(deactivate);
onUnmounted(() => {
  deactivate();
  observer?.disconnect();
  document.removeEventListener('visibilitychange', visibility);
  document.removeEventListener('keydown', escapePanel);
});
</script>

<template>
  <section
    class="creation-studio"
    aria-label="项目式 AI 创作"
    :class="{ 'projects-open': panels === 'projects', 'parameters-open': panels === 'parameters' }"
  >
    <button v-if="panels" class="studio-drawer-scrim" aria-label="关闭侧栏" @click="panels = null"></button>
    <aside class="creation-projects" aria-label="创作项目与会话">
      <header class="studio-sidebar-heading">
        <span><UIcon name="i-tabler-sparkles" aria-hidden="true" />创作空间</span
        ><button class="studio-drawer-close" aria-label="关闭项目栏" @click="panels = null">×</button>
      </header>
      <div class="project-actions">
        <UiButton variant="secondary" @click="projectDialog = true"
          ><UIcon name="i-tabler-folder-plus" aria-hidden="true" />新建项目</UiButton
        ><UiButton variant="ghost" @click="newChat"><UIcon name="i-tabler-plus" aria-hidden="true" />新聊天</UiButton>
      </div>
      <div class="project-picker">
        <label for="creation-project">项目</label
        ><select
          id="creation-project"
          :value="creation.activeProjectId"
          @change="selectProject(($event.target as HTMLSelectElement).value)"
        >
          <option value="" disabled>选择项目</option>
          <option v-for="project in creation.projects" :key="project.id" :value="project.id">
            {{ project.name }}
          </option></select
        ><button v-if="creation.projectCursor" class="studio-link" @click="creation.loadProjects(true)">
          加载更多项目
        </button>
      </div>
      <button
        v-if="creation.project"
        class="project-binding"
        :data-state="creation.project.state"
        :aria-expanded="projectExpanded"
        aria-controls="project-conversations"
        @click="projectExpanded = !projectExpanded"
      >
        <UIcon name="i-tabler-folder" aria-hidden="true" /><span
          >{{ projectStatus(creation.project.state)
          }}<small v-if="creation.project.accountLabel">{{ creation.project.accountLabel }}</small></span
        >
        <UIcon :name="projectExpanded ? 'i-tabler-chevron-down' : 'i-tabler-chevron-right'" aria-hidden="true" />
      </button>
      <div v-show="projectExpanded" class="session-search">
        <UIcon name="i-tabler-search" aria-hidden="true" /><input
          v-model="sessionSearch"
          aria-label="搜索当前项目聊天"
          placeholder="搜索聊天…"
        />
      </div>
      <div
        id="project-conversations"
        ref="sessionList"
        v-show="projectExpanded"
        class="session-list"
        @scroll="updateViewport"
      >
        <div v-if="sessions.length" class="virtual-sessions" :style="{ height: sessionWindow.totalSize + 'px' }">
          <button
            v-for="(session, index) in visibleSessions"
            :key="session.id"
            class="session-row"
            :aria-current="creation.activeSessionId === session.id ? 'page' : undefined"
            :style="{ transform: 'translateY(' + (sessionWindow.start + index) * 60 + 'px)' }"
            :title="session.title"
            @click="selectSession(session)"
          >
            <UIcon name="i-tabler-message" aria-hidden="true" /><span
              ><strong>{{ session.title }}</strong
              ><small>{{ timeLabel(session.updatedAt) }}</small></span
            >
          </button>
        </div>
        <p v-else class="studio-muted">{{ creation.loading ? '正在读取会话…' : '项目内的聊天会显示在这里' }}</p>
        <button
          v-if="creation.sessionCursors[creation.activeProjectId]"
          class="studio-link load-sessions"
          @click="creation.loadSessions(creation.activeProjectId, true)"
        >
          加载更多聊天
        </button>
      </div>
      <footer class="project-sidebar-footer">
        <span class="studio-status-dot"></span><span>素材保存在本机<small>项目与账号固定绑定</small></span>
      </footer>
    </aside>

    <main class="creation-conversation">
      <header class="conversation-heading">
        <button class="studio-drawer-toggle" aria-label="打开项目与聊天" @click="panels = 'projects'">
          <UIcon name="i-tabler-layout-sidebar-left-expand" />
        </button>
        <div>
          <span class="studio-eyebrow">{{ creation.project?.name || 'AI 创作' }}</span>
          <h1>{{ creation.session?.title || '让想法，成为画面' }}</h1>
        </div>
        <button class="parameter-toggle" aria-label="打开生成参数" @click="panels = 'parameters'">
          <UIcon name="i-tabler-adjustments" />
        </button>
      </header>
      <div v-if="creation.error || creation.catalog.fault" class="studio-notice" role="alert">
        {{ creation.error || creation.catalog.fault
        }}<button
          @click="
            creation.error = '';
            creation.loadCatalog(true);
          "
        >
          重新检查
        </button>
      </div>
      <div
        v-if="creation.project && !['ready', 'legacy'].includes(creation.project.state)"
        class="studio-notice"
        role="status"
      >
        {{ creation.project.error || projectStatus(creation.project.state)
        }}<button @click="checkProject">核查项目状态</button>
      </div>
      <div ref="timeline" class="creation-timeline" tabindex="0" aria-label="生成聊天记录" @scroll="observeScroll">
        <button
          v-if="creation.turnCursors[creation.activeSessionId]"
          class="studio-link older-turns"
          @click="olderTurns"
        >
          加载更早的记录
        </button>
        <div v-if="!creation.sessionTurns.length" class="conversation-empty">
          <div class="empty-symbol"><UIcon name="i-tabler-sparkles" aria-hidden="true" /></div>
          <span class="studio-eyebrow">CUTFLOW STUDIO</span>
          <h2>{{ creation.project ? '从一个想法开始' : '给创作，一个自己的空间' }}</h2>
          <p>
            {{
              creation.project
                ? '描述你想看到的画面。每一轮生成都会留在当前聊天，选中作品后还可以继续改图。'
                : '新建项目，连接你的 Google Flow。项目内可以开启多个独立聊天，让不同的想法各自生长。'
            }}
          </p>
          <UiButton v-if="!creation.project" @click="projectDialog = true"
            >新建第一个项目<UIcon name="i-tabler-arrow-up-right" aria-hidden="true" /></UiButton
          ><span v-else class="empty-shortcut">Ctrl + Enter 发送 · 支持参考图</span>
        </div>
        <div
          v-if="turnWindow.start > 0"
          aria-hidden="true"
          :style="{ height: turnPositions[turnWindow.start] + 'px' }"
        ></div>
        <article
          v-for="turn in visibleTurns"
          :key="turn.id"
          class="creation-turn"
          :data-turn-id="turn.id"
          :data-status="turn.job.status"
        >
          <div class="user-turn">
            <time>{{ timeLabel(turn.createdAt) }}</time>
            <div class="prompt-bubble">{{ turn.prompt }}</div>
            <div
              v-if="Array.isArray(turn.params.references) && turn.params.references.length"
              class="turn-reference-note"
            >
              <UIcon name="i-tabler-paperclip" aria-hidden="true" />{{ turn.params.references.length }} 张参考图
            </div>
          </div>
          <div class="assistant-turn">
            <header>
              <span class="turn-avatar"><UIcon name="i-tabler-sparkles" aria-hidden="true" /></span
              ><strong>{{ modelName(turn.params.model) }}</strong
              ><span class="turn-state" :data-status="turn.job.status">{{ statusLabel(turn.job.status) }}</span
              ><button class="studio-icon-button" aria-label="查看任务详情" @click="details = turn.job">
                <UIcon name="i-tabler-dots" />
              </button>
            </header>
            <div
              v-if="turn.job.result?.assets?.length"
              class="turn-assets"
              :class="{ multiple: turn.job.result.assets.length > 1 }"
            >
              <figure
                v-for="(asset, index) in turn.job.result.assets"
                :key="asset.path"
                :class="{ 'asset-selected': assetSelected(asset) }"
              >
                <button
                  class="asset-preview"
                  :aria-label="'打开第 ' + (index + 1) + ' 个作品原文件'"
                  @click="turn.params.kind === 'image' ? (originalAsset = asset) : openFile(asset.path)"
                >
                  <img
                    v-if="thumb(turn, asset, index)"
                    :src="mediaSource(thumb(turn, asset, index)!)"
                    :alt="turn.prompt + ' · 预览图'"
                    loading="lazy"
                    decoding="async"
                    width="720"
                    height="480"
                  />
                  <span v-else class="asset-placeholder"
                    ><UIcon
                      :name="turn.params.kind === 'image' ? 'i-tabler-photo' : 'i-tabler-video'"
                      aria-hidden="true"
                    />{{ turn.params.kind === 'image' ? '打开原图' : '打开视频' }}</span
                  >
                  <span v-if="turn.params.kind === 'video'" class="video-play"
                    ><UIcon name="i-tabler-player-play" aria-hidden="true"
                  /></span>
                </button>
                <figcaption>
                  <span
                    >{{ asset.width }} × {{ asset.height
                    }}{{ asset.duration ? ' · ' + asset.duration.toFixed(1) + ' 秒' : '' }}</span
                  ><span v-if="assetSelected(asset)" class="selected-label">下轮参考图</span>
                </figcaption>
                <p v-if="ratioMismatch(turn, asset)" class="ratio-warning">实际比例与所选不同，已保留原图</p>
                <div class="asset-actions">
                  <button
                    v-if="turn.params.kind === 'image' && creation.project?.state !== 'legacy'"
                    @click="useAsReference(asset)"
                  >
                    <UIcon name="i-tabler-photo-edit" aria-hidden="true" />继续改图</button
                  ><button @click="openFile(asset.path)">
                    <UIcon name="i-tabler-download" aria-hidden="true" />打开文件</button
                  ><button @click="editor.sendToEditor([asset.path])">
                    <UIcon name="i-tabler-movie" aria-hidden="true" />加入剪辑
                  </button>
                </div>
              </figure>
            </div>
            <p v-if="isRunning(turn.job)" class="generation-progress" role="status">
              <span class="generation-pulse"></span>{{ turn.job.stage
              }}<button :disabled="isStopping(turn.job)" @click="stopJob(turn.job)">
                {{ isStopping(turn.job) ? '正在停止…' : '停止生成' }}
              </button>
            </p>
            <p v-else-if="turn.job.error" class="turn-error">{{ turn.job.error.message }}</p>
            <button
              v-if="turn.job.canRetryDownload"
              class="studio-link"
              :disabled="creation.locked"
              @click="retryDownload(turn.job)"
            >
              只重试下载结果
            </button>
          </div>
        </article>
        <div
          v-if="turnWindow.end < creation.sessionTurns.length"
          aria-hidden="true"
          :style="{ height: turnPositions[creation.sessionTurns.length] - turnPositions[turnWindow.end] + 'px' }"
        ></div>
      </div>
      <button v-if="newResults" class="new-results" @click="bottom">
        <UIcon name="i-tabler-arrow-down" aria-hidden="true" />有新结果
      </button>
      <form class="conversation-composer" @submit.prevent="submit" @keydown.ctrl.enter.prevent="submit">
        <div v-if="automaticReference" class="composer-reference">
          <img v-if="automaticReference.thumbnail" :src="mediaSource(automaticReference.thumbnail)" alt="" /><UIcon
            v-else
            name="i-tabler-photo"
            aria-hidden="true"
          /><span>继续改图<small>已引用当前会话选中的作品</small></span
          ><button type="button" aria-label="移除自动参考图，改为独立生成" @click="draft.independent = true">×</button>
        </div>
        <div v-if="draft.references.length" class="composer-reference-files">
          <span v-for="path in draft.references" :key="path"
            >{{ path.split(/[\\/]/).pop()
            }}<button
              type="button"
              :aria-label="'移除参考图 ' + path"
              @click="draft.references = draft.references.filter((item) => item !== path)"
            >
              ×
            </button></span
          >
        </div>
        <label class="sr-only" for="ai-prompt">描述你想要的画面</label
        ><textarea
          id="ai-prompt"
          v-model="draft.prompt"
          :disabled="!creation.session || creation.project?.state !== 'ready'"
          placeholder="描述画面，或者告诉我这张图要怎么改…"
          rows="2"
        />
        <div v-if="notice" class="composer-message" role="status">{{ notice }}</div>
        <footer>
          <div>
            <button type="button" aria-label="添加参考图" :disabled="!creation.session" @click="chooseReferences">
              <UIcon name="i-tabler-paperclip" /></button
            ><button
              type="button"
              class="reference-library-trigger"
              aria-label="打开角色与场景参考库"
              @click="referencesOpen = true"
            >
              <UIcon name="i-tabler-library" /></button
            ><select v-model="draft.independent" aria-label="生成方式">
              <option :value="false">连续改图</option>
              <option :value="true">独立生成</option>
            </select>
          </div>
          <div>
            <span v-if="creation.locked" class="studio-muted">{{
              creation.pending[creation.activeSessionId] ? '提交中…' : '当前会话生成中'
            }}</span
            ><button
              v-if="creation.activeJob"
              class="send-generation stopping"
              type="button"
              :disabled="isStopping(creation.activeJob)"
              :aria-label="isStopping(creation.activeJob) ? '正在停止' : '停止当前生成'"
              @click="stopJob(creation.activeJob)"
            >
              <UIcon name="i-tabler-player-stop" /></button
            ><button v-else class="send-generation" type="submit" :disabled="!canGenerate" aria-label="发送生成请求">
              <UIcon name="i-tabler-arrow-up" />
            </button>
          </div>
        </footer>
      </form>
      <p class="composer-disclaimer">
        {{
          creation.project?.state === 'legacy'
            ? '旧作品只读保留；新生成请先选择已连接的项目。'
            : '结果自动保存到本机 · 取消后服务端可能仍在生成和计费'
        }}
      </p>
    </main>

    <aside class="creation-parameters" aria-label="生成参数">
      <header class="studio-sidebar-heading">
        <span><UIcon name="i-tabler-adjustments" aria-hidden="true" />生成参数</span
        ><button class="studio-drawer-close" aria-label="关闭参数栏" @click="panels = null">×</button>
      </header>
      <div class="parameter-content">
        <section>
          <label>创作类型</label>
          <div class="creation-kind">
            <button type="button" :aria-pressed="creation.kind === 'image'" @click="creation.kind = 'image'">
              <UIcon name="i-tabler-photo" aria-hidden="true" />图片</button
            ><button type="button" :aria-pressed="creation.kind === 'video'" @click="creation.kind = 'video'">
              <UIcon name="i-tabler-video" aria-hidden="true" />视频
            </button>
          </div>
        </section>
        <section class="model-section">
          <label for="ai-model-search">模型</label>
          <div class="parameter-search">
            <UIcon name="i-tabler-search" aria-hidden="true" /><input
              id="ai-model-search"
              v-model="modelSearch"
              placeholder="搜索模型"
            />
          </div>
          <div class="model-options">
            <button
              v-for="model in models"
              :key="model.id"
              type="button"
              :aria-pressed="draft.model === model.id"
              @click="
                draft.model = model.id;
                draft.modelSource = 'catalog';
              "
            >
              <span
                ><strong>{{ modelName(model.id) }}</strong
                ><small>{{ model.description || model.id }}</small></span
              ><UIcon v-if="draft.model === model.id" name="i-tabler-check" aria-hidden="true" />
            </button>
          </div>
          <p v-if="!models.length" class="studio-muted">还没有可用模型，请检查连接。</p>
          <button class="studio-link" @click="creation.loadCatalog(true)">刷新模型与能力</button>
          <details class="custom-model">
            <summary>自定义 GFlow 模型</summary>
            <label for="ai-model">模型 ID</label
            ><input id="ai-model" v-model="draft.model" placeholder="gflow-…" @input="draft.modelSource = 'custom'" />
            <p class="studio-muted">自定义 · 能力未确认</p>
          </details>
        </section>
        <section>
          <label for="ai-aspect">画面尺寸 / 比例</label
          ><select id="ai-aspect" v-model="draft.aspect">
            <option value="auto">模型默认</option>
            <option v-for="ratio in aspectOptions" :key="ratio" :value="ratio">{{ ratio }}</option>
          </select>
          <div class="aspect-preview" aria-hidden="true">
            <i :style="{ aspectRatio: draft.aspect === 'auto' ? '16 / 9' : draft.aspect.replace(':', ' / ') }"></i>
          </div>
        </section>
        <template v-if="creation.kind === 'image'"
          ><section>
            <label for="ai-image-size">画质 / 分辨率</label
            ><select id="ai-image-size" v-model="draft.imageSize">
              <option value="auto">默认画质</option>
              <option v-for="size in sizeOptions" :key="size" :value="size">{{ size }}</option>
            </select>
            <p class="studio-muted">2K / 4K 可能包含放大处理，实际尺寸以结果为准。</p>
          </section>
          <section>
            <label for="ai-count">生成数量</label
            ><select id="ai-count" v-model="draft.count">
              <option v-for="number in [1, 2, 3, 4]" :key="number" :value="String(number)">{{ number }} 张</option>
            </select>
            <p class="studio-muted">多张顺序生成，停止后保留已完成作品。</p>
          </section></template
        >
        <section v-else>
          <label for="ai-duration">视频时长</label
          ><select id="ai-duration" v-model="draft.duration">
            <option v-for="seconds in durationOptions" :key="seconds" :value="String(seconds)">{{ seconds }} 秒</option>
          </select>
        </section>
        <section>
          <label class="stream-label"><input v-model="draft.stream" type="checkbox" />接收生成进度</label>
          <p class="studio-muted">右侧设置只影响下一轮，已提交任务保持原参数。</p>
        </section>
      </div>
      <footer class="parameter-footer">
        <UiButton variant="ghost" @click="ui.setTab('settings')"
          ><UIcon name="i-tabler-plug-connected" aria-hidden="true" />连接设置</UiButton
        >
      </footer>
    </aside>
    <UiDialog
      v-model="projectDialog"
      title="新建创作项目"
      description="创建真实 Google Flow 项目，并固定所属账号。"
      size="compact"
      ><form class="project-create-form" @submit.prevent="newProject">
        <label for="new-project-name">项目名称</label
        ><input
          id="new-project-name"
          v-model="projectName"
          maxlength="80"
          placeholder="例如：玻璃森林里的故事"
          required
          :disabled="creatingProject"
        />
        <p class="studio-muted">
          系统自动选择可用账号。项目内的所有聊天会使用同一个 Flow 项目，创建项目不会发起图片生成。
        </p>
        <p v-if="!creation.catalog.projectBinding" class="studio-notice">
          正在检查项目连接能力；如果未就绪，请先更新 GFlow 插件并检查连接。
        </p>
        <p v-if="projectError" class="turn-error" role="alert">{{ projectError }}</p>
        <UiButton type="submit" :disabled="creatingProject || !projectName.trim()">{{
          creatingProject ? '正在创建 Flow 项目…' : '创建项目'
        }}</UiButton>
      </form></UiDialog
    >
    <UiDialog
      :model-value="!!originalAsset"
      title="作品原图"
      size="wide"
      @update:model-value="!$event && (originalAsset = null)"
      ><img v-if="originalAsset" class="original-image" :src="mediaSource(originalAsset.path)" alt="生成作品原图" />
      <p v-if="originalAsset" class="studio-muted">{{ originalAsset.width }} × {{ originalAsset.height }}</p>
      <UiButton v-if="originalAsset" variant="secondary" @click="openFile(originalAsset.path)"
        >打开本地原文件</UiButton
      ></UiDialog
    >
    <UiDialog :model-value="!!details" title="作品与任务详情" @update:model-value="!$event && (details = null)"
      ><template v-if="details"
        ><p>{{ statusLabel(details.status) }} · {{ modelName(details.params.model) }}</p>
        <p class="generation-prompt">{{ details.params.prompt }}</p>
        <p class="turn-error">{{ details.error?.message || details.stage }}</p>
        <p class="studio-muted">
          项目 {{ details.params.creationProjectId || '历史未归类' }} · 会话 {{ details.params.sessionId || '历史' }}
        </p>
        <UiButton
          v-if="details.canRetryDownload"
          variant="secondary"
          :disabled="creation.locked"
          @click="retryDownload(details)"
          >只重试下载结果</UiButton
        ></template
      ></UiDialog
    >
    <UiDialog v-model="referencesOpen" title="角色与场景参考库"
      ><div class="reference-library">
        <label for="reference-name">参考名称</label
        ><input id="reference-name" v-model="referenceName" placeholder="例如：主角正面" /><label for="reference-kind"
          >用途</label
        ><select id="reference-kind" v-model="referenceKind">
          <option value="character">角色</option>
          <option value="scene">场景</option></select
        ><UiButton
          variant="secondary"
          :disabled="!effectiveReferences.length || !referenceName.trim()"
          @click="saveReference"
          >保存当前参考图与提示词</UiButton
        ><button
          v-for="reference in creation.catalog.references"
          :key="reference.id"
          class="saved-reference"
          @click="addSavedReference(reference.path, reference.prompt)"
        >
          {{ reference.name }} · {{ reference.kind === 'scene' ? '场景' : '角色' }}
        </button>
        <p v-if="notice" class="turn-error">{{ notice }}</p>
      </div></UiDialog
    >
  </section>
</template>
<style src="./CutflowPages.css"></style>
<style src="./CreationStudio.css"></style>
