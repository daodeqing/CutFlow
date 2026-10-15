import { describe, expect, it } from 'vitest';

import type { DownloadTask } from '../../api/dto';
import { isCancellable, isPausable, isRetryable, matchesTransferSearch, sortTransferTasks } from './transferQueries';

const task = (id: string, title: string, status: DownloadTask['status'] = 'waiting'): DownloadTask =>
  ({ id, title, status, source_id: `source:${id}`, output_path: `downloads/${id}.mp4` }) as DownloadTask;

const metrics = {
  progress: (value: DownloadTask) => ({ one: 30, two: 80, three: 80 })[value.id] ?? 0,
  speed: (id: string) => ({ one: 4, two: 1, three: 8 })[id] ?? 0,
};

describe('transfer queries', () => {
  const tasks = [task('one', '视频 10'), task('two', '视频 2'), task('three', '视频 2', 'failed')];

  it('keeps stable ordering when progress values are equal', () => {
    expect(sortTransferTasks(tasks, 'progress_desc', metrics).map((value) => value.id)).toEqual([
      'two',
      'three',
      'one',
    ]);
  });

  it('sorts by actual completion time rather than queue order and leaves input intact', () => {
    const completed = [
      { ...task('late', '晚完成', 'completed'), completed_at: '2026-10-04T08:06:01Z' },
      { ...task('early', '早完成', 'completed'), completed_at: '2026-10-04T15:05:00+08:00' },
      { ...task('middle', '中间完成', 'completed'), completed_at: '2026-10-04T08:00:00Z' },
    ];
    expect(sortTransferTasks(completed, 'completed_desc', metrics).map((value) => value.id)).toEqual(['late', 'middle', 'early']);
    expect(sortTransferTasks(completed, 'queue', metrics).map((value) => value.id)).toEqual(['middle', 'early', 'late']);
    expect(completed.map((value) => value.id)).toEqual(['late', 'early', 'middle']);
  });

  it('shows newest additions first without mutating the stored queue', () => {
    expect(sortTransferTasks(tasks, 'queue', metrics).map((value) => value.id)).toEqual(['three', 'two', 'one']);
    expect(tasks.map((value) => value.id)).toEqual(['one', 'two', 'three']);
  });

  it('places unknown completion times last with a deterministic reverse-queue fallback', () => {
    const completed = [
      task('unknown', '旧记录', 'completed'),
      { ...task('known', '有时间', 'completed'), completed_at: '2026-10-04T08:00:00Z' },
      { ...task('invalid', '坏时间', 'completed'), completed_at: 'invalid' },
    ];
    expect(sortTransferTasks(completed, 'completed_desc', metrics).map((value) => value.id)).toEqual(['known', 'invalid', 'unknown']);
  });

  it('puts actionable failures before active and completed tasks', () => {
    const completed = task('done', '完成', 'completed');
    expect(sortTransferTasks([...tasks, completed], 'issue_first', metrics)[0]?.id).toBe('three');
  });

  it('searches title, source and output path case-insensitively', () => {
    expect(matchesTransferSearch(tasks[0]!, 'ONE', 'UP 主')).toBe(true);
  });

  it('defines bulk action eligibility from task status', () => {
    expect([isPausable('downloading'), isCancellable('paused'), isRetryable('completed')]).toEqual([true, true, true]);
  });
});
