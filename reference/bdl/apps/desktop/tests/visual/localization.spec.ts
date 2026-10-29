import { expect, test } from '@playwright/test'
import { installCreationStudioMock } from './creation-studio.mock'

for (const theme of ['light', 'dark'] as const) {
  test(theme + ' localization controls remain usable in a narrow window', async ({ page }) => {
    await page.setViewportSize({ width: 1024, height: 768 })
    await installCreationStudioMock(page, theme)
    await page.addInitScript(() => {
      const internals = (window as unknown as { __TAURI_INTERNALS__: { invoke: (command: string, args?: { method?: string }) => Promise<unknown> } }).__TAURI_INTERNALS__
      const original = internals.invoke
      internals.invoke = async (command, args) => {
        if (command === 'cutflow_call' && args?.method === 'cutflow.localize.watch.list') return { watches: [] }
        return original(command, args)
      }
    })
    await page.goto('/')
    await page.getByRole('tab', { name: '英文制作' }).click()
    const section = page.locator('.localize-page')
    await expect(section.getByRole('heading', { name: '英文制作' })).toBeVisible()
    await expect(section.getByRole('button', { name: '开始制作' })).toBeDisabled()
    await section.getByRole('textbox', { name: '视频链接' }).fill('https://www.bilibili.com/video/BV123')
    await section.getByRole('checkbox', { name: '我有权下载、翻译并发布所选视频及其中素材。' }).check()
    await expect(section.getByRole('button', { name: '开始制作' })).toBeEnabled()
    await section.getByText('自动监控收藏夹 / UP 主').click()
    await expect(section.getByRole('button', { name: '添加监控' })).toBeDisabled()
    expect(await page.evaluate(() => document.documentElement.scrollWidth)).toBe(1024)
  })
}
