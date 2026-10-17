import { mount } from '@vue/test-utils';
import { describe, expect, it } from 'vitest';
import ProgressBar from './ProgressBar.vue';

describe('liquid task progress', () => {
  it('flows only while a task is actively progressing', async () => {
    const wrapper = mount(ProgressBar, { props: { value: 68, status: 'downloading', label: '示例 · 下载中' } });
    expect(wrapper.classes()).toContain('is-flowing');
    expect(wrapper.attributes('aria-label')).toBe('示例 · 下载中');
    for (const status of ['paused', 'error', 'queued', 'done', 'warning', 'ready'] as const) {
      await wrapper.setProps({ status });
      expect(wrapper.classes()).not.toContain('is-flowing');
      expect(wrapper.attributes('data-status')).toBe(status);
      expect(wrapper.attributes('aria-valuenow')).toBe('68');
    }
    await wrapper.setProps({ status: 'downloading', value: 100 });
    expect(wrapper.classes()).not.toContain('is-flowing');
    await wrapper.setProps({ value: 0 });
    expect(wrapper.classes()).not.toContain('is-flowing');
  });
  it('preserves the actual downloaded proportion', () => {
    const wrapper = mount(ProgressBar, { props: { value: 34, max: 50 } });
    expect(wrapper.attributes('aria-valuenow')).toBe('68');
    expect(wrapper.find('.progress-fill').attributes('style')).toContain('width: 68%');
  });
  it.each([
    [-5, 100, 0],
    [150, 100, 100],
    [68, 0, 0],
    [NaN, 100, 0],
    [Infinity, 100, 0],
    [34, Infinity, 0],
  ])('bounds invalid progress %s / %s', (value, max, expected) => {
    const wrapper = mount(ProgressBar, { props: { value, max, status: 'downloading' } });
    expect(wrapper.attributes('aria-valuenow')).toBe(String(expected));
    expect(wrapper.classes()).not.toContain('is-flowing');
  });
});
