import { flushPromises, mount } from '@vue/test-utils'
import { h, KeepAlive, ref } from 'vue'
import { afterEach, beforeEach, expect, it, vi } from 'vitest'
import ParseActivityStatus from './ParseActivityStatus.vue'
import { parseProgress } from '../api/tauri'

vi.mock('../api/tauri', () => ({ parseProgress: vi.fn() }))
const mounted: Array<{ unmount(): void }> = []
const status = (sourceId: string) => {
  const wrapper = mount(ParseActivityStatus, { props: { sourceId } })
  mounted.push(wrapper)
  return wrapper
}
beforeEach(() => {
  vi.useFakeTimers()
  vi.mocked(parseProgress).mockReset()
  vi.spyOn(document, 'hidden', 'get').mockReturnValue(false)
})
afterEach(() => {
  for (const wrapper of mounted.splice(0)) wrapper.unmount()
  vi.useRealTimers()
  vi.restoreAllMocks()
})

it('shares one query and clock between labels for the same source', async () => {
  vi.mocked(parseProgress).mockResolvedValue({ active: true, waiting_seconds: 3, queued: false })
  const first = status('shared-source')
  const second = status('shared-source')
  await flushPromises()
  expect(parseProgress).toHaveBeenCalledTimes(1)
  expect(first.text()).toBe('休息 3 秒后继续')
  expect(second.text()).toBe(first.text())
  await vi.advanceTimersByTimeAsync(1_000)
  expect(parseProgress).toHaveBeenCalledTimes(2)
  first.unmount()
  await vi.advanceTimersByTimeAsync(1_000)
  expect(parseProgress).toHaveBeenCalledTimes(3)
  second.unmount()
  await vi.advanceTimersByTimeAsync(3_000)
  expect(parseProgress).toHaveBeenCalledTimes(3)
})

it('pauses hidden-window reads and immediately expires stale countdowns on return', async () => {
  const hidden = vi.spyOn(document, 'hidden', 'get')
  vi.mocked(parseProgress).mockResolvedValueOnce({ active: true, waiting_seconds: 3, queued: false })
    .mockImplementation(() => new Promise(() => {}))
  const wrapper = status('hidden-source')
  await flushPromises()
  expect(wrapper.text()).toContain('休息 3 秒')
  hidden.mockReturnValue(true)
  document.dispatchEvent(new Event('visibilitychange'))
  await vi.advanceTimersByTimeAsync(5_000)
  expect(parseProgress).toHaveBeenCalledTimes(1)
  hidden.mockReturnValue(false)
  document.dispatchEvent(new Event('visibilitychange'))
  await wrapper.vm.$nextTick()
  expect(wrapper.text()).toBe('正在解析…')
  expect(parseProgress).toHaveBeenCalledTimes(2)
})

it('releases a KeepAlive page subscription without cancelling or forgetting its status', async () => {
  vi.mocked(parseProgress).mockResolvedValue({ active: true, waiting_seconds: 0, queued: true })
  const shown = ref(true)
  const wrapper = mount({
    setup: () => () => h(KeepAlive, null, { default: () => shown.value ? h(ParseActivityStatus, { sourceId: 'cached-source' }) : null }),
  })
  mounted.push(wrapper)
  await flushPromises()
  expect(wrapper.text()).toBe('等待其他解析完成…')
  shown.value = false
  await wrapper.vm.$nextTick()
  await vi.advanceTimersByTimeAsync(3_000)
  expect(parseProgress).toHaveBeenCalledTimes(1)
  shown.value = true
  await wrapper.vm.$nextTick()
  expect(wrapper.text()).toBe('等待其他解析完成…')
  await flushPromises()
  expect(parseProgress).toHaveBeenCalledTimes(2)
})

it('expires the countdown after suspension while a status request stalls without overlapping reads', async () => {
  vi.mocked(parseProgress).mockResolvedValueOnce({ active: true, waiting_seconds: 3, queued: false })
    .mockImplementation(() => new Promise(() => {}))
  const wrapper = status('stalled-source')
  await flushPromises()
  await vi.advanceTimersByTimeAsync(1_000)
  vi.setSystemTime(Date.now() + 30_000)
  window.dispatchEvent(new Event('focus'))
  await wrapper.vm.$nextTick()
  expect(wrapper.text()).toBe('正在解析…')
  await vi.advanceTimersByTimeAsync(5_000)
  expect(parseProgress).toHaveBeenCalledTimes(2)
})

it('never applies a late previous-source response to the newly selected source', async () => {
  let finishOld!: (value: { active: boolean; waiting_seconds: number; queued: boolean }) => void
  vi.mocked(parseProgress).mockImplementation(sourceId => sourceId === 'old-source'
    ? new Promise(resolve => { finishOld = resolve })
    : Promise.resolve({ active: true, waiting_seconds: 0, queued: true }))
  const wrapper = status('old-source')
  await wrapper.setProps({ sourceId: 'new-source' })
  await flushPromises()
  expect(wrapper.text()).toBe('等待其他解析完成…')
  finishOld({ active: true, waiting_seconds: 99, queued: false })
  await flushPromises()
  expect(wrapper.text()).toBe('等待其他解析完成…')
})
