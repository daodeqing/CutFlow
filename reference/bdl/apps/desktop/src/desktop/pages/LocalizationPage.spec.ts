import { mount, flushPromises } from '@vue/test-utils';
import { createPinia, setActivePinia } from 'pinia';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import LocalizationPage from './LocalizationPage.vue';
const invoke = vi.hoisted(() => vi.fn());
const queueState = vi.hoisted(() => ({ tasks: [] as Array<{ id: string; title: string; status: string; output_path: string }> }));
vi.mock('@tauri-apps/api/core', () => ({ invoke, convertFileSrc: (path: string) => path }));
vi.mock('../../shared/stores/queue', () => ({ useQueueStore: () => ({ ...queueState, list: async () => {} }) }));
const buttons = { UiButton: { template: '<button :disabled="$attrs.disabled" :type="$attrs.type || \'button\'"><slot /></button>' } };
const wrappers: Array<{ unmount: () => void }> = [];
const calls = (method: string) => invoke.mock.calls.filter(([, value]) => value.method === method);
const catalog = { connection: { baseUrl: 'http://127.0.0.1:8317/v1' }, models: [{ id: 'provider-unknown', kind: 'unknown' }], references: [], fault: null };
const parsed = {
  source: { id: 'video:BV123', kind: 'video', input: 'https://www.bilibili.com/video/BV123', title: '示例', loaded_count: 1, total_count: 1, has_more: false },
  groups: [{ id: 'group-1', kind: 'video', title: '示例', page: null, items: [{ id: 'item-1', title: '示例', owner_name: null, cover_url: null, duration_seconds: 60, parts: [{ id: 'part-1', title: 'P1', aid: 1, bvid: 'BV123', cid: 1, duration_seconds: 60, streams: [
    { id: 'video-1080', kind: 'video', quality: { kind: 'quality', value: 80 }, codec: 'avc', bandwidth: 1, urls: ['https://fixture.invalid/video-80'], headers: [], acquired_at: '2026-01-01T00:00:00Z' },
    { id: 'video-720', kind: 'video', quality: { kind: 'quality', value: 64 }, codec: 'avc', bandwidth: 1, urls: ['https://fixture.invalid/video-64'], headers: [], acquired_at: '2026-01-01T00:00:00Z' },
  ], assets: [] }] }] }],
};
const completed = {
  id: 'local-1', operationId: 'op', method: 'cutflow.localize.run', params: {}, status: 'completed', stage: '已完成',
  result: { videoPath: 'D:/project/english.mp4', folderPath: 'D:/project', subtitlePath: 'D:/project/english.srt', title: 'Hello', sourceUrl: 'https://www.bilibili.com/video/BV1', warnings: ['请检查字幕'], reviewRequired: true },
};
let savedJobs: unknown[] = [];
let parsedSource = structuredClone(parsed);
function openPage() {
  const wrapper = mount(LocalizationPage, { global: { stubs: buttons } });
  wrappers.push(wrapper);
  return wrapper;
}
beforeEach(() => {
  setActivePinia(createPinia());
  savedJobs = [];
  queueState.tasks = [];
  parsedSource = structuredClone(parsed);
  invoke.mockImplementation(async (_command: string, input: { method: string }) => {
    switch (input.method) {
      case 'cutflow.creation.catalog': return catalog;
      case 'cutflow.bilibili.parse': return parsedSource;
      case 'cutflow.jobs.list': return { jobs: savedJobs };
      case 'cutflow.jobs.get': return { id: 'youtube-job', status: 'completed', stage: '已完成', method: 'cutflow.youtube.upload', params: {}, result: null };
      case 'cutflow.localize.watch.list': return { watches: [] };
      case 'cutflow.localize.watch.add': return { id: 'watch-1' };
      case 'cutflow.localize.watch.scan': return { started: true };
      case 'cutflow.localize.start': return { jobId: 'new-job' };
      case 'cutflow.x.compose': return { opened: true, automaticPosting: false };
      case 'cutflow.x.browser': return { opened: true, prefilledText: true, automaticUpload: false };
      case 'cutflow.youtube.upload': return { jobId: 'youtube-job', status: 'running', automaticUpload: true, published: false };
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
      model: 'gemini-3.8-flash-high', quality: 'best', url: 'https://www.bilibili.com/video/BV123',
    });
    expect(wrapper.find('select').text()).toContain('1080p · 80');
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
  it('marks a completed localization and opens the reviewed YouTube upload handoff', async () => {
    savedJobs = [completed];
    const wrapper = openPage(); await flushPromises();
    expect(wrapper.text()).toContain('英文视频标题');
    expect((wrapper.find('input[maxlength="100"]').element as HTMLInputElement).value).toBe('Hello');
    const checks = wrapper.findAll('input[type=checkbox]');
    await checks[checks.length - 1].setValue(true);
    const upload = wrapper.findAll('button').find((button) => button.text().includes('上传到 YouTube'))!;
    expect(upload.attributes('disabled')).toBeDefined();
    await wrapper.find('textarea').setValue('A short description for YouTube.');
    await checks[checks.length - 1].setValue(true);
    expect(upload.attributes('disabled')).toBeUndefined();
    await upload.trigger('click'); await flushPromises();
    expect(calls('cutflow.youtube.upload')).toHaveLength(1);
    expect(calls('cutflow.youtube.upload')[0][1].params).toMatchObject({ title: 'Hello', channelUrl: 'https://www.youtube.com/@deqingdao/videos' });
    expect(wrapper.text()).toContain('YouTube Studio');
  });
  it('highlights historical completed videos by source path and opens their matching result', async () => {
    queueState.tasks = [{ id: 'old-download', title: 'An earlier performance', status: 'completed', output_path: 'D:\\downloads\\source.mp4' }];
    savedJobs = [{ ...completed, result: { ...completed.result, sourcePath: 'd:/downloads/source.mp4' } }];
    const wrapper = openPage(); await flushPromises();
    await wrapper.find('input[value="task"]').setValue(true);
    const choice = wrapper.find('.task-options button');
    expect(choice.classes()).toContain('produced');
    expect(choice.text()).toContain('已制作');
    await choice.trigger('click');
    expect(wrapper.find('.task-picker summary').classes()).toContain('produced');
    expect(wrapper.find('.produced-state').text()).toContain('已完成英文制作');
  });
  it('only offers streams returned for the selected first part, including real 4K', async () => {
    const part = parsedSource.groups[0].items[0].parts[0];
    part.streams.push({ ...part.streams[0], id: '4k', quality: { kind: 'quality', value: 120 } });
    parsedSource.groups[0].items[0].parts.push({ ...part, id: 'part-2', streams: [{ ...part.streams[0], quality: { kind: 'quality', value: 127 } }] });
    const wrapper = openPage(); await flushPromises();
    await wrapper.find('input[type=url]').setValue('https://www.bilibili.com/video/BV123');
    await wrapper.findAll('button').find((button) => button.text() === '读取可用画质')!.trigger('click'); await flushPromises();
    expect(wrapper.find('select').text()).toContain('4K · 120');
    expect(wrapper.find('select').text()).not.toContain('8K');
  });
});
