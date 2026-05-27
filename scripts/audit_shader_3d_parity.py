"""Audit WGSL 3D shader bodies for missing helper functions vs the 2D body.

The bug pattern: a variation's `wgsl_2d` defines helper functions that
the body calls, but the `wgsl_3d` body calls the same helpers without
defining them. Caught one instance manually (iconattractor_js's
`ic_preset`), suspect there are more.

For each `pub static FOO: VariationDef`:
  1. Extract `wgsl_2d` and `wgsl_3d` raw strings (skip if 3D is None)
  2. Find all `fn NAME(` declarations in each body
  3. Find all `NAME(` call sites in each body
  4. **Bug signal**: a function called in body Y but not defined there,
     yet defined in body X. That's a cross-body call that will fail to
     compile when body Y's shader is active.

Calls that are unresolved in both bodies are presumed externals (WGSL
built-ins, shader-builder injected helpers like get_param, etc.) and
not flagged. Only flag the high-signal "defined elsewhere" case.

Read-only; no files modified. Run from repo root.
"""
from __future__ import annotations
import re
import sys
from pathlib import Path

try:
    sys.stdout.reconfigure(encoding="utf-8")
except AttributeError:
    pass

REPO_ROOT = Path(__file__).resolve().parent.parent
DEFS_DIR = REPO_ROOT / "src" / "variations" / "defs"

STATIC_RE = re.compile(
    r"^pub static ([A-Z_][A-Z0-9_]*): VariationDef\b",
    re.MULTILINE,
)
FN_DECL_RE = re.compile(r"\bfn\s+([a-zA-Z_][a-zA-Z0-9_]*)\s*\(")
# Call sites: identifier immediately followed by `(`, optionally with
# a `<...>` type arg in between (vec2<f32>(), array<f32,N>(), etc.).
# Catches keywords like `if`, `for`, `while` too — they're filtered out.
CALL_SITE_RE = re.compile(r"\b([a-zA-Z_][a-zA-Z0-9_]*)\s*(?:<[^>]+>\s*)?\(")
WGSL_KEYWORDS = {
    "if", "else", "for", "while", "switch", "case", "default",
    "return", "break", "continue", "discard", "let", "var",
    "const", "fn", "loop", "struct",
}


def find_raw_string(text: str, start_pos: int) -> tuple[int, int] | None:
    """Find the next `r#"..."#` raw-string literal starting at or after start_pos.

    Returns (content_start, content_end) bounds, or None.
    Handles r#"..."#, r##"..."##, etc.
    """
    i = start_pos
    n = len(text)
    while i < n:
        # Look for `r` followed by one or more `#` followed by `"`
        if text[i] == "r" and i + 1 < n:
            j = i + 1
            hashes = 0
            while j < n and text[j] == "#":
                hashes += 1
                j += 1
            if hashes > 0 and j < n and text[j] == '"':
                content_start = j + 1
                # Find matching `"<hashes># worth of #s>`
                close_pat = '"' + ("#" * hashes)
                end_idx = text.find(close_pat, content_start)
                if end_idx == -1:
                    return None
                return content_start, end_idx
        i += 1
    return None


def extract_field_raw_string(block: str, field: str) -> str | None:
    """For `field: r#"..."#` or `field: Some(r#"..."#)`, return the body."""
    # Find `<field>:`
    pat = re.compile(rf"\b{field}\s*:")
    m = pat.search(block)
    if not m:
        return None
    after = m.end()
    # Skip whitespace + optional `Some(`
    while after < len(block) and block[after].isspace():
        after += 1
    if block[after:after + 5] == "Some(":
        after += 5
        # Skip whitespace
        while after < len(block) and block[after].isspace():
            after += 1
    # Now expect a raw-string literal
    bounds = find_raw_string(block, after)
    if bounds is None:
        return None
    return block[bounds[0]:bounds[1]]


def fn_names(body: str) -> set[str]:
    return set(FN_DECL_RE.findall(body))


