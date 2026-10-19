import type { Page } from '@playwright/test'

const installTauriMock = async (
  page: Page,
  theme: 'light' | 'dark',
  environmentReady = true,
  accountLoggedIn = false,
  taskCreationFails = false,
  collectionLoadDelayMs = 0,
) => {
  await page.addInitScript(
    ({ selectedTheme, environmentReady, accountLoggedIn, taskCreationFails, collectionLoadDelayMs }) => {
      let callbackId = 1;
      let pagedFixtureLoaded = 9;
      const pagedFixtureItems = () =>
        Array.from({ length: Math.min(pagedFixtureLoaded, 9) }, (_, index) => ({
          id: `item:favorite:fixture:${index + 1}`,
          title: `分页视频 ${index + 1}`,
          owner_name: '测试用户',
          owner_mid: '42',
          cover_url: null,
          duration_seconds: 60 + index,
          parts: [
            {
              id: `part:favorite:fixture:${index + 1}`,
              title: `分页视频 ${index + 1}`,
              aid: index + 1,
              bvid: `BV1xx411c8m${index}`,
              cid: null,
              duration_seconds: 60 + index,
              streams: [],
              assets: [],
            },
          ],
        }));
      const invocations: string[] = [];
      Object.assign(window, {
        __BDL_TEST_INVOKES__: invocations,
        __TAURI_INTERNALS__: {
          metadata: { currentWindow: { label: 'main' } },
          transformCallback: () => callbackId++,
          unregisterCallback: () => undefined,
          convertFileSrc: (path: string) => path,
          invoke: async (
            command: string,
            args?: { request?: { kind?: string; input?: string; source_id?: string; limit?: number } },
          ) => {
            invocations.push(command);
            if (command === 'plugin:app|version') return '0.6.2';
            if (command === 'account_get') {
              return accountLoggedIn
                ? { logged_in: true, name: '测试用户', avatar_url: null, mid: '42', vip_label: null }
                : { logged_in: false, name: null, avatar_url: null, mid: null, vip_label: null };
            }
            if (command === 'settings_get') {
              return {
                settings_schema_version: 1,
                usage_notice_acknowledged: true,
                auto_check_updates: false,
                theme_preference: selectedTheme,
                download_dir: 'D:\\Downloads',
                naming_template: '{title}/P{part_index} - {part_title}.{ext}',
                quality: 'best',
                archive_mode: 'fast',
                archive_assets: { cover: true, subtitles: true, danmaku: true, nfo: true },
                output_extension: 'mp4',
                duplicate_naming_strategy: 'skip_existing',
                audio_quality: 'best',
                codec: 'auto',
                missing_quality_policy: 'lower',
                ffmpeg_path: null,
                retain_raw_streams: false,
                embed_cover: false,
                embed_subtitles: false,
                proxy_url: null,
                log_level: 'info',
                data_dir: null,
                concurrent_tasks: 1,
                retry_count: 3,
                segment_count: 4,
                global_speed_limit_bytes_per_second: null,
                startup_auto_recovery: false,
                auto_refresh_expired_urls: true,
              };
            }
            if (command === 'settings_update') return (args as unknown as { settings: unknown }).settings;
            if (command === 'environment_health') {
              return {
                ready: environmentReady,
                download_directory: {
                  status: 'ready',
                  path: 'D:\\Downloads',
                  message: '保存目录可写。',
                },
                ffmpeg: environmentReady
                  ? {
                      status: 'ready',
                      source: 'system',
                      path: 'D:\\ffmpeg.exe',
                      version: 'ffmpeg version test',
                      message: 'FFmpeg 可用。',
                    }
                  : {
                      status: 'missing',
                      source: 'system',
                      path: 'ffmpeg',
                      version: null,
                      message: '未找到可用的 FFmpeg。',
                    },
              };
            }
            if (command === 'parse_create_source') {
              if (args?.request?.input === 'favorite:fixture') {
                return {
                  source: {
                    id: 'favorite:fixture',
                    kind: 'favorite',
                    input: 'favorite:fixture',
                    title: '分页收藏夹',
                    loaded_count: pagedFixtureLoaded,
                    total_count: 401,
                    has_more: true,
                  },
                  groups: [
                    {
                      id: 'group:favorite:fixture',
                      kind: 'favorite',
                      title: '分页收藏夹',
                      page: {
                        page_number: 1,
                        page_size: 20,
                        loaded_count: pagedFixtureLoaded,
                        total_count: 401,
                        has_more: true,
                      },
                      items: pagedFixtureItems(),
                    },
                  ],
                };
              }
              const isCollectedFolder = args?.request?.input?.includes('/lists/8805852') === true;
              if (isCollectedFolder) {
                if (collectionLoadDelayMs > 0) {
                  await new Promise((resolve) => window.setTimeout(resolve, collectionLoadDelayMs));
                }
                const items = Array.from({ length: 5 }, (_, index) => ({
                  id: `item:collection:fixture:${index + 1}`,
                  title: `合集视频 ${index + 1}`,
                  owner_name: null,
                  owner_mid: '42',
                  cover_url: null,
                  duration_seconds: 60,
                  parts: [
                    {
                      id: `part:collection:fixture:${index + 1}`,
                      title: `合集视频 ${index + 1}`,
                      aid: index + 1,
                      bvid: `BV1xx411c7m${index}`,
                      cid: null,
                      duration_seconds: 60,
                      streams: [],
                      assets: [],
                    },
                  ],
                }));
                return {
                  source: {
                    id: 'collection:179632001:8805852',
                    kind: 'collection',
                    input: args?.request?.input ?? '',
                    title: '双赢之路',
                    loaded_count: 5,
                    total_count: 5,
                    has_more: false,
                  },
                  groups: [
                    {
                      id: 'group:collection:179632001:8805852',
                      kind: 'collection',
                      title: '双赢之路',
                      page: { page_number: 1, page_size: 20, loaded_count: 5, total_count: 5, has_more: false },
                      items,
                    },
                  ],
                };
              }
              return {
                source: {
                  id: 'video:fixture',
                  kind: 'video',
                  input: 'BV1xx411c7mD',
                  title: '测试视频',
                  loaded_count: 1,
                  total_count: 1,
                  has_more: false,
                },
                groups: [
                  {
                    id: 'group:video:fixture',
                    kind: 'video',
                    title: '测试视频',
                    page: null,
                    items: [
                      {
                        id: 'item:video:fixture',
                        title: '测试视频',
                        owner_name: '测试用户',
                        owner_mid: '42',
                        cover_url: null,
                        duration_seconds: 60,
                        parts: [
                          {
                            id: 'part:video:fixture',
                            title: '测试视频',
                            aid: 1,
                            bvid: 'BV1xx411c7mD',
                            cid: 2,
                            duration_seconds: 60,
                            streams: [],
                            assets: [],
                          },
                        ],
                      },
                    ],
                  },
                ],
              };
            }
            if (command === 'parse_load_more' && args?.request?.source_id === 'favorite:fixture') {
              await new Promise((resolve) => setTimeout(resolve, 100));
              pagedFixtureLoaded = Math.min(pagedFixtureLoaded + 20, 401);
              const hasMore = pagedFixtureLoaded < 401;
              return {
                source: {
                  id: 'favorite:fixture',
                  kind: 'favorite',
                  input: 'favorite:fixture',
                  title: '分页收藏夹',
                  loaded_count: pagedFixtureLoaded,
                  total_count: 401,
                  has_more: hasMore,
                },
                groups: [
                  {
                    id: 'group:favorite:fixture',
                    kind: 'favorite',
                    title: '分页收藏夹',
                    page: {
                      page_number: Math.ceil(pagedFixtureLoaded / 20),
                      page_size: 20,
                      loaded_count: pagedFixtureLoaded,
                      total_count: 401,
                      has_more: hasMore,
                    },
                    items: pagedFixtureItems(),
                  },
                ],
              };
            }
            if (command === 'account_library_list') {
              if (args?.request?.kind === 'collected_favorite') {
                return {
                  items: [
                    {
                      kind: 'collected_favorite',
                      media_id: '8805852',
                      title: '双赢之路',
                      description: '测试订阅合集',
                      cover_url:
                        'data:image/svg+xml,%3Csvg xmlns="http://www.w3.org/2000/svg" width="160" height="96"%3E%3Crect width="160" height="96" fill="%23f1f1f1"/%3E%3C/svg%3E',
                      owner_name: '合集作者',
                      owner_mid: '179632001',
                      media_count: 5,
                      source_url: 'https://space.bilibili.com/179632001/lists/8805852?type=season',
                    },
                  ],
                  total: 1,
                  page: 1,
                  page_size: 20,
                  has_more: false,
                };
              }
              return {
                items: [
                  {
                    kind: 'created_favorite',
                    media_id: '69',
                    title: 'Web',
                    description: '我的收藏夹',
                    cover_url: null,
                    owner_name: '测试用户',
                    owner_mid: '42',
                    media_count: 69,
                    source_url: 'https://space.bilibili.com/42/favlist?fid=69&ftype=create',
                  },
                ],
                total: 1,
                page: 1,
                page_size: 20,
                has_more: false,
              };
            }
            if (command === 'selection_create_tasks') {
              if (taskCreationFails) {
                throw {
                  code: 'bilibili_request_rejected',
                  message:
                    'Bilibili 暂时拒绝了请求（HTTP 412），可能是访问过于频繁或触发风控。请稍后重试；持续出现时请重新登录。',
                };
              }
              return { created: [], duplicates: [], skipped_existing: 0, requires_confirmation: false };
            }
            if (command === 'account_login_qr_start') {
              return {
                qr_url: 'https://example.test/login',
                qrcode_key: 'fixture-key',
                qr_image_svg:
                  '<svg xmlns="http://www.w3.org/2000/svg" width="180" height="180"><rect width="180" height="180" fill="white"/><rect x="20" y="20" width="140" height="140" fill="black"/></svg>',
                expires_in_seconds: 180,
              };
            }
            if (command === 'account_login_qr_poll') {
              return { status: 'waiting', message: '等待扫码', account: null };
            }
            if (command === 'queue_list') return [];
            if (command === 'queue_startup_recovery') {
              return { task_ids: [], auto_recovery_enabled: false };
            }
            if (command === 'plugin:event|listen') return callbackId++;
            return null;
          },
        },
        __TAURI_EVENT_PLUGIN_INTERNALS__: { unregisterListener: () => undefined },
      });
    },
    { selectedTheme: theme, environmentReady, accountLoggedIn, taskCreationFails, collectionLoadDelayMs },
  );
};

