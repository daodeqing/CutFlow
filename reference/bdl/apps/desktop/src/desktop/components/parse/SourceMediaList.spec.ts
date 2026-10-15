import { mount } from '@vue/test-utils';
import { describe, expect, it, vi } from 'vitest';
import SourceMediaList from './SourceMediaList.vue';
import MediaListRow from '../../../shared/ui/MediaListRow.vue';
import UiCheckbox from '../../../shared/ui/Checkbox.vue';
import type { NormalizedSourceTree } from '../../../shared/api/dto';

vi.mock('../../../shared/stores/queue', () => ({
  useQueueStore: () => ({ tasks: [], outputSizesByTask: {}, loadOutputSizes: vi.fn() }),
}));
vi.mock('../../../shared/stores/ui', () => ({ useUiStore: () => ({ pushToast: vi.fn() }) }));
const source: NormalizedSourceTree = {
  source: {
    id: 'test',
    kind: 'collection',
    title: '万条合集',
    input: '',
    loaded_count: 10000,
    total_count: 10000,
    has_more: false,
  },
  groups: [],
};

describe('source list virtualization', () => {
  it('bounds 10k rows, reaches the final row and preserves offscreen selection', async () => {
    const rows = Array.from({ length: 10000 }, (_, index) => ({
      id: `row:${index}`,
      title: `视频 ${index}`,
      partIds: [`part:${index}`],
    }));
    const wrapper = mount(SourceMediaList, {
      props: { source, rows, selectedIds: rows.map((row) => row.partIds[0]) },
      global: { stubs: { MediaListRow: true, UiCheckbox: true } },
    });
    expect(wrapper.findAllComponents(MediaListRow).length).toBeLessThan(30);
    expect(wrapper.findComponent(UiCheckbox).props('modelValue')).toBe(true);
    const scroll = wrapper.get('.source-list-body');
    Object.defineProperty(scroll.element, 'scrollTop', { value: 1040000 - 600, configurable: true });
    Object.defineProperty(scroll.element, 'clientHeight', { value: 600, configurable: true });
    await scroll.trigger('scroll');
    const last = wrapper.findAllComponents(MediaListRow).at(-1)!;
    expect(last.attributes('aria-posinset')).toBe('10000');
    expect(last.props('selected')).toBe(true);
    last.vm.$emit('toggle');
    expect(wrapper.emitted('toggle')?.[0]).toEqual(['row:9999']);
    wrapper.findComponent(UiCheckbox).vm.$emit('update:modelValue', false);
    expect(wrapper.emitted('toggleAll')).toHaveLength(1);
    await wrapper.setProps({ rows: rows.slice(0, 5) });
    expect(wrapper.findAllComponents(MediaListRow)).toHaveLength(5);
    expect(wrapper.findAll('.source-list-spacer')).toHaveLength(0);
    wrapper.unmount();
  });
});
