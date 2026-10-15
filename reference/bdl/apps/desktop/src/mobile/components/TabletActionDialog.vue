<script setup lang="ts">
import UiDialog from '../../shared/ui/Dialog.vue';

const open = defineModel<boolean>({ default: false });
defineProps<{ title: string; description?: string }>();
</script>
<template>
  <UiDialog v-model="open" :title="title" :description="description" size="compact">
    <div class="tablet-action-body"><slot /></div>
  </UiDialog>
</template>
<style scoped>
.tablet-action-body {
  display: grid;
  gap: 0.625rem;
  min-width: 0;
}

.tablet-action-body > :deep(.ui-button) {
  min-height: 2.25rem;
}
</style>
