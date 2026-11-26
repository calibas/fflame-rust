#!/usr/bin/env python3
"""One-shot: the gallery modules and the Python package take the fae name.

docs/projects/rename.md, step 3:
  fflame-render / fflame_render   -> fae-render / fae_render
  fflame-script / fflame_script   -> fae-script / fae_script
  pyfflame                        -> pyfae (python/pyfflame -> python/pyfae)
  fflame-python / fflame_python   -> fae-python / fae_python
  fflame.wasm                     -> fae.wasm
  fflame_gpu (the WASI import)    -> fae_gpu
  fflame_alloc/_call/_free/_output_*/_info_*  -> fae_*
  PYFFLAME_WASM                   -> PYFAE_WASM (the old name still read)
  tests/test_pyfflame.py          -> tests/test_pyfae.py

Left alone: the archive and the rename plan, links to the archived
docs/archive/projects/pyfflame.md, this script, the Cargo.lock files.
"""
import pathlib
import re
import subprocess

ROOT = pathlib.Path(__file__).resolve().parent.parent


def git(*args):
    return subprocess.run(["git", *args], cwd=ROOT, capture_output=True, text=True, check=True).stdout


# The moves first, so the edits land on the new paths.
git("mv", "python/pyfflame", "python/pyfae")
git("mv", "python/tests/test_pyfflame.py", "python/tests/test_pyfae.py")

TOKENS = [
    (r"\bfflame-render\b", "fae-render"),
    (r"\bfflame_render\b", "fae_render"),
    (r"\bfflame-script\b", "fae-script"),
    (r"\bfflame_script\b", "fae_script"),
    (r"\bfflame-python\b", "fae-python"),
    (r"\bfflame_python\b", "fae_python"),
    (r"\bfflame_gpu\b", "fae_gpu"),
    (r"\bfflame_(alloc|call|free|output_ptr|output_len|info_ptr|info_len)\b", r"fae_\1"),
    (r"\bfflame\.wasm\b", "fae.wasm"),
    (r"\bfflame\.Image\b", "pyfae.Image"),
    (r"\bPYFFLAME_WASM\b", "PYFAE_WASM"),
    (r"\btest_pyfflame\b", "test_pyfae"),
    (r"\bpyfflame\b", "pyfae"),
]
SKIP = {"docs/projects/rename.md", "scripts/rename_modules_fae.py"}
ARCHIVED_DOC = "projects/pyfflame.md"

pattern = "fflame-render|fflame_render|fflame-script|fflame_script|pyfflame|fflame-python|fflame_python|fflame_gpu|fflame_alloc|fflame_call|fflame_free|fflame_output|fflame_info|fflame\\.wasm|fflame\\.Image|PYFFLAME"
files = git("grep", "-l", "-E", pattern).split()
for rel in files:
    if rel in SKIP or rel.startswith("docs/archive/") or rel.endswith("Cargo.lock"):
        continue
    p = ROOT / rel
    s = p.read_bytes().decode("utf-8")
    n = s.replace(ARCHIVED_DOC, "\0ARCHIVED\0")
    for pat, rep in TOKENS:
        n = re.sub(pat, rep, n)
    n = n.replace("\0ARCHIVED\0", ARCHIVED_DOC)
    if n != s:
        p.write_bytes(n.encode("utf-8"))
        print("ok", rel)
