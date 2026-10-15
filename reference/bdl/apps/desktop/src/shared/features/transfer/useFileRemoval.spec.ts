import { nextTick } from 'vue';
import { describe, expect, it, vi } from 'vitest';
import { useFileRemoval } from './useFileRemoval';
import type { RemovalPreview } from '../../api/dto';
const preview: RemovalPreview = { token: 'server-token', task_ids: ['selected'], files: ['downloads/movie.mp4'], file_count: 1, total_bytes: 4096, preserved: ['shared.mp4'], roots: [] };
const flush = async () => { await Promise.resolve(); await nextTick(); };
describe('file deletion consent', () => {
  it('previews only explicit selection and cancelling never deletes', async () => {
    const load = vi.fn().mockResolvedValue(preview);
    const execute = vi.fn();
    const dialog = useFileRemoval('tasks', load, execute);
    dialog.show([]);
    expect(load).not.toHaveBeenCalled();
    dialog.show(['selected', 'selected']);
    await flush();
    expect(load).toHaveBeenCalledWith(['selected']);
    expect(execute).not.toHaveBeenCalled();
    dialog.open.value = false;
    await flush();
    expect(execute).not.toHaveBeenCalled();
  });
  it('sends the backend token once and requires a new preview after failure', async () => {
    const load = vi.fn().mockResolvedValue(preview);
    const execute = vi.fn().mockRejectedValueOnce(new Error('文件已变化')).mockResolvedValueOnce(undefined);
    const dialog = useFileRemoval('tasks', load, execute);
    dialog.show(['selected']); await flush();
    await dialog.confirm();
    expect(dialog.error.value).toBe('文件已变化');
    expect(dialog.open.value).toBe(true);
    await dialog.confirm();
    expect(execute).toHaveBeenCalledTimes(1);
    await dialog.refresh(); await dialog.confirm();
    expect(execute).toHaveBeenLastCalledWith('server-token');
    expect(dialog.open.value).toBe(false);
  });
  it('ignores late previews from a closed dialog', async () => {
    let resolve!: (value: RemovalPreview) => void;
    const load = vi.fn(() => new Promise<RemovalPreview>((done) => { resolve = done; }));
    const execute = vi.fn();
    const dialog = useFileRemoval('tasks', load, execute);
    dialog.show(['selected']); dialog.open.value = false; await nextTick();
    resolve(preview); await flush();
    expect(dialog.preview.value).toBeNull();
    expect(dialog.busy.value).toBe(false);
    await dialog.confirm(); expect(execute).not.toHaveBeenCalled();
  });

  it('copies deletion context and the original system error without credentials', async () => {
    const writeText = vi.fn().mockResolvedValue(undefined);
    vi.stubGlobal('navigator', { clipboard: { writeText } });
    try {
      const execute = vi.fn().mockRejectedValue(new Error('[stage=delete_file code=file_in_use] path=C:/downloads/movie.mp4 os_error=32\nCookie: SESSDATA=private-cookie\nhttps://cdn.test/file?token=private-token'));
      const dialog = useFileRemoval('tasks', vi.fn().mockResolvedValue(preview), execute);
      dialog.show(['selected']); await flush(); await dialog.confirm(); await dialog.copyError();
      const copied = writeText.mock.calls[0][0];
      expect(copied).toContain('任务：selected');
      expect(copied).toContain('os_error=32');
      expect(copied).toContain('C:/downloads/movie.mp4');
      expect(copied).not.toContain('private-cookie');
      expect(copied).not.toContain('private-token');
      expect(dialog.open.value).toBe(true);
      expect(dialog.copied.value).toBe(true);
    } finally { vi.unstubAllGlobals(); }
  });

  it('reports clipboard failure without replacing the deletion error and allows retry', async () => {
    const writeText = vi.fn().mockRejectedValueOnce(new Error('clipboard denied')).mockResolvedValueOnce(undefined);
    vi.stubGlobal('navigator', { clipboard: { writeText } });
    try {
      const dialog = useFileRemoval('tasks', vi.fn().mockResolvedValue(preview), vi.fn().mockRejectedValue(new Error('文件被占用')));
      dialog.show(['selected']); await flush(); await dialog.confirm(); await dialog.copyError();
      expect(dialog.copyNotice.value).toBe('复制失败，请重试');
      expect(dialog.error.value).toBe('文件被占用');
      expect(dialog.copied.value).toBe(false);
      await dialog.copyError();
      expect(dialog.copyNotice.value).toBe('');
      expect(dialog.copied.value).toBe(true);
      expect(dialog.error.value).toBe('文件被占用');
    } finally { vi.unstubAllGlobals(); }
  });
});
