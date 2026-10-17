import { onActivated, onDeactivated, onUnmounted, shallowRef, watch } from 'vue'
import { parseProgress } from '../../api/tauri'

export interface ParseActivitySnapshot {
  active: boolean
  waitingUntil: number
  queued: boolean
  now: number
}

type ActivityListener = (snapshot: ParseActivitySnapshot) => void
interface ActivityEntry {
  snapshot: ParseActivitySnapshot
  listeners: Set<ActivityListener>
  inFlight: boolean
  requestedAt: number
}

// 所有可见进度标签共用一个时钟，同一来源只发一份只读查询。
const POLL_INTERVAL_MS = 1_000
const MAX_CACHED_SOURCES = 64
const entries = new Map<string, ActivityEntry>()
let timer: ReturnType<typeof setTimeout> | undefined
let listening = false
const emptySnapshot = (): ParseActivitySnapshot => ({ active: false, waitingUntil: 0, queued: false, now: Date.now() })
const hasSubscribers = () => [...entries.values()].some(entry => entry.listeners.size > 0)

function publish(entry: ActivityEntry) {
  entry.snapshot = { ...entry.snapshot, now: Date.now() }
  for (const listener of entry.listeners) listener(entry.snapshot)
}

async function refresh(sourceId: string, entry: ActivityEntry) {
  if (entry.inFlight || document.hidden || !entry.listeners.size || Date.now() - entry.requestedAt < POLL_INTERVAL_MS) return
  entry.inFlight = true
  entry.requestedAt = Date.now()
  try {
    const progress = await parseProgress(sourceId)
    entry.snapshot = {
      active: progress.active,
      waitingUntil: progress.active ? entry.requestedAt + progress.waiting_seconds * 1_000 : 0,
      queued: progress.active && progress.queued,
      now: Date.now(),
    }
    if (!document.hidden) publish(entry)
  } catch {
    // 查询失败保留最后状态；倒计时仍按墙上时钟到期，不改变后台解析。
  } finally {
    entry.inFlight = false
  }
}

function stopClock() {
  clearTimeout(timer)
  timer = undefined
}

function scheduleClock() {
  if (timer !== undefined || document.hidden || !hasSubscribers()) return
  timer = setTimeout(() => {
    timer = undefined
    for (const [sourceId, entry] of entries) {
      if (!entry.listeners.size) continue
      publish(entry)
      void refresh(sourceId, entry)
    }
    scheduleClock()
  }, POLL_INTERVAL_MS)
}

function resumeVisible() {
  if (document.hidden) {
    stopClock()
    return
  }
  for (const [sourceId, entry] of entries) {
    if (!entry.listeners.size) continue
    publish(entry)
    void refresh(sourceId, entry)
  }
  scheduleClock()
}

function attachListeners() {
  if (listening) return
  document.addEventListener('visibilitychange', resumeVisible)
  window.addEventListener('focus', resumeVisible)
  listening = true
}

function detachListeners() {
  stopClock()
  document.removeEventListener('visibilitychange', resumeVisible)
  window.removeEventListener('focus', resumeVisible)
  listening = false
}

function trimCache() {
  for (const [sourceId, entry] of entries) {
    if (entries.size <= MAX_CACHED_SOURCES) break
    if (!entry.listeners.size && !entry.inFlight) entries.delete(sourceId)
  }
}

export function subscribeParseActivity(sourceId: string, listener: ActivityListener): () => void {
  let entry = entries.get(sourceId)
  if (!entry) {
    entry = { snapshot: emptySnapshot(), listeners: new Set(), inFlight: false, requestedAt: Number.NEGATIVE_INFINITY }
    entries.set(sourceId, entry)
  }
  const current = entry
  current.listeners.add(listener)
  attachListeners()
  if (!document.hidden) {
    listener({ ...current.snapshot, now: Date.now() })
    void refresh(sourceId, current)
    scheduleClock()
  }
  let released = false
  return () => {
    if (released) return
    released = true
    current.listeners.delete(listener)
    if (!hasSubscribers()) detachListeners()
    trimCache()
  }
}

export function useParseActivity(sourceId: () => string) {
  const activity = shallowRef<ParseActivitySnapshot>(emptySnapshot())
  let active = true
  let subscribedSource = ''
  let release: (() => void) | undefined
  const stop = () => {
    release?.()
    release = undefined
    subscribedSource = ''
  }
  const start = () => {
    const source = sourceId()
    if (!active || !source || (release && subscribedSource === source)) return
    stop()
    activity.value = emptySnapshot()
    subscribedSource = source
    release = subscribeParseActivity(source, snapshot => { activity.value = snapshot })
  }
  watch(sourceId, () => {
    stop()
    if (active) start()
  }, { immediate: true })
  onActivated(() => { active = true; start() })
  onDeactivated(() => { active = false; stop() })
  onUnmounted(() => { active = false; stop() })
  return activity
}
