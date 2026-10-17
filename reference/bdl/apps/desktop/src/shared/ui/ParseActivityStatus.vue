<script setup lang="ts">
import { computed } from 'vue'
import { useParseActivity } from '../features/parse/useParseActivity'

const props = defineProps<{ sourceId: string; stopping?: boolean }>()
const activity = useParseActivity(() => props.sourceId)
const seconds = computed(() => Math.max(0, Math.ceil((activity.value.waitingUntil - activity.value.now) / 1_000)))
</script>

<template>
  <span>{{ stopping ? '正在停止…' : seconds > 0 ? `休息 ${seconds} 秒后继续` : activity.queued ? '等待其他解析完成…' : '正在解析…' }}</span>
</template>
