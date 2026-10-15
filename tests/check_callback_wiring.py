"""Audit callback registration coverage; it does not prove runtime correctness."""
from pathlib import Path
import json
import re
root = Path(__file__).resolve().parents[1]
editor = root / 'editor/src/crates/concat'
rust = '\n'.join(p.read_text(encoding='utf-8') for p in (editor / 'src').rglob('*.rs'))
report = {}
for file in ['ui/app.slint', 'ui/editor.slint']:
    names = set(re.findall(r'\bcallback\s+([a-z][\w-]*)\s*\(', (editor/file).read_text(encoding='utf-8')))
    missing = sorted(name for name in names if not re.search(r'\bon_' + re.escape(name.replace('-', '_')) + r'\b', rust))
    report[file] = {'declarations': len(names), 'missing_registration': missing}
    assert not missing, f'{file}: unregistered callbacks: {missing}'
print(json.dumps(report, indent=2, ensure_ascii=False))
