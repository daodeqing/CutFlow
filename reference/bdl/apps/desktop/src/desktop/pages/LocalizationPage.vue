<script setup lang="ts">
import { computed, onActivated, onDeactivated, onMounted, onUnmounted, ref, watch } from 'vue';
import UiButton from '../../shared/ui/Button.vue';
import LocalizationModelInput from './LocalizationModelInput.vue';
import { cutflow, errorMessage, mediaSource, operationId, type AiModel, type CreationCatalog, type Job } from '../../shared/api/cutflow';
import type { DownloadTask, NormalizedSourceTree } from '../../shared/api/dto';
import { useQueueStore } from '../../shared/stores/queue';
import { useUiStore } from '../../shared/stores/ui';

type LocalizationResult = { videoPath: string; folderPath: string; subtitlePath: string; title: string; videoDescription?: string; postText?: string; sourcePath?: string; sourceTaskId?: string; sourceUrl?: string | null; warnings: string[]; reviewRequired: boolean };
type LocalizationJob = Omit<Job, 'result'> & { result?: LocalizationResult | null };
type Watch = { id: string; url: string; title: string; initialized: boolean; pending: unknown[]; issue: string };
type QualityOption = { value: string; label: string };
const ui = useUiStore();
const queue = useQueueStore();
const source = ref<'url' | 'task'>('url');
const url = ref('');
const taskId = ref('');
const taskPicker = ref<HTMLDetailsElement | null>(null);
const rights = ref(false);
const watchUrl = ref('');
const watchRights = ref(false);
const watches = ref<Watch[]>([]);
const watchBusy = ref(false);
const quality = ref('best');
const qualityOptions = ref<QualityOption[]>([{ value: 'best', label: '账号可用的最高画质（自动）' }]);
const qualityState = ref<'idle' | 'loading' | 'ready' | 'error'>('idle');
const qualityMessage = ref('填写 B 站链接后解析真实可用画质。');
const model = ref('gemini-3.8-flash-high');
const subtitleModel = ref('');
const danmakuModel = ref('');
const speechModel = ref('');
const postModel = ref('');
const trendHint = ref('');
const postBusy = ref(false);
const transcription = ref<'remote' | 'whisper'>('remote');
const whisperModel = ref('base');
const removeBottomText = ref(true);
const models = ref<AiModel[]>([]);
const jobs = ref<LocalizationJob[]>([]);
const youtubeJobs = ref<Job[]>([]);
const selectedJob = ref('');
const title = ref('');
const post = ref('');
const videoDescription = ref('');
const instruction = ref('');
const reviewed = ref(false);
const busy = ref(false);
const youtubePending = ref(false);
const message = ref('');
let timer: ReturnType<typeof setInterval> | undefined;
let qualityTimer: ReturnType<typeof setTimeout> | undefined;
let qualityRequest = 0;
let refreshing = false;
let refreshingWatch = false;
let initializedPost = '';
const finishedTasks = computed(() => queue.tasks.filter((task) => task.status === 'completed'));
const job = computed(() => jobs.value.find((item) => item.id === selectedJob.value));
const active = computed(() => jobs.value.some((item) => item.status === 'running' || item.status === 'queued'));
const canStart = computed(() => rights.value && !!model.value.trim() && !active.value && !busy.value && (source.value === 'url' ? !!url.value.trim() : !!taskId.value));
const result = computed(() => job.value?.status === 'completed' ? job.value.result : null);
const youtubeChannelUrl = 'https://www.youtube.com/@deqingdao/videos';
const titleError = computed(() => {
  const value = title.value.trim();
  if (!value) return '请输入英文视频标题。';
  if (value.length > 100) return '视频标题不能超过 100 个字符。';
  if (/[\u3400-\u4dbf\u4e00-\u9fff\uf900-\ufaff\u3040-\u30ff]/u.test(value)) return '视频标题必须使用英文。';
  return '';
});
const selectedTask = computed(() => finishedTasks.value.find((task) => task.id === taskId.value));
const selectedTaskProduced = computed(() => source.value === 'task' && !!selectedTask.value && isTaskProduced(selectedTask.value));
const selectedUrlProduced = computed(() => source.value === 'url' && !!url.value.trim() && jobs.value.some((item) =>
  item.status === 'completed' && sameUrl(String(item.params.url || item.result?.sourceUrl || ''), url.value)));
