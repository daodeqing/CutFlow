<script setup lang="ts">
import { computed, defineAsyncComponent } from 'vue'
import { useUiStore } from '../../shared/stores/ui'
import { useNavigationStatus } from '../../shared/features/navigation'
import UiTabs from '../../shared/ui/Tabs.vue'

const ui = useUiStore()
const { transferBadgeCount } = useNavigationStatus()
const pages = {
  parse: defineAsyncComponent(() => import('./ParsePage.vue')),
  library: defineAsyncComponent(() => import('./LibraryPage.vue')),
  transfer: defineAsyncComponent(() => import('./TransferPage.vue')),
}
const selected = computed({ get: () => ui.bilibiliSubTab, set: (value: string) => { if (value in pages) ui.bilibiliSubTab = value as keyof typeof pages } })
const tabs = computed(() => [
  { label: '链接解析 / 多 P', value: 'parse' },
  { label: '我的收藏与订阅', value: 'library' },
  { label: '下载队列', value: 'transfer', count: transferBadgeCount.value },
])
</script>
<template>
  <div class="bilibili-workspace">
    <header class="bilibili-heading">
      <UiTabs v-model="selected" :tabs="tabs" />
      <p>开始下载前可选择画质，默认获取当前账号可用的最高原视频画质；支持 8K、4K 和 HDR，实际可用范围取决于原视频与账号权限。</p>
    </header>
    <KeepAlive><component :is="pages[ui.bilibiliSubTab]" /></KeepAlive>
  </div>
</template>
<style scoped>
.bilibili-workspace { display: grid; grid-template-rows: auto minmax(0, 1fr); min-height: 0; }
.bilibili-heading { padding: 12px 22px 0; }
.bilibili-heading p { margin: 8px 0; font-size: 12px; color: var(--color-muted); line-height: 1.6; }
</style>
