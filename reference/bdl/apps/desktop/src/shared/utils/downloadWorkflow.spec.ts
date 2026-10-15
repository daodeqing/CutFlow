import { describe, expect, it } from 'vitest'
import { builtinDownloadPresets, legacyBuiltinDownloadPresets, cloneWorkflow, downloadPresetsError, legacyWorkflow, migrateDownloadPresets, workflowError, workflowOutputs, workflowExtension } from './downloadWorkflow'
import { createPinia, setActivePinia } from 'pinia'
import { useSettingsStore } from '../stores/settings'

describe('download recipes', () => {
  it('provides three clear common presets and supports custom content-only output', () => {
    const presets = builtinDownloadPresets()
    expect(downloadPresetsError(presets, 'video')).toBeNull()
    expect(presets.map((p) => p.name)).toEqual(['快速下载', '下载全部资源', '封装 MKV（视频+字幕）'])
    const subtitles = legacyBuiltinDownloadPresets().find((p) => p.id === 'subtitles')!.workflow
    expect(subtitles.video.enabled || subtitles.audio.enabled).toBe(false)
    expect(workflowOutputs(subtitles)).toEqual(['字幕 SRT'])
    expect(workflowOutputs(presets.find((p) => p.id === 'mkv')!.workflow)).toEqual(['MKV 音频+视频（内含字幕）'])
  })
  it('migrates old custom audio and sidecar choices without resetting them', () => {
    setActivePinia(createPinia())
    const old = { ...useSettingsStore().saved, download_presets: [], selected_download_preset: '', media_mode: 'audio_only' as const, audio_output_format: 'm4s' as const, archive_mode: 'custom' as const, archive_assets: { cover: false, subtitles: true, danmaku: true, nfo: false }, subtitle_format: 'ass' as const, danmaku_format: 'html' as const }
    const migrated = migrateDownloadPresets(old)
    const selected = migrated.download_presets.find((p) => p.id === migrated.selected_download_preset)!
    expect(selected.workflow).toEqual(legacyWorkflow(old))
    expect(selected.name).toBe('历史配置')
    expect(workflowOutputs(selected.workflow)).toEqual(['独立音频 M4S', '字幕 ASS', '弹幕 HTML'])
    expect(migrateDownloadPresets({ ...old, ...migrated })).toEqual(migrated)
  })
  it('keeps copied recipes independent and rejects empty or incompatible output', () => {
    const original = builtinDownloadPresets()[0].workflow
    const copy = cloneWorkflow(original)
    copy.video.enabled = false
    copy.audio.enabled = false
    expect(workflowError(copy)).toBe('至少选择一种内容')
    expect(original.video.enabled).toBe(true)
    copy.subtitles.enabled = true
    copy.subtitles.embed = true
    expect(workflowError(copy)).toContain('MKV')
  })
  it('rejects duplicate names and a removed default', () => {
    const presets = builtinDownloadPresets()
    expect(downloadPresetsError(presets, 'removed')).toContain('默认')
    presets[1].name = presets[0].name
    expect(downloadPresetsError(presets, 'video')).toContain('重复')
  })
  it('previews raw-only output extensions and does not duplicate unchanged XML', () => {
    const flow = legacyBuiltinDownloadPresets().find((p) => p.id === 'subtitles')!.workflow
    flow.subtitles.save = false
    flow.subtitles.retain_original = true
    expect(workflowExtension(flow)).toBe('json')
    expect(workflowOutputs(flow)).toEqual(['原始字幕'])
    flow.subtitles.enabled = false
    flow.danmaku.enabled = true
    flow.danmaku.retain_original = true
    expect(workflowExtension(flow)).toBe('xml')
    expect(workflowOutputs(flow)).toEqual(['弹幕 XML'])
  })
})
