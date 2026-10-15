<script setup lang="ts">
import { ref, watch } from 'vue';
import UiCheckbox from './Checkbox.vue';
import { coverUrl } from '../utils/coverUrl';
import { formatDuration } from '../utils/duration';

const props = defineProps<{
  title: string;
  cover?: string | null;
  secondary?: string | null;
  secondaryIcon?: string;
  published?: string | null;
  duration?: number | null;
  sizeLabel?: string | null;
  selected?: boolean;
  disabled?: boolean;
  playable?: boolean;
  playbackLabel?: string;
}>();
const emit = defineEmits<{ toggle: []; play: [] }>();
const failed = ref(false);
watch(
  () => props.cover,
  () => {
    failed.value = false;
  },
);
</script>

<template>
  <article
    class="media-list-row"
    :class="{ 'is-selected': selected }"
    :data-selected="Boolean(selected)"
    role="listitem"
  >
    <div class="media-selection">
      <UiCheckbox
        :model-value="Boolean(selected)"
        :label="`选择 ${title}`"
        :disabled="disabled"
        compact
        @update:model-value="emit('toggle')"
      />
    </div>
    <div class="media-cover">
      <img
        v-if="cover && !failed"
        :src="coverUrl(cover) ?? undefined"
        referrerpolicy="no-referrer"
        alt=""
        loading="lazy"
        @error="failed = true"
      />
      <UIcon v-else-if="!playable" name="i-tabler-video" class="cover-placeholder" aria-hidden="true" />
      <button
        v-if="playable"
        class="media-play"
        type="button"
        :aria-label="playbackLabel || `播放 ${title}`"
        :title="playbackLabel || `播放 ${title}`"
        @click.stop="emit('play')"
      >
        <UIcon name="i-tabler-player-play" class="media-play-icon" aria-hidden="true" />
      </button>
    </div>
    <div class="media-copy">
      <UTooltip
        :text="title"
        :delay-duration="350"
        :ui="{
          content: 'h-auto max-w-xl whitespace-normal break-words text-left leading-5',
          text: 'whitespace-normal overflow-visible',
        }"
      >
        <button
          class="media-title"
          type="button"
          :aria-label="`${selected ? '取消选择' : '选择'} ${title}`"
          :aria-pressed="Boolean(selected)"
          :disabled="disabled"
          @click="emit('toggle')"
        >
          {{ title }}
        </button>
      </UTooltip>
      <div v-if="secondary || published" class="media-metadata">
        <span v-if="secondary" class="media-author" :title="secondary"
          ><UIcon :name="secondaryIcon || 'i-tabler-user'" class="media-author-icon" aria-hidden="true" />{{
            secondary
          }}</span
        >
        <span v-if="published" class="media-date" :title="`发布于 ${published}`">{{ published }}</span>
      </div>
      <slot name="detail" />
    </div>
    <div class="media-facts">
      <span v-if="duration && duration > 0" class="media-duration" aria-label="时长"
        ><UIcon name="i-tabler-clock" aria-hidden="true" />{{ formatDuration(duration) }}</span
      >
      <span v-if="sizeLabel && sizeLabel !== '--'" class="media-size" aria-label="大小"
        ><UIcon name="i-tabler-file" aria-hidden="true" />{{ sizeLabel }}</span
      >
      <slot name="facts" />
    </div>
    <div v-if="$slots.actions" class="media-actions" @click.stop @keydown.stop><slot name="actions" /></div>
  </article>
</template>

<style scoped>
.media-list-row {
  min-width: 0;
  min-height: 6rem;
  display: grid;
  grid-template-columns: 1.5rem 7.75rem minmax(0, 1fr) auto;
  align-items: center;
  gap: 0.875rem;
  padding: 0.6875rem 0.875rem;
  border: 0.0625rem solid var(--color-border);
  border-radius: var(--radius-8);
  background: var(--color-surface);
  transition:
    background var(--duration-fast) var(--ease-out),
    border-color var(--duration-fast) var(--ease-out);
}

