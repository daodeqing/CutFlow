import { mount, flushPromises } from '@vue/test-utils';
import { createPinia, setActivePinia } from 'pinia';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import LocalizationPage from './LocalizationPage.vue';
const invoke = vi.hoisted(() => vi.fn());
vi.mock('@tauri-apps/api/core', () => ({ invoke, convertFileSrc: (path: string) => path }));
vi.mock('../../shared/stores/queue', () => ({ useQueueStore: () => ({ tasks: [], list: async () => {} }) }));
const buttons = { UiButton: { template: '<button :disabled="$attrs.disabled" :type="$attrs.type || \'button\'"><slot /></button>' } };
const wrappers: Array<{ unmount: () => void }> = [];
const calls = (method: string) => invoke.mock.calls.filter(([, value]) => value.method === method);
const catalog = { connection: { baseUrl: 'http://127.0.0.1:8317/v1' }, models: [{ id: 'provider-unknown', kind: 'unknown' }], references: [], fault: null };
const completed = {
  id: 'local-1', operationId: 'op', method: 'cutflow.localize.run', params: {}, status: 'completed', stage: '已完成',
  result: { videoPath: 'D:/project/english.mp4', folderPath: 'D:/project', subtitlePath: 'D:/project/english.srt', title: 'Hello', sourceUrl: 'https://www.bilibili.com/video/BV1', warnings: ['请检查字幕'], reviewRequired: true },
};
let savedJobs: unknown[] = [];
function openPage() {
  const wrapper = mount(LocalizationPage, { global: { stubs: buttons } });
  wrappers.push(wrapper);
  return wrapper;
}
beforeEach(() => {
  setActivePinia(createPinia());
  savedJobs = [];
  invoke.mockImplementation(async (_command: string, input: { method: string }) => {
    switch (input.method) {
      case 'cutflow.creation.catalog': return catalog;
      case 'cutflow.jobs.list': return { jobs: savedJobs };
      case 'cutflow.localize.watch.list': return { watches: [] };
      case 'cutflow.localize.watch.add': return { id: 'watch-1' };
      case 'cutflow.localize.watch.scan': return { started: true };
      case 'cutflow.localize.start': return { jobId: 'new-job' };
      case 'cutflow.x.compose': return { opened: true, automaticPosting: false };
      case 'cutflow.x.browser': return { opened: true, prefilledText: true, automaticUpload: false };
      case 'cutflow.localize.post': return { videoDescription: 'A tour of a construction site.', postText: 'A construction site worth a closer look.' };
      default: throw new Error('Unexpected call: ' + input.method);
    }
  });
});
afterEach(() => { wrappers.splice(0).forEach((wrapper) => wrapper.unmount()); });
describe('English localization', () => {
  it('requires rights and submits the chosen model with the low-storage default', async () => {
    const wrapper = openPage(); await flushPromises();
    expect(wrapper.find('input[id]').exists()).toBe(true);
    await wrapper.find('input[type=url]').setValue('https://www.bilibili.com/video/BV123');
    await wrapper.find('form').trigger('submit');
    expect(calls('cutflow.localize.start')).toHaveLength(0);
    await wrapper.find('input[type=checkbox]').setValue(true);
    await wrapper.find('form').trigger('submit'); await flushPromises();
    expect(calls('cutflow.localize.start')).toHaveLength(1);
    expect(calls('cutflow.localize.start')[0][1].params).toMatchObject({
      model: 'gemini-3.8-flash-high', quality: '80', url: 'https://www.bilibili.com/video/BV123',
    });
  });
  it('keeps the model list open during other updates and allows custom values', async () => {
    const wrapper = openPage(); await flushPromises();
    const input = wrapper.find('input[placeholder="留空使用默认模型"]');
    await wrapper.find('button[aria-label="选择字幕翻译模型"]').trigger('click');
    expect(wrapper.find('[role="listbox"]').exists()).toBe(true);
    await wrapper.find('input[placeholder="粘贴与视频相关的热点或关键词"]').setValue('construction');
    expect(wrapper.find('[role="listbox"]').exists()).toBe(true);
    await wrapper.find('[role="option"]').trigger('click');
    expect((input.element as HTMLInputElement).value).toBe('provider-unknown');
    expect(wrapper.find('[role="listbox"]').exists()).toBe(false);
    await input.setValue('my-custom-model');
    expect((input.element as HTMLInputElement).value).toBe('my-custom-model');
  });
  it('regenerates an existing post without creating a video task', async () => {
    savedJobs = [completed];
    const wrapper = openPage(); await flushPromises();
    await wrapper.find('textarea[placeholder="例如：写得更有悬念，突出施工现场的整洁"]').setValue('Make the hook punchier');
    await wrapper.findAll('button').find((button) => button.text().includes('重新生成简介和推文'))!.trigger('click');
    await flushPromises();
    expect(calls('cutflow.localize.post')).toHaveLength(1);
    expect(calls('cutflow.localize.post')[0][1].params).toMatchObject({ instruction: 'Make the hook punchier', jobId: 'local-1' });
    expect(calls('cutflow.localize.start')).toHaveLength(0);
    const texts = wrapper.findAll('textarea');
    expect((texts[0].element as HTMLTextAreaElement).value).toContain('A tour of a construction site.');
    expect((texts[1].element as HTMLTextAreaElement).value).toContain('A construction site worth a closer look.');
    expect((texts[1].element as HTMLTextAreaElement).value).toContain('https://www.bilibili.com/video/BV1');
  });
  it('only enables monitoring after confirming rights for future videos', async () => {
    const wrapper = openPage(); await flushPromises();
    const input = wrapper.find('input[placeholder="https://space.bilibili.com/…"]');
    await input.setValue('https://space.bilibili.com/123/video');
    const add = () => wrapper.findAll('button').find((button) => button.text().includes('添加监控'))!;
    expect(add().attributes('disabled')).toBeDefined();
    await wrapper.findAll('input[type=checkbox]')[2].setValue(true);
    await add().trigger('click'); await flushPromises();
    expect(calls('cutflow.localize.watch.add')).toHaveLength(1);
    expect(calls('cutflow.localize.watch.scan')).toHaveLength(1);
  });
  it('opens X only after the result has been marked reviewed', async () => {
    savedJobs = [completed];
    const wrapper = openPage(); await flushPromises();
    const xButton = () => wrapper.findAll('button').find((button) => button.text().includes('打开 CutFlow 内的 X 窗口'))!;
    expect(xButton().attributes('disabled')).toBeDefined();
    expect(calls('cutflow.x.compose')).toHaveLength(0);
    const checks = wrapper.findAll('input[type=checkbox]');
    await checks[checks.length - 1].setValue(true);
    await xButton().trigger('click'); await flushPromises();
    expect(calls('cutflow.x.compose')).toHaveLength(1);
    await wrapper.findAll('button').find((button) => button.text().includes('用浏览器打开 X'))!.trigger('click'); await flushPromises();
    expect(calls('cutflow.x.browser')[0][1].params.text).toContain('Hello');
    expect(calls('cutflow.x.browser')[0][1].params.text).toContain('https://www.bilibili.com/video/BV1');
    expect(wrapper.text()).toContain('确认账号并上传视频');
  });
});
