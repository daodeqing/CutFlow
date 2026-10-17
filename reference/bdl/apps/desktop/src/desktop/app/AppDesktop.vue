<script setup lang="ts">
import { ref } from 'vue';
import WabiVisuals from '../../shared/ui/WabiVisuals.vue';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { useAccountStore } from '../../shared/stores/account';
import { useUiStore } from '../../shared/stores/ui';
import { useUpdateStore } from '../../shared/stores/update';
import { isMacPlatform } from '../../shared/platform/environment';
import brandMark from '../../assets/cutflow-mark.svg';
import AppearanceMenu from '../../shared/ui/AppearanceMenu.vue';
import AppAccountMenu from './AppAccountMenu.vue';
import { navItems, useNavigationStatus } from '../../shared/features/navigation';
const ui = useUiStore();
const designOpen = ref(false);
const navCollapsed = ref(false);
const account = useAccountStore();
const updater = useUpdateStore();
const isMacOs = isMacPlatform();
const appWindow = getCurrentWindow();
const minimizeWindow = () => {
  void appWindow.minimize();
};
const toggleMaximizeWindow = () => {
  void appWindow.toggleMaximize();
};
const closeWindow = () => {
  void appWindow.close();
};
const { transferBadgeCount, attentionCount, queueHealthLabel, aggregateSpeedLabel } = useNavigationStatus();
</script>
<template>
  <main
    class="app-shell"
    :class="{ 'platform-macos': isMacOs, 'nav-collapsed': navCollapsed || ui.activeTab === 'creation' }"
  >
    <WabiVisuals v-model="designOpen" />
    <header class="app-titlebar" data-tauri-drag-region @dblclick="toggleMaximizeWindow">
      <div class="titlebar-brand" data-tauri-drag-region>
        <span class="titlebar-mark" aria-hidden="true"><img :src="brandMark" alt="" /></span>
        <strong data-tauri-drag-region>CutFlow</strong>
        <span class="titlebar-name" data-tauri-drag-region>素材 · 剪辑 · AI 创作</span>
        <span class="titlebar-version tabular-nums" data-tauri-drag-region>v{{ updater.currentVersion }}</span>
      </div>
      <div class="titlebar-actions">
        <button v-if="updater.hasUpdate" class="titlebar-update" type="button" @click="ui.setTab('settings')">
          <UIcon name="i-tabler-arrow-up-circle" aria-hidden="true" />
          可更新至 v{{ updater.availableVersion }}
        </button>
        <button
          class="wabi-spec-trigger"
          type="button"
          aria-label="Wabi 液体玻璃方案与样例图"
          @dblclick.stop
          @click="designOpen = true"
        >
          <UIcon name="i-tabler-stack-2" aria-hidden="true" /><span>Wabi 液体玻璃方案与样例图</span>
        </button>
        <AppearanceMenu />
        <AppAccountMenu />
        <div v-if="!isMacOs" class="window-controls" aria-label="窗口控制">
          <button type="button" aria-label="最小化" @click.stop="minimizeWindow">
            <UIcon name="i-tabler-minus" aria-hidden="true" />
          </button>
          <button type="button" aria-label="最大化或还原" @click.stop="toggleMaximizeWindow">
            <UIcon name="i-tabler-square" aria-hidden="true" />
          </button>
          <button class="close" type="button" aria-label="关闭" @click.stop="closeWindow">
            <UIcon name="i-tabler-x" aria-hidden="true" />
          </button>
        </div>
      </div>
    </header>

    <aside class="side-nav" aria-label="主导航">
      <button
        class="nav-collapse-toggle"
        type="button"
        :aria-expanded="!navCollapsed"
        :aria-label="navCollapsed ? '展开导航' : '收起导航'"
        @click="navCollapsed = !navCollapsed"
      >
        <UIcon name="i-tabler-layout-sidebar" aria-hidden="true" /><span>{{
          navCollapsed ? '展开导航' : '工作空间'
        }}</span>
      </button>
      <nav class="nav-list">
        <button
          v-for="item in navItems"
          :key="item.value"
          class="nav-item"
          :class="{ active: ui.activeTab === item.value }"
          type="button"
          :aria-current="ui.activeTab === item.value ? 'page' : undefined"
          :title="`${item.label} · Ctrl+${item.shortcut}`"
          :aria-label="item.label"
          @click="ui.setTab(item.value)"
        >
          <UIcon :name="item.icon" class="nav-icon" aria-hidden="true" />
          <span class="nav-copy">
            <strong>{{ item.label }}</strong>
            <small>{{ item.description }}</small>
          </span>
          <span v-if="item.value === 'bilibili' && transferBadgeCount > 0" class="nav-badge">
            {{ transferBadgeCount > 99 ? '99+' : transferBadgeCount }}
          </span>
        </button>
      </nav>

      <div class="nav-status">
        <span class="status-beacon" :class="{ attention: attentionCount > 0 }" aria-hidden="true"></span>
        <span>
          <strong>{{ queueHealthLabel }}</strong>
          <small class="tabular-nums">{{ aggregateSpeedLabel }} · {{ account.statusLabel }}</small>
        </span>
      </div>
    </aside>

    <slot />
  </main>
</template>
<style scoped src="./AppDesktop.css"></style>
