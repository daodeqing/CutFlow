<script setup lang="ts">
const model = defineModel<string>({ required: true });
defineProps<{ tabs: Array<{ value: string; label: string; count?: number }>; stackCounts?: boolean }>();
</script>
<template>
  <div class="mobile-tabs" :class="{ 'mobile-tabs-counts-stacked': stackCounts }" role="tablist">
    <button
      v-for="tab in tabs"
      :key="tab.value"
      type="button"
      role="tab"
      :aria-selected="model === tab.value"
      @click="model = tab.value"
    >
      <span>{{ tab.label }}</span
      ><span v-if="tab.count !== undefined" class="tab-count">{{ tab.count }}</span>
    </button>
  </div>
</template>
<style scoped>
.mobile-tabs {
  display: flex;
  width: 100%;
  min-width: 0;
  border-bottom: 0.0625rem solid var(--color-border);
}

button {
  position: relative;
  flex: 1 1 0;
  min-width: 0;
  min-height: 2.5rem;
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 0.25rem;
  border: 0;
  background: transparent;
  color: var(--color-muted);
  font-size: var(--mobile-font-item-title);
  white-space: nowrap;
  padding: 0.3125rem 0.125rem;
}

button[aria-selected='true'] {
  color: var(--color-accent-strong);
  font-weight: 700;
}

button[aria-selected='true']::after {
  content: '';
  position: absolute;
  height: 0.1875rem;
  width: 2.25rem;
  bottom: 0;
  left: calc(50% - 1.125rem);
  background: var(--color-accent);
  border-radius: 0.125rem;
}

.tab-count {
  display: inline;
  flex: 0 0 auto;
  font-size: var(--mobile-font-body);
  font-weight: 400;
  font-variant-numeric: tabular-nums;
  color: var(--color-muted);
  line-height: 1.4;
}

button[aria-selected='true'] .tab-count {
  color: currentcolor;
}

.mobile-tabs-counts-stacked button {
  flex-direction: column;
  gap: 0;
  padding-block: 0.125rem;
}

button:focus-visible {
  outline: 0.125rem solid var(--color-accent);
  outline-offset: -0.1875rem;
}
</style>
