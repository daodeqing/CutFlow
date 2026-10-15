<script setup lang="ts">
import UiButton from '../../shared/ui/Button.vue';
import UiInlineNotice from '../../shared/ui/InlineNotice.vue';

import ParseDownloadPlanner from '../../shared/features/parse/ParseDownloadPlanner.vue';
import ParseBatchWorkspace from '../components/parse/ParseBatchWorkspace.vue';
import ParseResultWorkspace from '../components/parse/ParseResultWorkspace.vue';
import { useParsePage } from '../../shared/features/parse/useParsePage';
const {
  parse,
  inputMode,
  singleInput,
  createLoading,
  activeStage,
  submitInput,
  switchInputMode,
  pasteSingleInput,
  pasteInput,
  openDownloadSettings,
  runNoticeAction,
} = useParsePage();
</script>
<template>
  <section class="mobile-page mobile-parse-page">
    <UiInlineNotice
      v-if="parse.notice"
      :tone="parse.notice.tone"
      :action-label="parse.notice.actionLabel"
      @action="runNoticeAction"
      >{{ parse.notice.message }}</UiInlineNotice
    >
    <template v-if="activeStage === 'source'">
      <form class="mobile-parse-form" @submit.prevent="submitInput">
        <UiButton class="parse-paste" variant="ghost" :disabled="createLoading" @click="pasteInput"
          ><UIcon name="i-tabler-clipboard" />粘贴</UiButton
        >
        <label class="parse-behavior"
          ><span>解析范围</span
          ><select
            aria-label="解析范围"
            :value="inputMode"
            :disabled="createLoading"
            @change="switchInputMode(($event.target as HTMLSelectElement).value as 'single' | 'batch')"
          >
            <option value="batch">视频及所属合集</option>
            <option value="single">仅当前视频</option>
          </select></label
        >
        <textarea
          v-if="inputMode === 'batch'"
          id="mobile-source-input"
          v-model="parse.input"
          aria-label="Bilibili 链接或 BV / AV"
          placeholder="链接 / BV / AV（多条分行）"
          :disabled="createLoading"
          rows="3"
        ></textarea>
        <textarea
          v-else
          id="mobile-source-input"
          v-model="singleInput"
          aria-label="视频链接或 BV / AV"
          placeholder="视频链接 / BV / AV"
          :disabled="createLoading"
          rows="3"
          @paste.prevent="pasteSingleInput"
        ></textarea>
        <UiButton class="parse-primary" type="submit" :disabled="createLoading"
          >{{ createLoading ? '正在解析…' : '开始解析' }}<UIcon name="i-tabler-arrow-right"
        /></UiButton>
      </form>
    </template>
    <template v-else
      ><ParseBatchWorkspace v-if="parse.isBatch" embedded @download="openDownloadSettings" /><ParseResultWorkspace
        v-else
        embedded
        @download="openDownloadSettings"
    /></template>
    <ParseDownloadPlanner ref="download-planner" />
  </section>
</template>
<style scoped>
.mobile-parse-page {
  overflow-y: auto;
}

.mobile-parse-page:has(.mobile-result-workspace) {
  overflow: hidden;
}

.mobile-parse-page:has(.mobile-parse-form) {
  justify-content: flex-start;
  align-items: center;
}

.mobile-parse-form {
  display: grid;
  grid-template-columns: auto minmax(0, 1fr);
  align-items: center;
  gap: 0.75rem;
  width: 100%;
  margin-block: auto;
  flex-shrink: 0;
}

.parse-paste {
  min-height: 2.75rem;
}

.parse-paste :deep(.iconify),
.parse-primary :deep(.iconify) {
  width: 1.25rem;
  height: 1.25rem;
  flex-shrink: 0;
}

textarea {
  grid-column: 1 / -1;
  width: 100%;
  min-height: 7.5rem;
  min-width: 0;
  resize: vertical;
  border: 0.0625rem solid var(--color-border);
  border-radius: 0.75rem;
  background: var(--color-panel);
  color: var(--color-text);
  padding: 0.75rem;
  font-size: var(--mobile-parse-font, 0.875rem);
  line-height: 1.65;
}

textarea:focus {
  outline: 0.125rem solid var(--color-accent);
  outline-offset: 0.125rem;
}

.parse-behavior {
  display: flex;
  align-items: center;
  justify-content: flex-end;
  gap: 0.5rem;
  color: var(--color-muted);
  font-size: var(--mobile-parse-font, 0.875rem);
}

.parse-behavior span {
  display: none;
}

select {
  border: 0.0625rem solid var(--color-border);
  border-radius: 0.5rem;
  min-height: 2.75rem;
  max-width: 100%;
  padding-inline: 0.5rem;
  background: var(--color-surface);
  color: var(--color-text);
}

.parse-primary {
  grid-column: 1 / -1;
  width: 100%;
  min-height: var(--mobile-parse-primary-height, 3.125rem);
  border-radius: 0.875rem;
  justify-content: center;
  gap: 0.5rem;
  padding-inline: 1.125rem;
  font-size: var(--mobile-parse-primary-font, 1rem);
}

.mobile-layout-rail .mobile-parse-form {
  width: 75%;
}

.mobile-layout-rail .parse-behavior span {
  display: inline;
  white-space: nowrap;
}

.mobile-layout-rail textarea {
  min-height: 8rem;
}

.mobile-layout-rail .parse-paste,
.mobile-layout-rail select,
.mobile-layout-rail .parse-primary {
  min-height: 2.5rem;
  height: 2.5rem;
  font-size: var(--mobile-parse-font, 0.875rem);
}

.mobile-layout-rail .parse-primary {
  min-height: 2.75rem;
  height: 2.75rem;
  padding-inline: 0.75rem;
}
</style>