const youtubeReady = computed(() => !!result.value?.videoPath && !titleError.value && !!videoDescription.value.trim());
const youtubeJob = computed(() => youtubeJobs.value.find((item) => item.params.videoPath === result.value?.videoPath));
const youtubeBusy = computed(() => youtubePending.value || youtubeJobs.value.some((item) => ['queued', 'running'].includes(item.status)));

function sameUrl(left: string, right: string) {
  return left.trim().replace(/\/$/, '').toLowerCase() === right.trim().replace(/\/$/, '').toLowerCase();
}
function normalizedPath(path?: string) {
  return (path || '').replace(/^\\\\\?\\/, '').replace(/\\/g, '/').toLowerCase();
}
function completedTaskJob(task: Pick<DownloadTask, 'id' | 'output_path'>) {
  return jobs.value.find((item) => item.status === 'completed' && !!item.result?.videoPath && (
    item.params.taskId === task.id || item.result.sourceTaskId === task.id
    || (!!task.output_path && normalizedPath(item.result.sourcePath) === normalizedPath(task.output_path))
  ));
}
function isTaskProduced(task: Pick<DownloadTask, 'id' | 'output_path'>) {
  return !!completedTaskJob(task);
}
function selectTask(task: DownloadTask) {
  taskId.value = task.id;
  if (taskPicker.value) taskPicker.value.open = false;
  const completed = completedTaskJob(task);
  if (completed) selectedJob.value = completed.id;
}
function qualityLabel(value: number) {
  return ({ 127: '8K · 127', 126: '杜比视界 · 126', 125: 'HDR · 125', 120: '4K · 120', 116: '1080p60 · 116', 112: '1080p+ · 112', 80: '1080p · 80', 74: '720p60 · 74', 64: '720p · 64', 32: '480p · 32', 16: '360p · 16' } as Record<number, string>)[value] || `B站画质 ${value}`;
}
async function probeQuality() {
  if (qualityTimer) clearTimeout(qualityTimer);
  qualityTimer = undefined;
  const input = url.value.trim();
  const request = ++qualityRequest;
  if (!input) {
    qualityState.value = 'idle'; qualityMessage.value = '填写 B 站链接后解析真实可用画质。';
    qualityOptions.value = [{ value: 'best', label: '账号可用的最高画质（自动）' }]; quality.value = 'best'; return;
  }
  qualityState.value = 'loading'; qualityMessage.value = '正在查询该链接和当前账号的真实可用画质…';
  try {
    const tree = await cutflow<NormalizedSourceTree>('cutflow.bilibili.parse', { url: input });
    if (request !== qualityRequest) return;
    const firstPart = tree.groups.flatMap((group) => group.items).flatMap((item) => item.parts)[0];
    const values = [...new Set((firstPart?.streams || [])
      .filter((stream) => stream.kind === 'video' && stream.urls.length > 0 && stream.quality.kind === 'quality')
      .map((stream) => stream.quality.kind === 'quality' ? stream.quality.value : 0).filter((value) => value > 0))].sort((left, right) => right - left);
    qualityOptions.value = [{ value: 'best', label: values.length ? `最高可用画质（${qualityLabel(values[0])}）` : '账号可用的最高画质（自动）' }, ...values.map((value) => ({ value: String(value), label: qualityLabel(value) }))];
    if (!qualityOptions.value.some((option) => option.value === quality.value)) quality.value = 'best';
    qualityState.value = values.length ? 'ready' : 'error';
    qualityMessage.value = values.length ? `已确认 ${values.length} 档真实可用视频画质；最高为 ${qualityLabel(values[0])}。` : '解析结果没有返回可选视频画质，将由下载器按账号实际能力选择最高画质。';
  } catch (error) {
    if (request !== qualityRequest) return;
    qualityState.value = 'error'; qualityMessage.value = `暂时无法查询画质：${errorMessage(error)}。仍可使用自动最高画质。`;
    qualityOptions.value = [{ value: 'best', label: '账号可用的最高画质（自动）' }]; quality.value = 'best';
  }
}
function scheduleQualityProbe() {
  if (qualityTimer) clearTimeout(qualityTimer);
  qualityRequest += 1;
  quality.value = 'best';
  qualityState.value = 'idle';
  qualityOptions.value = [{ value: 'best', label: '账号可用的最高画质（自动）' }];
  qualityTimer = setTimeout(() => { void probeQuality(); }, 450);
}

