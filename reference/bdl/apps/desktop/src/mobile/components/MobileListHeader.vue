<template>
  <header class="mobile-list-header">
    <slot />
    <slot name="actions" />
  </header>
</template>
<style scoped>
.mobile-list-header {
  display: grid;
  grid-template-columns: minmax(0, 1fr) auto;
  align-items: center;
  min-width: 0;
  flex-shrink: 0;
}
</style>
