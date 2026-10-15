<script setup lang="ts">
import { computed, onMounted, onBeforeUnmount, ref, watch } from 'vue';
import type { TaskActionKind, TransferTaskView } from '../../../shared/stores/transferView';
import MediaListRow from '../../../shared/ui/MediaListRow.vue';
import UiCheckbox from '../../../shared/ui/Checkbox.vue';
import UiIconButton from '../../../shared/ui/IconButton.vue';
import UiProgressBar from '../../../shared/ui/ProgressBar.vue';
import UiStatusBadge from '../../../shared/ui/StatusBadge.vue';
import TaskActionMenu from '../../../shared/ui/TaskActionMenu.vue';
import { calculateVirtualWindow } from '../../../shared/utils/virtualWindow';

const ROW_STRIDE = 104;
const props = withDefaults(
  defineProps<{
    views: TransferTaskView[];
    selectedTaskId: string | null;
    selectedTaskIds: string[];
    loading?: boolean;
    mode?: 'transfer' | 'completed';
  }>(),
  { loading: false, mode: 'transfer' },
);
const emit = defineEmits<{
  inspectTask: [taskId: string];
  toggleTaskSelection: [taskId: string];
  toggleVisibleSelection: [taskIds: string[], selected: boolean];
  taskAction: [taskId: string, action: Exclude<TaskActionKind, 'none'>];
  openContextMenu: [taskId: string, event: MouseEvent];
  visibleTasks: [taskIds: string[]];
}>();
const visibleIds = computed(() => props.views.map((view) => view.id));
const selected = computed(() => new Set(props.selectedTaskIds));
const selection = computed<boolean | 'indeterminate'>(() => {
  const count = visibleIds.value.filter((id) => selected.value.has(id)).length;
  return count === 0 ? false : count === visibleIds.value.length ? true : 'indeterminate';
});
const scrollOffset = ref(0);
const viewportSize = ref(600);
const scrollElement = ref<HTMLElement | null>(null);
let resizeObserver: ResizeObserver | undefined;
onMounted(() => {
  if (!scrollElement.value || typeof ResizeObserver === 'undefined') return;
  resizeObserver = new ResizeObserver(() => {
    if (scrollElement.value) viewportSize.value = scrollElement.value.clientHeight;
  });
  resizeObserver.observe(scrollElement.value);
});
onBeforeUnmount(() => resizeObserver?.disconnect());
const taskWindow = computed(() =>
  calculateVirtualWindow(props.views.length, scrollOffset.value, viewportSize.value, ROW_STRIDE),
);
const renderedViews = computed(() =>
  props.views.slice(taskWindow.value.start, taskWindow.value.end).map((view, offset) => ({
    view,
    index: taskWindow.value.start + offset,
  })),
);
const updateViewport = (event: Event) => {
  const element = event.currentTarget as HTMLElement;
  scrollOffset.value = element.scrollTop;
  viewportSize.value = element.clientHeight;
};
watch(
  () => renderedViews.value.map(({ view }) => `${view.id}:${view.isCompleted}`).join('\n'),
  () =>
    emit(
      'visibleTasks',
      renderedViews.value.map(({ view }) => view.id),
    ),
  { immediate: true },
);
const canPlay = (view: TransferTaskView) =>
  view.primaryAction === 'open_file' || (!view.isCompleted && Boolean(view.sourceUrl));
const play = (view: TransferTaskView) => emit('taskAction', view.id, view.isCompleted ? 'open_file' : 'open_source');
</script>

