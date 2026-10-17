import { defineStore } from 'pinia'

export type CreationKind = 'image' | 'video'
export interface CreationDraft {
  model: string; modelSource: 'catalog' | 'custom'; prompt: string; aspect: string
  imageSize: string; count: string; duration: string; references: string[]; stream: boolean
}
const emptyDraft = (): CreationDraft => ({ model: '', modelSource: 'catalog', prompt: '', aspect: 'auto', imageSize: 'auto', count: '1', duration: '8', references: [], stream: true })

// 草稿按创作类别隔离，仅保存在本次应用内存中，不写入密钥或任务历史。
export const useCreationStore = defineStore('creation', {
  state: () => ({ kind: 'image' as CreationKind, drafts: { image: emptyDraft(), video: emptyDraft() } }),
})
