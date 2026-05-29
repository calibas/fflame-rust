"""Rename variations to match upstream Apophysis/JWildfire canonical casing.

Acts on the 36 case-only mismatches reported by
`compare_variation_names.py`. For each (old, new) pair, replaces:

  - `name: "<old>",`     →  `name: "<new>",`
  - `variation_<old>`    →  `variation_<new>`   (function defs + calls)

Pattern targets:
  - src/variations/defs/*.rs (the canonical place)
  - tests/visual/configs/**/*.fflame (smoke test configs, JSON keys)
  - assets/presets/*.fflame (built-in presets)

The static identifiers (`pub static CURL_3D`) and the file paths are
NOT renamed — those are Rust-internal names that don't appear in the
.flame XML wire format.

Renames are applied longest-first so that `post_curl3d` -> `post_curl3D`
fires before `curl3d` -> `curl3D`, avoiding double-substitution.

Run from repo root: `python scripts/rename_variations.py`
By default does a dry run and prints what would change.
Pass `--apply` to actually write the files.
"""
from __future__ import annotations
import os
import re
import sys

RENAMES: list[tuple[str, str]] = [
    # 3D suffix family — 19 entries (longer prefixes first within group).
    ("hypertile3d1", "hypertile3D1"),
    ("hypertile3d2", "hypertile3D2"),
    ("post_julia3dq", "post_julia3Dq"),
    ("post_curl3d", "post_curl3D"),
    ("waves2_3d", "waves2_3D"),
    ("loonie_3d", "loonie_3D"),
    ("lineart3d", "linearT3D"),
    ("butterfly3d", "butterfly3D"),
    ("hypertile3d", "hypertile3D"),
    ("spherical3d", "spherical3D"),
    ("tangent3d", "tangent3D"),
    ("julia3dq", "julia3Dq"),
    ("julia3dz", "julia3Dz"),
    ("splits3d", "splits3D"),
    ("square3d", "square3D"),
    ("julia3d", "julia3D"),
    ("blob3d", "blob3D"),
    ("blur3d", "blur3D"),
    ("crop3d", "crop3D"),
    ("curl3d", "curl3D"),
    ("pie3d", "pie3D"),
    # b* / e* camelCase series — 9 entries (Faber's bipolar + elliptic).
    ("bcollide", "bCollide"),
    ("bmod", "bMod"),
    ("bswirl", "bSwirl"),
    ("ecollide", "eCollide"),
    ("emod", "eMod"),
    ("epush", "ePush"),
    ("erotate", "eRotate"),
    ("escale", "eScale"),
    ("eswirl", "eSwirl"),
    # Misc camelCase — 5 entries.
    ("lineart", "linearT"),
    ("jubiq", "jubiQ"),
    ("lazytravis", "lazyTravis"),
    ("ptransform", "pTransform"),
    ("sphericaln", "sphericalN"),
    # Reverse case — 1 entry (we have capital, upstream lowercase).
    ("nPolar", "npolar"),
]

# Sort longest-first to avoid prefix substitution issues
# (post_curl3d before curl3d, etc.).
RENAMES.sort(key=lambda p: -len(p[0]))

# Targets:
TARGETS = [
    ("src/variations/defs", ".rs"),
    ("tests/visual/configs", ".fflame"),
    ("assets/presets", ".fflame"),
]


def build_substitutions() -> list[tuple[re.Pattern, str, str]]:
    """For each rename, build the regex-and-replacement pairs.

    Two patterns per rename:
      A. `name: "OLD",`  →  `name: "NEW",`  (the VariationDef field)
         and `"OLD"` in JSON keys for .fflame configs.
      B. `variation_OLD` (word boundary)  →  `variation_NEW`
         for WGSL function definitions and call sites.

    For .fflame JSON files, the variation name appears as a quoted
    object key. We match `"OLD"` exactly (with quotes) so we don't
    catch substrings inside other identifiers.
    """
    subs = []
    for old, new in RENAMES:
        # Pattern A: quoted exact name (handles both `name: "old"` and
        # `"old":` JSON keys).
        subs.append((re.compile(r'"' + re.escape(old) + r'"'), '"' + new + '"', f'quoted {old!r}'))
        # Pattern B: `variation_OLD` as a whole identifier.
        subs.append((re.compile(r'\bvariation_' + re.escape(old) + r'\b'), 'variation_' + new, f'variation_{old}'))
    return subs


def apply_to_file(path: str, subs: list[tuple[re.Pattern, str, str]]) -> tuple[str, int]:
    """Return (new_content, total_replacements). Doesn't write."""
    with open(path, encoding="utf-8") as f:
        content = f.read()
    original = content
    total = 0
    for pat, repl, _label in subs:
        new_content, n = pat.subn(repl, content)
        if n > 0:
            total += n
            content = new_content
    return content, total, original


def main() -> None:
    apply = "--apply" in sys.argv
    subs = build_substitutions()
    overall = {"files": 0, "edits": 0}
    for base, ext in TARGETS:
        if not os.path.isdir(base):
            print(f"  (skip — missing: {base})")
            continue
        for root, _dirs, files in os.walk(base):
            for fname in files:
                if not fname.endswith(ext):
                    continue
                path = os.path.join(root, fname)
                new_content, edits, original = apply_to_file(path, subs)
                if edits > 0:
                    short = path.replace("\\", "/")
                    print(f"  {short}: {edits} edit{'s' if edits != 1 else ''}")
                    overall["files"] += 1
                    overall["edits"] += edits
                    if apply:
                        with open(path, "w", encoding="utf-8", newline="") as f:
                            f.write(new_content)

    print()
    if apply:
        print(f"APPLIED: {overall['edits']} edits across {overall['files']} files")
    else:
        print(f"DRY RUN: would make {overall['edits']} edits across {overall['files']} files")
        print("Re-run with --apply to write.")


if __name__ == "__main__":
    main()
