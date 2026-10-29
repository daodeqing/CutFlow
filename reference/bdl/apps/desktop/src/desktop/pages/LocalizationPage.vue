<script setup lang="ts">
import { computed, onActivated, onDeactivated, onMounted, onUnmounted, ref, watch } from 'vue';
import UiButton from '../../shared/ui/Button.vue';
import { cutflow, errorMessage, mediaSource, operationId, type AiModel, type CreationCatalog, type Job } from '../../shared/api/cutflow';
import { useQueueStore } from '../../shared/stores/queue';
import { useUiStore } from '../../shared/stores/ui';

type LocalizationResult = { videoPath: string; folderPath: string; subtitlePath: string; title: string; sourceUrl?: string | null; warnings: string[]; reviewRequired: boolean };
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
const transcription = ref<'remote' | 'whisper'>('remote');
const whisperModel = ref('base');
const removeBottomText = ref(true);
const models = ref<AiModel[]>([]);
const jobs = ref<LocalizationJob[]>([]);
const selectedJob = ref('');
const post = ref('');
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
      speechModel: speechModel.value.trim(), postModel: postModel.value.trim(), transcription: transcription.value,
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
  post.value = [value.title, value.sourceUrl].filter(Boolean).join('\n\n');
  reviewed.value = false;
});
watch(selectedJob, () => { reviewed.value = false; });
watch(post, () => { reviewed.value = false; });
async function start() {
  if (!canStart.value) return;
  busy.value = true; message.value = '';
  try {
    const response = await cutflow<{ jobId: string }>('cutflow.localize.start', {
      operationId: operationId(), model: model.value.trim(), rights: '用户确认享有制作及发布权限',
      ...(source.value === 'url' ? { url: url.value.trim() } : { taskId: taskId.value }),
      subtitleModel: subtitleModel.value.trim(), danmakuModel: danmakuModel.value.trim(),
      speechModel: speechModel.value.trim(), postModel: postModel.value.trim(),
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
        <label class="field">默认 AI 模型<input v-model="model" list="localization-models" autocomplete="off" /></label>
        <datalist id="localization-models"><option v-for="item in models" :key="item.id" :value="item.id" /></datalist>
        <details><summary>各步骤分别选择模型</summary><div class="model-fields">
          <label class="field">字幕翻译<input v-model="subtitleModel" list="localization-models" placeholder="留空使用默认模型" /></label>
          <label class="field">弹幕翻译<input v-model="danmakuModel" list="localization-models" placeholder="留空使用默认模型" /></label>
          <label class="field">语音识别<input v-model="speechModel" list="localization-models" placeholder="留空使用默认模型" /></label>
          <label class="field">英文文案<input v-model="postModel" list="localization-models" placeholder="留空使用默认模型" /></label>
        </div></details>
        <label class="field">没有独立字幕时<select v-model="transcription"><option value="remote">通过所选 AI 服务识别音频</option><option value="whisper">本地 Whisper 识别</option></select></label>
        <label v-if="transcription === 'whisper'" class="field">Whisper 模型<select v-model="whisperModel"><option value="tiny">tiny · 占用较小</option><option value="base">base · 约 141 MiB</option><option value="small">small · 约 465 MiB</option></select></label>
        <p class="hint">AI 服务是否支持音频取决于实际代理；它也可能将请求转发到云端。本地 Whisper 首次使用可能下载模型。</p>
        <label class="check-line"><input v-model="removeBottomText" type="checkbox" /> 尝试修复画面底部的烧录字幕</label>
        <p class="hint">此方法可能模糊原画面，无法保证清除滚动弹幕。请在发布前检查成片。</p>
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
          <label class="field">英文文案<textarea v-model="post" rows="4" /></label>
          <UiButton variant="secondary" @click="copy(post)">复制文案</UiButton>
          <label class="check-line"><input v-model="reviewed" type="checkbox" /> 我已审核视频、字幕、弹幕和文案，确认发布权限。</label>
          <UiButton :disabled="!reviewed" @click="openX">打开 X 发布窗口</UiButton>
          <p class="hint">请在 X 确认个人账号，手动粘贴文案、上传成片并发布。视频时长和大小限制取决于 X 账号；Cindy 的机器人连接不等于个人账号发帖授权。</p>
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
