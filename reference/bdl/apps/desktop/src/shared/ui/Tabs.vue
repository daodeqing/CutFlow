<script setup lang="ts">
import { computed } from 'vue';

export interface TabItem {
  label: string;
  value: string;
  count?: number;
}

const model = defineModel<string>({ required: true });
const { tabs, stretch = false } = defineProps<{
  tabs: TabItem[];
  stretch?: boolean;
}>();

const items = computed(() =>
  tabs.map((tab) => ({
    label: tab.label,
    value: tab.value,
    badge: tab.count,
  })),
);
</script>

<template>
  <UTabs
    :model-value="model"
    :items="items"
    class="ui-tabs min-w-0"
    color="primary"
    variant="link"
    size="sm"
    :content="false"
    :ui="{
      root: 'w-full',
      list: 'mb-0 min-h-9 items-stretch gap-5 overflow-x-auto overflow-y-hidden border-b border-(--color-border) bg-transparent p-0',
      indicator: 'hidden',
      trigger: [
        'relative rounded-none px-1 py-0 text-[0.8125rem]',
        stretch ? 'min-w-0 flex-1 justify-center' : 'min-w-fit grow-0 justify-start',
      ].join(' '),
      trailingBadge: 'min-w-5 bg-(--color-panel) px-1.5 text-[0.6875rem] tabular-nums text-(--color-muted) ring-0',
    }"
    @update:model-value="(value: string | number) => (model = String(value))"
  />
</template>

<style scoped>
.ui-tabs {
  width: 100%;
}

.ui-tabs :deep([data-slot='trigger']) {
  min-height: 2.25rem;
  color: var(--color-muted);
  font-weight: 500;
}

.ui-tabs :deep([data-slot='trigger'][data-state='active']) {
  color: var(--color-accent-strong);
  font-weight: 700;
}

.ui-tabs :deep([data-slot='trigger'][data-state='active']::after) {
  position: absolute;
  right: 0.25rem;
  bottom: 0;
  left: 0.25rem;
  height: 0.125rem;
  border-radius: 62.4375rem 62.4375rem 0 0;
  background: var(--color-accent);
  content: '';
}

.ui-tabs :deep([data-slot='trigger'][data-state='inactive']:hover:not(:disabled)) {
  color: var(--color-text);
}

.ui-tabs :deep([data-slot='trigger']:focus-visible) {
  outline-offset: -0.1875rem;
}
</style>
