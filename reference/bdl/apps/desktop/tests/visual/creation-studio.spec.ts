import { expect, test, type Page } from '@playwright/test'
import { mkdir, writeFile } from 'node:fs/promises'
import { resolve } from 'node:path'
import { installCreationStudioMock, type FixtureCall, type StudioFixtureOptions } from './creation-studio.mock'

const evidence = resolve(process.cwd(), '../../../../docs/verification-20261018-creation')
const pageErrors = new WeakMap<Page, string[]>()
test.beforeEach(({ page }) => {
  const errors: string[] = []
  pageErrors.set(page, errors)
  page.on('pageerror', error => errors.push(error.message))
})
test.afterEach(({ page }) => { expect(pageErrors.get(page)).toEqual([]) })

const openStudio = async (page: Page, theme: 'light' | 'dark' = 'light', options: StudioFixtureOptions = {}) => {
  await installCreationStudioMock(page, theme, options)
  await page.goto('/')
  await page.getByRole('button', { name: 'AI 创作', exact: true }).click()
  await expect(page.locator('.creation-studio')).toBeVisible()
  await expect(page.locator('.creation-turn').first()).toBeVisible()
}
const calls = (page: Page, method: string) => page.evaluate(method => (
  window as unknown as { __CREATION_FIXTURE__: { calls: FixtureCall[] } }
).__CREATION_FIXTURE__.calls.filter(call => call.method === method), method)
const control = (page: Page, action: 'releaseSession' | 'setHidden' | 'completeLatest', args: unknown[] = []) =>
  page.evaluate(({ action, args }) => {
    const fixture = (window as unknown as { __CREATION_FIXTURE__: Record<string, (...args: unknown[]) => unknown> }).__CREATION_FIXTURE__
    return fixture[action](...args)
  }, { action, args })
const screenshot = async (page: Page, name: string) => {
  await mkdir(evidence, { recursive: true })
  await page.screenshot({ path: resolve(evidence, name + '.png'), animations: 'disabled' })
}
const submit = async (page: Page, prompt: string) => {
  await page.locator('#ai-prompt').fill(prompt)
  await page.locator('.conversation-composer').evaluate(form => (form as HTMLFormElement).requestSubmit())
}

for (const theme of ['light', 'dark'] as const) {
  for (const width of [1440, 1024]) {
    test(theme + ' ' + width + ' keeps creation controls and scrolling within the viewport', async ({ page }) => {
      await page.setViewportSize({ width, height: 900 })
      await openStudio(page, theme)
      await expect(page.locator('.creation-projects')).toBeVisible()
      await expect(page.locator('.creation-conversation')).toBeVisible()
      await expect(page.locator('.creation-parameters')).toBeVisible()
      await expect(page.locator('#ai-prompt')).toBeInViewport()
      expect(await page.evaluate(() => document.documentElement.scrollWidth)).toBe(width)
      const timeline = page.locator('.creation-timeline')
      expect(await timeline.evaluate(el => el.scrollHeight > el.clientHeight)).toBe(true)
      await expect(page.locator('.creation-parameters button[aria-pressed="true"]').first()).toBeVisible()
      const projectGroup = page.locator('.project-binding')
      await projectGroup.click()
      await expect(projectGroup).toHaveAttribute('aria-expanded', 'false')
      await expect(page.locator('.session-list')).toBeHidden()
      await projectGroup.click()
      await expect(projectGroup).toHaveAttribute('aria-expanded', 'true')
      await expect(page.locator('.session-list')).toBeVisible()
      await screenshot(page, 'creation-studio-' + theme + '-' + width)
      expect((await calls(page, 'cutflow.hub.snapshot')).length).toBe(0)
    })
  }
  test(theme + ' 390 opens project and parameter drawers without cutting off the composer', async ({ page }) => {
    await page.setViewportSize({ width: 390, height: 844 })
    await openStudio(page, theme)
    await expect(page.locator('#ai-prompt')).toBeInViewport()
    await page.getByRole('button', { name: '打开项目与聊天', exact: true }).click()
    await expect(page.locator('.creation-projects')).toBeInViewport()
    await screenshot(page, 'creation-project-drawer-' + theme + '-390')
    await page.getByRole('button', { name: '关闭项目栏', exact: true }).click()
    await page.getByRole('button', { name: '打开生成参数', exact: true }).click()
    await expect(page.locator('.creation-parameters')).toBeInViewport()
    await screenshot(page, 'creation-parameter-drawer-' + theme + '-390')
    await page.getByRole('button', { name: '关闭参数栏', exact: true }).click()
    await expect(page.locator('#ai-prompt')).toBeInViewport()
    expect(await page.evaluate(() => document.documentElement.scrollWidth)).toBe(390)
  })
}

