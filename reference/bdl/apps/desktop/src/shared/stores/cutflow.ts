import { defineStore } from 'pinia'
import { useUiStore } from './ui'
import { operationId } from '../api/cutflow'
import type { Project } from '../api/cutflow'

interface PendingImport { files: string[]; operationId: string; jobId?: string; target?: Project }
const loadPending = (): PendingImport | null => {
  try { return JSON.parse(localStorage.getItem('cutflow.pendingImport') || 'null') as PendingImport | null }
  catch { return null }
}

export const useCutflowStore = defineStore('cutflow', {
  state: () => ({ pending: loadPending() }),
  actions: {
    savePending(pending: PendingImport | null) {
      this.pending = pending
      localStorage.setItem('cutflow.pendingImport', JSON.stringify(pending))
    },
    sendToEditor(files: string[]) {
      const current = this.pending
      if (current?.jobId) {
        useUiStore().pushToast('请先检查上一批素材的导入结果', 'warning')
      } else {
        this.savePending({ files: [...new Set([...(current?.files || []), ...files])], operationId: operationId() })
      }
      useUiStore().setTab('editor')
    },
  },
})
