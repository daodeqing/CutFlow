<script setup lang="ts">
import { computed } from 'vue';
import SourceMediaList from '../parse/SourceMediaList.vue';
import UiButton from '../../../shared/ui/Button.vue'
import UiEmptyState from '../../../shared/ui/EmptyState.vue'
import UiInlineNotice from '../../../shared/ui/InlineNotice.vue'
import UiPagination from '../../../shared/ui/Pagination.vue'
import SelectionActionBar from '../../../shared/ui/SelectionActionBar.vue'
import ExternalLinkButton from '../../../shared/ui/ExternalLinkButton.vue'
import SourceParseControls from '../parse/SourceParseControls.vue'
import SourceLoadStatus from '../parse/SourceLoadStatus.vue'
import { useLibraryFolderDetail } from "../../../shared/features/library/useLibraryFolderDetail";
import type { AccountLibraryFolder } from "../../../shared/api/dto";
const props = defineProps<{folder: AccountLibraryFolder; loadingInitial?: boolean}>();
const emit = defineEmits<{back: []; download: []}>();
const { parse, pageSize, currentPage, loadBatchSize, source, sourceId, items, totalCount, totalPages, pageItems, selectedCount, sourceRequestLoading, pacedParsing, pacedWaiting, pacedStopping, loading, activeError, hasMore, itemOwnerName, toggleItem, toggleCurrentPageSelection, clearSelection, downloadSelected, parseMore, parseAll, stopParsing, downloadAll, goToPage } = useLibraryFolderDetail(props, () => emit("download"));
const rows = computed(() => pageItems.value.map((item) => ({ id: item.id, title: item.title, meta: itemOwnerName(item), partIds: item.parts.map((part) => part.id) })));
</script>
<template>
  <section class="flex min-h-0 flex-1 flex-col gap-4" :aria-busy="loading">
    <header
      class="library-folder-header flex min-w-0 items-center justify-between gap-4 border-b border-(--color-border) pb-4"
    >
      <div class="flex min-w-0 items-center gap-3">
        <button
          type="button"
          class="grid size-8 shrink-0 place-items-center rounded-md text-(--color-muted) hover:bg-(--color-panel) hover:text-(--color-text)"
          aria-label="返回内容集合"
          @click="emit('back')"
        >
          <UIcon name="i-tabler-arrow-left" class="size-4" aria-hidden="true" />
        </button>
        <h2 class="truncate m-0 text-base text-(--color-text)" :title="folder.title">
          <ExternalLinkButton
            :href="folder.source_url"
            :label="`在 Bilibili 打开 ${folder.title}`"
            :show-icon="false"
          >
            <span class="truncate text-(--color-text)">{{ folder.title }}</span>
          </ExternalLinkButton>
        </h2>
      </div>
      <div v-if="!loadingInitial" class="source-function-toolbar">
        <SourceLoadStatus :loaded="source?.source.loaded_count ?? items.length" :total="totalCount" />
        <SourceParseControls
          :source-id="sourceId ?? undefined"
          v-if="hasMore || pacedParsing"
          v-model:batch-size="loadBatchSize"
          :has-more="hasMore"
          :loading="sourceRequestLoading"
          :parsing-all="pacedParsing"
          :waiting="pacedWaiting"
          :stopping="pacedStopping"
          @parse-batch="parseMore"
          @parse-all="parseAll"
          @parse-and-download="sourceId && parse.startBackgroundDownload(sourceId)"
          @stop="stopParsing"
        />
        <UiButton
          v-if="!hasMore"
          size="compact"
          variant="secondary"
          :disabled="loading || totalCount === 0"
          @click="downloadAll"
        >
          下载全部
        </UiButton>
        <UiButton size="compact" :disabled="loading || selectedCount === 0" @click="downloadSelected">
          下载所选 ({{ selectedCount }})
        </UiButton>
      </div>
    </header>

    <UiInlineNotice v-if="parse.notice" :tone="parse.notice.tone">{{ parse.notice.message }}</UiInlineNotice>
    <UiInlineNotice v-if="activeError" tone="danger">{{ activeError }}</UiInlineNotice>

    <div v-if="loadingInitial" class="library-detail-skeleton" role="status" aria-label="正在加载合集内容">
      <div v-for="index in 6" :key="index" class="library-detail-skeleton-row" aria-hidden="true"><span></span><i></i></div>
    </div>
    <SourceMediaList
      v-else-if="source && pageItems.length"
      class="min-h-0 flex-1"
      label="集合内容"
      selection-label="全选本页"
      :source="source"
      :rows="rows"
      :fallback-owner="folder.owner_name"
      :selected-ids="parse.activeSelection"
      :disabled="loading"
      @toggle="(id) => { const item = pageItems.find((item) => item.id === id); if (item) toggleItem(item); }"
      @toggle-all="toggleCurrentPageSelection"
    />

    <UiEmptyState
      v-else-if="!parse.notice && !activeError"
      title="这个集合暂时没有内容"
      icon="i-tabler-folder-open"
      layout="stacked"
      compact
      embedded
    />

    <SelectionActionBar v-if="!loadingInitial" :selected-count="selectedCount" :total-count="totalCount">
      <template #leading>
        <div class="library-pagination-status">
          <span class="library-selection-count">已选 {{ selectedCount }} / {{ totalCount }}</span>
          <UiPagination
            :page="currentPage"
            :total="totalCount"
            :items-per-page="pageSize"
            :disabled="loading"
            label="集合内容分页"
            @update:page="goToPage"
          />
          <span>第 {{ currentPage }} / {{ totalPages }} 页</span>
        </div>
      </template>
      <template #selection>
        <UiButton
          v-if="selectedCount > 0"
          size="compact"
          variant="ghost"
          :disabled="loading"
          @click="clearSelection"
        >
          取消选择
        </UiButton>
      </template>
    </SelectionActionBar>
  </section>
</template>

<style scoped>
.library-pagination-status {
  min-height: 28px;
  display: inline-flex;
  flex-wrap: wrap;
  align-items: center;
  gap: var(--space-xs);
  color: var(--color-muted);
  font-size: var(--font-12);
  line-height: 1;
  white-space: nowrap;
}

.library-selection-count {
  color: var(--color-text);
  font-weight: 650;
  font-variant-numeric: tabular-nums;
}

.source-function-toolbar {
  min-width: 0;
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  justify-content: flex-end;
  gap: var(--space-xs);
}

.library-detail-skeleton {
  min-height: 0;
  flex: 1;
  display: flex;
  flex-direction: column;
  gap: 8px;
  overflow: hidden;
}

.library-detail-skeleton-row {
  height: 96px;
  flex-shrink: 0;
  display: flex;
  align-items: center;
  gap: 16px;
  padding: 12px 48px;
  border: 1px solid var(--color-border);
  border-radius: var(--radius-8);
}

.library-detail-skeleton-row span {
  width: 124px;
  height: 70px;
  border-radius: var(--radius-6);
  background: var(--color-panel);
}

.library-detail-skeleton-row i {
  width: 40%;
  height: 14px;
  border-radius: var(--radius-4);
  background: var(--color-panel);
}

@media (width <= 900px) {
  .library-folder-header { flex-wrap: wrap; }
}
</style>
