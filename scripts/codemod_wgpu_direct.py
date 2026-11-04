"""One-shot codemod: the engine names `wgpu` directly, not through
`egui_wgpu::wgpu`.

The two are the same crate on every target that has egui-wgpu (both are
direct dependencies and unify to one wgpu 29), so this changes nothing
for the desktop or browser builds. It is for WASI (wasm/python), where
egui-wgpu does not compile (it assumes a browser on wasm32) and wgpu
runs on its custom backend.

The app and its UI (src/app, src/ui, src/main.rs) keep `egui_wgpu`:
they draw egui and are not part of a WASI build.

Run from the repository root: python scripts/codemod_wgpu_direct.py
"""

import pathlib
import re

SKIP = ("src/app/", "src/ui/", "src/main.rs")

changed = 0
for path in sorted(pathlib.Path("src").rglob("*.rs")):
    rel = path.as_posix()
    if rel.startswith(SKIP):
        continue
    raw = path.read_bytes().decode("utf-8")
    if "egui_wgpu::wgpu" not in raw:
        continue
    # `use egui_wgpu::wgpu;` becomes nothing: `wgpu` is in scope as a crate.
    s = re.sub(r"^[ \t]*use egui_wgpu::wgpu;[ \t]*\r?\n", "", raw, flags=re.M)
    s = s.replace("egui_wgpu::wgpu", "wgpu")
    if s != raw:
        path.write_bytes(s.encode("utf-8"))
        changed += 1
        print(rel)
print(f"{changed} files")
