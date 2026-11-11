"""One-shot codemod: move the formula language out of the escape engine
(docs/archive/projects/rhai-plugins.md, the compiler's own feature, 2026-11-02).

src/escape/lang/ becomes src/lang/ (`crate::lang`, the `lang` feature,
which `engine-escape` turns on), so a build without the escape engine --
a gallery module -- can compile Rhai effects and variations to WGSL.

- `git mv src/escape/lang src/lang`
- `crate::escape::lang` -> `crate::lang` (and the crate's name from the
  gallery crates and the Python bindings) in every .rs file
- `escape/mod.rs` re-exports it (`pub use crate::lang;`), so the escape
  engine's own relative paths (`super::lang::...`) keep resolving
- source paths in comments and docs, `src/escape/lang/` -> `src/lang/`
  (not docs/archive, which records what was)

Run from the repository root. Keeps each file's line endings."""
import pathlib
import subprocess

ROOT = pathlib.Path('.')


def rewrite(path, pairs):
    raw = path.read_bytes().decode('utf-8')
    s = raw
    for old, new in pairs:
        s = s.replace(old, new)
    if s != raw:
        path.write_bytes(s.encode('utf-8'))
        return True
    return False


subprocess.run(['git', 'mv', 'src/escape/lang', 'src/lang'], check=True)

code_pairs = [
    ('crate::escape::lang', 'crate::lang'),
    ('fractal_flame_wgpu::escape::lang', 'fractal_flame_wgpu::lang'),
    ('src/escape/lang/', 'src/lang/'),
]
changed = 0
for base in ['src', 'wasm', 'python', 'build.rs']:
    p = ROOT / base
    files = [p] if p.is_file() else [f for f in p.rglob('*.rs') if 'target' not in f.parts]
    for f in files:
        changed += rewrite(f, code_pairs)

# The escape engine keeps its path to the language as a re-export.
mod_rs = ROOT / 'src/escape/mod.rs'
raw = mod_rs.read_bytes().decode('utf-8')
nl = '\r\n' if '\r\n' in raw else '\n'
assert raw.count(f'pub mod lang;{nl}') == 1, 'escape/mod.rs declares lang once'
raw = raw.replace(
    f'pub mod lang;{nl}',
    f'/// The formula language, its own module since 2026-11-02 (`crate::lang`,{nl}'
    f'/// the `lang` feature): re-exported, so the engine\'s paths to it stay.{nl}'
    f'pub use crate::lang;{nl}',
)
mod_rs.write_bytes(raw.encode('utf-8'))

doc_pairs = [
    ('src/escape/lang/', 'src/lang/'),
    ('../../src/escape/lang', '../../src/lang'),
    ('../src/escape/lang', '../src/lang'),
]
docs = 0
for f in list((ROOT / 'docs').rglob('*.md')) + [ROOT / 'CLAUDE.md']:
    if 'archive' in f.parts:
        continue
    docs += rewrite(f, doc_pairs)
print(f'moved src/escape/lang to src/lang; {changed} source files and {docs} docs rewritten')
