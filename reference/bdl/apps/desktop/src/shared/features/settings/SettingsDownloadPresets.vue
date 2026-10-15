<script setup lang="ts">
import SettingsCard from './SettingsCard.vue';
import { computed, ref } from 'vue';
import type { DownloadPreset } from '../../api/dto';
import UiButton from '../../ui/Button.vue';
import UiDialog from '../../ui/Dialog.vue';
import UiSelect from '../../ui/Select.vue';
import UiTextField from '../../ui/TextField.vue';
import UiInlineNotice from '../../ui/InlineNotice.vue';
import UiDisclosure from '../../ui/Disclosure.vue';
import DownloadWorkflowEditor from './DownloadWorkflowEditor.vue';
import MediaPreferenceEditor from './MediaPreferenceEditor.vue';
import {
  builtinDownloadPresets,
  cloneWorkflow,
  downloadPresetsError,
  workflowOutputs,
} from '../../utils/downloadWorkflow';
import { audioQualityOptions, codecOptions, missingQualityOptions, videoQualityOptions } from './settingsCatalog';
import type { SettingsForm } from './useSettingsForm';

const { form } = defineProps<{ form: SettingsForm }>();
const { settingsVideoQuality, settingsCodec, settingsAudioQuality, settingsMissingQualityPolicy } = form;
const settings = form.settings;
const presets = computed(() => settings.draft.download_presets);
const selected = computed(() => presets.value.find((preset) => preset.id === settings.draft.selected_download_preset));
const disabled = computed(() => settings.loading || settings.saving || saving.value);
const saving = ref(false);
const open = ref(false);
const creating = ref(false);
const edited = ref<DownloadPreset | null>(null);
const templateId = ref('video');
const templates = builtinDownloadPresets();
const modalError = computed(() => {
  if (!edited.value) return null;
  const next = creating.value
    ? [...presets.value, edited.value]
    : presets.value.map((p) => (p.id === edited.value?.id ? edited.value : p));
  return downloadPresetsError(next, edited.value.id);
});
const chooseTemplate = (id: string) => {
  templateId.value = id;
  const template = templates.find((p) => p.id === id);
  if (template && edited.value) edited.value.workflow = cloneWorkflow(template.workflow);
};
const begin = (create: boolean) => {
  creating.value = create;
  templateId.value = 'video';
  let name = '新预设';
  for (let suffix = 2; presets.value.some((p) => p.name.trim() === name); suffix++) name = `新预设 ${suffix}`;
  edited.value = create
    ? { id: `custom-${crypto.randomUUID()}`, name, workflow: cloneWorkflow(templates[0].workflow) }
    : selected.value
      ? { ...selected.value, workflow: cloneWorkflow(selected.value.workflow) }
      : null;
  open.value = edited.value !== null;
};
const persist = async (items: DownloadPreset[], id: string) => {
  saving.value = true;
  try {
    if (await settings.saveAppPreferences({ download_presets: items, selected_download_preset: id }))
      open.value = false;
  } finally {
    saving.value = false;
  }
};
const save = async () => {
  if (!edited.value || modalError.value) return;
  const item = { ...edited.value, name: edited.value.name.trim(), workflow: cloneWorkflow(edited.value.workflow) };
  const items = creating.value ? [...presets.value, item] : presets.value.map((p) => (p.id === item.id ? item : p));
  await persist(items, item.id);
};
const remove = async () => {
  if (!edited.value || presets.value.length <= 1) return;
  const items = presets.value.filter((p) => p.id !== edited.value?.id);
  await persist(
    items,
    items.some((p) => p.id === settings.draft.selected_download_preset)
      ? settings.draft.selected_download_preset
      : items[0].id,
  );
};
</script>

<template>
  <section class="settings-block">
    <SettingsCard title="默认预设">
      <div class="flex items-end gap-2">
        <div class="min-w-0 flex-1">
          <UiSelect
            v-model="settings.draft.selected_download_preset"
            label="下载预设"
            :options="presets.map((p) => ({ label: p.name, value: p.id }))"
            :disabled="disabled"
          />
        </div>
        <UiButton variant="secondary" :disabled="disabled || presets.length >= 32" @click="begin(true)">新增</UiButton>
        <UiButton variant="ghost" :disabled="disabled || !selected" @click="begin(false)">编辑</UiButton>
      </div>
      <div
        v-if="selected"
        class="rounded-lg border border-(--color-border) bg-(--color-panel) p-3"
        aria-label="预设概况"
      >
        <strong class="text-sm">概况</strong>
        <p class="m-0 mt-2 text-sm leading-relaxed text-(--color-muted)">
          {{ workflowOutputs(selected.workflow).join('、') }}
        </p>
      </div>
    </SettingsCard>
    <SettingsCard title="质量选择">
      <p class="settings-note">用于所有下载预设。</p>
      <div class="settings-inline-grid">
        <UiSelect
          v-model="settingsVideoQuality"
          label="视频清晰度"
          :options="videoQualityOptions"
          :disabled="disabled || settings.draft.media_preferences.video.length > 0"
        />
        <UiSelect
          v-model="settingsCodec"
          label="视频编码偏好"
          :options="codecOptions"
          :disabled="disabled || settings.draft.media_preferences.video.length > 0"
        />
        <UiSelect
          v-model="settingsAudioQuality"
          label="音频质量"
          :options="audioQualityOptions"
          :disabled="disabled || settings.draft.media_preferences.audio.length > 0"
        />
        <UiSelect
          v-model="settingsMissingQualityPolicy"
          label="指定质量不可用时"
          :options="missingQualityOptions"
          :disabled="disabled"
        />
      </div>
      <UiDisclosure title="优先顺序" description="按顺序选择画质和音质">
        <MediaPreferenceEditor v-model="settings.draft.media_preferences" :disabled="disabled" />
      </UiDisclosure>
    </SettingsCard>
    <UiInlineNotice v-if="settings.downloadPresetError" tone="danger">{{
      settings.downloadPresetError
    }}</UiInlineNotice>
  </section>
  <UiDialog
    v-model="open"
    :title="creating ? '新增预设' : '编辑预设'"
    description="选择内容、转换和封装方式"
    :dismissible="!saving"
    :show-close="!saving"
  >
    <div v-if="edited" class="preset-editor">
      <div class="preset-editor-fields">
        <UiTextField v-model="edited.name" label="预设名称" :disabled="saving" :maxlength="40" />
        <UiSelect
          v-if="creating"
          :model-value="templateId"
          label="使用模板"
          :options="templates.map((p) => ({ label: p.name, value: p.id }))"
          :disabled="saving"
          @update:model-value="chooseTemplate"
        />
      </div>
      <DownloadWorkflowEditor :key="edited.id + templateId" v-model="edited.workflow" :disabled="saving" />
      <UiInlineNotice v-if="modalError" tone="danger">{{ modalError }}</UiInlineNotice>
    </div>
    <template #footer>
      <UiButton v-if="!creating" variant="ghost" :disabled="saving || presets.length <= 1" @click="remove"
        >删除预设</UiButton
      >
      <div class="flex flex-1 justify-end gap-2">
        <UiButton variant="secondary" :disabled="saving" @click="open = false">取消</UiButton>
        <UiButton :disabled="saving || !!modalError" @click="save">保存预设</UiButton>
      </div>
    </template>
  </UiDialog>
</template>

<style scoped>
.preset-editor {
  min-width: 0;
  display: grid;
  gap: var(--space-16);
}

.preset-editor-fields {
  min-width: 0;
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(min(100%, 12rem), 1fr));
  align-items: start;
  gap: var(--space-12);
}
</style>
