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
      <p>登录账号后获取实际可用画质。严格 4K 请选择「4K / 120」和「阻止创建任务」；会员也可能遇到无 4K 的视频。</p>
    </header>
    <KeepAlive><component :is="pages[ui.bilibiliSubTab]" /></KeepAlive>
  </div>
</template>
<style scoped>
.bilibili-workspace { display: grid; grid-template-rows: auto minmax(0, 1fr); min-height: 0; }
.bilibili-heading { padding: 12px 22px 0; }
.bilibili-heading p { margin: 8px 0; font-size: 12px; color: var(--color-muted); line-height: 1.6; }
</style>
