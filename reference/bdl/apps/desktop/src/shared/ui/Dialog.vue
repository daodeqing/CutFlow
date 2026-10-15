<script setup lang="ts">
import { computed } from 'vue';

const model = defineModel<boolean>({ default: false });
const {
  title,
  description,
  fixedHeight = false,
  size = 'default',
  dismissible = true,
  showClose = true,
} = defineProps<{
  title: string;
  description?: string;
  fixedHeight?: boolean;
  size?: 'compact' | 'default' | 'wide';
  dismissible?: boolean;
  showClose?: boolean;
}>();

const modalUi = computed(() => ({
  overlay: 'bdl-dialog-overlay',
  content: [
    'bdl-dialog-content',
    size === 'wide' ? 'max-w-[57.5rem]' : size === 'compact' ? 'max-w-[24rem]' : 'max-w-[32.5rem]',
    fixedHeight ? 'bdl-dialog-fixed' : '',
  ].join(' '),
  header: 'min-w-0 shrink-0',
  wrapper: 'min-w-0 flex-1 pr-8',
  title: 'min-w-0 truncate',
  description: 'truncate',
  body: fixedHeight ? 'min-h-0 flex-1 overflow-hidden pt-0 sm:pt-0' : 'min-h-0 overflow-y-auto',
  footer: 'shrink-0 justify-end flex-wrap sm:flex-nowrap',
}));
</script>

<template>
  <UModal
    v-model:open="model"
    :title
    :description
    :dismissible
    scrollable
    :ui="modalUi"
    close-icon="i-tabler-x"
    :close="showClose ? { color: 'neutral', variant: 'ghost', size: 'sm' } : false"
  >
    <template #body>
      <div class="dialog-body" :class="{ 'dialog-body-fixed': fixedHeight }">
        <slot />
      </div>
    </template>
    <template v-if="$slots.footer" #footer>
      <slot name="footer" />
    </template>
  </UModal>
</template>

<style>
/* The scrollable modal nests content in its overlay, so grid centering needs no CSS translate. */
.bdl-dialog-overlay {
  --bdl-dialog-viewport-height: 100vh;
  --bdl-dialog-gap-top: max(1rem, var(--bdl-layout-safe-area-top));
  --bdl-dialog-gap-bottom: max(1rem, var(--bdl-layout-safe-area-bottom));
  --bdl-dialog-available-height: calc(
    var(--bdl-dialog-viewport-height) - var(--bdl-dialog-gap-top) - var(--bdl-dialog-gap-bottom)
  );

  padding: var(--bdl-dialog-gap-top) max(1rem, var(--bdl-layout-safe-area-right)) var(--bdl-dialog-gap-bottom)
    max(1rem, var(--bdl-layout-safe-area-left));
}

.bdl-dialog-content {
  width: 100%;
  max-height: var(--bdl-dialog-available-height);
  overflow: hidden;
}

.bdl-dialog-fixed {
  height: 40rem;
}

@supports (height: 100dvh) {
  .bdl-dialog-overlay {
    --bdl-dialog-viewport-height: 100dvh;
  }
}
</style>

<style scoped>
.dialog-body {
  min-width: 0;
  min-height: 0;
  display: grid;
  gap: var(--space-16);
}

.dialog-body-fixed {
  height: 100%;
  display: flex;
  flex-direction: column;
  overflow: hidden;
}

.dialog-body-fixed > :deep(.ui-tabs) {
  flex-shrink: 0;
}
</style>
