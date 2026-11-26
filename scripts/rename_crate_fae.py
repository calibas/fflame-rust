#!/usr/bin/env python3
"""One-shot: the library crate `fractal_flame_wgpu` becomes `fae`.

docs/projects/rename.md, step 3. The identifier everywhere it names the
crate: sources, benches, tests, the gallery and Python crates (their
dependency key and `use`s), the web build (its wasm and JS names), the
editor page's import, the WASM test pages, scripts, live docs.

Left alone: the archive and the rename plan (they record the old name),
earlier one-shot codemods, the Cargo.lock files (cargo rewrites them), and
the storage test's list of internal names that must stay out of the data
path (it keeps the old ones; `fae` is short enough to be in a user name).
"""
import pathlib
import re
import subprocess

ROOT = pathlib.Path(__file__).resolve().parent.parent

SKIP = {
    "docs/projects/rename.md",
    "scripts/rename_crate_fae.py",
    "scripts/move_lang_out_of_escape.py",
}
SKIP_PREFIX = ("docs/archive/",)
SKIP_SUFFIX = ("Cargo.lock",)

files = subprocess.run(["git", "grep", "-l", "fractal_flame_wgpu"], cwd=ROOT,
                       capture_output=True, text=True, check=True).stdout.split()

for rel in files:
    if rel in SKIP or rel.startswith(SKIP_PREFIX) or rel.endswith(SKIP_SUFFIX):
        continue
    p = ROOT / rel
    s = p.read_bytes().decode("utf-8")
    if rel == "src/storage/backend.rs":
        # The internal names the data path must not show: keep the old.
        keep = '["fractal_flame_wgpu", "fractal-flame", "fflame", "flame_wgpu"]'
        assert s.count(keep) == 1
        s = s.replace(keep, "\0KEEP\0")
    n = re.sub(r"\bfractal_flame_wgpu\b", "fae", s)
    if rel == "src/storage/backend.rs":
        n = n.replace("\0KEEP\0", keep)
    if n != s:
        p.write_bytes(n.encode("utf-8"))
        print(f"{len(re.findall(r'fractal_flame_wgpu', s)):4} {rel}")
