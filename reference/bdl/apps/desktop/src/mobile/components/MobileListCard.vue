<script setup lang="ts">
import { ref } from 'vue';
const element = ref<HTMLDivElement>();
defineExpose({ element });
</script>

<template>
  <div ref="element" class="mobile-list-card"><slot /></div>
</template>

<style scoped>
.mobile-list-card {
  width: 100%;
  min-height: 0;
  flex: 0 1 auto;
  overflow: hidden auto;
  padding: 0.4375rem 0.75rem;
  border-radius: 0.75rem;
  background: var(--mobile-card);
  box-shadow: var(--mobile-card-shadow);
}
</style>
