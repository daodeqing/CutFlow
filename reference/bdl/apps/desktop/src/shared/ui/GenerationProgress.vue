<script setup lang="ts">
import { computed } from 'vue';
import type { Job } from '../api/cutflow';
import UiProgressBar from './ProgressBar.vue';

const { job, stopping = false } = defineProps<{ job: Job; stopping?: boolean }>();
defineEmits<{ stop: [] }>();

const downloading = computed(() => /^downloading(?::|$)/.test(job.stage));
const percent = computed(() => {
  const value = job.progress?.percent;
  if (typeof value !== 'number' || !Number.isFinite(value)) return undefined;
  return Math.min(job.status === 'completed' ? 100 : 99, Math.max(0, Math.round(value)));
});
const stage = computed(() => {
  const value = job.progress?.stage || job.stage;
  if (stopping) return '正在停止，请等待当前操作结束';
  if (value === 'queued') return '正在排队';
  if (value === 'generating') return '正在生成';
  if (value === 'downloading') return '正在保存作品';
  return value || '正在等待服务响应';
});
</script>

<template>
  <div class="generation-progress" :class="{ 'is-stopping': stopping }" aria-live="polite" role="status">
    <div class="generation-progress-heading">
      <span class="generation-spinner" aria-hidden="true"></span>
      <span class="generation-stage">{{ stage }}</span>
      <strong v-if="percent !== undefined" class="generation-percent">阶段进度 {{ percent }}%</strong>
      <span v-else class="generation-indeterminate-label">等待进度</span>
      <button type="button" :disabled="stopping" @click="$emit('stop')">
        {{ stopping ? '正在停止…' : downloading ? '停止下载' : '停止生成' }}
      </button>
    </div>
    <UiProgressBar
      v-if="percent !== undefined"
      :value="percent"
      :status="stopping ? 'paused' : 'downloading'"
      :label="downloading ? '作品下载阶段进度' : '作品生成阶段进度'"
    />
    <div v-else class="generation-indeterminate" role="progressbar" :aria-label="downloading ? '作品下载中，尚未返回百分比' : '作品生成中，服务尚未返回百分比'">
      <span></span>
    </div>
    <small>{{ downloading ? '正在保存已有结果，不会重新生成或重复计费。' : '按生成、放大与保存等阶段更新，阶段耗时不相等。' }}</small>
  </div>
</template>

<style scoped>
.generation-progress {
  display: grid;
  gap: 9px;
  margin-top: 12px;
  padding: 13px 15px;
  border: 1px solid var(--color-border);
  border-radius: 12px;
  background: var(--color-accent-faint);
  color: var(--color-muted);
  font-size: 12px;
  line-height: 1.7;
}

.generation-progress-heading {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 8px;
}

.generation-stage {
  flex: 1;
  min-width: 100px;
  overflow-wrap: anywhere;
  color: var(--color-text);
}

.generation-percent {
  color: var(--color-accent-strong);
  font-size: 12px;
  font-variant-numeric: tabular-nums;
  white-space: nowrap;
}

.generation-progress button {
  min-height: 28px;
  color: var(--color-danger);
  white-space: nowrap;
}

.generation-progress button:disabled {
  cursor: wait;
  opacity: 0.65;
}

.generation-spinner {
  width: 13px;
  height: 13px;
  flex-shrink: 0;
  border: 2px solid var(--color-border);
  border-top-color: var(--color-accent);
  border-radius: 50%;
  animation: creation-progress-spin 900ms linear infinite;
}

.generation-indeterminate {
  height: 6px;
  overflow: hidden;
  border-radius: 999px;
  background: var(--color-progress-track);
}

.generation-indeterminate > span {
  display: block;
  width: 32%;
  height: 100%;
  border-radius: inherit;
  background: var(--color-accent);
  animation: creation-progress-pending 1.8s ease-in-out infinite;
}

.generation-progress small {
  font-size: 10px;
}

.is-stopping .generation-spinner,
.is-stopping .generation-indeterminate > span {
  animation-play-state: paused;
}

@keyframes creation-progress-spin {
  to { transform: rotate(360deg); }
}

@keyframes creation-progress-pending {
  from { transform: translateX(-110%); }
  to { transform: translateX(320%); }
}

@media (prefers-reduced-motion: reduce) {
  .generation-spinner,
  .generation-indeterminate > span { animation: none; }
}
</style>
