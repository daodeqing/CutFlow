<script setup lang="ts">
import { computed, onActivated, onDeactivated, onMounted, onUnmounted, ref, watch } from 'vue';
import UiButton from '../../shared/ui/Button.vue';
import LocalizationModelInput from './LocalizationModelInput.vue';
import { cutflow, errorMessage, mediaSource, operationId, type AiModel, type CreationCatalog, type Job } from '../../shared/api/cutflow';
import { useQueueStore } from '../../shared/stores/queue';
import { useUiStore } from '../../shared/stores/ui';

type LocalizationResult = { videoPath: string; folderPath: string; subtitlePath: string; title: string; videoDescription?: string; postText?: string; sourceUrl?: string | null; warnings: string[]; reviewRequired: boolean };
type LocalizationJob = Omit<Job, 'result'> & { result?: LocalizationResult | null };
type Watch = { id: string; url: string; title: string; initialized: boolean; pending: unknown[]; issue: string };
const ui = useUiStore();
const queue = useQueueStore();
const source = ref<'url' | 'task'>('url');
const url = ref('');
const taskId = ref('');
const rights = ref(false);
const watchUrl = ref('');
const watchRights = ref(false);
const watches = ref<Watch[]>([]);
const watchBusy = ref(false);
const quality = ref('80');
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
const selectedJob = ref('');
const post = ref('');
const videoDescription = ref('');
const instruction = ref('');
const reviewed = ref(false);
const busy = ref(false);
const message = ref('');
let timer: ReturnType<typeof setInterval> | undefined;
let refreshing = false;
let refreshingWatch = false;
let initializedPost = '';
const finishedTasks = computed(() => queue.tasks.filter((task) => task.status === 'completed'));
const job = computed(() => jobs.value.find((item) => item.id === selectedJob.value));
const active = computed(() => jobs.value.some((item) => item.status === 'running' || item.status === 'queued'));
const canStart = computed(() => rights.value && !!model.value.trim() && !active.value && !busy.value && (source.value === 'url' ? !!url.value.trim() : !!taskId.value));
const result = computed(() => job.value?.status === 'completed' ? job.value.result : null);

async function refresh() {
  if (refreshing) return;
  refreshing = true;
  try {
    const response = await cutflow<{ jobs: LocalizationJob[] }>('cutflow.jobs.list');
    jobs.value = response.jobs.filter((item) => item.method === 'cutflow.localize.run').reverse();
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
watch(result, (value) => {
  if (!value || initializedPost === selectedJob.value) return;
  initializedPost = selectedJob.value;
  post.value = [value.postText || value.title, value.sourceUrl].filter(Boolean).join('\n\n');
  videoDescription.value = value.videoDescription || '';
  reviewed.value = false;
});
watch(selectedJob, () => { reviewed.value = false; });
watch(post, () => { reviewed.value = false; });
watch(videoDescription, () => { reviewed.value = false; });
async function start() {
  if (!canStart.value) return;
  busy.value = true; message.value = '';
  try {
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
    const generated = await cutflow<{ postText: string; videoDescription: string }>('cutflow.localize.post', {
      jobId: id, model: postModel.value.trim() || model.value.trim(), trendHint: trendHint.value.trim(), instruction: instruction.value.trim(),
    });
    if (selectedJob.value === id && post.value === previous && videoDescription.value === previousDescription) {
      post.value = [generated.postText, result.value?.sourceUrl].filter(Boolean).join('\n\n');
      videoDescription.value = generated.videoDescription;
      message.value = '简介和推文已重新生成，请审核后使用。';
    } else { message.value = '简介或推文已变化，本次生成结果没有覆盖你的编辑。'; }
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
function stopPolling() { if (timer) clearInterval(timer); timer = undefined; }
onMounted(() => { adoptTask(ui.localizeTaskId); void load(); });
onActivated(() => { stopPolling(); void refresh(); void refreshWatch(); timer = setInterval(() => { void refresh(); void refreshWatch(); }, 3000); });
onDeactivated(stopPolling); onUnmounted(stopPolling);
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
        <label v-else class="field">下载任务<select v-model="taskId"><option value="">请选择</option><option v-for="task in finishedTasks" :key="task.id" :value="task.id">{{ task.title }}</option></select></label>
        <label v-if="source === 'url'" class="field">下载画质<select v-model="quality"><option value="64">720p · 更省空间</option><option value="80">1080p · 推荐</option><option value="best">账号可用的最高画质</option></select></label>
        <p class="hint">链接默认处理第一个分 P，画质不足时允许降低；已有下载会复用其可用的独立字幕和弹幕。原始视频不会被覆盖。</p>
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
            <div v-for="watch in watches" :key="watch.id" class="watch-item">
              <strong>{{ watch.title || watch.url }}</strong>
              <p class="hint">{{ watch.initialized ? '监控中' : '等待建立基线' }} · 待处理 {{ watch.pending.length }}</p>
              <p v-if="watch.issue" class="notice">{{ watch.issue }}</p>
              <UiButton size="compact" variant="secondary" @click="removeWatch(watch)">停止监控</UiButton>
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
          </div>
          <p class="hint">浏览器打开后请确认账号并上传视频；也可把指令粘贴给 Cindy，让它在获得浏览器权限后协助上传。最终发布需要你确认，能否上传取决于登录状态、浏览器权限和 X 账号。</p>
        </template>
        <p v-if="message" role="status" class="notice">{{ message }}</p>
      </section>
    </div>
  </section>
</template>

<style scoped>
.localize-page { min-height: 0; overflow: auto; padding: 18px 22px 32px; }
.localize-heading h1 { font-size: 20px; margin: 0; }
.localize-heading p { color: var(--color-muted); font-size: 13px; line-height: 1.6; }
.localize-layout { display: grid; grid-template-columns: minmax(0, 1fr) minmax(0, 1fr); gap: 16px; align-items: start; }
.localize-panel { padding: 20px; display: grid; gap: 12px; min-width: 0; }
.localize-panel h2 { font-size: 15px; margin: 8px 0 0; }
.field { display: grid; gap: 6px; font-size: 13px; min-width: 0; }
.field input, .field select, .field textarea { width: 100%; min-width: 0; box-sizing: border-box; font: inherit; color: inherit; background: var(--color-surface); border: 1px solid var(--color-border); border-radius: 8px; padding: 9px 10px; }
.field textarea { resize: vertical; }
.source-choice, .actions { display: flex; gap: 12px; flex-wrap: wrap; }
.source-choice label, .check-line { display: flex; align-items: flex-start; gap: 8px; font-size: 13px; line-height: 1.5; }
.source-choice input, .check-line input { flex: none; margin-top: 3px; }
.hint { font-size: 12px; line-height: 1.6; margin: 0; color: var(--color-muted); }
.status, .notice { font-size: 13px; line-height: 1.6; overflow-wrap: anywhere; margin: 0; }
summary { cursor: pointer; font-size: 13px; }
.model-fields { display: grid; gap: 10px; margin-top: 12px; }
.watch-setup { border-top: 1px solid var(--color-border); padding-top: 12px; }
.watch-item { display: grid; gap: 8px; border: 1px solid var(--color-border); border-radius: 8px; padding: 10px; font-size: 13px; overflow-wrap: anywhere; }
.preview { width: 100%; max-height: 360px; background: #000; }
@media (max-width: 800px) { .localize-page { padding: 14px 12px 24px; } .localize-layout { grid-template-columns: minmax(0, 1fr); } }
</style>