test('late responses remain in their original conversation after rapid session changes', async ({ page }) => {
  await page.setViewportSize({ width: 1440, height: 900 })
  await openStudio(page, 'light', { delayedSessionId: 'session-2' })
  await page.locator('.session-row').filter({ hasText: '会话 2' }).click()
  await expect.poll(async () => (await calls(page, 'cutflow.creation.turns.list')).some(call => call.params.sessionId === 'session-2')).toBe(true)
  await page.locator('.session-row').filter({ hasText: '会话 3' }).click()
  await expect(page.locator('.creation-timeline')).toContainText('会话 3 独立内容')
  await control(page, 'releaseSession')
  await expect(page.locator('.creation-timeline')).toContainText('会话 3 独立内容')
  await expect(page.locator('.creation-timeline')).not.toContainText('会话 2 独立内容')
  await page.locator('.session-row').filter({ hasText: '会话 2' }).click()
  await expect(page.locator('.creation-timeline')).toContainText('会话 2 独立内容')
})

test('continued image editing reuses the selected image and stays in the original project and session', async ({ page }) => {
  await page.setViewportSize({ width: 1440, height: 900 })
  await openStudio(page)
  await page.locator('.creation-turn').last().getByRole('button', { name: '继续改图', exact: true }).click()
  await submit(page, '把猫咪背景改成黄昏森林')
  await expect.poll(async () => (await calls(page, 'cutflow.generate')).length).toBe(1)
  const generation = (await calls(page, 'cutflow.generate'))[0].params
  expect(generation.creationProjectId).toBe('project-1')
  expect(generation.sessionId).toBe('session-1')
  expect(generation.references).toEqual(expect.arrayContaining([expect.stringContaining('D:/fixture/')]))
  await expect(page.locator('.creation-turn').last()).toContainText('把猫咪背景改成黄昏森林')
})

test('500 conversations use a window and 1000 historical turns load in 50-item pages', async ({ page }) => {
  test.setTimeout(60_000)
  await page.setViewportSize({ width: 1440, height: 900 })
  await openStudio(page, 'light', { sessionCount: 500, turnCount: 1000 })
  expect(await page.locator('.creation-turn').count()).toBeLessThanOrEqual(20)
  const firstPages = await calls(page, 'cutflow.creation.turns.list')
  expect(firstPages[0].params.limit).toBe(50)
  await page.getByRole('button', { name: '加载更早的记录', exact: true }).click()
  await expect.poll(async () => (await calls(page, 'cutflow.creation.turns.list')).length).toBe(2)
  expect(await page.locator('.creation-turn').count()).toBeLessThanOrEqual(20)
  const turnPages = await calls(page, 'cutflow.creation.turns.list')
  expect(turnPages[1].params.cursor).toBeTruthy()
  for (let index = 0; index < 9; index++) await page.getByRole('button', { name: '加载更多聊天', exact: true }).click()
  const sessionPages = await calls(page, 'cutflow.creation.sessions.list')
  expect(sessionPages).toHaveLength(10)
  expect(sessionPages.every(call => call.params.limit === 50)).toBe(true)
  expect(await page.locator('.session-row').count()).toBeLessThan(40)
  await expect(page.getByRole('button', { name: '加载更多聊天', exact: true })).toBeHidden()
  await screenshot(page, 'creation-heavy-windowed')
})

