import { describe, expect, it } from 'vitest'

import {
  formatSpeedLimit,
  speedLimitMbError,
  toBytesPerSecond,
  toMbPerSecondInput,
} from './speedLimit'

describe('speed limit conversion', () => {
  it('treats an empty value as unlimited', () => {
    expect(toBytesPerSecond('')).toBeUndefined()
    expect(speedLimitMbError('')).toBeNull()
  })

  it('converts decimal MB per second to exact bytes per second', () => {
    expect(toBytesPerSecond('2.5')).toBe(2_500_000)
    expect(toMbPerSecondInput(2_500_000)).toBe('2.5')
  })

  it('round-trips persisted byte limits without losing small values', () => {
    for (const bytesPerSecond of [1, 64 * 1024, 2_621_441, 10 * 1024 * 1024 * 1024]) {
      expect(toBytesPerSecond(toMbPerSecondInput(bytesPerSecond))).toBe(bytesPerSecond)
    }
  })

  it('rejects zero and values above ten GiB per second', () => {
    expect(speedLimitMbError('0')).toContain('大于 0')
    expect(speedLimitMbError('10737.418241')).toContain('10737.41824')
    expect(speedLimitMbError('0.0000001')).toContain('1 B/s')
    expect(speedLimitMbError('0.0000005')).toContain('1 B/s')
  })

  it('formats task limits for compact queue UI', () => {
    expect(formatSpeedLimit(2_500_000)).toBe('2.5 MB/s')
    expect(formatSpeedLimit(null)).toBe('不限速')
    expect(formatSpeedLimit(1)).toBe('1 B/s')
  })
})
