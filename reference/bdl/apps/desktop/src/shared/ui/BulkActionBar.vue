<script setup lang="ts">
import { computed } from 'vue'

import UiButton from './Button.vue'

const {
  selectedCount,
  completedCount,
  canPause,
  canCancel,
  canResume,
  canRetry,
  canRefreshRetry,
  canRemove,
  loading = false,
} = defineProps<{
  selectedCount: number
  completedCount: number
  canPause: boolean
  canCancel: boolean
  canResume: boolean
  canRetry: boolean
  canRefreshRetry: boolean
  canRemove: boolean
  loading?: boolean
}>()

const emit = defineEmits<{
  pause: []
  cancel: []
  resume: []
  retry: []
  refreshRetry: []
  remove: []
  deleteFiles: []
  clearCompleted: []
  refresh: []
}>()

const moreItems = computed(() => [
  [
    {
      label: '暂停全部',
      icon: 'i-tabler-player-pause',
      disabled: loading || !canPause,
      onSelect: () => emit('pause'),
    },
    {
      label: '取消全部',
      icon: 'i-tabler-x',
      disabled: loading || !canCancel,
      onSelect: () => emit('cancel'),
    },
    {
      label: '继续全部',
      icon: 'i-tabler-player-play',
      disabled: loading || !canResume,
      onSelect: () => emit('resume'),
    },
  ],
])
</script>

<template>
  <div class="bulk-action-bar" :class="{ selected: selectedCount > 0 }">
    <div v-if="selectedCount > 0" class="bulk-copy">
      <strong>已选择 {{ selectedCount }} 个</strong>
    </div>

    <div v-if="selectedCount > 0" class="bulk-actions">
      <UiButton v-if="canPause" size="compact" variant="secondary" :disabled="loading" @click="emit('pause')"
        >暂停</UiButton
      >
      <UiButton v-if="canCancel" size="compact" variant="secondary" :disabled="loading" @click="emit('cancel')"
        >取消</UiButton
      >
      <UiButton v-if="canResume" size="compact" variant="secondary" :disabled="loading" @click="emit('resume')"
        >继续</UiButton
      >
      <UiButton v-if="canRetry" size="compact" variant="secondary" :disabled="loading" @click="emit('retry')"
        >重试</UiButton
      >
      <UiButton
        v-if="canRefreshRetry"
        size="compact"
        variant="secondary"
        :disabled="loading"
        @click="emit('refreshRetry')"
      >
        刷新链接并重试
      </UiButton>
      <UiButton v-if="canRemove" size="compact" variant="secondary" title="移除任务记录并清理临时文件，保留成品" :disabled="loading" @click="emit('remove')"
        >移除任务</UiButton
      >
      <UiButton v-if="canRemove" size="compact" variant="danger" :disabled="loading" @click="emit('deleteFiles')">删除任务及文件</UiButton>
    </div>

    <div v-else class="bulk-actions">
      <UiButton
        size="compact"
        variant="secondary"
        title="删除已完成的任务记录，保留下载文件"
        :disabled="loading || completedCount === 0"
        @click="emit('clearCompleted')"
      >
        移除已完成记录
      </UiButton>
      <UiButton size="compact" variant="secondary" :disabled="loading" @click="emit('refresh')">刷新</UiButton>
      <UDropdownMenu
        :items="moreItems"
        :content="{ align: 'end', sideOffset: 4, collisionPadding: 12 }"
        :ui="{ content: 'min-w-32' }"
      >
        <UiButton size="compact" variant="secondary" :disabled="loading">更多</UiButton>
      </UDropdownMenu>
    </div>
  </div>
</template>

<style scoped>
.bulk-action-bar {
  min-width: 0;
  min-height: 2rem;
  display: flex;
  align-items: center;
  justify-content: flex-end;
  gap: var(--space-12);
}

.bulk-action-bar.selected {
  min-height: 2.5rem;
  justify-content: space-between;
  padding: var(--space-8) var(--space-12);
  border: 0.0625rem solid var(--color-border);
  border-radius: var(--radius-8);
  border-color: color-mix(in oklab, var(--color-accent) 38%, var(--color-border));
  background: var(--color-selected-surface);
}

.bulk-copy {
  min-width: 0;
  display: grid;
  gap: 0.125rem;
}

.bulk-copy strong {
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  color: var(--color-text);
  font-size: var(--font-13);
}

.bulk-actions {
  display: flex;
  flex-wrap: wrap;
  justify-content: flex-end;
  gap: var(--space-4);
}

@media (width <= 1180px) {
  .bulk-action-bar.selected {
    align-items: flex-start;
    flex-direction: column;
  }

  .bulk-actions {
    justify-content: flex-start;
  }
}
</style>
