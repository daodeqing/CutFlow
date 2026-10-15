import { computed } from 'vue';
import { useQueueStore } from '../stores/queue';
import type { AppTab } from '../stores/ui';

export const navItems: Array<{ value: AppTab; label: string; description: string; icon: string; shortcut: string }> = [
  { value: 'bilibili', label: 'B站素材', description: '登录、收藏与下载', icon: 'i-tabler-brand-bilibili', shortcut: '1' },
  { value: 'editor', label: '剪辑', description: '完整 Concat 编辑器', icon: 'i-tabler-movie', shortcut: '2' },
  { value: 'creation', label: 'AI 创作', description: '图片与视频生成', icon: 'i-tabler-sparkles', shortcut: '3' },
  { value: 'settings', label: '设置', description: '偏好与维护', icon: 'i-tabler-adjustments', shortcut: '4' },
  { value: 'about', label: '关于', description: '版本与链接', icon: 'i-tabler-info-circle', shortcut: '5' },
];

export const useNavigationStatus = () => {
  const queue = useQueueStore();
  const transferBadgeCount = computed(
    () => queue.tasks.filter((task) => task.status !== 'completed' && task.status !== 'cancelled').length,
  );
  const attentionCount = computed(
    () => queue.tasks.filter((task) => task.status === 'failed' || task.status === 'cancelled').length,
  );
  const scheduledTaskCount = computed(
    () =>
      queue.tasks.filter(
        (task) => task.status === 'waiting' && task.scheduled_at && Date.parse(task.scheduled_at) > Date.now(),
      ).length,
  );
  const queueHealthLabel = computed(() => {
    if (queue.loading) return '同步队列';
    if (attentionCount.value > 0) return `${attentionCount.value} 项需处理`;
    if (scheduledTaskCount.value === transferBadgeCount.value && scheduledTaskCount.value > 0) {
      return `${scheduledTaskCount.value} 项已定时`;
    }
    if (transferBadgeCount.value > 0) return `${transferBadgeCount.value} 项进行中`;
    return '队列空闲';
  });
  const aggregateSpeedLabel = computed(() => {
    const bytesPerSecond = queue.totalSpeedBytesPerSecond();
    if (bytesPerSecond <= 0 || !Number.isFinite(bytesPerSecond)) return '0 B/s';
    const units = ['B/s', 'KB/s', 'MB/s', 'GB/s'];
    let value = bytesPerSecond;
    let unitIndex = 0;
    while (value >= 1024 && unitIndex < units.length - 1) {
      value /= 1024;
      unitIndex += 1;
    }
    return `${value >= 100 || unitIndex === 0 ? value.toFixed(0) : value.toFixed(1)} ${units[unitIndex]}`;
  });
  return { transferBadgeCount, attentionCount, queueHealthLabel, aggregateSpeedLabel };
};
