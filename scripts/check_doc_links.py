#!/usr/bin/env python3
"""Find markdown links in the docs that point at files which do not exist.

    python scripts/check_doc_links.py

Reports LIVE docs separately from `docs/archive/`. That split is the
point: a broken link in a live doc sends a reader nowhere, while one
inside an archived doc is usually *correct* — an archived document
should record what it said at the time, not be rewritten to match a
tree that moved on around it.

Exits non-zero when a live doc is broken, so this can gate a docs
change without demanding the archive be rewritten.
"""
import sys

import io, re, os, glob
live, archived = [], 0
for path in glob.glob('docs/**/*.md', recursive=True) + ['CLAUDE.md']:
    if not os.path.exists(path):
        continue
    s = io.open(path, encoding='utf-8').read()
    for m in re.finditer(r'\[([^\]]*)\]\(([^)]+\.md)\)', s):
        t = m.group(2)
        if t.startswith('http'):
            continue
        if not os.path.exists(os.path.normpath(os.path.join(os.path.dirname(path), t))):
            norm = path.replace(os.sep, '/')
            if 'archive' in norm:
                archived += 1
            else:
                live.append((norm, t))
print(f"{archived} broken links inside docs/archive/ "
      "(usually correct - archived docs record what they said)")
print(f"{len(live)} broken links in LIVE docs:")
for p, t in live:
    print(f"   {p}  ->  {t}")
sys.exit(1 if live else 0)