.media-list-row:has(.media-actions) {
  grid-template-columns: 1.5rem 7.75rem minmax(0, 1fr) auto auto;
}

.media-list-row:hover {
  background: var(--color-hover-surface);
}

.media-list-row.is-selected {
  border-color: var(--color-accent);
  background: var(--color-accent-faint);
}

.media-selection {
  display: grid;
  place-items: center;
}

.media-cover {
  position: relative;
  width: 7.75rem;
  height: 4.375rem;
  overflow: hidden;
  display: grid;
  place-items: center;
  border-radius: var(--radius-6);
  background: var(--color-inset);
}

.media-cover img {
  width: 100%;
  height: 100%;
  object-fit: cover;
}

.cover-placeholder {
  width: 1.5625rem;
  height: 1.5625rem;
  color: var(--color-dimmed);
}

.media-play {
  position: absolute;
  inset: 0;
  display: grid;
  place-items: center;
  border: 0;
  background: transparent;
  color: white;
  cursor: pointer;
}

.media-play:hover {
  background: rgb(0 0 0 / 15%);
}

.media-play-icon {
  width: 2rem;
  height: 2rem;
  padding: 0.5rem;
  border-radius: 50%;
  background: rgb(0 0 0 / 60%);
}

.media-copy {
  min-width: 0;
  display: grid;
  align-content: center;
  gap: 0.375rem;
}

.media-title {
  display: block;
  width: 100%;
  min-width: 0;
  overflow: hidden;
  border: 0;
  padding: 0;
  background: transparent;
  color: var(--color-text-strong);
  font-size: var(--font-14);
  font-weight: 650;
  line-height: 1.375rem;
  text-align: left;
  text-overflow: ellipsis;
  white-space: nowrap;
  cursor: pointer;
}

.media-title:disabled {
  cursor: default;
}

.media-title:focus-visible,
.media-play:focus-visible {
  outline: 0.125rem solid var(--color-focus-outline);
  outline-offset: -0.125rem;
}

.media-metadata {
  min-width: 0;
  display: flex;
  gap: 0.75rem;
  color: var(--color-muted);
  font-size: var(--font-12);
  line-height: 1.125rem;
}

.media-author {
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.media-author-icon {
  display: inline-block;
  width: 0.8125rem;
  height: 0.8125rem;
  margin-right: 0.3125rem;
  vertical-align: -0.125rem;
}

.media-date {
  flex-shrink: 0;
  font-variant-numeric: tabular-nums;
}

.media-facts {
  display: flex;
  align-items: center;
  justify-content: flex-end;
  flex-wrap: wrap;
  gap: 0.75rem;
  color: var(--color-muted);
  font-size: var(--font-12);
  font-variant-numeric: tabular-nums;
}

.media-facts > span {
  display: inline-flex;
  align-items: center;
  gap: 0.3125rem;
  white-space: nowrap;
}

.media-facts :deep(.iconify) {
  width: 0.875rem;
  height: 0.875rem;
}

.media-actions {
  display: flex;
  align-items: center;
  gap: 0.25rem;
}

@media (width <= 1050px) {
  .media-list-row,
  .media-list-row:has(.media-actions) {
    gap: 0.625rem;
    padding-inline: 0.625rem;
  }

  .media-facts {
    flex-direction: column;
    align-items: flex-end;
    gap: 0.3125rem;
  }

  .media-metadata {
    gap: 0.375rem;
  }
}

@media (width <= 700px) {
  .media-list-row,
  .media-list-row:has(.media-actions) {
    grid-template-columns: 1.375rem 5.5rem minmax(0, 1fr) auto;
    gap: 0.5rem;
  }

  .media-list-row:has(.media-actions) {
    grid-template-columns: 1.375rem 5.5rem minmax(0, 1fr) auto auto;
  }

  .media-cover {
    width: 5.5rem;
    height: 3.125rem;
  }

  .media-metadata {
    flex-wrap: wrap;
  }
}

@media (prefers-reduced-motion: reduce) {
  .media-list-row {
    transition: none;
  }
}
</style>