async function refresh() {
  if (refreshing) return;
  refreshing = true;
  try {
    const response = await cutflow<{ jobs: LocalizationJob[] }>('cutflow.jobs.list');
    jobs.value = response.jobs.filter((item) => item.method === 'cutflow.localize.run').reverse();
    youtubeJobs.value = response.jobs.filter((item) => item.method === 'cutflow.youtube.upload').reverse() as unknown as Job[];
    if (!selectedJob.value && jobs.value.length) selectedJob.value = jobs.value[0].id;
  } catch (error) { message.value = errorMessage(error); }
  finally { refreshing = false; }
}
async function refreshWatch() {
  if (refreshingWatch) return;
  refreshingWatch = true;
  try { watches.value = (await cutflow<{ watches: Watch[] }>('cutflow.localize.watch.list')).watches; }
  catch (error) { message.value = errorMessage(error); }
  finally { refreshingWatch = false; }
}
async function scanWatches() {
  try { await cutflow('cutflow.localize.watch.scan'); message.value = '正在检查监控来源；首次只记录已有视频。'; }
  catch (error) { message.value = errorMessage(error); }
}
async function addWatch() {
  if (!watchUrl.value.trim() || !watchRights.value || !model.value.trim()) return;
  watchBusy.value = true;
  try {
    await cutflow('cutflow.localize.watch.add', { url: watchUrl.value.trim(), model: model.value.trim(), rights: '用户确认享有此来源新增视频的制作及发布权限',
      quality: quality.value, subtitleModel: subtitleModel.value.trim(), danmakuModel: danmakuModel.value.trim(),
      speechModel: speechModel.value.trim(), postModel: postModel.value.trim(), trendHint: trendHint.value.trim(), transcription: transcription.value,
      whisperModel: whisperModel.value, removeBottomText: removeBottomText.value });
    watchUrl.value = ''; watchRights.value = false;
    await refreshWatch(); await scanWatches();
  } catch (error) { message.value = errorMessage(error); }
  finally { watchBusy.value = false; }
}
async function removeWatch(watch: Watch) {
  if (!window.confirm('停止此来源监控？尚未开始的待处理视频会移出列表，已创建任务不受影响。')) return;
  try { await cutflow('cutflow.localize.watch.remove', { watchId: watch.id }); await refreshWatch(); }
  catch (error) { message.value = errorMessage(error); }
}
async function load() {
  await Promise.all([
    queue.list(),
    cutflow<CreationCatalog>('cutflow.creation.catalog').then((catalog) => { models.value = catalog.models.filter((item) => item.kind === 'text' || item.kind === 'unknown'); }).catch((error) => { message.value = errorMessage(error); }),
    refresh(), refreshWatch(),
  ]);
}
function adoptTask(id: string) {
  if (!id) return;
  source.value = 'task'; taskId.value = id; ui.localizeTaskId = '';
}
watch(() => ui.localizeTaskId, adoptTask);
watch(url, scheduleQualityProbe);
function hydrateResult(value: LocalizationResult | null | undefined) {
  if (!value || initializedPost === selectedJob.value) return;
  initializedPost = selectedJob.value;
  title.value = value.title || 'Video';
  post.value = [value.postText || value.title, value.sourceUrl].filter(Boolean).join('\n\n');
  videoDescription.value = value.videoDescription || '';
  reviewed.value = false;
}
watch(result, hydrateResult, { immediate: true });
watch(selectedJob, () => { reviewed.value = false; hydrateResult(result.value); });
watch(title, () => { reviewed.value = false; });
watch(post, () => { reviewed.value = false; });
watch(videoDescription, () => { reviewed.value = false; });
async function start() {
  if (!canStart.value) return;
  busy.value = true; message.value = '';
  try {
    if (source.value === 'url' && qualityState.value !== 'ready') await probeQuality();
    const response = await cutflow<{ jobId: string }>('cutflow.localize.start', {
      operationId: operationId(), model: model.value.trim(), rights: '用户确认享有制作及发布权限',
      ...(source.value === 'url' ? { url: url.value.trim() } : { taskId: taskId.value }),
      subtitleModel: subtitleModel.value.trim(), danmakuModel: danmakuModel.value.trim(),
      speechModel: speechModel.value.trim(), postModel: postModel.value.trim(), trendHint: trendHint.value.trim(),
      transcription: transcription.value, whisperModel: whisperModel.value, removeBottomText: removeBottomText.value, quality: quality.value,
    });
    selectedJob.value = response.jobId; initializedPost = '';
    await refresh();
  } catch (error) { message.value = errorMessage(error); }
  finally { busy.value = false; }
}
async function cancel() {
  if (!job.value) return;
  try { await cutflow('cutflow.jobs.cancel', { jobId: job.value.id }); message.value = '已请求停止，已保存素材会保留。'; await refresh(); }
  catch (error) { message.value = errorMessage(error); }
}
async function openFolder() {
  if (!result.value) return;
  try { await cutflow('cutflow.files.open', { path: result.value.folderPath }); }
  catch (error) { message.value = errorMessage(error); }
}
async function copy(value: string) {
  try { await navigator.clipboard.writeText(value); message.value = '已复制。'; }
  catch { message.value = '无法自动复制，请选中文字手动复制。'; }
}
async function regeneratePost() {
  if (!job.value || !result.value || postBusy.value) return;
  const id = job.value.id;
  const previous = post.value;
  const previousDescription = videoDescription.value;
  postBusy.value = true; message.value = '';
  try {
    const previousTitle = title.value;
    const generated = await cutflow<{ title: string; postText: string; videoDescription: string }>('cutflow.localize.post', {
      jobId: id, model: postModel.value.trim() || model.value.trim(), trendHint: trendHint.value.trim(), instruction: instruction.value.trim(),
    });
    if (selectedJob.value === id && post.value === previous && videoDescription.value === previousDescription && title.value === previousTitle) {
      title.value = generated.title || previousTitle;
      post.value = [generated.postText, result.value?.sourceUrl].filter(Boolean).join('\n\n');
      videoDescription.value = generated.videoDescription;
      message.value = '英文标题、简介和推文已重新生成，请审核后使用。';
    } else { message.value = '标题、简介或推文已变化，本次生成结果没有覆盖你的编辑。'; }
  } catch (error) { message.value = errorMessage(error); }
  finally { postBusy.value = false; }
}
async function openBrowser() {
  if (!reviewed.value) return;
  try { await cutflow('cutflow.x.browser', { text: post.value }); }
  catch (error) { message.value = errorMessage(error); }
}
function cindyUploadPrompt() {
  if (!result.value) return '';
  return '请使用 Cindy 连接的浏览器打开 X，确认是我的账号，将下面的视频上传并填入文案，保留给我审核，不要点击发布。\n\n视频路径：' + result.value.videoPath + '\n\n文案：\n' + post.value;
}
async function openX() {
  if (!reviewed.value) return;
  try { await cutflow('cutflow.x.compose'); }
  catch (error) { message.value = errorMessage(error); }
}
async function uploadYouTube() {
  if (!reviewed.value || !result.value || !youtubeReady.value || youtubeBusy.value) return;
  youtubePending.value = true; message.value = '';
  try {
    const metadata = `Title:\n${title.value.trim()}\n\nDescription:\n${videoDescription.value.trim()}`;
    let copied = true;
    try { await navigator.clipboard.writeText(metadata); } catch { copied = false; }
    const started = await cutflow<{ jobId?: string }>('cutflow.youtube.upload', {
      operationId: operationId(),
      videoPath: result.value.videoPath,
      title: title.value.trim(),
      description: videoDescription.value.trim(),
      channelUrl: youtubeChannelUrl,
    });
    if (!started.jobId) throw new Error('YouTube 上传准备没有返回任务编号，请检查当前 Chrome。');
    youtubeJobs.value.unshift({ id: started.jobId, operationId: '', method: 'cutflow.youtube.upload', params: { videoPath: result.value.videoPath }, status: 'running', stage: '等待 Chrome 扩展带入成片、标题和简介' });
    message.value = copied ? '已打开当前 Chrome；扩展将自动带入成片、英文标题和简介，请在 YouTube Studio 完成审核。' : '已打开当前 Chrome；扩展将自动带入信息，剪贴板备份失败时可手动复制。';
    await refresh();
  } catch (error) { message.value = errorMessage(error); }
  finally { youtubePending.value = false; }
}
function stopPolling() { if (timer) clearInterval(timer); timer = undefined; }
onMounted(() => { adoptTask(ui.localizeTaskId); void load(); if (url.value.trim()) void probeQuality(); });
onActivated(() => { stopPolling(); void refresh(); void refreshWatch(); timer = setInterval(() => { void refresh(); void refreshWatch(); }, 3000); });
onDeactivated(stopPolling); onUnmounted(() => { stopPolling(); if (qualityTimer) clearTimeout(qualityTimer); qualityRequest += 1; });
</script>

