<script setup lang="ts">
import { computed } from 'vue';
import type { DownloadMediaMode, SettingsSnapshot, ArchiveAssetSelection } from '../api/dto';
import UiCheckbox from './Checkbox.vue';
import UiSelect from './Select.vue';

const mode = defineModel<DownloadMediaMode>('mode', { required: true });
const format = defineModel<'mp4' | 'mkv'>('format', { required: true });
const audioFormat = defineModel<SettingsSnapshot['audio_output_format']>('audioFormat', { required: true });
const subtitleFormat = defineModel<SettingsSnapshot['subtitle_format']>('subtitleFormat', { required: true });
const danmakuFormat = defineModel<SettingsSnapshot['danmaku_format']>('danmakuFormat', { required: true });
const assets = defineModel<ArchiveAssetSelection>('assets', { required: true });
const cover = defineModel<boolean>('embedCover', { required: true });
const subtitles = defineModel<boolean>('embedSubtitles', { required: true });
const raw = defineModel<boolean>('retainRawStreams', { required: true });
const { disabled = false, embeddingSupported = true } = defineProps<{ disabled?: boolean; embeddingSupported?: boolean }>();
const includesVideo = computed(() => mode.value !== 'audio_only');
const includesAudio = computed(() => mode.value !== 'video_only');
const modes = [{ label: '音频+视频', value: 'audio_video' }, { label: '仅视频', value: 'video_only' }, { label: '仅音频', value: 'audio_only' }];
const formats = [{ label: 'MP4', value: 'mp4' }, { label: 'MKV', value: 'mkv' }];
const audioFormats = [{ label: '不转换（m4s）', value: 'm4s' }, { label: '转为 MP3', value: 'mp3' }];
const subtitleFormats = [{ label: '转换为 SRT', value: 'srt' }, { label: '转换为 ASS', value: 'ass' }];
const danmakuFormats = [{ label: 'XML', value: 'xml' }, { label: 'HTML（离线播放）', value: 'html' }, { label: 'SRT（字幕显示）', value: 'srt' }, { label: 'ASS（滚动弹幕）', value: 'ass' }];
const setAsset = (key: keyof ArchiveAssetSelection, value: boolean) => { assets.value = { ...assets.value, [key]: value }; };
const downloadCover = computed({ get: () => assets.value.cover || (includesVideo.value && cover.value), set: (value: boolean) => { setAsset('cover', value); if (!value) cover.value = false; } });
const downloadSubtitles = computed({ get: () => assets.value.subtitles || (includesVideo.value && subtitles.value), set: (value: boolean) => { setAsset('subtitles', value); if (!value) subtitles.value = false; } });
const downloadDanmaku = computed({ get: () => assets.value.danmaku, set: (value: boolean) => setAsset('danmaku', value) });
const nfo = computed({ get: () => assets.value.nfo, set: (value: boolean) => setAsset('nfo', value) });
const outputFormat = computed({ get: () => format.value, set: (value: string) => { if (value !== 'mkv') { cover.value = false; subtitles.value = false; } format.value = value === 'mkv' ? 'mkv' : 'mp4'; } });
const embedCover = computed({ get: () => cover.value, set: (value: boolean) => { if (value) format.value = 'mkv'; cover.value = value; } });
const embedSubtitles = computed({ get: () => subtitles.value, set: (value: boolean) => { if (value) format.value = 'mkv'; subtitles.value = value; } });
</script>

<template>
  <div class="grid min-w-0 gap-4">
    <UiSelect v-model="mode" label="下载内容" :options="modes" :disabled="disabled" helper="自动记住上次选择" />
    <section v-if="includesVideo" class="grid gap-3" aria-label="视频设置">
      <h3 class="m-0 text-sm font-semibold">视频</h3>
      <slot name="video" />
      <UiSelect v-model="outputFormat" label="封装格式" :options="formats" :disabled="disabled" :helper="embeddingSupported ? '勾选嵌入时自动使用 MKV' : undefined" />
    </section>
    <section v-if="includesAudio" class="grid gap-3" aria-label="音频设置">
      <h3 class="m-0 text-sm font-semibold">音频</h3>
      <slot name="audio" />
      <UiSelect v-if="!includesVideo" v-model="audioFormat" label="音频输出" :options="audioFormats" :disabled="disabled" :helper="audioFormat === 'm4s' ? '原样保存下载的音频，无需 FFmpeg' : '重新编码为 MP3，可能损失音质'" />
    </section>
    <UiCheckbox v-if="includesVideo || audioFormat === 'mp3'" v-model="raw" label="保留原始视频/音频轨道" :disabled="disabled" />
    <section class="grid gap-3" aria-label="封面设置">
      <h3 class="m-0 text-sm font-semibold">封面</h3>
      <UiCheckbox v-model="downloadCover" label="下载封面" :disabled="disabled" />
      <UiCheckbox v-if="downloadCover && includesVideo" v-model="embedCover" label="嵌入封面" :disabled="disabled || !embeddingSupported" />
    </section>
    <section class="grid gap-3" aria-label="字幕设置">
      <h3 class="m-0 text-sm font-semibold">字幕</h3>
      <UiCheckbox v-model="downloadSubtitles" label="下载字幕" :disabled="disabled" />
      <template v-if="downloadSubtitles">
        <UiSelect v-model="subtitleFormat" label="字幕格式" :options="subtitleFormats" :disabled="disabled" />
        <UiCheckbox v-if="includesVideo" v-model="embedSubtitles" label="嵌入字幕" :disabled="disabled || !embeddingSupported" />
      </template>
    </section>
    <section class="grid gap-3" aria-label="弹幕设置">
      <h3 class="m-0 text-sm font-semibold">弹幕</h3>
      <UiCheckbox v-model="downloadDanmaku" label="下载弹幕" :disabled="disabled" />
      <UiSelect v-if="downloadDanmaku" v-model="danmakuFormat" label="弹幕格式" :options="danmakuFormats" :disabled="disabled" />
    </section>
    <p v-if="!embeddingSupported && includesVideo && (downloadCover || downloadSubtitles)" class="m-0 text-xs text-(--color-muted)">Android 暂不支持嵌入，封面和字幕会保存为独立文件。</p>
    <UiCheckbox v-model="nfo" label="生成 NFO" :disabled="disabled" />
  </div>
</template>