def call_names(body: str) -> set[str]:
    """All identifiers used as function calls in `body`, minus keywords."""
    return {n for n in CALL_SITE_RE.findall(body) if n not in WGSL_KEYWORDS}


def main() -> int:
    bugs: list[tuple[str, str, str, set[str]]] = []
    # (file, static_name, direction, missing_names)
    # direction is "3D body calls 2D-only function" or vice versa
    name_diffs: list[tuple[str, str, set[str], set[str]]] = []
    # (file, static_name, only_in_2d, only_in_3d) — informational

    total_statics = 0
    has_3d_count = 0
    no_3d_count = 0

    for path in sorted(DEFS_DIR.glob("*.rs")):
        if path.name == "mod.rs":
            continue
        text = path.read_text(encoding="utf-8", errors="replace")
        for m in STATIC_RE.finditer(text):
            total_statics += 1
            static_name = m.group(1)
            block_start = m.start()
            next_static = text.find("\npub static ", m.end())
            block_end = next_static if next_static != -1 else len(text)
            block = text[block_start:block_end]

            body_2d = extract_field_raw_string(block, "wgsl_2d")
            body_3d = extract_field_raw_string(block, "wgsl_3d")

            if body_2d is None:
                continue
            if body_3d is None:
                no_3d_count += 1
                continue
            has_3d_count += 1

            defs_2d = fn_names(body_2d)
            defs_3d = fn_names(body_3d)
            calls_2d = call_names(body_2d)
            calls_3d = call_names(body_3d)

            # Bug check 1: 3D body calls a function not defined in 3D body
            # but defined in 2D body. That's a real cross-body call — the
            # 3D shader will fail to compile when this variation is active.
            unresolved_3d = calls_3d - defs_3d
            cross_calls_3d_to_2d = unresolved_3d & defs_2d
            if cross_calls_3d_to_2d:
                bugs.append((path.name, static_name,
                             "3D body calls function only defined in 2D",
                             cross_calls_3d_to_2d))

            # Bug check 2: same in reverse (symmetric — much rarer since
            # 2D was the primary port surface).
            unresolved_2d = calls_2d - defs_2d
            cross_calls_2d_to_3d = unresolved_2d & defs_3d
            if cross_calls_2d_to_3d:
                bugs.append((path.name, static_name,
                             "2D body calls function only defined in 3D",
                             cross_calls_2d_to_3d))

            # Informational: function defined in one body but not the
            # other, AND not called from the other side. Usually
            # intentional renames (hex_seg60_2d vs hex_seg60_3d), but
            # listed so you can eyeball.
            only_2d = (defs_2d - defs_3d) - cross_calls_3d_to_2d
            only_3d = (defs_3d - defs_2d) - cross_calls_2d_to_3d
            if only_2d or only_3d:
                name_diffs.append((path.name, static_name, only_2d, only_3d))

    print("=" * 70)
    print("WGSL 3D shader parity audit")
    print("=" * 70)
    print()
    print(f"Total statics inspected           : {total_statics}")
    print(f"  with both wgsl_2d and wgsl_3d   : {has_3d_count}")
    print(f"  with wgsl_3d: None              : {no_3d_count}")
    print()
    print(f"BUGS (function called in one body, defined only in the other): {len(bugs)}")
    if bugs:
        print()
        for filename, name, direction, names in bugs:
            print(f"  {name}  ({filename})")
            print(f"    {direction}: {', '.join(sorted(names))}")
    else:
        print("  None found.")
    print()
    print(f"Informational — function-name mismatches (probably intentional renames): {len(name_diffs)}")
    if name_diffs:
        print()
        for filename, name, only_2d, only_3d in name_diffs:
            print(f"  {name}  ({filename})")
            if only_2d:
                print(f"    defined only in 2D body: {', '.join(sorted(only_2d))}")
            if only_3d:
                print(f"    defined only in 3D body: {', '.join(sorted(only_3d))}")
    return 0 if not bugs else 1


if __name__ == "__main__":
    raise SystemExit(main())
