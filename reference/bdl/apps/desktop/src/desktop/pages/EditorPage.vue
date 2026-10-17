<script setup lang="ts">
import { onActivated, onDeactivated, onUnmounted, ref } from 'vue'
import { open } from '@tauri-apps/plugin-dialog'
import UiButton from '../../shared/ui/Button.vue'
import { cutflow, errorMessage, operationId, type EditorStatus, type Job, type RecentProject } from '../../shared/api/cutflow'
import { useCutflowStore } from '../../shared/stores/cutflow'

const store = useCutflowStore()
const status = ref<EditorStatus>({ running: false })
const recents = ref<RecentProject[]>([])
const busy = ref(false)
const message = ref('')
let timer: ReturnType<typeof setInterval> | undefined

const refresh = async (selectTarget = false) => {
  try {
    status.value = await cutflow<EditorStatus>('cutflow.editor.status')
    recents.value = await cutflow<RecentProject[]>('cutflow.editor.recents')
    if (store.pending && !store.pending.jobId && status.value.current && (selectTarget || !store.pending.target)) {
      store.savePending({ ...store.pending, target: { ...status.value.current } })
    }
    if (store.pending?.jobId) {
      const job = await cutflow<Job>('cutflow.jobs.get', { jobId: store.pending.jobId })
      if (job.status === 'completed') { store.savePending(null); message.value = '素材已复制到工程并加入素材库。可在剪辑器中撤销。' }
      else if (job.status === 'result_unknown') {
        message.value = '导入结果待核查，请先检查工程素材库；不会自动重复导入。'
      } else if (!['running', 'queued'].includes(job.status)) {
        store.savePending({ files: store.pending.files, operationId: operationId(), target: store.pending.target })
        message.value = job.error?.message || job.stage
      }
    }
  } catch (error) { message.value = errorMessage(error) }
}
const launch = async (projectPath?: string) => {
  busy.value = true; message.value = ''
    try { await cutflow('cutflow.editor.open', projectPath ? { projectPath } : {}); await refresh(true) }
  catch (error) { message.value = errorMessage(error) }
  finally { busy.value = false }
}
const chooseProject = async () => {
  const selected = await open({ directory: true, title: '选择 Concat 工程文件夹' })
  if (typeof selected === 'string') await launch(selected)
}
const importPending = async () => {
  const pending = store.pending
  const project = pending?.target
  if (!project || !pending || pending.jobId) return
  busy.value = true; message.value = ''
  try {
    const result = await cutflow<{ jobId: string }>('cutflow.assets.import', {
      projectPath: project.projectPath, operationId: pending.operationId, expectedRevision: project.revision, files: pending.files, place: false,
    })
    store.savePending({ ...pending, jobId: result.jobId })
    message.value = '正在验证并复制素材；导入完成前请保持当前工程。'
  } catch (error) { message.value = errorMessage(error); await refresh() }
  finally { busy.value = false }
}
onActivated(() => { void refresh(); timer = setInterval(() => { if (!busy.value) void refresh() }, 3000) })
const stop = () => { clearInterval(timer); timer = undefined }
onDeactivated(stop)
onUnmounted(stop)
</script>
<template>
  <section class="cutflow-page">
    <header class="cutflow-page-heading"><div><p class="eyebrow">CONCAT EDITOR</p><h1>让素材进入你的作品</h1><p>在独立剪辑窗口中完成导入、时间线、字幕、配音和导出。</p></div><span class="connection-badge">{{ status.running ? '剪辑器已连接' : '剪辑器未打开' }}</span></header>
    <section class="cutflow-card editor-launch">
      <UIcon name="i-tabler-movie" aria-hidden="true" />
      <div><h2>{{ status.current ? '当前工程' : '打开完整剪辑器' }}</h2><p class="file-path">{{ status.current?.projectPath || '打开后可新建工程，也可继续已有工程。' }}</p></div>
      <div class="cutflow-actions"><UiButton :disabled="busy" @click="launch()">{{ busy ? '连接中…' : status.running ? '唤起剪辑窗口' : '打开 / 新建工程' }}</UiButton><UiButton variant="secondary" :disabled="busy" @click="chooseProject">打开已有工程</UiButton></div>
    </section>
    <section v-if="store.pending" class="cutflow-card" aria-labelledby="pending-title">
      <h2 id="pending-title">待导入素材 · {{ store.pending.files.length }} 项</h2>
      <ul class="file-list"><li v-for="file in store.pending.files" :key="file">{{ file }}</li></ul>
      <p>目标工程：{{ store.pending.target?.projectPath || '请先在剪辑窗口新建或选择工程，再刷新状态。' }}</p>
      <p>素材复制到工程素材库，时间线由你安排。人工编辑或切换工程时会拒绝导入，素材继续保留。</p>
      <div class="cutflow-actions"><UiButton :disabled="busy || !status.current || !store.pending.target || !!store.pending.jobId" @click="importPending">{{ store.pending.jobId ? '导入中…' : '确认导入此工程' }}</UiButton><UiButton variant="secondary" @click="refresh(true)">改用当前工程</UiButton><UiButton variant="ghost" :disabled="!!store.pending.jobId" @click="store.savePending(null)">移出待导入列表</UiButton></div>
    </section>
    <p v-if="message" class="cutflow-notice" role="status">{{ message }}</p>
    <section class="cutflow-card"><h2>最近工程</h2><p v-if="!recents.length">还没有最近工程。从剪辑窗口创建第一个工程即可。</p><button v-for="project in recents" :key="project.path" class="recent-project" :disabled="busy" @click="launch(project.path)"><strong>{{ project.name }}</strong><span>{{ project.path }}</span><UIcon name="i-tabler-arrow-up-right" aria-hidden="true" /></button></section>
    <details class="cutflow-card"><summary>进阶 · AI 流程化剪辑</summary><p>MCP 和命令行保留实时编辑、草稿、字幕与配音接口。剪辑器独立运行；退出外部 AI 不会关闭软件。接口说明见安装目录的「创作与外部AI.md」。</p></details>
  </section>
</template>
<style src="./CutflowPages.css"></style>
