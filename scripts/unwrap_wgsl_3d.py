"""Mass refactor: `wgsl_3d: Some(r#"..."#)` -> `wgsl_3d: r#"..."#`.

Run from repo root: `python scripts/unwrap_wgsl_3d.py`
Dry-run by default; pass `--apply` to write.

The pattern is multi-line raw strings, so we use a non-greedy regex
over the full file content. Per `VariationDef.wgsl_3d` becoming
`&'static str` instead of `Option<&'static str>`, every literal
`Some(r#"...body..."#)` gets unwrapped.

We only touch files under `src/variations/defs/` since those are the
local variation definitions.
"""
from __future__ import annotations
import os
import re
import sys

# Multi-line raw string with single `#` delimiter — the `(?s)` flag
# (DOTALL) lets `.` match newlines.
PATTERN = re.compile(r'wgsl_3d:\s*Some\(\s*r#"(.*?)"#\s*\)', re.DOTALL)


def transform(content: str) -> tuple[str, int]:
    return PATTERN.subn(r'wgsl_3d: r#"\1"#', content)


def main() -> None:
    apply = "--apply" in sys.argv
    root = "src/variations/defs"
    if not os.path.isdir(root):
        print(f"missing dir: {root}")
        sys.exit(1)

    total_files = 0
    total_subs = 0
    for dirpath, _dirs, files in os.walk(root):
        for fname in files:
            if not fname.endswith(".rs"):
                continue
            path = os.path.join(dirpath, fname)
            with open(path, encoding="utf-8") as f:
                content = f.read()
            new_content, n = transform(content)
            if n > 0:
                total_files += 1
                total_subs += n
                short = path.replace("\\", "/")
                print(f"  {short}: {n} edit{'s' if n != 1 else ''}")
                if apply:
                    with open(path, "w", encoding="utf-8", newline="") as f:
                        f.write(new_content)

    print()
    if apply:
        print(f"APPLIED: {total_subs} edits across {total_files} files")
    else:
        print(f"DRY RUN: would make {total_subs} edits across {total_files} files")
        print("Re-run with --apply to write.")


if __name__ == "__main__":
    main()
