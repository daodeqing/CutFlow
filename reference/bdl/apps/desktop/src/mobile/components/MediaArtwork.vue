<script setup lang="ts">
import { ref, watch } from 'vue';
import { coverUrl } from '../../shared/utils/coverUrl';
import { formatDuration } from '../../shared/utils/duration';
const props = defineProps<{ src?: string | null; duration?: number | null; showFallbackIcon?: boolean }>();
const failed = ref(false);
watch(
  () => props.src,
  () => {
    failed.value = false;
  },
);
</script>
<template>
  <span class="media-artwork">
    <img
      v-if="src && !failed"
      referrerpolicy="no-referrer"
      :src="coverUrl(src) ?? undefined"
      alt=""
      loading="lazy"
      @error="failed = true"
    />
    <UIcon v-else-if="showFallbackIcon !== false" name="i-tabler-video" class="artwork-fallback" aria-hidden="true" />
    <span v-if="duration != null && duration > 0" class="artwork-duration">{{ formatDuration(duration) }}</span>
  </span>
</template>
<style scoped>
.media-artwork {
  position: relative;
  display: grid;
  place-items: center;
  overflow: hidden;
  background: var(--mobile-artwork);
  border-radius: 0.25rem;
  flex-shrink: 0;
}

img {
  width: 100%;
  height: 100%;
  object-fit: cover;
}

.artwork-fallback {
  width: 1.75rem;
  height: 1.75rem;
  color: var(--color-muted);
}

.artwork-duration {
  position: absolute;
  right: 0.1875rem;
  bottom: 0.1875rem;
  padding: 0.0625rem 0.25rem;
  border-radius: 0.1875rem;
  background: rgb(0 0 0 / 52%);
  color: white;
  font-size: var(--mobile-font-micro);
  line-height: 1.25;
  font-variant-numeric: tabular-nums;
}
</style>
