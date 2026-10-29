<script setup lang="ts">
import { ref, useId } from 'vue';
import type { AiModel } from '../../shared/api/cutflow';

const props = withDefaults(defineProps<{ modelValue: string; models: AiModel[]; label: string; placeholder?: string }>(), { placeholder: '' });
const emit = defineEmits<{ 'update:modelValue': [value: string] }>();
const root = ref<HTMLElement>();
const open = ref(false);
const id = useId();
function select(value: string) {
  emit('update:modelValue', value);
  open.value = false;
}
function closeOnBlur(event: FocusEvent) {
  if (!root.value?.contains(event.relatedTarget as Node | null)) open.value = false;
}
</script>

<template>
  <div ref="root" class="model-field" @focusout="closeOnBlur" @keydown.esc.stop="open = false">
    <label :for="id">{{ props.label }}</label>
    <div class="model-input">
      <input :id="id" :value="modelValue" :placeholder="placeholder" autocomplete="off"
        :aria-expanded="open" :aria-controls="id + '-list'" aria-haspopup="listbox" role="combobox" aria-autocomplete="list"
        @input="emit('update:modelValue', ($event.target as HTMLInputElement).value)"
        @keydown.down.prevent="open = true" />
      <button type="button" class="model-toggle" :aria-label="'选择' + label + '模型'"
        :aria-expanded="open" :aria-controls="id + '-list'" aria-haspopup="listbox" @click="open = !open">▾</button>
    </div>
    <div v-if="open" :id="id + '-list'" class="model-options" role="listbox" :aria-label="label + '模型列表'">
      <button v-for="item in models" :key="item.id" type="button" role="option" :aria-selected="modelValue === item.id"
        @mousedown.prevent @click="select(item.id)">{{ item.id }}</button>
      <p v-if="models.length === 0">暂无可选模型，可直接输入模型名。</p>
    </div>
  </div>
</template>

<style scoped>
.model-field { position: relative; display: grid; gap: 6px; min-width: 0; font-size: 13px; }
.model-input { display: flex; min-width: 0; background: var(--color-surface); border: 1px solid var(--color-border); border-radius: 8px; }
.model-input:focus-within { outline: 2px solid var(--color-focus-outline); outline-offset: 1px; }
.model-input input { flex: 1; min-width: 0; border: 0; padding: 9px 10px; background: transparent; color: inherit; font: inherit; outline: 0; }
.model-toggle { flex: none; min-width: 42px; border: 0; background: transparent; color: inherit; cursor: pointer; font: inherit; }
.model-options { position: absolute; z-index: 25; top: 100%; left: 0; right: 0; display: grid; max-height: 220px; overflow-y: auto; margin-top: 4px; padding: 4px; background: var(--color-surface); border: 1px solid var(--color-border); border-radius: 8px; box-shadow: var(--shadow-overlay); }
.model-options button { text-align: left; overflow-wrap: anywhere; border: 0; border-radius: 5px; padding: 9px 10px; background: transparent; color: inherit; font: inherit; cursor: pointer; }
.model-options button:hover, .model-options button:focus-visible, .model-options button[aria-selected="true"] { background: var(--color-control); outline: 0; }
.model-options p { margin: 0; padding: 9px 10px; color: var(--color-muted); }
</style>
