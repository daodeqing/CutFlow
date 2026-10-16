import { invoke, convertFileSrc } from '@tauri-apps/api/core'

export interface CutflowError { code: string; message: string }
export interface AiModel { id: string; description: string; kind: string; maxReferences: number; duration: number | null; aspect: string | null }
export interface Asset { path: string; width?: number; height?: number; duration?: number; hasAudio?: boolean }
export interface Job {
  id: string; operationId: string; method: string; params: Record<string, unknown>; status: string; stage: string
  canRetryDownload?: boolean; result?: { assets?: Asset[]; thumbnail?: string } | null; error?: CutflowError | null
}
export interface Reference { id: string; kind: string; name: string; path: string; prompt: string }
export interface HubSnapshot { connection: { baseUrl: string }; models: AiModel[]; references: Reference[]; jobs: Job[]; fault: string | null }
export interface Project { projectPath: string; revision: number }
export interface EditorStatus { running: boolean; current?: Project | null }
export interface RecentProject { path: string; name: string }

export const errorMessage = (error: unknown): string => {
  if (error && typeof error === 'object' && 'message' in error) return String(error.message)
  return String(error)
}

export const cutflow = <T = unknown>(method: string, params: Record<string, unknown> = {}) =>
  invoke<T>('cutflow_call', { method, params })

export const mediaSource = (path: string) => convertFileSrc(path)
export const operationId = () => crypto.randomUUID()
