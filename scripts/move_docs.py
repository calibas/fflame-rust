#!/usr/bin/env python3
"""Move docs and rewrite every reference to them (a housekeeping codemod).

    python scripts/move_docs.py MOVES.txt [--dry-run]

MOVES.txt holds one move a line, `old/path.md -> new/path.md`, both
repo-relative; blank lines and `#` comments are skipped. For each move
the file is `git mv`-ed, and then:

- every repo-relative mention of the old path -- `docs/projects/x.md`
  in a code comment, a backticked path in a doc, CLAUDE.md -- becomes
  the new path, in .rs, .wgsl, .py, .md, .toml, .yml, .yaml and .txt
  files;
- every markdown link in the docs and CLAUDE.md that resolves to a moved
  file is re-pointed at it, relative to the linking doc's own (possibly
  new) location; and a moved doc's own relative links are re-based, so
  an archived doc's links keep working.

`scripts/check_doc_links.py` is the check afterwards.
"""
import os
import re
import subprocess
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
TEXT = ('.rs', '.wgsl', '.py', '.md', '.toml', '.yml', '.yaml', '.txt')
SKIP_DIRS = {'.git', 'target', 'node_modules', 'output', 'pkg'}
LINK = re.compile(r'(\[[^\]]*\]\()([^)\s]+?\.md)(#[^)\s]*)?(\))')


def norm(p):
    return os.path.normpath(p).replace(os.sep, '/')


def read_moves(path):
    moves = {}
    for line in open(path, encoding='utf-8'):
        line = line.strip()
        if not line or line.startswith('#'):
            continue
        old, new = (norm(s.strip()) for s in line.split('->'))
        moves[old] = new
    return moves


def text_files():
    for dirpath, dirs, files in os.walk(ROOT):
        dirs[:] = [d for d in dirs if d not in SKIP_DIRS]
        for f in files:
            if f.endswith(TEXT):
                yield norm(os.path.relpath(os.path.join(dirpath, f), ROOT))


def main():
    moves = read_moves(sys.argv[1])
    dry = '--dry-run' in sys.argv
    for old in moves:
        if not os.path.exists(os.path.join(ROOT, old)):
            sys.exit(f'missing: {old}')
    # Where each file will live, for re-basing a moved doc's own links.
    where = lambda p: moves.get(p, p)
    changed = 0
    for rel in text_files():
        path = os.path.join(ROOT, rel)
        try:
            s = io_read(path)
        except UnicodeDecodeError:
            continue
        out = s
        # Markdown links, resolved from the doc's OLD place, written from
        # its NEW one.
        if rel.endswith('.md'):
            here_old = os.path.dirname(rel)
            here_new = os.path.dirname(where(rel))

            def relink(m):
                target = m.group(2)
                if target.startswith(('http:', 'https:', '/')):
                    return m.group(0)
                abs_old = norm(os.path.join(here_old, target))
                if abs_old not in moves and where(rel) == rel:
                    return m.group(0)
                if not os.path.exists(os.path.join(ROOT, abs_old)) and abs_old not in moves:
                    return m.group(0)
                new_target = norm(os.path.relpath(where(abs_old), here_new or '.'))
                return f'{m.group(1)}{new_target}{m.group(3) or ""}{m.group(4)}'

            out = LINK.sub(relink, out)
        # Repo-relative mentions anywhere.
        for old, new in moves.items():
            out = re.sub(r'(?<![\w/.-])' + re.escape(old) + r'(?![\w-])', new, out)
        if out != s:
            changed += 1
            print(f'rewrote {rel}')
            if not dry:
                io_write(path, out)
    for old, new in moves.items():
        print(f'move {old} -> {new}')
        if not dry:
            os.makedirs(os.path.join(ROOT, os.path.dirname(new)), exist_ok=True)
            subprocess.run(['git', 'mv', old, new], cwd=ROOT, check=True)
    print(f'{len(moves)} moves, {changed} files rewritten{" (dry run)" if dry else ""}')


def io_read(path):
    with open(path, encoding='utf-8', newline='') as f:
        return f.read()


def io_write(path, s):
    with open(path, 'w', encoding='utf-8', newline='') as f:
        f.write(s)


if __name__ == '__main__':
    main()
