#!/usr/bin/env python3
"""One-shot: pyfae's docs, examples and tests say `.fae` (docs/projects/rename.md,
step 4). Loading and saving never looked at the extension; a `.fflame`
still loads. References to the tests/visual/configs/2d fixtures, which
stay `.fflame`, are untouched."""
import pathlib
import re

ROOT = pathlib.Path(__file__).resolve().parent.parent
KEPT = re.compile(r"(plastic-sierpinski|simple-linear|subflame-[a-z0-9-]+)\.fflame")

EXACT = {
    "python/pyfae/__init__.py": [
        ("settings -- what a `.fflame` file holds.", "settings -- what a `.fae` file holds (or a `.fflame`, its name before\n    2026-11-26)."),
        ('"""Read a `.fflame` file."""', '"""Read a `.fae` (or `.fflame`) file."""'),
        ('"""A `.fflame` from a JSON string.', '"""A `.fae` config from a JSON string.'),
        ('"""The `.fflame` JSON, exactly as the app writes it."""', '"""The `.fae` JSON, exactly as the app writes it."""'),
    ],
    "python/tests/test_pyfae.py": [
        ("def test_fflame_round_trip():", "def test_config_round_trip():"),
        ('path = os.path.join(d, "t.fflame")', 'path = os.path.join(d, "t.fae")'),
    ],
}

for p in sorted(ROOT.glob("python/**/*")):
    if not p.is_file() or p.suffix not in (".py", ".md", ".toml") or "target" in p.parts:
        continue
    rel = p.relative_to(ROOT).as_posix()
    raw = p.read_bytes().decode("utf-8")
    nl = "\r\n" if "\r\n" in raw else "\n"
    s = raw.replace("\r\n", "\n")
    for a, b in EXACT.get(rel, []):
        assert s.count(a) == 1, (rel, a)
        s = s.replace(a, b)
    s = KEPT.sub(lambda m: m.group(0).replace(".fflame", "\0KEEP\0"), s)
    s = re.sub(r"\.fflame\b", ".fae", s)
    s = s.replace("\0KEEP\0", ".fflame")
    s = s.replace("\n", nl)
    if s != raw:
        p.write_bytes(s.encode("utf-8"))
        print("ok", rel)
