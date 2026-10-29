import { defineStore } from 'pinia'
import { FEEDBACK_AUTO_DISMISS_MS } from './feedback'
import { isMobilePlatform } from '../platform/environment'
import type { SettingsSectionId } from '../features/settings/settingsSections'

export type AppTab = 'parse' | 'library' | 'transfer' | 'settings' | 'about' | 'bilibili' | 'creation' | 'editor'
type NavigationGuard = (leave: () => void) => void

export interface ToastMessage {
  id: number
  tone: 'info' | 'success' | 'warning' | 'danger'
  message: string
  action?: ToastAction
}

export interface ToastAction {
  label: string
  tab: AppTab
}

export const useUiStore = defineStore('ui', {
  state: () => ({
    activeTab: (isMobilePlatform() ? 'parse' : 'bilibili') as AppTab,
    bilibiliSubTab: 'parse' as 'parse' | 'library' | 'transfer' | 'localize',
    localizeTaskId: '',
    mobilePersonalPage: 'home' as 'home' | 'settings' | 'about',
    drawerOpen: false,
    dialogOpen: false,
    toasts: [] as ToastMessage[],
    nextToastId: 1,
    loginDialogOpen: false,
    environmentDialogOpen: false,
    settingsNavigationGuard: null as NavigationGuard | null,
    settingsSection: 'settings-download' as SettingsSectionId,
  }),
  actions: {
    setTab(tab: AppTab) {
      if (!isMobilePlatform() && (tab === 'parse' || tab === 'library' || tab === 'transfer')) {
        this.bilibiliSubTab = tab
        tab = 'bilibili'
      }
      if (tab === this.activeTab) return
      const leave = () => { this.activeTab = tab }
      const guard = this.activeTab === 'settings' ? this.settingsNavigationGuard : null
      if (guard) guard(leave)
      else leave()
    },
    openSettings(section: SettingsSectionId) {
      this.settingsSection = section
      this.setTab('settings')
    },
    setSettingsNavigationGuard(guard: NavigationGuard | null) {
      this.settingsNavigationGuard = guard
    },
    openLoginDialog() {
      this.loginDialogOpen = true
    },
    openEnvironmentDialog() {
      this.environmentDialogOpen = true
    },
    pushToast(message: string, tone: ToastMessage['tone'] = 'info', action?: ToastAction) {
      const id = this.nextToastId
      this.toasts.push({ id, message, tone, action })
      this.nextToastId += 1

      if (tone !== 'danger' && typeof window !== 'undefined') {
        window.setTimeout(() => {
          this.removeToast(id)
        }, FEEDBACK_AUTO_DISMISS_MS)
      }
    },
    removeToast(id: number) {
      this.toasts = this.toasts.filter((toast) => toast.id !== id)
    },
  },
})