export interface StudioFixtureOptions {
  sessionCount?: number
  turnCount?: number
  delayedSessionId?: string
  initialRunning?: boolean
}
export interface FixtureCall { method: string; params: Record<string, unknown> }

export const installCreationStudioMock = async (
  page: Page,
  theme: 'light' | 'dark',
  options: StudioFixtureOptions = {},
) => {
  // 复用旧工作区的 Tauri 初始化夹具，所有生成和项目调用只在浏览器内模拟。
  await installTauriMock(page, theme, true, true)
  await page.addInitScript(({ options }) => {
    const clone = <T>(value: T): T => JSON.parse(JSON.stringify(value))
    const stamp = 1_792_267_200_000
    const image = 'data:image/svg+xml,' + encodeURIComponent('<svg xmlns="http://www.w3.org/2000/svg" width="960" height="640"><defs><linearGradient id="a" x2="1" y2="1"><stop stop-color="#123d42"/><stop offset="1" stop-color="#2dd4bf"/></linearGradient></defs><rect width="960" height="640" fill="url(#a)"/><circle cx="480" cy="305" r="130" fill="#c9f5e4"/><path d="M360 230L380 110L455 185M505 185L585 110L605 235" fill="#c9f5e4"/><circle cx="433" cy="285" r="14" fill="#123d42"/><circle cx="525" cy="285" r="14" fill="#123d42"/><text x="480" y="525" text-anchor="middle" fill="#ecfffa" font-family="sans-serif" font-size="32">CutFlow · UI fixture</text></svg>')
    const draft = () => ({ model: 'gflow-nano-banana-2-lite', modelSource: 'catalog', prompt: '', aspect: 'auto', imageSize: 'auto', count: '1', duration: '8', references: [], stream: true, independent: false, selectedAssetPath: '' })
    const projects = [{ id: 'project-1', name: '光影猫咪项目', state: 'ready', flowProjectId: 'flow-project-1', instanceId: 'fixture-instance', accountLabel: '测试账号', createdAt: stamp, updatedAt: stamp }]
    const sessions = Array.from({ length: options.sessionCount || 4 }, (_, index) => ({
      id: 'session-' + (index + 1), projectId: 'project-1', title: index === 0 ? '猫咪创作' : '会话 ' + (index + 1),
      createdAt: stamp - index * 1_000, updatedAt: stamp - index * 1_000,
      drafts: { image: draft(), video: { ...draft(), model: 'gflow-veo-3.1-lite' } },
    }))
    const turns: Record<string, Array<Record<string, unknown>>> = {}
    const jobs: Array<Record<string, unknown>> = []
    const revisions = new Map<string, number>()
    let revision = 1
    let nextId = 0
    const completed = (sessionId: string, prompt: string, index: number) => {
      const id = 'fixture-turn-' + sessionId + '-' + index
      const params = { creationProjectId: 'project-1', sessionId, turnId: id, kind: 'image', model: 'gflow-nano-banana-2-lite', prompt, aspect_ratio: 'auto', image_size: 'auto', n: 1 }
      const asset = { path: 'D:/fixture/' + id + '.png', width: 960, height: 640 }
      const job = { id: 'job-' + id, operationId: id, method: 'cutflow.generate', params, status: 'completed', stage: '已保存', createdAt: stamp + index, result: { assets: [asset], thumbnail: asset.path } }
      const turn = { id, sessionId, jobId: job.id, prompt, params, createdAt: stamp + index, job }
      turns[sessionId] ||= []
      turns[sessionId].push(turn)
      jobs.push(job)
      revisions.set(job.id, revision)
      return turn
    }
    for (const session of sessions) {
      const count = session.id === 'session-1' ? options.turnCount || 4 : 1
      for (let index = 0; index < count; index++) completed(session.id, session.id === 'session-1' ? '猫咪画面 ' + (index + 1) : session.title + ' 独立内容', index)
    }
    const calls: Array<{ method: string; params: Record<string, unknown> }> = []
    let releaseSession: (() => void) | undefined
    let hidden = false
    Object.defineProperty(document, 'hidden', { configurable: true, get: () => hidden })
    Object.defineProperty(document, 'visibilityState', { configurable: true, get: () => hidden ? 'hidden' : 'visible' })
    const target = window as unknown as {
      __TAURI_INTERNALS__: { invoke: (command: string, args?: Record<string, unknown>) => Promise<unknown>; convertFileSrc: (path: string) => string }
      __CREATION_FIXTURE__: unknown
    }
    const control = {
      calls, projects, sessions, jobs, turns,
      releaseSession: () => releaseSession?.(),
      setHidden: (value: boolean) => { hidden = value; document.dispatchEvent(new Event('visibilitychange')) },
      completeLatest: () => {
        const job = jobs.findLast(item => item.status === 'running')
        if (!job) return
        const asset = { path: 'D:/fixture/completed-latest.png', width: 960, height: 640 }
        job.status = 'completed'; job.stage = '已保存'; job.result = { assets: [asset], thumbnail: asset.path }
        revision++; revisions.set(String(job.id), revision)
      },
    }
    target.__CREATION_FIXTURE__ = control
    const original = target.__TAURI_INTERNALS__.invoke
    target.__TAURI_INTERNALS__.convertFileSrc = path => path.startsWith('D:/fixture/') ? image : path
    const pageSlice = <T extends { id: string }>(items: T[], params: Record<string, unknown>) => {
      const start = params.cursor ? items.findIndex(item => item.id === params.cursor) + 1 : 0
      const limit = Math.min(50, Math.max(1, Number(params.limit) || 50))
      const values = items.slice(start, start + limit)
      return { values, nextCursor: start + limit < items.length ? values.at(-1)?.id : null }
    }
    target.__TAURI_INTERNALS__.invoke = async (command, args) => {
      if (command !== 'cutflow_call') return original(command, args)
      const method = String(args?.method)
      const params = (args?.params || {}) as Record<string, unknown>
      calls.push({ method, params: clone(params) })
      if (method === 'cutflow.creation.catalog') return clone({
        connection: { baseUrl: 'http://localhost:8317/v1' }, workspaceId: 'fixture-workspace', instanceId: 'fixture-instance', projectBinding: true, revision,
        models: [
          { id: 'gflow-nano-banana-2-lite', description: '轻量图片模型', kind: 'image', maxReferences: 3, duration: null, aspect: null, aspects: ['1:1', '16:9', '9:16', '4:3', '3:4'], imageSizes: ['1K', '2K', '4K'] },
          { id: 'gflow-nano-banana-pro', description: '高画质图片模型', kind: 'image', maxReferences: 3, duration: null, aspect: null, aspects: ['1:1', '16:9', '9:16'], imageSizes: ['1K', '2K', '4K'] },
          { id: 'gflow-veo-3.1-lite', description: '视频模型', kind: 'video', maxReferences: 1, duration: 8, aspect: null, aspects: ['16:9', '9:16'], durations: [4, 6, 8] },
        ], references: [], fault: null,
      })
      if (method === 'cutflow.creation.projects.list') {
        const result = pageSlice(projects, params)
        return clone({ projects: result.values, nextCursor: result.nextCursor })
      }
      if (method === 'cutflow.creation.projects.create') {
        const project = { id: 'created-project-' + ++nextId, name: String(params.name), state: 'ready', flowProjectId: 'flow-created-' + nextId, instanceId: 'fixture-instance', accountLabel: '测试账号', createdAt: stamp + nextId, updatedAt: stamp + nextId }
        projects.unshift(project)
        return clone(project)
      }
      if (method === 'cutflow.creation.projects.get') return clone(projects.find(item => item.id === params.projectId))
      if (method === 'cutflow.creation.sessions.list') {
        const result = pageSlice(sessions.filter(item => item.projectId === params.projectId), params)
        return clone({ sessions: result.values, nextCursor: result.nextCursor })
      }
      if (method === 'cutflow.creation.sessions.create') {
        const session = { id: 'created-session-' + ++nextId, projectId: String(params.projectId), title: String(params.title), createdAt: stamp + nextId, updatedAt: stamp + nextId, drafts: { image: draft(), video: { ...draft(), model: 'gflow-veo-3.1-lite' } } }
        sessions.unshift(session)
        turns[session.id] = []
        return clone(session)
      }
      if (method === 'cutflow.creation.turns.list') {
        const sessionId = String(params.sessionId)
        const ordered = [...(turns[sessionId] || [])].sort((a, b) => Number(b.createdAt) - Number(a.createdAt))
        const result = pageSlice(ordered as Array<{ id: string }>, params)
        const response = clone({ turns: result.values, nextCursor: result.nextCursor })
        if (sessionId === options.delayedSessionId) return new Promise(resolve => { releaseSession = () => resolve(response) })
        return response
      }
      if (method === 'cutflow.creation.drafts.save') {
        const session = sessions.find(item => item.id === params.sessionId)
        if (session) session.drafts[String(params.kind) as 'image' | 'video'] = clone(params.draft) as ReturnType<typeof draft>
        return { saved: true }
      }
      if (method === 'cutflow.jobs.list') return clone({
        revision,
        jobs: jobs.filter(job => params.activeOnly ? job.status === 'running' || job.status === 'queued' : params.sinceRevision ? (revisions.get(String(job.id)) || 0) > Number(params.sinceRevision) : true),
      })
      if (method === 'cutflow.generate') {
        const id = 'generated-' + ++nextId
        const generationParams = clone(params)
        const sessionId = String(params.sessionId)
        const job = { id: 'job-' + id, operationId: params.operationId, method, params: generationParams, status: 'running', stage: 'generating', createdAt: Date.now(), result: null as null | Record<string, unknown> }
        const turn = { id: String(params.turnId), sessionId, jobId: job.id, prompt: String(params.prompt), params: generationParams, createdAt: Date.now(), job }
        jobs.push(job); (turns[sessionId] ||= []).push(turn)
        revision++; revisions.set(job.id, revision)
        if (!options.initialRunning) setTimeout(() => {
          const asset = { path: 'D:/fixture/' + id + '.png', width: params.aspect_ratio === '1:1' ? 1024 : 960, height: params.aspect_ratio === '1:1' ? 1024 : 640 }
          job.status = 'completed'; job.stage = '已保存'; job.result = { assets: [asset], thumbnail: asset.path }
          revision++; revisions.set(job.id, revision)
        }, 80)
        return { jobId: job.id, status: job.status }
      }
      if (method === 'cutflow.jobs.cancel') {
        const job = jobs.find(item => item.id === params.jobId)
        if (job) { job.status = 'cancelled'; revision++; revisions.set(String(job.id), revision) }
        return { requested: true }
      }
      if (method === 'cutflow.jobs.retryDownload') return { jobId: params.jobId }
      if (method === 'cutflow.editor.status') return { running: false, current: null }
      if (method === 'cutflow.editor.recents') return []
      if (method === 'cutflow.hub.snapshot') throw new Error('Creation studio must not request all-history snapshots')
      return original(command, args)
    }
  }, { options })
}
