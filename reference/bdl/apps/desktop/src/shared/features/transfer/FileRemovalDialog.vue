<script setup lang="ts">
import UiDialog from '../../ui/Dialog.vue';
import UiButton from '../../ui/Button.vue';
import UiInlineNotice from '../../ui/InlineNotice.vue';
import type { FileRemovalController } from './useFileRemoval';
const { controller } = defineProps<{ controller: FileRemovalController }>();
const { kind, open, busy, executing, preview, error, errorDetails, copied, copyNotice, copyError, canConfirm, confirm, refresh } = controller;
const bytes = (value: number) => value >= 1024 ** 3 ? `${(value / 1024 ** 3).toFixed(1)} GB` : value >= 1024 ** 2 ? `${(value / 1024 ** 2).toFixed(1)} MB` : `${Math.ceil(value / 1024)} KB`;
</script>
<template>
  <UiDialog v-model="open" :title="kind === 'tasks' ? '删除任务及文件' : '清理临时文件'" :dismissible="!busy" :show-close="!busy">
    <p v-if="busy" role="status">{{ executing ? '正在停止任务并清理文件…' : '正在核对文件…' }}</p>
    <template v-if="preview">
      <p v-if="kind === 'tasks'">移除这 {{ preview.task_ids.length }} 个任务，删除下列成品、附件和下载临时文件。此操作无法撤销。</p>
      <p v-else>只清理扫描目录中未被任务使用的下载临时文件；排队、暂停和下载中的任务文件会保留。</p>
      <strong>{{ preview.file_count }} 个{{ kind === 'tasks' ? '成品或附件' : '临时文件' }} · {{ bytes(preview.total_bytes) }}</strong>
      <details v-if="preview.files.length" class="removal-details">
        <summary>查看将删除的文件</summary>
        <ul><li v-for="file in preview.files" :key="file">{{ file }}</li></ul>
      </details>
      <details v-if="preview.preserved.length" class="removal-details">
        <summary>查看保留的文件 / 限制</summary>
        <ul><li v-for="file in preview.preserved" :key="file">{{ file }}</li></ul>
      </details>
      <details v-if="kind === 'temp'" class="removal-details">
        <summary>扫描目录</summary>
        <ul><li v-for="root in preview.roots" :key="root">{{ root }}</li></ul>
        <p>历史下载目录若未列在这里，请在文件管理器中检查。</p>
      </details>
    </template>
    <UiInlineNotice v-if="error" tone="danger">{{ error }}{{ kind === 'tasks' ? ' 未移除的任务记录仍保留；部分文件可能已清理。' : '' }}</UiInlineNotice>
    <details v-if="errorDetails" class="removal-details">
      <summary>错误详情</summary>
      <ul><li>{{ errorDetails }}</li></ul>
    </details>
    <UiInlineNotice v-if="copyNotice" tone="danger">{{ copyNotice }}</UiInlineNotice>
    <template #footer>
      <UiButton v-if="error" variant="secondary" @click="copyError">{{ copied ? '已复制' : '复制错误信息' }}</UiButton>
      <UiButton variant="secondary" :disabled="busy" @click="open = false">取消</UiButton>
      <UiButton v-if="!canConfirm" variant="secondary" :disabled="busy" @click="refresh">重新查看文件</UiButton>
      <UiButton variant="danger" :disabled="busy || !canConfirm || (kind === 'temp' && !preview?.file_count)" @click="confirm">{{ kind === 'tasks' ? '确认删除任务及文件' : '确认清理' }}</UiButton>
    </template>
  </UiDialog>
</template>
<style scoped>
.removal-details {
  min-width: 0;
  font-size: var(--font-13);
}

.removal-details summary {
  cursor: pointer;
  color: var(--color-muted);
}

.removal-details ul {
  padding-left: var(--space-20);
  margin-top: var(--space-8);
}

.removal-details li {
  overflow-wrap: anywhere;
  margin-bottom: var(--space-6);
}
</style>
