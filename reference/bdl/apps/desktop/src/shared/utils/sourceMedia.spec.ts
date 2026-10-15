import { describe, expect, it } from 'vitest';
import type { DownloadTask, NormalizedSourceTree } from '../api/dto';
import { sourceMediaByPart, completedTaskForMedia } from './sourceMedia';

const tree = (): NormalizedSourceTree => ({
  source: {
    id: 'video:test',
    kind: 'video',
    title: '专题名称',
    input: 'BVtest',
    loaded_count: 1,
    total_count: 1,
    has_more: false,
  },
  groups: [
    {
      id: 'group',
      kind: 'video',
      title: '专题名称',
      page: null,
      items: [
        {
          id: 'item',
          title: '视频',
          owner_name: null,
          cover_url: 'https://example.com/cover.jpg',
          publish_date: '2026-09-27',
          duration_seconds: 120,
          parts: [1, 2].map((page) => ({
            id: `part:${page}`,
            title: `P${page}`,
            aid: 123,
            bvid: 'BVtest',
            cid: null,
            duration_seconds: 60,
            streams: [],
            assets: [],
          })),
        },
      ],
    },
  ],
});
const task = (page = 1): DownloadTask => ({
  id: `task:${page}`,
  title: '视频',
  source_id: 'video:test',
  status: 'completed',
  resources: [],
  output_path: `downloads/p${page}.mp4`,
  refresh_intent: { input: { kind: 'video_bvid', bvid: 'BVtest' }, cid: page, page_number: page },
  media_selection: { video_quality: 'best', audio_quality: 'best', video_codec: 'avc', container: 'mp4' },
  scheduled_at: null,
  speed_limit_bytes_per_second: null,
});

describe('source media identity and metadata', () => {
  it('keeps metadata and uses the topic when the API omits the author', () => {
    const media = sourceMediaByPart(tree()).get('part:2')!;
    expect(media.secondary).toBe('专题名称');
    expect(media.published).toBe('2026-09-27');
    expect(media.duration).toBe(60);
    expect(media.cover).toBe('https://example.com/cover.jpg');
    expect(media.url).toBe('https://www.bilibili.com/video/BVtest?p=2');
    expect(sourceMediaByPart(tree(), '合集作者').get('part:1')?.secondary).toBe('合集作者');
  });

  it('only plays a completed matching part, never a different part or unfinished task', () => {
    const media = sourceMediaByPart(tree()).get('part:2')!;
    expect(completedTaskForMedia(media, [task(1)])).toBeUndefined();
    expect(completedTaskForMedia(media, [{ ...task(2), status: 'downloading' }])).toBeUndefined();
    expect(completedTaskForMedia(media, [task(1), task(2)])?.id).toBe('task:2');
    media.part.cid = 99;
    expect(completedTaskForMedia(media, [task(2)])).toBeUndefined();
    const exact = task(2);
    exact.refresh_intent!.cid = 99;
    expect(completedTaskForMedia(media, [exact])?.id).toBe('task:2');
  });

  it('retains episode URLs and matches episode downloads even when BV IDs also exist', () => {
    const source = tree();
    source.groups[0].items[0].parts[0].id = 'part:bangumi:10:20:30';
    source.groups[0].items[0].parts[0].cid = 30;
    const media = sourceMediaByPart(source).get('part:bangumi:10:20:30')!;
    expect(media.url).toBe('https://www.bilibili.com/bangumi/play/ep20');
    const downloaded = task();
    downloaded.refresh_intent = { input: { kind: 'bangumi_episode', ep_id: 20 }, cid: 30 };
    expect(completedTaskForMedia(media, [downloaded])?.id).toBe(downloaded.id);
  });
});