<template>
  <section class="transfer-table" aria-label="传输任务">
    <div class="transfer-selection-bar">
      <UiCheckbox
        :model-value="selection"
        label="选择当前筛选任务"
        :disabled="!views.length || loading"
        @update:model-value="emit('toggleVisibleSelection', visibleIds, selection !== true)"
      />
      <span>{{ views.length }} 项</span>
    </div>
    <div ref="scrollElement" class="transfer-scroll" @scroll="updateViewport">
      <div class="virtual-task-list" role="list" aria-label="传输任务" :style="{ height: `${taskWindow.totalSize}px` }">
        <MediaListRow
          v-for="row in renderedViews"
          :key="row.view.id"
          class="task-table-row"
          :class="{ inspected: selectedTaskId === row.view.id }"
          :style="{ transform: `translateY(${row.index * ROW_STRIDE}px)` }"
          :aria-posinset="row.index + 1"
          :aria-setsize="views.length"
          :title="row.view.displayTitle"
          :cover="row.view.coverUrl"
          secondary-icon="i-tabler-video"
          :secondary="row.view.mediaLabel || row.view.subtitle || '下载任务'"
          :duration="row.view.durationSeconds"
          :size-label="row.view.sizeLabel"
          :selected="selected.has(row.view.id)"
          :disabled="loading"
          :playable="canPlay(row.view)"
          :playback-label="
            row.view.isCompleted ? `播放本地文件 ${row.view.displayTitle}` : `播放 ${row.view.displayTitle}`
          "
          @toggle="emit('toggleTaskSelection', row.view.id)"
          @play="play(row.view)"
          @contextmenu.prevent="emit('openContextMenu', row.view.id, $event)"
        >
          <template #detail>
            <div class="task-detail-line">
              <UiStatusBadge :status="row.view.statusBadge">{{ row.view.statusLabel }}</UiStatusBadge>
              <template v-if="!row.view.isCompleted">
                <UiProgressBar :value="row.view.progressValue" />
                <span>{{ row.view.progressLabel }}</span>
                <span v-if="row.view.speedLabel !== '--'">{{ row.view.speedLabel }}</span>
                <span v-if="row.view.etaLabel !== '--'">剩余 {{ row.view.etaLabel }}</span>
              </template>
              <span v-else class="task-location" :title="row.view.fullLocation">{{ row.view.fullLocation }}</span>
              <span v-if="row.view.issueLabel !== '-'" class="task-issue" :title="row.view.issueLabel">{{
                row.view.issueLabel
              }}</span>
            </div>
          </template>
          <template #actions>
            <UiIconButton
              v-if="row.view.primaryAction !== 'none' && row.view.primaryAction !== 'open_file'"
              :icon="row.view.primaryActionIcon"
              :label="row.view.primaryActionLabel"
              variant="ghost"
              size="compact"
              :disabled="loading"
              @click="emit('taskAction', row.view.id, row.view.primaryAction)"
            />
            <TaskActionMenu
              :view="row.view"
              :disabled="loading"
              :show-primary="false"
              show-inspect
              @inspect="emit('inspectTask', row.view.id)"
              @action="(action) => emit('taskAction', row.view.id, action)"
            />
          </template>
        </MediaListRow>
      </div>
    </div>
  </section>
</template>

<style scoped>
.transfer-table {
  min-width: 0;
  min-height: 0;
  height: 100%;
  display: flex;
  flex-direction: column;
}

.transfer-selection-bar {
  flex-shrink: 0;
  display: flex;
  align-items: center;
  justify-content: space-between;
  min-height: 32px;
  padding: 0 14px 8px;
  color: var(--color-muted);
  font-size: var(--font-12);
}

.transfer-scroll {
  flex: 1;
  min-height: 0;
  overflow: auto;
  scrollbar-gutter: stable;
  padding-right: 4px;
}

.virtual-task-list {
  position: relative;
  min-width: 0;
}

.task-table-row {
  position: absolute;
  inset: 0 0 auto;
  height: 96px;
}

.task-table-row.inspected {
  outline: 1px solid var(--color-accent);
  outline-offset: -1px;
}

.task-detail-line {
  min-width: 0;
  display: flex;
  align-items: center;
  gap: 8px;
  font-size: var(--font-11);
  color: var(--color-muted);
  white-space: nowrap;
}

.task-detail-line :deep(.progress-track) {
  width: 60px;
  min-width: 30px;
  height: 4px;
}

.task-location,
.task-issue {
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
}

.task-issue {
  color: var(--color-danger);
}
</style>
