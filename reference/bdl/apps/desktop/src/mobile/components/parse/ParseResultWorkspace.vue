<script setup lang="ts">
import UiButton from '../../../shared/ui/Button.vue';
import UiEmptyState from '../../../shared/ui/EmptyState.vue';
import UiInlineNotice from '../../../shared/ui/InlineNotice.vue';

import ParseResultListMobile from './ParseResultListMobile.vue';
import { computed, ref } from 'vue';
import MobileListActions from '../MobileListActions.vue';
import MobileListSearch from '../MobileListSearch.vue';
import UiCheckbox from '../../../shared/ui/Checkbox.vue';
import { useMobileListSelection } from '../useMobileListSelection';

import MobileSourceMenu from '../MobileSourceMenu.vue';
import { useParseResultWorkspace } from '../../../shared/features/parse/useParseResultWorkspace';
const { embedded = false } = defineProps<{ embedded?: boolean }>();
const emit = defineEmits<{ download: [] }>();
const {
  downloadAllLoaded,
  loadBatchSize,
  activeSource,
  selectedIds,
  selectedCount,
  tableRows,
  sourceRequestLoading,
  pacedParsing,

  pacedStopping,
  activeLoading,
  activeError,
  hasMore,
  allRowsSelected,
  canCreateTasks,
  toggleAllResults,
  clearSelection,
  loadMore,
  parseAll,
  stopParsing,
  parseAndDownload,
  returnToSource,
  toggleNode,
} = useParseResultWorkspace(() => emit('download'));
const query = ref('');
const searchOpen = ref(false);
const toolsOpen = ref(false);
const { managing, toggleManaging, select } = useMobileListSelection(clearSelection);
const visibleRows = computed(() => {
  const term = query.value.trim().toLocaleLowerCase();
  return term
    ? tableRows.value.filter((row) => `${row.title} ${row.meta ?? ''}`.toLocaleLowerCase().includes(term))
    : tableRows.value;
});
</script>
<template>
  <section v-if="activeSource" class="mobile-result-workspace" :class="{ embedded }">
    <header class="mobile-detail-heading">
      <button
        class="mobile-icon-button"
        type="button"
        aria-label="返回解析首页"
        :disabled="activeLoading"
        @click="returnToSource"
      >
        <UIcon name="i-tabler-arrow-left" />
      </button>
      <div class="mobile-detail-copy">
        <h2 :title="activeSource.source.title">{{ activeSource.source.title }}</h2>
        <p>
          已加载 {{ activeSource.source.loaded_count
          }}<template v-if="activeSource.source.total_count"> / {{ activeSource.source.total_count }}</template> 项
        </p>
      </div>
      <MobileListActions
        v-model:search-open="searchOpen"
        search-label="搜索视频"
        options-label="内容操作"
        manage-label="管理视频"
        :managing="managing"
        @options="toolsOpen = true"
        @manage="toggleManaging"
      />
    </header>
    <MobileListSearch v-model="query" v-model:open="searchOpen" label="筛选视频" placeholder="筛选标题或作者" />
    <MobileSourceMenu
      v-model:open="toolsOpen"
      v-model:batch-size="loadBatchSize"
      :has-more="hasMore"
      :busy="sourceRequestLoading"
      :parsing="pacedParsing"
      :stopping="pacedStopping"
      @load="loadMore"
      @all="parseAll"
      @download-all="hasMore ? parseAndDownload() : downloadAllLoaded()"
      @stop="stopParsing"
    />
    <ParseResultListMobile
      v-if="visibleRows.length"
      :rows="visibleRows"
      :source="activeSource"
      :managing="managing"
      :selected-ids="selectedIds"
      :disabled="activeLoading"
      @toggle="(id) => select(() => toggleNode(id))"
    >
      <button
        v-if="hasMore || pacedParsing"
        type="button"
        class="mobile-load-more"
        :disabled="activeLoading"
        @click="loadMore"
      >
        {{ pacedParsing ? '正在加载更多内容…' : sourceRequestLoading ? '加载中…' : '继续加载内容' }}
      </button>
    </ParseResultListMobile>
    <UiEmptyState
      v-else
      :title="query ? '没有匹配的视频' : '没有可选择内容'"
      :description="query ? '修改关键词，或关闭搜索以清除筛选。' : '当前来源没有可下载的视频或分集。'"
      icon="i-tabler-folder-open"
      compact
      embedded
    />
    <UiInlineNotice v-if="activeError" tone="danger">{{ activeError }}</UiInlineNotice>
    <footer class="mobile-download-footer mobile-download-bar">
      <UiCheckbox
        v-if="managing"
        :model-value="allRowsSelected ? true : selectedCount > 0 ? 'indeterminate' : false"
        label="全选已加载"
        :disabled="activeLoading"
        @update:model-value="toggleAllResults"
      />
      <span
        >已选 <strong>{{ selectedCount }}</strong> 项</span
      ><UiButton size="compact" :disabled="!canCreateTasks" @click="emit('download')"
        >下载所选 ({{ selectedCount }})</UiButton
      >
    </footer>
  </section>
</template>
<style scoped>
.mobile-result-workspace {
  min-height: 0;
  display: flex;
  flex: 1;
  flex-direction: column;
  gap: 0.375rem;
  overflow: visible;
}
</style>