test('new results preserve an older reading position and offer an explicit return to latest', async ({ page }) => {
  await page.setViewportSize({ width: 1440, height: 900 })
  await openStudio(page, 'light', { turnCount: 20, initialRunning: true })
  await submit(page, '新结果到达时不要跳走')
  await expect.poll(async () => (await calls(page, 'cutflow.generate')).length).toBe(1)
  const timeline = page.locator('.creation-timeline')
  await timeline.evaluate(el => { el.scrollTop = 240; el.dispatchEvent(new Event('scroll')) })
  const before = await timeline.evaluate(el => el.scrollTop)
  await control(page, 'completeLatest')
  await expect(page.getByRole('button', { name: '有新结果', exact: true })).toBeVisible({ timeout: 20_000 })
  expect(Math.abs(await timeline.evaluate(el => el.scrollTop) - before)).toBeLessThan(5)
  await page.getByRole('button', { name: '有新结果', exact: true }).click()
  await expect(page.locator('.creation-turn').last()).toContainText('新结果到达时不要跳走')
  await expect.poll(async () => timeline.evaluate(el => el.scrollHeight - el.clientHeight - el.scrollTop)).toBeLessThan(5)
})

test('model, ratio and image size changes only affect the next generation', async ({ page }) => {
  await page.setViewportSize({ width: 1440, height: 900 })
  await openStudio(page)
  const previousText = await page.locator('.creation-turn').first().innerText()
  await page.getByRole('button', { name: '选择生成模型', exact: true }).click()
  await page.locator('.creation-model-popover').getByRole('button', { name: /Nano Banana Pro/ }).click()
  await page.locator('#ai-aspect').selectOption('1:1')
  await page.locator('#ai-image-size').selectOption('4K')
  expect(await page.locator('.creation-turn').first().innerText()).toBe(previousText)
  await submit(page, '新的正方形高清猫咪')
  await expect.poll(async () => (await calls(page, 'cutflow.generate')).length).toBe(1)
  const generation = (await calls(page, 'cutflow.generate'))[0].params
  expect(generation.model).toBe('gflow-nano-banana-pro')
  expect(generation.aspect_ratio).toBe('1:1')
  expect(generation.image_size).toBe('4K')
})

test('the model picker overlays parameter fields and closes with keyboard, outside click and selection', async ({ page }) => {
  await page.setViewportSize({ width: 1024, height: 900 })
  await openStudio(page)
  const trigger = page.getByRole('button', { name: '选择生成模型', exact: true })
  const popover = page.locator('.creation-model-popover')
  const ratio = page.locator('#ai-aspect')
  const originalBounds = await ratio.boundingBox()
  await expect(trigger).toHaveAttribute('aria-expanded', 'false')
  await trigger.focus()
  await trigger.press('Enter')
  await expect(popover).toBeVisible()
  await expect(page.getByRole('textbox', { name: '搜索模型', exact: true })).toBeFocused()
  expect(await ratio.boundingBox()).toEqual(originalBounds)
  expect(await page.locator('.creation-parameters .creation-model-popover').count()).toBe(0)
  await page.keyboard.press('Escape')
  await expect(popover).toBeHidden()
  await expect(trigger).toBeFocused()
  await trigger.click()
  await expect(popover).toBeVisible()
  await page.locator('.conversation-heading h1').click()
  await expect(popover).toBeHidden()
  await trigger.click()
  await popover.getByRole('button', { name: /Nano Banana Pro/ }).click()
  await expect(popover).toBeHidden()
  await expect(trigger).toContainText('Nano Banana Pro')
  expect(await ratio.boundingBox()).toEqual(originalBounds)
})

