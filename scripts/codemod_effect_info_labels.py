"""One-shot codemod: give every `EffectInfo { ... }` literal in
src/effects/mod.rs the new `labels: None,` field, after its
`provenance:` line (P2 of docs/projects/rhai-plugins.md).

Keeps the file's line endings. Run from the repository root."""
import re

PATH = 'src/effects/mod.rs'
raw = open(PATH, 'rb').read().decode('utf-8')
crlf = '\r\n' in raw
s = raw.replace('\r\n', '\n')

out = []
i = 0
count = 0
for m in re.finditer(r'EffectInfo \{\n', s):
    start = m.end()
    # The literal's first `provenance:` line at one indent deeper than
    # the opening line.
    line_start = s.rfind('\n', 0, m.start()) + 1
    indent = len(s[line_start:m.start()]) - len(s[line_start:m.start()].lstrip())
    field = ' ' * (indent + 4)
    pm = re.compile(r'^' + re.escape(field) + r'provenance:[^\n]*\n', re.M).search(s, start)
    if not pm:
        continue
    out.append(s[i:pm.end()])
    out.append(field + 'labels: None,\n')
    i = pm.end()
    count += 1
out.append(s[i:])
s = ''.join(out)
if crlf:
    s = s.replace('\n', '\r\n')
open(PATH, 'wb').write(s.encode('utf-8'))
print('added labels to', count, 'literals')
