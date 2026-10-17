<script setup lang="ts">
import { computed, nextTick, onActivated, onDeactivated, onUnmounted, ref, watch } from 'vue'
import { open } from '@tauri-apps/plugin-dialog'
import UiButton from '../../shared/ui/Button.vue'
import { cutflow, errorMessage, mediaSource, operationId, type AiModel, type Asset, type HubSnapshot, type Job, type Reference } from '../../shared/api/cutflow'
import { useCutflowStore } from '../../shared/stores/cutflow'
import { useCreationStore } from '../../shared/stores/creation'
import { useUiStore } from '../../shared/stores/ui'

const ui = useUiStore()
const store = useCutflowStore()
const creation = useCreationStore()
const kind = computed({ get: () => creation.kind, set: value => { creation.kind = value } })
const draft = computed(() => creation.drafts[kind.value])
const models = ref<AiModel[]>([])
const savedReferences = ref<Reference[]>([])
const jobs = ref<Job[]>([])
const baseUrl = ref('')
const submitting = ref(false)
const refreshingModels = ref(false)
const refreshing = ref(false)
const operationError = ref('')
const snapshotFault = ref('')
const cancelNotice = ref('')
const stoppingIds = ref<string[]>([])
const receiptId = ref('')
const selectedJobId = ref('')
const selectedAssetIndex = ref(0)
const overlay = ref<'history' | 'models' | 'more' | 'details' | 'references' | null>(null)
const dialog = ref<HTMLDialogElement>()
const returnFocus = ref<HTMLElement>()
const modelSearch = ref('')
const referenceName = ref('')
const referenceKind = ref('character')
let timer: ReturnType<typeof setInterval> | undefined
let active = false
let epoch = 0
let catalogRevision = 0
let refreshPromise: Promise<void> | undefined

