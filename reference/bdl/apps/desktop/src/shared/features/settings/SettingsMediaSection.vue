<script setup lang="ts">
import SettingsDownloadPresets from './SettingsDownloadPresets.vue';
import type { SettingsForm } from './useSettingsForm';
const { form } = defineProps<{ form: SettingsForm }>();
</script>

<template>
  <SettingsDownloadPresets :form="form" />
</template>
