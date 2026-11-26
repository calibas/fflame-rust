#!/usr/bin/env python3
"""One-shot: the repository's name and the config extension in text.

docs/projects/rename.md, step 5:
- `fflame-rust` -> `fractal-art-editor` (the Help menu's GitHub links,
  release.py's labels, docs); the samplers' attribution names the app.
- `.fflame` -> `.fae` in the live docs, except the tests/visual/configs/2d
  fixtures, which stay `.fflame`. The File Formats descriptions say by hand
  that `.fflame` is still read.
Left alone: the archive, the rename plan, generated reports, and the
storage comment that records the old repository name on purpose.
"""
import pathlib
import re
import subprocess

ROOT = pathlib.Path(__file__).resolve().parent.parent
SKIP = {"docs/projects/rename.md", "scripts/rename_text_fae.py", "src/storage/backend.rs"}
KEPT = re.compile(r"(plastic-sierpinski|simple-linear|subflame-[a-z0-9-]+)\.fflame")


def git(*args):
    return subprocess.run(["git", *args], cwd=ROOT, capture_output=True, text=True, check=True).stdout


def rewrite(rel, fn):
    p = ROOT / rel
    raw = p.read_bytes().decode("utf-8")
    nl = "\r\n" if "\r\n" in raw else "\n"
    s = fn(raw.replace("\r\n", "\n"))
    s = s.replace("\n", nl)
    if s != raw:
        p.write_bytes(s.encode("utf-8"))
        print("ok", rel)


for rel in git("grep", "-l", "fflame-rust").split():
    if rel in SKIP or rel.startswith("docs/archive/"):
        continue
    def repo(s, rel=rel):
        if rel == "src/variations/defs/solid_samplers.rs":
            return s.replace("//! - fflame-rust (original)", "//! - Fractal Art Editor (original)")
        return s.replace("fflame-rust", "fractal-art-editor")
    rewrite(rel, repo)

for rel in git("grep", "-l", r"\.fflame", "--", "*.md").split():
    if rel in SKIP or rel.startswith("docs/archive/") or rel.startswith("docs/generated/"):
        continue
    def ext(s):
        s = KEPT.sub(lambda m: m.group(0).replace(".fflame", "\0K\0"), s)
        s = re.sub(r"\.fflame\b", ".fae", s)
        return s.replace("\0K\0", ".fflame")
    rewrite(rel, ext)