const modelKind = (item: AiModel) => /^gflow-(veo|omni)-/.test(item.id) ? 'video' : item.kind
const modelName = (id: unknown) => {
  const value = String(id || '')
  const names: Record<string, string> = {
    'gflow-nano-banana-2-lite': 'Nano Banana 2 Lite', 'gflow-nano-banana-2.1': 'Nano Banana 2.1',
    'gflow-nano-banana-pro': 'Nano Banana Pro', 'gflow-omni-1.1-flash': 'Omni 1.1 Flash',
    'gflow-veo-3.1-lite': 'Veo 3.1 Lite', 'gflow-veo-3.1-fast': 'Veo 3.1 Fast', 'gflow-veo-3.1-quality': 'Veo 3.1 Quality',
  }
  return names[value] || value || '未选择模型'
}
const capability = computed(() => models.value.find(item => item.id === draft.value.model))
const missingModel = computed(() => draft.value.modelSource === 'catalog' && !!draft.value.model && !capability.value)
const visibleModels = computed(() => models.value.filter(item => item.id.startsWith('gflow-') && modelKind(item) === kind.value))
const filteredModels = computed(() => visibleModels.value.filter(item => (item.id + modelName(item.id)).toLowerCase().includes(modelSearch.value.toLowerCase())))
const referenceLimit = computed(() => capability.value?.maxReferences)
const durationOptions = computed(() => draft.value.model.includes('omni') ? [4, 6, 8, 10] : [4, 6, 8])
const visibleJobs = computed(() => jobs.value.filter(job => job.method === 'cutflow.generate').slice().reverse())
const activeJobs = computed(() => visibleJobs.value.filter(job => ['running', 'queued'].includes(job.status)))
const selectedJob = computed(() => visibleJobs.value.find(job => job.id === selectedJobId.value) || visibleJobs.value[0])
const assets = computed(() => selectedJob.value?.result?.assets || [])
const asset = computed(() => assets.value[selectedAssetIndex.value] || assets.value[0])
const currentTask = computed(() => activeJobs.value.find(job => job.id === selectedJobId.value) || activeJobs.value[0])
const locked = computed(() => submitting.value || !!receiptId.value || activeJobs.value.length > 0)
const stopping = computed(() => !!currentTask.value && stoppingIds.value.includes(currentTask.value.id))
const canSubmit = computed(() => !!baseUrl.value && !!draft.value.model.trim() && !!draft.value.prompt.trim() && !missingModel.value && !locked.value)
const statusLabel = (status: string) => ({ queued: '等待中', running: '生成中', completed: '已保存', failed: '失败', result_unknown: '结果待核查', interrupted: '已中断', cancelled: '已停止' }[status] || status)
const ratioMismatch = (job: Job, media: Asset) => {
  if (job.params.kind !== 'image' || !media.width || !media.height) return false
  const ratio = String(job.params.aspect_ratio || 'auto')
  if (ratio === 'auto') return false
  const [width, height] = ratio.split(':').map(Number)
  return width > 0 && height > 0 && Math.abs(media.width / media.height / (width / height) - 1) > 0.03
}
function selectJob(job: Job) { selectedJobId.value = job.id; selectedAssetIndex.value = 0; closeOverlay() }
function closeOverlay() { overlay.value = null }
async function showOverlay(value: NonNullable<typeof overlay.value>) {
  returnFocus.value = document.activeElement instanceof HTMLElement ? document.activeElement : undefined
  overlay.value = value
}
watch(overlay, async value => {
  await nextTick()
  if (value) {
    if (dialog.value && !dialog.value.open) dialog.value.showModal()
  } else {
    dialog.value?.close()
    returnFocus.value?.focus()
  }
})
function defaultModel() {
  if (draft.value.model) return
  const preferred = kind.value === 'image' ? visibleModels.value.find(item => item.id === 'gflow-nano-banana-2-lite') : undefined
  const item = preferred || visibleModels.value[0]
  if (item) { draft.value.model = item.id; draft.value.modelSource = 'catalog' }
}
watch(kind, defaultModel)
function chooseModel(item: AiModel) {
  draft.value.model = item.id; draft.value.modelSource = 'catalog'
  if (!durationOptions.value.includes(Number(draft.value.duration))) draft.value.duration = '8'
  closeOverlay()
}
async function refresh() {
  if (refreshPromise) return refreshPromise
  const ticket = epoch
  const revision = catalogRevision
  refreshing.value = true
  refreshPromise = (async () => {
    try {
      const snapshot = await cutflow<HubSnapshot>('cutflow.hub.snapshot')
      if (!active || ticket !== epoch) return
      jobs.value = snapshot.jobs
      if (revision === catalogRevision) models.value = snapshot.models
      savedReferences.value = snapshot.references
      baseUrl.value = snapshot.connection.baseUrl
      snapshotFault.value = snapshot.fault || ''
      defaultModel()
      if (!selectedJobId.value && visibleJobs.value[0]) selectedJobId.value = visibleJobs.value[0].id
      if (receiptId.value && jobs.value.some(job => job.id === receiptId.value)) receiptId.value = ''
      stoppingIds.value = stoppingIds.value.filter(id => !jobs.value.some(job => job.id === id && !['running', 'queued'].includes(job.status)))
    } catch (reason) {
      if (active && ticket === epoch) snapshotFault.value = errorMessage(reason)
    }
  })().finally(() => { refreshPromise = undefined; refreshing.value = false })
  return refreshPromise
}
async function refreshModels() {
  if (refreshingModels.value) return
  const ticket = epoch
  refreshingModels.value = true; operationError.value = ''
  try {
    const result = await cutflow<{ models: AiModel[] }>('cutflow.models.list')
    if (!active || ticket !== epoch) return
    catalogRevision++; models.value = result.models; defaultModel()
  } catch (reason) { if (active && ticket === epoch) operationError.value = errorMessage(reason) }
  finally { refreshingModels.value = false }
}
function addReferences(paths: string[], referencePrompt?: string) {
  const merged = [...new Set([...draft.value.references, ...paths])]
  if (referenceLimit.value !== undefined && merged.length > referenceLimit.value) {
    operationError.value = '当前模型最多支持 ' + referenceLimit.value + ' 张参考图，请先移除多余图片。'
    return false
  }
  draft.value.references = merged
  if (referencePrompt) draft.value.prompt = referencePrompt
  operationError.value = ''
  return true
}
async function chooseReferences() {
  try {
    const paths = await open({ multiple: true, filters: [{ name: '参考图片', extensions: ['png', 'jpg', 'jpeg', 'webp'] }] })
    if (Array.isArray(paths)) addReferences(paths)
    else if (typeof paths === 'string') addReferences([paths])
  } catch (reason) { operationError.value = errorMessage(reason) }
}
async function saveReference() {
  if (!draft.value.references[0] || !referenceName.value.trim()) return
  try {
    await cutflow('cutflow.references.save', { path: draft.value.references[0], name: referenceName.value.trim(), kind: referenceKind.value, prompt: draft.value.prompt })
    referenceName.value = ''; await refresh()
  } catch (reason) { operationError.value = errorMessage(reason) }
}
async function submit() {
  if (locked.value) return
  operationError.value = ''; cancelNotice.value = ''
  const value = draft.value
  const known = capability.value
  if (!value.model.startsWith('gflow-')) { operationError.value = '请选择 GFlow 图片或视频模型'; return }
  if (missingModel.value) { operationError.value = '所选模型已不在服务列表中，请重新选择模型。'; return }
  if (known && modelKind(known) !== kind.value) { operationError.value = '所选模型与当前图片／视频类别不匹配。'; return }
  if (!known && ((/^gflow-(veo|omni)-/.test(value.model) && kind.value !== 'video') || (/^gflow-nano-banana-/.test(value.model) && kind.value !== 'image'))) {
    operationError.value = '所选模型与当前图片／视频类别不匹配。'; return
  }
  if (referenceLimit.value !== undefined && value.references.length > referenceLimit.value) { operationError.value = '参考图数量超过当前模型上限。'; return }
  if (!baseUrl.value || !value.prompt.trim() || !value.model.trim()) return
  submitting.value = true
  try {
    const result = await cutflow<{ jobId: string }>('cutflow.generate', {
      operationId: operationId(), model: value.model.trim(), prompt: value.prompt.trim(), kind: kind.value,
      references: [...value.references], maxGenerationCalls: kind.value === 'image' ? Number(value.count) : 1,
      n: kind.value === 'image' ? Number(value.count) : 1, aspect_ratio: value.aspect,
      image_size: kind.value === 'image' ? value.imageSize : undefined,
      duration: kind.value === 'video' ? Number(value.duration) : undefined, stream: value.stream,
    })
    receiptId.value = result.jobId; selectedJobId.value = result.jobId; selectedAssetIndex.value = 0
    await refresh()
  } catch (reason) { operationError.value = errorMessage(reason) }
  finally { submitting.value = false }
}
async function stopJob(job: Job) {
  if (stoppingIds.value.includes(job.id)) return
  stoppingIds.value.push(job.id); operationError.value = ''
  try {
    const result = await cutflow<{ message?: string }>('cutflow.jobs.cancel', { jobId: job.id })
    cancelNotice.value = result.message || '已请求停止，正在等待任务结束。已完成的素材会保留。'
    await refresh()
  } catch (reason) {
    stoppingIds.value = stoppingIds.value.filter(id => id !== job.id)
    operationError.value = errorMessage(reason)
  }
}
async function retryDownload(job: Job) {
  if (locked.value) return
  submitting.value = true
  try { await cutflow('cutflow.jobs.retryDownload', { jobId: job.id }); receiptId.value = job.id; await refresh() }
  catch (reason) { operationError.value = errorMessage(reason) }
  finally { submitting.value = false }
}
async function openFile(path: string) {
  try { await cutflow('cutflow.files.open', { path }) }
  catch (reason) { operationError.value = errorMessage(reason) }
}
const stopPolling = () => { active = false; epoch++; clearInterval(timer); timer = undefined; closeOverlay() }
onActivated(() => { active = true; epoch++; void refresh(); timer = setInterval(() => { void refresh() }, 1800) })
onDeactivated(stopPolling)
onUnmounted(stopPolling)
</script>

