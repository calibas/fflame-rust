#!/usr/bin/env python3
"""One-shot: tracked `.fflame` files become `.fae`, and every reference to
one of them by name follows. docs/projects/rename.md, step 4.

Moved: every tracked `*.fflame` except the six in tests/visual/configs/2d/,
which stay `.fflame` so the suite keeps reading the old extension.
Rewritten: `<name>.fflame` for exactly the moved names, anywhere in a
tracked text file (code, tests, scripts, examples, docs, the generated
census reports) -- not in the archive, the rename plan or this script.
The app's own handling of the extension (one helper reading both, writing
`.fae`) is edited by hand, not here.
"""
import pathlib
import re
import subprocess

ROOT = pathlib.Path(__file__).resolve().parent.parent
KEEP_DIR = "tests/visual/configs/2d/"
SKIP = {"docs/projects/rename.md", "scripts/rename_ext_fae.py"}


def git(*args):
    return subprocess.run(["git", *args], cwd=ROOT, capture_output=True, text=True, check=True).stdout


moved = [f for f in git("ls-files", "*.fflame").split() if not f.startswith(KEEP_DIR)]
kept_names = {f.rsplit("/", 1)[-1] for f in git("ls-files", "*.fflame").split() if f.startswith(KEEP_DIR)}
names = {f.rsplit("/", 1)[-1] for f in moved}
assert not names & kept_names, names & kept_names

for f in moved:
    git("mv", f, f[: -len(".fflame")] + ".fae")
print(f"moved {len(moved)} files")

# Longest names first, so `a-b.fflame` is not half-matched by `b.fflame`
# (the guard below also refuses a match preceded by a name character).
alts = "|".join(re.escape(n[: -len(".fflame")]) for n in sorted(names, key=len, reverse=True))
pat = re.compile(r"(?<![A-Za-z0-9_.-])(" + alts + r")\.fflame\b")

changed = 0
for rel in git("grep", "-l", "-I", ".fflame").split():
    if rel in SKIP or rel.startswith("docs/archive/"):
        continue
    p = ROOT / rel
    s = p.read_bytes().decode("utf-8")
    n, k = pat.subn(r"\1.fae", s)
    if k:
        p.write_bytes(n.encode("utf-8"))
        changed += k
        print(f"{k:5} {rel}")
print(f"{changed} references")
