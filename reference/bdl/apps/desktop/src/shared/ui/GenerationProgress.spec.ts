import { mount } from '@vue/test-utils';
import { describe, expect, it } from 'vitest';
import type { Job } from '../api/cutflow';
import GenerationProgress from './GenerationProgress.vue';

const job = (progress?: Job['progress']): Job => ({
  id: 'job-a', operationId: 'op-a', method: 'cutflow.generate', params: { kind: 'image' },
  status: 'running', stage: 'generating', progress,
});

describe('creation stage progress', () => {
  it('shows a provider stage percentage without claiming elapsed-time completion', async () => {
    const wrapper = mount(GenerationProgress, { props: { job: job({ stage: '正在放大图片', percent: 82, basis: 'stage' }) } });
    expect(wrapper.text()).toContain('阶段进度 82%');
    expect(wrapper.text()).toContain('正在放大图片');
    expect(wrapper.get('[role="progressbar"]').attributes('aria-valuenow')).toBe('82');
    expect(wrapper.find('.generation-spinner').exists()).toBe(true);
    await wrapper.setProps({ job: job({ stage: '正在保存作品', percent: 100, basis: 'stage' }) });
    expect(wrapper.text()).toContain('阶段进度 99%');
  });

  it('animates an indeterminate state when an older provider supplies no numeric progress', () => {
    const wrapper = mount(GenerationProgress, { props: { job: job() } });
    expect(wrapper.text()).toContain('正在生成');
    expect(wrapper.text()).toContain('等待进度');
    expect(wrapper.text()).not.toContain('%');
    expect(wrapper.get('[role="progressbar"]').attributes('aria-valuenow')).toBeUndefined();
    expect(wrapper.find('.generation-indeterminate').exists()).toBe(true);
  });

  it('requests stop once and disables the control while stopping', async () => {
    const wrapper = mount(GenerationProgress, { props: { job: job({ stage: '正在生成', percent: 48, basis: 'stage' }) } });
    await wrapper.get('button').trigger('click');
    expect(wrapper.emitted('stop')).toHaveLength(1);
    await wrapper.setProps({ stopping: true });
    expect(wrapper.get('button').attributes('disabled')).toBeDefined();
    expect(wrapper.text()).toContain('阶段进度 48%');
    expect(wrapper.classes()).toContain('is-stopping');
    expect(wrapper.get('[role="progressbar"]').attributes('data-status')).toBe('paused');
  });
});