<template>
  <section class="localize-page">
    <div class="localize-heading"><div><h1>英文制作</h1><p>下载或选择已完成的视频，制作英文字幕和精选弹幕，审核后发布。</p></div></div>
    <div class="localize-layout">
      <form class="panel localize-panel" @submit.prevent="start">
        <h2>1. 选择视频</h2>
        <div class="source-choice">
          <label><input v-model="source" type="radio" value="url" /> B 站链接</label>
          <label><input v-model="source" type="radio" value="task" /> 已完成下载</label>
        </div>
        <label v-if="source === 'url'" class="field">视频链接<input v-model="url" type="url" placeholder="https://www.bilibili.com/video/…" /></label>
        <div v-if="source === 'url'" class="quality-row">
          <label class="field">下载画质<select v-model="quality" :disabled="qualityState === 'loading'"><option v-for="option in qualityOptions" :key="option.value" :value="option.value">{{ option.label }}</option></select></label>
          <UiButton type="button" variant="secondary" :disabled="!url.trim() || qualityState === 'loading'" @click="probeQuality">{{ qualityState === 'loading' ? '查询中…' : '读取可用画质' }}</UiButton>
        </div>
        <div v-else class="field">
          <span id="localize-download-label">下载任务</span>
          <details ref="taskPicker" class="task-picker">
            <summary aria-labelledby="localize-download-label" :class="{ produced: selectedTaskProduced }">
              <span>{{ selectedTask?.title || '请选择已完成的下载' }}</span>
              <span v-if="selectedTaskProduced" class="produced-badge">已制作</span>
              <UIcon name="i-tabler-chevron-down" aria-hidden="true" />
            </summary>
            <div class="task-options" aria-label="已完成下载">
              <button v-for="task in finishedTasks" :key="task.id" type="button" :aria-pressed="taskId === task.id" :class="{ produced: isTaskProduced(task) }" @click="selectTask(task)">
                <span>{{ task.title }}</span><span v-if="isTaskProduced(task)" class="produced-badge">已制作</span>
              </button>
              <p v-if="!finishedTasks.length" class="hint">暂无已完成下载，请先下载视频。</p>
            </div>
          </details>
        </div>
        <p v-if="source === 'task' && selectedTaskProduced" class="produced-state" role="status">该下载已完成英文制作；已打开对应成片，重新开始会创建新任务。</p>
        <p v-if="source === 'url'" class="hint quality-message" :data-state="qualityState">{{ qualityMessage }}<span v-if="selectedUrlProduced" class="produced-inline"> · ✓ 已制作</span></p>
        <p class="hint">链接默认处理第一个分 P。明确选择的画质不可用时会提示失败；自动最高按原视频和账号权限选择，不会放大画面。已有下载会复用其可用的独立字幕和弹幕。</p>
        <label class="check-line"><input v-model="rights" type="checkbox" /> 我有权下载、翻译并发布所选视频及其中素材。</label>
        <h2>2. 模型与画面</h2>
        <LocalizationModelInput v-model="model" :models="models" label="默认 AI 模型" />
        <details><summary>各步骤分别选择模型</summary><div class="model-fields">
          <LocalizationModelInput v-model="subtitleModel" :models="models" label="字幕翻译" placeholder="留空使用默认模型" />
          <LocalizationModelInput v-model="danmakuModel" :models="models" label="弹幕翻译" placeholder="留空使用默认模型" />
          <LocalizationModelInput v-model="speechModel" :models="models" label="语音识别" placeholder="留空使用默认模型" />
          <LocalizationModelInput v-model="postModel" :models="models" label="英文文案" placeholder="留空使用默认模型" />
        </div></details>
        <label class="field">参考的 X 话题（选填）<input v-model="trendHint" maxlength="300" placeholder="粘贴与视频相关的热点或关键词" /></label>
        <p class="hint">文案基于视频内容生成；未自动获取 X 实时热榜，不会把猜测当作热点。</p>
        <label class="field">没有独立字幕时<select v-model="transcription"><option value="remote">通过所选 AI 服务识别音频</option><option value="whisper">本地 Whisper 识别</option></select></label>
        <label v-if="transcription === 'whisper'" class="field">Whisper 模型<select v-model="whisperModel"><option value="tiny">tiny · 占用较小</option><option value="base">base · 约 141 MiB</option><option value="small">small · 约 465 MiB</option></select></label>
        <p class="hint">AI 服务是否支持音频取决于实际代理；它也可能将请求转发到云端。本地 Whisper 首次使用可能下载模型。</p>
        <label class="check-line"><input v-model="removeBottomText" type="checkbox" /> 用深色字幕条遮挡底部原字幕</label>
        <p class="hint">字幕条会遮住画面底部的一小部分，不会拉伸画面；其他位置的烧录文字可能保留。取消勾选可完整保留画面。</p>
        <UiButton type="submit" :disabled="!canStart">{{ busy ? '正在创建…' : '开始制作' }}</UiButton>
        <details class="watch-setup"><summary>自动监控收藏夹 / UP 主</summary>
          <div class="model-fields">
            <p class="hint">每小时检查新增视频，使用当前模型和画质设置自动制作。首次只记录已有视频，不回填历史内容。</p>
            <label class="field">收藏夹或 UP 主空间链接<input v-model="watchUrl" type="url" placeholder="https://space.bilibili.com/…" /></label>
            <label class="check-line"><input v-model="watchRights" type="checkbox" /> 我有权制作并发布此来源以后新增的视频。</label>
            <UiButton variant="secondary" :disabled="watchBusy || !watchUrl.trim() || !watchRights || !model.trim()" @click="addWatch">添加监控</UiButton>
            <div v-for="watchItem in watches" :key="watchItem.id" class="watch-item">
              <strong>{{ watchItem.title || watchItem.url }}</strong>
              <p class="hint">{{ watchItem.initialized ? '监控中' : '等待建立基线' }} · 待处理 {{ watchItem.pending.length }}</p>
              <p v-if="watchItem.issue" class="notice">{{ watchItem.issue }}</p>
              <UiButton size="compact" variant="secondary" @click="removeWatch(watchItem)">停止监控</UiButton>
            </div>
            <UiButton v-if="watches.length" variant="secondary" @click="scanWatches">立即检查</UiButton>
            <p class="hint">CutFlow 需要运行或驻留托盘；完全退出后暂停监控。任务完成后仍需审核并手动发布。</p>
          </div>
        </details>
      </form>
      <section class="panel localize-panel" aria-label="制作进度与发布审核">
        <h2>3. 制作进度</h2>
        <label v-if="jobs.length" class="field">制作记录<select v-model="selectedJob"><option v-for="item in jobs" :key="item.id" :value="item.id">{{ item.stage }} · {{ item.id.slice(0, 8) }}</option></select></label>
        <p v-if="job" role="status" class="status">{{ job.stage }}</p><p v-else class="hint">开始后会在这里显示进度。</p>
        <UiButton v-if="job && (job.status === 'running' || job.status === 'queued')" variant="secondary" @click="cancel">停止制作</UiButton>
        <template v-if="result">
          <h2>4. 审核并发布</h2>
          <video class="preview" :src="mediaSource(result.videoPath)" controls preload="metadata" />
          <p v-for="warning in result.warnings" :key="warning" class="hint">{{ warning }}</p>
          <label class="field">成片路径<input :value="result.videoPath" readonly @focus="($event.target as HTMLInputElement).select()" /></label>
          <div class="actions"><UiButton variant="secondary" @click="openFolder">打开成片文件夹</UiButton><UiButton variant="secondary" @click="copy(result.videoPath)">复制路径</UiButton></div>
          <label class="field">英文视频标题<input v-model="title" maxlength="100" autocomplete="off" placeholder="English title (100 characters maximum)" /><span class="field-meta" :class="{ invalid: titleError }"><span>{{ title.length }}/100</span><span v-if="titleError">{{ titleError }}</span></span></label>
          <UiButton variant="secondary" :disabled="postBusy" @click="copy(title)">复制视频标题</UiButton>
          <label class="field">视频英文简介<textarea v-model="videoDescription" rows="3" /></label>
          <UiButton variant="secondary" @click="copy(videoDescription)">复制视频简介</UiButton>
          <label class="field">X 英文推文<textarea v-model="post" rows="4" /></label>
          <UiButton variant="secondary" @click="copy(post)">复制推文</UiButton>
          <label class="field">给 AI 的修改要求<textarea v-model="instruction" rows="2" maxlength="600" placeholder="例如：写得更有悬念，突出施工现场的整洁" /></label>
          <UiButton variant="secondary" :disabled="postBusy" @click="regeneratePost">{{ postBusy ? '正在生成简介和推文…' : '按要求重新生成简介和推文' }}</UiButton>
          <label class="check-line"><input v-model="reviewed" type="checkbox" /> 我已审核视频、字幕、弹幕和文案，确认发布权限。</label>
          <div class="actions">
            <UiButton :disabled="!reviewed" @click="openBrowser">用浏览器打开 X 并填写文案</UiButton>
            <UiButton variant="secondary" :disabled="!reviewed" @click="copy(cindyUploadPrompt())">复制给 Cindy 的上传指令</UiButton>
            <UiButton variant="secondary" :disabled="!reviewed" @click="openX">打开 CutFlow 内的 X 窗口</UiButton>
            <UiButton variant="secondary" :disabled="!reviewed || !youtubeReady || youtubeBusy" @click="uploadYouTube">{{ youtubeBusy ? '正在打开 YouTube…' : '上传到 YouTube' }}</UiButton>
          </div>
          <p v-if="youtubeJob" class="notice" role="status">{{ youtubeJob.status === 'completed' ? '成片已选择，英文标题和简介已填入；上传处理与最终发布请在 YouTube Studio 核查。' : youtubeJob.error?.message || youtubeJob.stage }}</p>
          <p class="hint">YouTube 目标频道：<a :href="youtubeChannelUrl" target="_blank" rel="noreferrer">{{ youtubeChannelUrl }}</a>。首次使用请在 Chrome 的 <code>chrome://extensions</code> 加载 CutFlow 安装目录中的 <code>youtube-extension</code> 文件夹；按钮会复用当前 Chrome 登录会话，自动选择成片并填入标题、简介，最后仍由你在 Studio 审核并发布。</p>
          <p class="hint">X 浏览器打开后请确认账号并上传视频；也可把指令粘贴给 Cindy，让它在获得浏览器权限后协助上传。最终发布需要你确认，能否上传取决于登录状态、浏览器权限和 X 账号。</p>
        </template>
        <p v-if="message" role="status" class="notice">{{ message }}</p>
      </section>
    </div>
  </section>