test('generation progress animates and updates stage percentages while image cards load originals', async ({ page }) => {
  await page.setViewportSize({ width: 1440, height: 900 })
  await page.emulateMedia({ reducedMotion: 'no-preference' })
  await installCreationStudioMock(page, 'light', { initialRunning: true })
  await page.addInitScript(() => {
    const target = window as unknown as {
      __TAURI_INTERNALS__: { invoke: (command: string, args?: Record<string, unknown>) => Promise<unknown>; convertFileSrc: (path: string) => string }
      __CREATION_FIXTURE__: { jobs: Array<Record<string, unknown>> }
      __PROGRESS_FIXTURE__: { percent: number; stage: string }
    }
    target.__PROGRESS_FIXTURE__ = { percent: 37, stage: '正在生成第 1 张' }
    const invoke = target.__TAURI_INTERNALS__.invoke
    const convert = target.__TAURI_INTERNALS__.convertFileSrc
    target.__TAURI_INTERNALS__.convertFileSrc = path => convert(path) + (path.endsWith('card-thumb.jpg') ? '#thumbnail' : '#original')
    target.__TAURI_INTERNALS__.invoke = async (command, args) => {
      const response = await invoke(command, args)
      if (command !== 'cutflow_call') return response
      if (args?.method === 'cutflow.creation.turns.list') {
        const listing = response as { turns: Array<{ job: { result?: { assets?: Array<{ thumbnail?: string }> } } }> }
        for (const turn of listing.turns) {
          for (const asset of turn.job.result?.assets || []) asset.thumbnail = 'D:/fixture/card-thumb.jpg'
        }
      }
      if (args?.method === 'cutflow.jobs.list') {
        const listing = response as { jobs: Array<Record<string, unknown>> }
        const running = target.__CREATION_FIXTURE__.jobs.findLast(job => job.status === 'running')
        if (running) {
          listing.jobs = listing.jobs.filter(job => job.id !== running.id)
          listing.jobs.push({ ...JSON.parse(JSON.stringify(running)), progress: { ...target.__PROGRESS_FIXTURE__ } })
        }
      }
      return response
    }
  })
  await page.goto('/')
  await page.getByRole('button', { name: 'AI 创作', exact: true }).click()
  const image = page.locator('.turn-assets img').last()
  await expect(image).toHaveAttribute('src', /#original$/)
  await expect(image).toHaveAttribute('loading', 'lazy')
  await expect(page.locator('.composer-reference img')).toHaveAttribute('src', /#thumbnail$/)
  await submit(page, '只在模拟夹具中检查进度')
  const progress = page.locator('.creation-turn').last().locator('.generation-progress')
  await expect(progress.locator('.generation-stage')).toHaveText('正在生成第 1 张')
  await expect(progress.locator('.generation-percent')).toHaveText('阶段进度 37%')
  await expect(progress.getByRole('progressbar')).toHaveAttribute('aria-valuenow', '37')
  expect(await progress.locator('.generation-spinner').evaluate(el => getComputedStyle(el).animationName)).toContain('creation-progress-spin')
  expect(await progress.locator('.progress-fill').evaluate(el => getComputedStyle(el).animationName)).toContain('wabi-liquid-flow')
  await page.evaluate(() => {
    (window as unknown as { __PROGRESS_FIXTURE__: { percent: number; stage: string } }).__PROGRESS_FIXTURE__ = { percent: 84, stage: '正在放大至 4K' }
  })
  await expect(progress.locator('.generation-stage')).toHaveText('正在放大至 4K')
  await expect(progress.locator('.generation-percent')).toHaveText('阶段进度 84%')
  await expect(progress.getByRole('progressbar')).toHaveAttribute('aria-valuenow', '84')
  await page.emulateMedia({ reducedMotion: 'reduce' })
  await expect.poll(() => progress.locator('.generation-spinner').evaluate(el => getComputedStyle(el).animationName)).toBe('none')
  expect((await calls(page, 'cutflow.generate')).length).toBe(1)
})

test('hidden windows pause generation status reads and resume without sending a paid generation', async ({ page }) => {
  await page.setViewportSize({ width: 1440, height: 900 })
  await openStudio(page, 'light', { initialRunning: true })
  await submit(page, '后台状态轮询测试')
  await expect.poll(async () => (await calls(page, 'cutflow.generate')).length).toBe(1)
  await control(page, 'setHidden', [true])
  const before = (await calls(page, 'cutflow.jobs.list')).length
  await page.waitForTimeout(3_000)
  expect((await calls(page, 'cutflow.jobs.list')).length).toBe(before)
  await control(page, 'setHidden', [false])
  await expect.poll(async () => (await calls(page, 'cutflow.jobs.list')).length).toBeGreaterThan(before)
  expect((await calls(page, 'cutflow.generate')).length).toBe(1)
})

test('new projects obtain a separate bound project and an independent first conversation', async ({ page }) => {
  await page.setViewportSize({ width: 1440, height: 900 })
  await openStudio(page)
  await page.getByRole('button', { name: '新建项目', exact: true }).click()
  await page.getByLabel('项目名称', { exact: true }).fill('城市夜景')
  await page.getByRole('button', { name: '创建项目', exact: true }).click()
  await expect.poll(async () => (await calls(page, 'cutflow.creation.projects.create')).length).toBe(1)
  await expect.poll(async () => (await calls(page, 'cutflow.creation.sessions.create')).length).toBe(1)
  await expect(page.locator('.creation-timeline')).not.toContainText('猫咪画面')
  await submit(page, '城市的雨夜')
  await expect.poll(async () => (await calls(page, 'cutflow.generate')).length).toBe(1)
  expect((await calls(page, 'cutflow.generate'))[0].params.creationProjectId).toMatch(/^created-project-/)
})

test('records 30 warm navigation samples in Chromium with simulated Tauri and no paid calls', async ({ page, browserName }) => {
  test.setTimeout(90_000)
  await page.setViewportSize({ width: 1440, height: 900 })
  await openStudio(page, 'light', { sessionCount: 500, turnCount: 1000 })
  const targets = [{ tab: 'bilibili', label: 'B站素材' }, { tab: 'editor', label: '剪辑' }, { tab: 'settings', label: '设置' }, { tab: 'about', label: '关于' }, { tab: 'creation', label: 'AI 创作' }]
  for (const target of targets) {
    await page.getByRole('button', { name: target.label, exact: true }).click()
    await expect(page.locator('.main-region')).toHaveAttribute('data-page', target.tab)
    await expect(page.locator('.workspace-loading')).toHaveCount(0)
  }
  const samples: Array<{ tab: string; durationMs: number }> = []
  for (let index = 0; index < 30; index++) {
    const target = targets[index % targets.length]
    await page.evaluate(target => {
      const fixture = window as unknown as { __NAV_RESULT__: Promise<number> }
      fixture.__NAV_RESULT__ = new Promise(resolve => {
        const button = [...document.querySelectorAll('.nav-list button')].find(button => button.getAttribute('aria-label') === target.label)!
        button.addEventListener('click', () => {
          const start = performance.now()
          const complete = () => {
            const workspace = document.querySelector('.main-region') as HTMLElement | null
            if (workspace?.dataset.page !== target.tab || workspace.querySelector('.workspace-loading')) { requestAnimationFrame(complete); return }
            requestAnimationFrame(() => requestAnimationFrame(() => resolve(performance.now() - start)))
          }
          complete()
        }, { capture: true, once: true })
      })
    }, target)
    await page.getByRole('button', { name: target.label, exact: true }).click()
    const durationMs = await page.evaluate(() => (window as unknown as { __NAV_RESULT__: Promise<number> }).__NAV_RESULT__)
    samples.push({ tab: target.tab, durationMs })
  }
  const ordered = samples.map(item => item.durationMs).sort((a, b) => a - b)
  const p95 = ordered[Math.ceil(ordered.length * 0.95) - 1]
  const report = {
    measuredAt: new Date().toISOString(), browser: browserName, viewport: { width: 1440, height: 900 },
    environment: 'Local Chromium, Vite development server, simulated Tauri API. Not WebView2 and not live Google Flow.',
    definition: 'Navigation DOM click dispatch until target workspace exists without loading indicator, followed by two requestAnimationFrame callbacks. Actionability and driver transport excluded.',
    fixture: { serverSessions: 500, serverHistoricalTurns: 1000, loadedTurns: 50 },
    medianMs: ordered[Math.floor(ordered.length / 2)], p95Ms: p95, maxMs: ordered.at(-1), samples,
  }
  await mkdir(evidence, { recursive: true })
  await writeFile(resolve(evidence, 'navigation-performance.json'), JSON.stringify(report, null, 2) + '\n')
  expect(samples).toHaveLength(30)
  expect(p95).toBeLessThanOrEqual(100)
  expect((await calls(page, 'cutflow.generate')).length).toBe(0)
})

test('cold workspace navigation shows loading immediately while the new module is delayed', async ({ page }) => {
  await page.setViewportSize({ width: 1440, height: 900 })
  await installCreationStudioMock(page, 'light')
  await page.route('**/src/desktop/pages/CreationPage.vue*', async route => {
    await new Promise(resolve => setTimeout(resolve, 500))
    await route.continue()
  })
  await page.goto('/')
  await expect(page.locator('.bilibili-workspace')).toBeVisible()
  await page.evaluate(() => {
    const target = window as unknown as { __COLD_LOADING_MS__: Promise<number> }
    const button = document.querySelector('.nav-list button[aria-label="AI 创作"]')!
    target.__COLD_LOADING_MS__ = new Promise(resolve => {
      button.addEventListener('click', () => {
        const start = performance.now()
        const observer = new MutationObserver(() => {
          if (!document.querySelector('.workspace-loading')) return
          observer.disconnect()
          resolve(performance.now() - start)
        })
        observer.observe(document.querySelector('.main-region')!, { childList: true, subtree: true })
      }, { capture: true, once: true })
    })
  })
  await page.getByRole('button', { name: 'AI 创作', exact: true }).click()
  await expect(page.locator('.workspace-loading')).toBeVisible()
  const delayMs = await page.evaluate(() => (window as unknown as { __COLD_LOADING_MS__: Promise<number> }).__COLD_LOADING_MS__)
  expect(delayMs).toBeLessThan(100)
  await expect(page.locator('.creation-studio')).toBeVisible()
  await mkdir(evidence, { recursive: true })
  await writeFile(resolve(evidence, 'cold-navigation.json'), JSON.stringify({
    environment: 'Local Chromium, Vite dev server, simulated Tauri. Not WebView2.',
    injectedModuleDelayMs: 500, clickToLoadingDomMs: delayMs,
  }, null, 2) + '\n')
})

test('returning to the cached creation workspace keeps the earlier reading position', async ({ page }) => {
  await page.setViewportSize({ width: 1440, height: 900 })
  await openStudio(page, 'light', { turnCount: 20 })
  const timeline = page.locator('.creation-timeline')
  await timeline.evaluate(el => { el.scrollTop = 240; el.dispatchEvent(new Event('scroll')) })
  const before = await timeline.evaluate(el => el.scrollTop)
  await page.getByRole('button', { name: '设置', exact: true }).click()
  await expect(page.locator('.main-region')).toHaveAttribute('data-page', 'settings')
  await page.getByRole('button', { name: 'AI 创作', exact: true }).click()
  await expect(timeline).toBeVisible()
  await expect.poll(async () => Math.abs(await timeline.evaluate(el => el.scrollTop) - before)).toBeLessThan(5)
})
