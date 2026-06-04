"""Diff a JWildfire variation list against our registry.

Reads a one-name-per-line file (default
`output/jwildfire-script-vars.txt`) and prints which names are NOT
present in our variation registry. Matching is case-insensitive and
also looks at the `aliases: &[...]` field on each `VariationDef`, so
JWF's `julia3D` matches our `julia3d` and our `linear` matches
JWF's `linear3D` alias.

Usage:
    python scripts/diff_jwf_list.py [path/to/list.txt]

Read-only — no files modified.
"""
from __future__ import annotations
import os
import re
import sys
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent
DEFS_DIR = REPO_ROOT / "src" / "variations" / "defs"
DEFAULT_LIST = REPO_ROOT / "output" / "jwildfire-script-vars.txt"

# Each `pub static FOO: VariationDef = VariationDef { name: "bar", aliases: &["..."], ... }`.
# Capture both the canonical name and the aliases array body.
DEF_RE = re.compile(
    r'pub\s+static\s+\w+\s*:\s*VariationDef\s*=\s*VariationDef\s*\{[^}]*?'
    r'name\s*:\s*"([^"]+)"[^}]*?'
    r'aliases\s*:\s*&\s*\[([^\]]*)\]',
    re.DOTALL,
)
ALIAS_STR_RE = re.compile(r'"([^"]+)"')


def collect_registry() -> set[str]:
    """Return the case-insensitive set of every name + alias in our defs."""
    names: set[str] = set()
    for path in DEFS_DIR.rglob("*.rs"):
        text = path.read_text(encoding="utf-8")
        for m in DEF_RE.finditer(text):
            names.add(m.group(1).lower())
            for alias in ALIAS_STR_RE.findall(m.group(2)):
                names.add(alias.lower())
    return names


def main() -> int:
    list_path = Path(sys.argv[1]) if len(sys.argv) > 1 else DEFAULT_LIST
    if not list_path.exists():
        print(f"missing list file: {list_path}")
        return 1

    registry = collect_registry()
    print(f"Loaded {len(registry)} names + aliases from {DEFS_DIR}")

    requested = [
        line.strip()
        for line in list_path.read_text(encoding="utf-8").splitlines()
        if line.strip() and not line.startswith("#")
    ]
    print(f"Loaded {len(requested)} names from {list_path}")
    print()

    have = []
    missing = []
    for name in requested:
        if name.lower() in registry:
            have.append(name)
        else:
            missing.append(name)

    print(f"Already implemented: {len(have)} / {len(requested)}")
    print(f"Missing:             {len(missing)} / {len(requested)}")
    print()
    if missing:
        print("Missing (sorted):")
        for name in sorted(missing, key=str.lower):
            print(f"  - {name}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
