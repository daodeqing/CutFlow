<script setup lang="ts">
import { onMounted, ref } from 'vue';
import UiButton from '../../ui/Button.vue';
import { cutflow, errorMessage, type CreationCatalog, type AiModel } from '../../api/cutflow';

const baseUrl = ref('');
const apiKey = ref('');
const busy = ref(false);
const message = ref('');
const testModel = ref('');
const textModels = ref<AiModel[]>([]);
const load = async () => {
  try {
    const snapshot = await cutflow<CreationCatalog>('cutflow.creation.catalog');
    baseUrl.value = snapshot.connection.baseUrl;
    textModels.value = snapshot.models.filter(
      (item) => item.kind === 'text' || (/^(gpt-|api-6|gemini-)/.test(item.id) && !/image|banana/.test(item.id)),
    );
    testModel.value = textModels.value.find((item) => item.id === 'gpt-6-luna')?.id || textModels.value[0]?.id || '';
  } catch (error) {
    message.value = errorMessage(error);
  }
};
const save = async (check: boolean) => {
  busy.value = true;
  message.value = '';
  try {
    await cutflow('cutflow.connection.save', { baseUrl: baseUrl.value, apiKey: apiKey.value });
    apiKey.value = '';
    message.value = '设置已保存，密钥加密保存在 portable 内。';
    if (check) {
      const result = await cutflow<{ models: AiModel[] }>('cutflow.models.list');
      textModels.value = result.models.filter((item) => item.kind === 'text');
      testModel.value ||=
        textModels.value.find((item) => item.id === 'gpt-6-luna')?.id || textModels.value[0]?.id || '';
      message.value = `已读取 ${result.models.length} 个模型；尚未验证实际回复。可点击下方测试回复。`;
    }
  } catch (error) {
    message.value = errorMessage(error);
  } finally {
    busy.value = false;
  }
};
const testReply = async () => {
  busy.value = true;
  message.value = '';
  try {
    await cutflow('cutflow.connection.save', { baseUrl: baseUrl.value, apiKey: apiKey.value });
    apiKey.value = '';
    const result = await cutflow<{ model: string; answer: string }>('cutflow.connection.test', {
      model: testModel.value,
    });
    message.value = `回复已验证 · ${result.model}：${result.answer}`;
  } catch (error) {
    message.value = errorMessage(error);
  } finally {
    busy.value = false;
  }
};
onMounted(load);
</script>
<template>
  <form class="cutflow-ai-settings" @submit.prevent="save(false)">
    <label for="flow-base-url">服务地址</label
    ><input
      id="flow-base-url"
      v-model="baseUrl"
      type="url"
      placeholder="http://127.0.0.1:8000 或 https://你的服务/v1"
      required
    />
    <p>可填写包含或不包含 /v1 的地址。使用 GET /v1/models 和 POST /v1/chat/completions。</p>
    <label for="flow-api-key">API Key</label
    ><input id="flow-api-key" v-model="apiKey" type="password" autocomplete="off" placeholder="留空保留已保存的密钥" />
    <div class="settings-ai-actions">
      <UiButton type="submit" :disabled="busy || !baseUrl">保存连接</UiButton
      ><UiButton variant="secondary" :disabled="busy || !baseUrl" @click="save(true)">{{
        busy ? '检查中…' : '检查连接 / 刷新模型'
      }}</UiButton>
    </div>
    <label for="flow-test-model">测试回复模型</label
    ><select id="flow-test-model" v-model="testModel">
      <option v-for="item in textModels" :key="item.id" :value="item.id">{{ item.id }}</option>
    </select>
    <UiButton variant="secondary" :disabled="busy || !baseUrl || !testModel" @click="testReply"
      >测试实际回复（少量用量）</UiButton
    >
    <p v-if="message" role="status">{{ message }}</p>
        <p>服务需提供图片或视频生成模型。普通聊天模型未必能生成媒体；创作页仅显示已验证的 Google Flow 媒体模型。</p>
    <details>
      <summary>进阶 · 外部 AI 控制</summary>
      <p>
        安装目录 tools/cutflow-agent.exe 支持 mcp 和 call。主窗口与编辑器分别保持独立进程，外部 AI
        退出不会关闭软件。关闭窗口时有任务会转入托盘；需要停止全部本地任务时，从托盘明确退出。
      </p>
    </details>
  </form>
</template>
<style scoped>
.cutflow-ai-settings {
  display: grid;
  gap: 12px;
  max-width: 650px;
}

.cutflow-ai-settings label {
  font-size: 13px;
  font-weight: 600;
}

.cutflow-ai-settings input,
.cutflow-ai-settings select {
  min-height: 40px;
  padding: 10px 12px;
  color: var(--color-text);
  background: var(--color-surface);
  border: 1px solid var(--color-border);
  border-radius: 6px;
}

.cutflow-ai-settings p {
  color: var(--color-muted);
  font-size: 12px;
  line-height: 1.8;
  margin: 0;
  overflow-wrap: anywhere;
}

.cutflow-ai-settings input:focus-visible {
  outline: 2px solid var(--color-accent);
  outline-offset: 2px;
}

.settings-ai-actions {
  display: flex;
  flex-wrap: wrap;
  gap: 10px;
  padding: 8px 0;
}

.cutflow-ai-settings summary {
  cursor: pointer;
  padding: 12px 0;
  font-size: 13px;
}
</style>