</template>

<style scoped>
.localize-page {
  min-height: 0;
  overflow: auto;
  padding: 18px 22px 32px;
}

.localize-heading h1 {
  margin: 0;
  font-size: 20px;
}

.localize-heading p {
  color: var(--color-muted);
  font-size: 13px;
  line-height: 1.6;
}

.localize-layout {
  display: grid;
  grid-template-columns: minmax(0, 1fr) minmax(0, 1fr);
  gap: 16px;
  align-items: start;
}

.localize-panel {
  display: grid;
  min-width: 0;
  gap: 12px;
  padding: 20px;
}

.localize-panel h2 {
  margin: 8px 0 0;
  font-size: 15px;
}

.field {
  display: grid;
  min-width: 0;
  gap: 6px;
  font-size: 13px;
}

.field input,
.field select,
.field textarea {
  box-sizing: border-box;
  width: 100%;
  min-width: 0;
  padding: 9px 10px;
  border: 1px solid var(--color-border);
  border-radius: 8px;
  background: var(--color-surface);
  color: inherit;
  font: inherit;
}

.field textarea {
  resize: vertical;
}

.field-meta {
  display: flex;
  justify-content: space-between;
  gap: 8px;
  color: var(--color-muted);
  font-size: 11px;
  line-height: 1.4;
}

