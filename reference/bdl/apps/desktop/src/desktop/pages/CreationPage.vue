<script setup lang="ts">
import { computed, onActivated, onDeactivated, onUnmounted, ref } from 'vue'
import { open } from '@tauri-apps/plugin-dialog'
import UiButton from '../../shared/ui/Button.vue'
import UiTabs from '../../shared/ui/Tabs.vue'
import { cutflow, errorMessage, mediaSource, operationId, type AiModel, type HubSnapshot, type Job, type Reference } from '../../shared/api/cutflow'
import { useCutflowStore } from '../../shared/stores/cutflow'
import { useUiStore } from '../../shared/stores/ui'

const ui = useUiStore()
const store = useCutflowStore()
const kind = ref('image')
const model = ref('')
const prompt = ref('')
const references = ref<string[]>([])
const models = ref<AiModel[]>([])
const savedReferences = ref<Reference[]>([])
const jobs = ref<Job[]>([])
const baseUrl = ref('')
const stream = ref(true)
const busy = ref(false)
const refreshing = ref(false)
const error = ref('')
const referenceName = ref('')
const referenceKind = ref('character')
let timer: ReturnType<typeof setInterval> | undefined
const visibleModels = computed(() => models.value.filter(item => item.kind === kind.value || item.kind === 'unknown'))
const capability = computed(() => models.value.find(item => item.id === model.value))
const visibleJobs = computed(() => jobs.value.filter(job => job.method === 'cutflow.generate').slice().reverse())
const refresh = async () => {
  try {
    const snapshot = await cutflow<HubSnapshot>('cutflow.hub.snapshot')
    jobs.value = snapshot.jobs; models.value = snapshot.models; savedReferences.value = snapshot.references; baseUrl.value = snapshot.connection.baseUrl
    if (snapshot.fault) error.value = snapshot.fault
  } catch (reason) { error.value = errorMessage(reason) }
}
const refreshModels = async () => {
  refreshing.value = true; error.value = ''
  try { const result = await cutflow<{ models: AiModel[] }>('cutflow.models.list'); models.value = result.models }
  catch (reason) { error.value = errorMessage(reason) }
  finally { refreshing.value = false }
}
const chooseReferences = async () => {
  const paths = await open({ multiple: true, filters: [{ name: '参考图片', extensions: ['png', 'jpg', 'jpeg', 'webp'] }] })
  if (Array.isArray(paths)) references.value = [...new Set([...references.value, ...paths])]
}
const saveReference = async () => {
  if (!references.value[0] || !referenceName.value.trim()) return
  try { await cutflow('cutflow.references.save', { path: references.value[0], name: referenceName.value, kind: referenceKind.value, prompt: prompt.value }); referenceName.value = ''; await refresh() }
  catch (reason) { error.value = errorMessage(reason) }
}
const submit = async () => {
  busy.value = true; error.value = ''
  try {
    await cutflow('cutflow.generate', { operationId: operationId(), model: model.value.trim(), prompt: prompt.value.trim(), kind: kind.value, references: references.value, maxGenerationCalls: 1, stream: stream.value })
    await refresh()
  } catch (reason) { error.value = errorMessage(reason) }
  finally { busy.value = false }
}
const jobAction = async (job: Job, method: string) => {
  try { await cutflow(method, { jobId: job.id }); await refresh() }
  catch (reason) { error.value = errorMessage(reason) }
}
const openFile = async (path: string) => {
  try { await cutflow('cutflow.files.open', { path }) }
  catch (reason) { error.value = errorMessage(reason) }
}
const statusLabel = (status: string) => ({ queued: '等待中', running: '生成处理中', completed: '已保存', failed: '失败', result_unknown: '结果待核查', interrupted: '已中断', cancelled: '已取消' }[status] || status)
onActivated(() => { void refresh(); timer = setInterval(() => { void refresh() }, 1800) })
const stop = () => { clearInterval(timer); timer = undefined }
onDeactivated(stop)
onUnmounted(stop)
</script>
<template>
  <section class="cutflow-page creation-page">
    <header class="cutflow-page-heading"><div><p class="eyebrow">AI STUDIO</p><h1>从一个想法，开始创作</h1><p>连接兼容 OpenAI 的图片与视频服务，作品自动保存到本地。</p></div><UiButton variant="secondary" @click="ui.setTab('settings')"><UIcon name="i-tabler-plug-connected" aria-hidden="true" />连接设置</UiButton></header>
    <p v-if="!baseUrl" class="cutflow-notice">先在设置中填写服务地址和 API Key。无需创建剪辑工程。</p>
    <div class="creation-layout">
      <form class="cutflow-card generation-form" @submit.prevent="submit">
        <UiTabs v-model="kind" :tabs="[{ label: '生成图片', value: 'image' }, { label: '生成视频', value: 'video' }]" />
        <label for="ai-model">模型</label><div class="model-field"><input id="ai-model" v-model="model" list="ai-models" placeholder="选择模型或填写模型 ID" required /><UiButton variant="ghost" :disabled="refreshing" @click="refreshModels">{{ refreshing ? '刷新中…' : '刷新' }}</UiButton></div>
        <datalist id="ai-models"><option v-for="item in visibleModels" :key="item.id" :value="item.id">{{ item.description || item.kind }}</option></datalist>
        <p class="field-help">{{ capability?.aspect ? `比例 ${capability.aspect}` : '比例由服务或模型变体决定' }}{{ capability?.duration ? ` · ${capability.duration} 秒` : '' }}。{{ capability ? `参考图上限 ${capability.maxReferences} 张。` : '自定义模型能力由服务判断。' }}</p>
        <label for="ai-prompt">描述你想要的画面</label><textarea id="ai-prompt" v-model="prompt" rows="7" placeholder="主体、场景、动作、镜头与光线…" required></textarea>
        <div class="cutflow-actions"><strong>参考图片 · {{ references.length }}</strong><UiButton variant="secondary" @click="chooseReferences">添加参考图</UiButton></div>
        <ul v-if="references.length" class="reference-files"><li v-for="path in references" :key="path"><span :title="path">{{ path.split(/[\\/]/).pop() }}</span><button type="button" :aria-label="`移除参考图 ${path}`" @click="references = references.filter(item => item !== path)">移除</button></li></ul>
        <label class="checkbox-label"><input v-model="stream" type="checkbox" />流式接收进度（不支持时可关闭）</label>
        <p v-if="error" class="cutflow-notice error" role="alert">{{ error }}</p>
        <UiButton type="submit" :disabled="busy || !baseUrl || !model.trim() || !prompt.trim()">{{ busy ? '提交中…' : kind === 'image' ? '生成图片' : '生成视频' }}</UiButton>
        <p class="field-help">每次提交一项生成。断流时请先核查服务端；本地取消后服务端仍可能继续并计费。</p>
        <details><summary>角色 / 场景参考库</summary><p class="field-help">保存第一张参考图及当前提示词，便于重复使用。</p><label for="reference-name">参考名称</label><input id="reference-name" v-model="referenceName" placeholder="例如：主角正面 / 客厅夜景" /><label for="reference-kind">用途</label><select id="reference-kind" v-model="referenceKind"><option value="character">角色</option><option value="scene">场景</option></select><UiButton variant="secondary" :disabled="!references.length || !referenceName.trim()" @click="saveReference">保存到参考库</UiButton><button v-for="reference in savedReferences" :key="reference.id" type="button" class="saved-reference" @click="references = [...new Set([...references, reference.path])]; prompt = reference.prompt || prompt">{{ reference.name }} · {{ reference.kind === 'scene' ? '场景' : '角色' }}</button></details>
      </form>
      <section class="generation-results" aria-label="生成任务与结果">
        <div class="results-heading"><h2>我的创作 <span>{{ visibleJobs.length }}</span></h2><UiButton variant="ghost" @click="refresh">刷新任务</UiButton></div>
        <div v-if="!visibleJobs.length" class="creation-empty"><UIcon name="i-tabler-photo-spark" aria-hidden="true" /><h3>第一张画面，从这里诞生</h3><p>写下提示词，选择模型，生成后即可预览和送入剪辑。</p></div>
        <article v-for="job in visibleJobs" :key="job.id" class="cutflow-card generation-result">
          <header><span class="connection-badge" :class="{ warning: job.status === 'result_unknown' }">{{ statusLabel(job.status) }}</span><small>{{ job.params.model }}</small></header>
          <template v-for="asset in job.result?.assets || []" :key="asset.path"><img v-if="job.params.kind === 'image'" :src="mediaSource(asset.path)" :alt="String(job.params.prompt)" loading="lazy" /><video v-else :src="mediaSource(asset.path)" :poster="job.result?.thumbnail ? mediaSource(job.result.thumbnail) : undefined" controls preload="metadata"></video><p class="field-help">{{ asset.width }} × {{ asset.height }}{{ asset.duration ? ` · ${asset.duration.toFixed(1)} 秒` : '' }}</p><div class="cutflow-actions"><UiButton @click="store.sendToEditor([asset.path])">送入剪辑</UiButton><UiButton variant="secondary" @click="openFile(asset.path)">打开本地文件</UiButton></div></template>
          <p class="generation-prompt">{{ job.params.prompt }}</p><p class="field-help" role="status">{{ job.error?.message || job.stage }}</p>
          <UiButton v-if="['queued', 'running'].includes(job.status)" variant="secondary" @click="jobAction(job, 'cutflow.jobs.cancel')">停止本地等待</UiButton>
          <UiButton v-if="['failed', 'cancelled', 'interrupted'].includes(job.status)" variant="secondary" @click="jobAction(job, 'cutflow.jobs.retryDownload')">只重试下载结果</UiButton>
        </article>
      </section>
    </div>
  </section>
</template>
<style src="./CutflowPages.css"></style>
