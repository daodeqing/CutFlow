const MB = 1_000_000
const MAX_BYTES_PER_SECOND = 10 * 1024 * 1024 * 1024

export const speedLimitMbError = (value: string): string | null => {
  const trimmed = value.trim()
  if (!trimmed) {
    return null
  }

  const limit = Number(trimmed)
  if (!Number.isFinite(limit) || limit <= 0) {
    return '限速必须大于 0 MB/s，留空表示不限速。'
  }
  if (limit * MB < 1) {
    return '限速至少为 1 B/s（0.000001 MB/s）。'
  }
  if (limit * MB > MAX_BYTES_PER_SECOND) {
    return `限速不能超过 ${MAX_BYTES_PER_SECOND / MB} MB/s。`
  }
  return null
}

export const toBytesPerSecond = (value: string): number | undefined => {
  if (!value.trim()) {
    return undefined
  }
  if (speedLimitMbError(value)) {
    return undefined
  }
  return Math.round(Number(value) * MB)
}

export const toMbPerSecondInput = (value: number | null | undefined): string => {
  if (!value || value <= 0) {
    return ''
  }
  return String(Number((value / MB).toFixed(6)))
}

export const formatSpeedLimit = (value: number | null | undefined): string => {
  if (!value || value <= 0) {
    return '不限速'
  }
  if (value < 1000) {
    return `${Math.round(value)} B/s`
  }
  if (value < MB) {
    return `${Number((value / 1000).toFixed(1))} KB/s`
  }
  return `${Number((value / MB).toFixed(1))} MB/s`
}
