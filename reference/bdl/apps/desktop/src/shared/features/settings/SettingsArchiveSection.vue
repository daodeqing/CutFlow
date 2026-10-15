<script setup lang="ts">
import UiCheckbox from '../../ui/Checkbox.vue';
import type { SettingsForm } from './useSettingsForm';

const { form } = defineProps<{ form: SettingsForm }>();
const {
  settings,
  settingsArchiveCover,
  settingsArchiveSubtitles,
  settingsArchiveDanmaku,
  settingsArchiveNfo,
} = form;
</script>

<template>
  <section class="settings-block">
    <p class="settings-note">按需勾选，保存为视频旁的独立文件。</p>
    <div class="archive-option-grid">
      <UiCheckbox v-model="settingsArchiveCover" label="保存封面" :disabled="settings.loading || settings.saving" />
      <UiCheckbox v-model="settingsArchiveSubtitles" label="保存字幕" :disabled="settings.loading || settings.saving" />
      <UiCheckbox v-model="settingsArchiveDanmaku" label="保存弹幕" :disabled="settings.loading || settings.saving" />
      <UiCheckbox v-model="settingsArchiveNfo" label="生成 NFO" :disabled="settings.loading || settings.saving" />
    </div>
  </section>
</template>