<template>
  <section class="cutflow-page creation-page" aria-label="AI 创作工作台">
    <header class="creation-toolbar">
      <div class="creation-title"><h1>AI 创作</h1><span class="studio-tag">GFlow</span></div>
      <div class="cutflow-actions">
        <UiButton variant="ghost" @click="ui.setTab('settings')"><UIcon name="i-tabler-plug-connected" aria-hidden="true" />连接设置</UiButton>
        <UiButton variant="secondary" @click="showOverlay('history')"><UIcon name="i-tabler-history" aria-hidden="true" />历史作品 {{ visibleJobs.length }}</UiButton>
      </div>
    </header>

    <section class="preview-workspace" aria-label="当前作品预览">
      <header class="preview-toolbar">
        <div class="preview-description"><span v-if="selectedJob" class="job-status" :data-status="selectedJob.status">{{ statusLabel(selectedJob.status) }}</span><span>{{ selectedJob ? modelName(selectedJob.params.model) : '创作预览' }}</span></div>
        <UiButton v-if="selectedJob" variant="ghost" size="compact" @click="showOverlay('details')">作品详情</UiButton>
      </header>
      <div class="preview-stage">
        <template v-if="asset">
          <img v-if="selectedJob?.params.kind === 'image'" :src="mediaSource(asset.path)" :alt="String(selectedJob.params.prompt)" />
          <video v-else :src="mediaSource(asset.path)" :poster="selectedJob?.result?.thumbnail ? mediaSource(selectedJob.result.thumbnail) : undefined" controls preload="metadata"></video>
        </template>
        <div v-else class="preview-empty" role="status">
          <UIcon :name="currentTask || receiptId ? 'i-tabler-sparkles' : 'i-tabler-photo'" aria-hidden="true" />
          <h2>{{ selectedJob?.status === 'failed' ? '这次生成未完成' : currentTask || receiptId ? '正在创作，结果会显示在这里' : '把想法变成画面' }}</h2>
          <p>{{ selectedJob?.error?.message ? '查看作品详情了解失败原因，程序不会自动重发。' : '在下方描述画面，图片或视频会自动保存到本地。' }}</p>
          <UiButton v-if="selectedJob?.canRetryDownload" variant="secondary" :disabled="locked" @click="retryDownload(selectedJob)">只重试下载结果</UiButton>
        </div>
      </div>
      <footer class="preview-footer">
        <div class="asset-summary">
          <span v-if="asset">{{ asset.width }} × {{ asset.height }}{{ asset.duration ? ' · ' + asset.duration.toFixed(1) + ' 秒' : '' }}</span>
          <span v-else>{{ selectedJob?.stage || '生成后的素材可加入剪辑工程' }}</span>
          <span v-if="asset && selectedJob && ratioMismatch(selectedJob, asset)" class="ratio-warning">比例与所选 {{ selectedJob.params.aspect_ratio }} 不一致 <button type="button" @click="showOverlay('details')">详情</button></span>
        </div>
        <div v-if="assets.length > 1" class="asset-thumbnails" aria-label="本次生成的素材">
          <button v-for="(item, index) in assets" :key="item.path" type="button" :aria-label="'查看第 ' + (index + 1) + ' 张'" :aria-pressed="index === selectedAssetIndex" @click="selectedAssetIndex = index"><img :src="mediaSource(item.path)" alt="" /></button>
        </div>
        <div v-if="asset" class="cutflow-actions">
          <UiButton size="compact" @click="store.sendToEditor([asset.path])">加入剪辑</UiButton>
          <UiButton variant="secondary" size="compact" @click="openFile(asset.path)">打开文件</UiButton>
        </div>
      </footer>
    </section>

    <form class="creation-composer" @submit.prevent="submit" @keydown.ctrl.enter.prevent="submit">
      <div class="composer-options">
        <div class="creation-kind" role="group" aria-label="创作类别">
          <button type="button" :aria-pressed="kind === 'image'" @click="kind = 'image'">图片</button>
          <button type="button" :aria-pressed="kind === 'video'" @click="kind = 'video'">视频</button>
        </div>
        <button class="model-trigger" type="button" :title="draft.model || '选择模型'" :aria-invalid="missingModel" @click="showOverlay('models')">{{ modelName(draft.model) }}<UIcon name="i-tabler-chevron-down" aria-hidden="true" /></button>
        <label class="compact-option" for="ai-aspect"><span>比例</span><select id="ai-aspect" v-model="draft.aspect"><option value="auto">默认</option><option value="16:9">16:9</option><option value="9:16">9:16</option><template v-if="kind === 'image'"><option value="1:1">1:1</option><option value="4:3">4:3</option><option value="3:4">3:4</option></template></select></label>
        <template v-if="kind === 'image'">
          <label class="compact-option" for="ai-image-size"><span>分辨率</span><select id="ai-image-size" v-model="draft.imageSize"><option value="auto">默认</option><option v-for="size in ['1K', '2K', '4K']" :key="size" :value="size">{{ size }}</option></select></label>
          <label class="compact-option" for="ai-count"><span>数量</span><select id="ai-count" v-model="draft.count"><option v-for="number in [1, 2, 3, 4]" :key="number" :value="String(number)">{{ number }} 张</option></select></label>
        </template>
        <label v-else class="compact-option" for="ai-duration"><span>时长</span><select id="ai-duration" v-model="draft.duration"><option v-for="seconds in durationOptions" :key="seconds" :value="String(seconds)">{{ seconds }} 秒</option></select></label>
      </div>
      <label class="sr-only" for="ai-prompt">描述你想要的画面</label>
      <textarea id="ai-prompt" v-model="draft.prompt" rows="3" placeholder="描述主体、场景、动作与光线…" required></textarea>
      <div v-if="snapshotFault || operationError || !baseUrl || missingModel" class="composer-notice" role="alert">
        <span>{{ operationError || snapshotFault || (missingModel ? '所选模型已失效，请重新选择。' : '请先在连接设置中填写服务地址和密钥。') }}</span>
        <button v-if="snapshotFault" type="button" @click="refresh">刷新</button>
        <button type="button" @click="showOverlay('details')">详情</button>
      </div>
      <footer class="composer-footer">
        <div class="cutflow-actions">
          <UiButton variant="ghost" size="compact" @click="chooseReferences"><UIcon name="i-tabler-paperclip" aria-hidden="true" />参考图{{ draft.references.length ? ' · ' + draft.references.length : '' }}</UiButton>
          <UiButton v-if="draft.references.length" variant="ghost" size="compact" @click="showOverlay('references')">管理</UiButton>
          <UiButton variant="ghost" size="compact" @click="showOverlay('more')">更多设置</UiButton>
          <span v-if="draft.modelSource === 'custom'" class="custom-model-note">自定义 · 能力未确认</span>
        </div>
        <div class="composer-submit">
          <span v-if="locked" class="task-summary" role="status">{{ stopping ? '等待任务停止' : activeJobs.length > 1 ? activeJobs.length + ' 项生成中' : '正在处理当前任务' }}</span>
          <UiButton v-if="currentTask" variant="danger" :disabled="stopping" @click="stopJob(currentTask)">{{ stopping ? '正在停止…' : '停止生成' }}</UiButton>
          <UiButton v-else type="submit" :disabled="!canSubmit">{{ submitting || receiptId ? '提交中…' : kind === 'image' ? '生成图片' : '生成视频' }}<UIcon name="i-tabler-arrow-up-right" aria-hidden="true" /></UiButton>
        </div>
      </footer>
    </form>

    <dialog ref="dialog" aria-labelledby="creation-dialog-title" class="creation-dialog" :class="{ 'history-drawer': overlay === 'history' }" @close="closeOverlay" @cancel="closeOverlay" @click="($event.target === dialog) && closeOverlay()">
      <div class="creation-dialog-body">
        <header><h2 id="creation-dialog-title">{{ overlay === 'history' ? '历史作品' : overlay === 'models' ? '选择模型' : overlay === 'more' ? '更多设置' : overlay === 'references' ? '参考图片' : '作品与任务详情' }}</h2><UiButton variant="ghost" size="compact" aria-label="关闭面板" @click="closeOverlay">关闭</UiButton></header>
        <template v-if="overlay === 'history'">
          <div class="cutflow-actions"><span>{{ visibleJobs.length }} 项创作</span><UiButton variant="ghost" :disabled="refreshing" @click="refresh">{{ refreshing ? '刷新中…' : '刷新任务' }}</UiButton></div>
          <p v-if="!visibleJobs.length" class="field-help">还没有作品。在下方写下第一个想法。</p>
          <article v-for="job in visibleJobs" :key="job.id" class="history-item" :class="{ selected: selectedJob?.id === job.id }">
            <button type="button" class="history-select" @click="selectJob(job)">
              <img v-if="job.result?.thumbnail || job.result?.assets?.[0]?.path && job.params.kind === 'image'" :src="mediaSource(job.result.thumbnail || job.result.assets![0].path)" alt="" loading="lazy" />
              <span v-else class="history-placeholder"><UIcon name="i-tabler-photo" aria-hidden="true" /></span>
              <span><strong>{{ String(job.params.prompt || '未命名作品') }}</strong><small>{{ modelName(job.params.model) }} · {{ statusLabel(job.status) }}</small></span>
            </button>
            <UiButton v-if="['running', 'queued'].includes(job.status)" variant="secondary" size="compact" :disabled="stoppingIds.includes(job.id)" @click="stopJob(job)">{{ stoppingIds.includes(job.id) ? '正在停止…' : '停止生成' }}</UiButton>
          </article>
        </template>
        <template v-else-if="overlay === 'models'">
          <div class="model-picker"><label for="ai-model-search">搜索模型</label><input id="ai-model-search" v-model="modelSearch" type="search" placeholder="模型名称或 ID" autofocus /><div class="model-options" @wheel.stop><button v-for="item in filteredModels" :key="item.id" type="button" :aria-pressed="item.id === draft.model" @click="chooseModel(item)"><strong>{{ modelName(item.id) }}</strong><small>{{ item.id }}</small></button><p v-if="!filteredModels.length" class="field-help">没有匹配的 GFlow 模型。</p></div><UiButton variant="secondary" :disabled="refreshingModels" @click="refreshModels">{{ refreshingModels ? '刷新中…' : '刷新模型' }}</UiButton></div>
          <details class="custom-model"><summary>手动填写 GFlow 模型 ID</summary><label for="ai-model">模型 ID</label><input id="ai-model" v-model="draft.model" placeholder="gflow-…" @input="draft.modelSource = 'custom'" /><p class="field-help">自定义模型的能力由服务决定，不会自动切换或重发。</p><UiButton variant="secondary" @click="closeOverlay">使用此模型</UiButton></details>
        </template>
        <template v-else-if="overlay === 'more'">
          <label class="checkbox-label"><input v-model="draft.stream" type="checkbox" />接收流式进度</label>
          <p class="field-help">多张图片按顺序生成并保存；停止后不再提交下一张，已完成素材保留。实际尺寸以作品信息为准。</p>
          <section class="reference-library"><h3>角色与场景参考库</h3><p class="field-help">保存第一张参考图及当前提示词。{{ referenceLimit !== undefined ? '当前模型最多 ' + referenceLimit + ' 张参考图。' : '自定义模型参考图数量由服务判断。' }}</p><label for="reference-name">参考名称</label><input id="reference-name" v-model="referenceName" placeholder="例如：主角正面" /><label for="reference-kind">用途</label><select id="reference-kind" v-model="referenceKind"><option value="character">角色</option><option value="scene">场景</option></select><UiButton variant="secondary" :disabled="!draft.references.length || !referenceName.trim()" @click="saveReference">保存到参考库</UiButton><button v-for="reference in savedReferences" :key="reference.id" class="saved-reference" type="button" @click="addReferences([reference.path], reference.prompt) && closeOverlay()">{{ reference.name }} · {{ reference.kind === 'scene' ? '场景' : '角色' }}</button></section>
          <p v-if="operationError" class="cutflow-notice error" role="alert">{{ operationError }}</p>
        </template>
        <template v-else-if="overlay === 'references'">
          <p class="field-help">{{ referenceLimit !== undefined ? '当前模型最多 ' + referenceLimit + ' 张。' : '自定义模型参考图能力未确认。' }}</p>
          <ul class="reference-files"><li v-for="path in draft.references" :key="path"><span>{{ path.split(/[\\/]/).pop() }}</span><button type="button" :aria-label="'移除参考图 ' + path" @click="draft.references = draft.references.filter(item => item !== path)">移除</button></li></ul>
          <UiButton variant="secondary" @click="chooseReferences">添加参考图</UiButton><p v-if="operationError" class="cutflow-notice error" role="alert">{{ operationError }}</p>
        </template>
        <template v-else-if="overlay === 'details'">
          <p v-if="operationError || snapshotFault" class="cutflow-notice error" role="alert">{{ operationError || snapshotFault }}</p>
          <template v-if="selectedJob"><p><strong>{{ statusLabel(selectedJob.status) }}</strong> · {{ modelName(selectedJob.params.model) }}</p><p class="file-path">{{ selectedJob.params.model }}</p><p class="generation-prompt">{{ selectedJob.params.prompt }}</p><p class="task-detail">{{ selectedJob.error?.message || selectedJob.stage }}</p><p v-if="cancelNotice" class="cutflow-notice">{{ cancelNotice }}</p><p v-if="asset && ratioMismatch(selectedJob, asset)" class="cutflow-notice">服务返回的比例与所选 {{ selectedJob.params.aspect_ratio }} 不一致，已保留原图；没有裁剪或再次生成。</p><UiButton v-if="selectedJob.canRetryDownload" variant="secondary" :disabled="locked" @click="retryDownload(selectedJob)">只重试下载结果</UiButton></template>
          <p v-else class="field-help">连接成功仅表示服务可访问，生成结果以实际任务为准。</p>
        </template>
      </div>
    </dialog>
  </section>
</template>
<style src="./CutflowPages.css"></style>
