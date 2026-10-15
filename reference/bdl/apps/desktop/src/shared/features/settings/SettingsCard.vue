<script setup lang="ts">
defineProps<{ title?: string }>();
</script>
<template>
  <section class="settings-card" :aria-label="title">
    <h3 v-if="title" class="settings-card-title">{{ title }}</h3>
    <slot />
  </section>
</template>
<style scoped>
.settings-card {
  min-width: 0;
  display: grid;
  align-content: start;
  gap: var(--space-12);
  border: 0.0625rem solid var(--mobile-card-border, var(--color-border));
  border-radius: var(--radius-12);
  background: var(--mobile-card, var(--color-surface));
  padding: var(--space-16);
  box-shadow: var(--mobile-card-shadow, var(--shadow-panel));
}

.settings-card-title {
  margin: 0;
  color: var(--color-text);
  font-size: var(--font-13);
  font-weight: 700;
  line-height: 1.5;
}
</style>
