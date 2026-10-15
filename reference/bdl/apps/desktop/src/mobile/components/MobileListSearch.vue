<script setup lang="ts">
const query = defineModel<string>({ required: true });
const open = defineModel<boolean>('open', { required: true });
defineProps<{ label: string; placeholder: string; disabled?: boolean }>();
</script>
<template>
  <label v-if="open" class="mobile-list-search">
    <span class="sr-only">{{ label }}</span
    ><UIcon name="i-tabler-search" />
    <input v-model="query" type="search" :placeholder="placeholder" :disabled="disabled" />
    <button
      type="button"
      aria-label="关闭搜索"
      @click="
        query = '';
        open = false;
      "
    >
      <UIcon name="i-tabler-x" />
    </button>
  </label>
</template>
<style scoped>
.mobile-list-search {
  height: 2.125rem;
  flex-shrink: 0;
  display: grid;
  grid-template-columns: 1.125rem minmax(0, 1fr) 1.875rem;
  align-items: center;
  gap: 0.375rem;
  padding-left: 0.625rem;
  border: 0.0625rem solid var(--color-border);
  border-radius: 0.5625rem;
  background: var(--color-inset);
}

.mobile-list-search:focus-within {
  border-color: var(--color-accent);
  outline: 0.125rem solid var(--color-focus-outline);
  outline-offset: -0.125rem;
}

.mobile-list-search > :deep(svg) {
  color: var(--color-muted);
}

.mobile-list-search input {
  min-width: 0;
  border: 0;
  outline: 0;
  background: transparent;
  color: var(--color-text);
  font-size: var(--mobile-font-item-title);
}

.mobile-list-search button {
  width: 1.875rem;
  height: 1.875rem;
  display: grid;
  place-items: center;
  border: 0;
  background: transparent;
  color: var(--color-muted);
}
</style>
