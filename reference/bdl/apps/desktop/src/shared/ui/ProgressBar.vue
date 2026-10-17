<script setup lang="ts">
import { computed } from 'vue';

const {
  value,
  max = 100,
  status = 'ready',
  label = '任务进度',
} = defineProps<{
  value: number;
  max?: number;
  status?: 'ready' | 'downloading' | 'queued' | 'done' | 'warning' | 'error' | 'paused';
  label?: string;
}>();

// 无效数值与越界值不能污染实际进度，也不能误触发流动动画。
const percent = computed(() => {
  if (!Number.isFinite(value) || !Number.isFinite(max) || max <= 0) return 0;
  return Math.min(100, Math.max(0, (value / max) * 100));
});
const flowing = computed(() => status === 'downloading' && percent.value > 0 && percent.value < 100);
</script>

<template>
  <div
    class="progress-track wabi-liquid-progress-track"
    :class="{ 'is-flowing': flowing }"
    :data-status="status"
    role="progressbar"
    :aria-label="label"
    :aria-valuenow="percent"
    aria-valuemin="0"
    aria-valuemax="100"
  >
    <span class="progress-fill" :style="{ width: percent + '%' }" />
  </div>
</template>

<style scoped>
.progress-track {
  position: relative;
  width: 100%;
  height: 0.5rem;
  overflow: hidden;
  isolation: isolate;
  border-radius: 999px;
  background: var(--color-progress-track);
  box-shadow:
    inset 0 1px 3px rgb(10 24 28 / 18%),
    0 1px 0 var(--wabi-specular);
}

.progress-fill {
  position: relative;
  display: block;
  height: 100%;
  overflow: hidden;
  border-radius: inherit;
  background: linear-gradient(90deg, #0b6959, #0d9488, #14b8a6, #5eead4, #14b8a6, #0d9488, #0b6959);
  background-size: 200% 100%;
  box-shadow:
    inset 0 1px 0 rgb(255 255 255 / 65%),
    inset 0 -1px 1px rgb(6 78 59 / 40%);
}

.is-flowing .progress-fill {
  animation: wabi-liquid-flow 2.4s linear infinite;
}

.is-flowing .progress-fill::after {
  position: absolute;
  inset: 0;
  width: 55%;
  background: linear-gradient(90deg, transparent, rgb(255 255 255 / 65%), transparent);
  content: '';
  pointer-events: none;
  animation: wabi-caustic-sweep 1.85s ease-in-out infinite;
}

.is-flowing .progress-fill::before {
  position: absolute;
  inset: 0 0 0 auto;
  width: 6px;
  border-radius: inherit;
  background: radial-gradient(ellipse at right, #fff, #99f6e4 40%, transparent 85%);
  content: '';
  pointer-events: none;
}

[data-status='paused'] .progress-fill,
[data-status='warning'] .progress-fill {
  background: linear-gradient(90deg, #b45309, #f59e0b);
}

[data-status='error'] .progress-fill {
  background: linear-gradient(90deg, #b91c1c, #f87171);
}

[data-status='queued'] .progress-fill,
[data-status='ready'] .progress-fill {
  background: var(--color-muted);
}

@keyframes wabi-liquid-flow {
  from {
    background-position: 200% 50%;
  }

  to {
    background-position: 0% 50%;
  }
}

@keyframes wabi-caustic-sweep {
  from {
    transform: translateX(-110%) skewX(-18deg);
  }

  to {
    transform: translateX(290%) skewX(-18deg);
  }
}

@media (prefers-reduced-motion: reduce) {
  .is-flowing .progress-fill,
  .is-flowing .progress-fill::after {
    animation: none;
  }

  .is-flowing .progress-fill::after {
    opacity: 0;
  }
}
</style>