.field-meta.invalid {
  color: var(--color-danger, #b42318);
}

.quality-row {
  display: grid;
  grid-template-columns: minmax(0, 1fr) auto;
  gap: 8px;
  align-items: end;
}

.quality-row > button {
  min-height: 38px;
  white-space: nowrap;
}

.localize-panel summary {
  cursor: pointer;
  font-size: 13px;
}

.task-picker {
  position: relative;
}

.task-picker > summary {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
  min-height: 38px;
  box-sizing: border-box;
  padding: 9px 10px;
  border: 1px solid var(--color-border);
  border-radius: 8px;
  background: var(--color-surface);
  color: inherit;
  list-style: none;
}

.task-picker > summary::-webkit-details-marker {
  display: none;
}

.task-picker > summary > span:first-child {
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.task-picker > summary.produced {
  border-color: color-mix(in srgb, var(--color-accent, #0f8f7a) 55%, var(--color-border));
  background: color-mix(in srgb, var(--color-accent, #0f8f7a) 10%, var(--color-surface));
}

.task-options {
  position: absolute;
  z-index: 2;
  top: calc(100% + 4px);
  right: 0;
  left: 0;
  display: grid;
  max-height: 240px;
  overflow: auto;
  padding: 4px;
  border: 1px solid var(--color-border);
  border-radius: 8px;
  background: var(--color-surface);
  box-shadow: 0 12px 28px color-mix(in srgb, #000 16%, transparent);
}

.task-options button {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
  min-width: 0;
  padding: 9px 10px;
  border: 0;
  border-radius: 6px;
  background: transparent;
  color: inherit;
  font: inherit;
  text-align: left;
}

.task-options button:hover,
.task-options button[aria-pressed='true'] {
  background: var(--color-surface-raised, color-mix(in srgb, var(--color-accent, #0f8f7a) 8%, transparent));
}

.task-options button.produced {
  background: color-mix(in srgb, var(--color-accent, #0f8f7a) 12%, transparent);
  color: var(--color-accent-strong, #087f6b);
  font-weight: 650;
}

.task-options button > span:first-child {
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.produced-badge {
  flex: none;
  color: var(--color-accent-strong, #087f6b);
  font-size: 11px;
  font-weight: 700;
  white-space: nowrap;
}

.quality-message[data-state='ready'] {
  color: var(--color-accent-strong, #087f6b);
}

.quality-message[data-state='error'] {
  color: var(--color-warning-strong, #9a6700);
}

.produced-state {
  margin: 0;
  padding: 9px 11px;
  border: 1px solid color-mix(in srgb, var(--color-accent, #0f8f7a) 45%, var(--color-border));
  border-radius: 8px;
  background: color-mix(in srgb, var(--color-accent, #0f8f7a) 12%, transparent);
  color: var(--color-accent-strong, #087f6b);
  font-size: 12px;
  font-weight: 650;
  line-height: 1.5;
}

.produced-inline {
  color: var(--color-accent-strong, #087f6b);
  font-weight: 650;
}

.actions a,
.hint a {
  color: var(--color-accent-strong, #087f6b);
  text-underline-offset: 2px;
}

.source-choice,
.actions {
  display: flex;
  flex-wrap: wrap;
  gap: 12px;
}

.source-choice label,
.check-line {
  display: flex;
  align-items: flex-start;
  gap: 8px;
  font-size: 13px;
  line-height: 1.5;
}

.source-choice input,
.check-line input {
  flex: none;
  margin-top: 3px;
}

.hint {
  margin: 0;
  color: var(--color-muted);
  font-size: 12px;
  line-height: 1.6;
}

.status,
.notice {
  overflow-wrap: anywhere;
  margin: 0;
  font-size: 13px;
  line-height: 1.6;
}

.model-fields {
  display: grid;
  gap: 10px;
  margin-top: 12px;
}

.watch-setup {
  padding-top: 12px;
  border-top: 1px solid var(--color-border);
}

.watch-item {
  display: grid;
  gap: 8px;
  overflow-wrap: anywhere;
  padding: 10px;
  border: 1px solid var(--color-border);
  border-radius: 8px;
  font-size: 13px;
}

.preview {
  width: 100%;
  max-height: 360px;
  background: #000;
}

@media (width <= 800px) {
  .localize-page {
    padding: 14px 12px 24px;
  }

  .localize-layout {
    grid-template-columns: minmax(0, 1fr);
  }
}

@media (width <= 520px) {
  .quality-row {
    grid-template-columns: minmax(0, 1fr);
  }

  .quality-row > button {
    width: 100%;
  }
}
</style>
