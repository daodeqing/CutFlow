import ctypes
from ctypes import wintypes
import json
from pathlib import Path


class FileOperation(ctypes.Structure):
    _fields_ = [('window', wintypes.HWND), ('operation', wintypes.UINT),
                ('source', wintypes.LPCWSTR), ('destination', wintypes.LPCWSTR),
                ('flags', wintypes.WORD), ('aborted', wintypes.BOOL),
                ('mapping', ctypes.c_void_p), ('progress_title', wintypes.LPCWSTR)]


def recycle(path):
    root = Path(__file__).resolve().parents[1]
    target = Path(path).resolve(strict=True)
    if target == root or not target.is_relative_to(root):
        raise RuntimeError('Recycle target must remain inside this project: ' + str(target))
    if target.is_dir():
        size = sum(item.stat().st_size for item in target.rglob('*') if item.is_file() and not item.is_symlink())
    else:
        size = target.stat().st_size
    operation = FileOperation(None, 3, str(target) + '\0\0', None, 0x40 | 0x10 | 0x4 | 0x400, False, None, None)
    shell = ctypes.WinDLL('shell32', use_last_error=True)
    shell.SHFileOperationW.argtypes = [ctypes.POINTER(FileOperation)]
    shell.SHFileOperationW.restype = ctypes.c_int
    result = shell.SHFileOperationW(ctypes.byref(operation))
    if result or operation.aborted or target.exists():
        raise RuntimeError(f'Recycle failed ({result}): {target}')
    record = {'path': str(target.relative_to(root)), 'bytes': size,
              'sizeMiB': round(size / 1048576, 3), 'status': 'recycled'}
    log = root / 'docs/清理记录.json'
    records = json.loads(log.read_text(encoding='utf-8')) if log.exists() else []
    records.append(record)
    log.write_text(json.dumps(records, ensure_ascii=False, indent=2) + '\n', encoding='utf-8')
    print(json.dumps(record, ensure_ascii=False), flush=True)
    return record
