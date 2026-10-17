<script setup lang="ts">
import { computed, defineAsyncComponent } from 'vue';
import { useUiStore } from '../shared/stores/ui';
import type { Component } from 'vue';
import BackgroundParseStatus from '../shared/ui/BackgroundParseStatus.vue';
const { mobile = false } = defineProps<{ mobile?: boolean }>();
const ui = useUiStore();
const desktopPages: Record<string, Component> = {
  bilibili: defineAsyncComponent(() => import('../desktop/pages/BilibiliPage.vue')),
  creation: defineAsyncComponent(() => import('../desktop/pages/CreationPage.vue')),
  editor: defineAsyncComponent(() => import('../desktop/pages/EditorPage.vue')),
  parse: defineAsyncComponent(() => import('../desktop/pages/ParsePage.vue')),
  library: defineAsyncComponent(() => import('../desktop/pages/LibraryPage.vue')),
  transfer: defineAsyncComponent(() => import('../desktop/pages/TransferPage.vue')),
  settings: defineAsyncComponent(() => import('../desktop/pages/SettingsPage.vue')),
  about: defineAsyncComponent(() => import('../desktop/pages/AboutPage.vue')),
};
const mobilePages: Record<string, Component> = {
  parse: defineAsyncComponent(() => import('../mobile/pages/ParsePage.vue')),
  library: defineAsyncComponent(() => import('../mobile/pages/LibraryPage.vue')),
  transfer: defineAsyncComponent(() => import('../mobile/pages/TransferPage.vue')),
  settings: defineAsyncComponent(() => import('../mobile/pages/PersonalPage.vue')),
  about: defineAsyncComponent(() => import('../mobile/pages/AboutPage.vue')),
};
const activePageComponent = computed(() => (mobile ? mobilePages : desktopPages)[ui.activeTab]);
</script>
<template>
  <section class="main-region" :data-page="ui.activeTab" :class="{ 'workspace-mobile': mobile }">
    <BackgroundParseStatus />
    <KeepAlive>
      <Suspense :timeout="0">
        <component :is="activePageComponent" :key="ui.activeTab" />
        <template #fallback>
          <div class="workspace-loading" role="status">
            <span></span>
            正在准备工作区
          </div>
        </template>
      </Suspense>
    </KeepAlive>
  </section>
</template>
<style scoped>
.main-region {
  grid-column: 2;
  grid-row: 2;
  min-width: 0;
  min-height: 0;
  display: grid;
  grid-template-rows: minmax(0, 1fr);
  overflow: hidden;
}

.main-region:has(> [aria-label='后台解析下载']) {
  grid-template-rows: auto minmax(0, 1fr);
}

.workspace-loading {
  display: grid;
  place-content: center;
  justify-items: center;
  gap: var(--space-sm);
  color: var(--color-muted);
  font-size: var(--font-13);
}

.workspace-loading span {
  width: 34px;
  height: 3px;
  overflow: hidden;
  border-radius: 2px;
  background: var(--color-panel-strong);
}

.workspace-loading span::after {
  width: 50%;
  height: 100%;
  display: block;
  background: var(--color-accent);
  content: '';
  animation: loading-track 900ms var(--ease-out) infinite alternate;
}

@keyframes loading-track {
  from {
    transform: translateX(-100%);
  }

  to {
    transform: translateX(200%);
  }
}

@media (prefers-reduced-motion: reduce) {
  .workspace-loading span::after {
    animation: none;
  }
}

.main-region.workspace-mobile {
  grid-area: workspace;
  padding: 0;
}
</style>
