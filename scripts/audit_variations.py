"""Audit script for the variations corpus.

Reports:
  1. Defined variations    (`pub static FOO: VariationDef` declarations)
  2. Registered variations (entries in `ALL_VARIATIONS` array in mod.rs)
  3. Documented variations (have a `///` doc-comment block)
  4. Authored variations   (have a `# Authors` block)
  5. Author tally          (each `/// - Name` line counted)
  6. Param description coverage
  7. Discrepancies         (defined but not registered, etc.)
  8. Name-variant suspects (e.g., differing capitalization of the same author)

Run from repo root: `python scripts/audit_variations.py`
Read-only — no files modified.
"""
from __future__ import annotations
import os
import re
import sys
from collections import Counter, defaultdict
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent
DEFS_DIR = REPO_ROOT / "src" / "variations" / "defs"
MOD_FILE = DEFS_DIR / "mod.rs"

# Pattern: capture each `pub static NAME: VariationDef = VariationDef {` and
# the optional `///` block immediately preceding it (with possible blank
# lines between `///` lines and the static).
STATIC_RE = re.compile(
    r"((?:^[ \t]*///[^\n]*\n)+)?^pub static ([A-Z_][A-Z0-9_]*): VariationDef\b",
    re.MULTILINE,
)
PARAM_RE = re.compile(r"param!\(", re.MULTILINE)
AUTHOR_LINE_RE = re.compile(r"^[ \t]*///[ \t]*-[ \t]+(.+?)[ \t]*$", re.MULTILINE)


def find_macro_close(text: str, open_paren_idx: int) -> int:
    """Find matching `)` for the `(` at `open_paren_idx`, ignoring string contents."""
    depth = 1
    i = open_paren_idx + 1
    n = len(text)
    while i < n:
        c = text[i]
        if c == '"':
            i += 1
            while i < n:
                if text[i] == "\\":
                    i += 2
                    continue
                if text[i] == '"':
                    i += 1
                    break
                i += 1
            continue
        if c == "(":
            depth += 1
        elif c == ")":
            depth -= 1
            if depth == 0:
                return i
        i += 1
    return -1


def split_top_level(inner: str) -> list[str]:
    """Split `inner` on top-level commas, respecting strings and nested parens."""
    args: list[str] = []
    cur: list[str] = []
    depth = 0
    in_str = False
    j = 0
    n = len(inner)
    while j < n:
        ch = inner[j]
        if in_str:
            cur.append(ch)
            if ch == "\\" and j + 1 < n:
                cur.append(inner[j + 1])
                j += 2
                continue
            if ch == '"':
                in_str = False
        else:
            if ch == '"':
                cur.append(ch)
                in_str = True
            elif ch == "(":
                cur.append(ch)
                depth += 1
            elif ch == ")":
                cur.append(ch)
                depth -= 1
            elif ch == "," and depth == 0:
                args.append("".join(cur).strip())
                cur = []
            else:
                cur.append(ch)
        j += 1
    if cur:
        args.append("".join(cur).strip())
    return args


def count_param_desc(block: str) -> tuple[int, int]:
    """Return (total_params, params_with_description) in the parameters block."""
    total = 0
    with_desc = 0
    for m in PARAM_RE.finditer(block):
        open_idx = m.end() - 1  # the `(` of `param!(`
        close_idx = find_macro_close(block, open_idx)
        if close_idx == -1:
            continue
        args = split_top_level(block[open_idx + 1:close_idx])
        total += 1
        # The description (when present) is always the last arg, and is
        # always a string literal. Other args (name, display) are also
        # strings, but they're not in the trailing position when a
        # description is omitted, so checking the last arg is enough.
        # param!(name, display, type, default, min, max [, description])
        # param!(name, display, angle, default [, description])
        if args and args[-1].startswith('"') and args[-1].endswith('"'):
            # Must also not be the display_name slot (arg index 1), so
            # require at least 5 args (the smallest legal `with-desc` form
            # is angle+desc = 5 args).
            if len(args) >= 5:
                with_desc += 1
    return total, with_desc


def extract_registered(mod_text: str) -> list[str]:
    """Pull the list of registered variation names from ALL_VARIATIONS."""
    start_re = re.compile(r"^pub static ALL_VARIATIONS: &\[&VariationDef\] = &\[", re.MULTILINE)
    m = start_re.search(mod_text)
    if not m:
        return []
    body_start = m.end()
    # Find matching `];` (the array close)
    end_idx = mod_text.find("\n];", body_start)
    if end_idx == -1:
        return []
    body = mod_text[body_start:end_idx]
    return re.findall(r"^[ \t]*&([A-Z_][A-Z0-9_]*)\s*,", body, re.MULTILINE)


def main() -> int:
    if not DEFS_DIR.is_dir():
        print(f"ERROR: defs dir not found: {DEFS_DIR}", file=sys.stderr)
        return 1

    # Collect all defined statics across defs/*.rs (excluding mod.rs itself)
    defined: dict[str, dict] = {}  # NAME → { file, doc_lines, has_authors, authors, param_total, param_with_desc }
    for path in sorted(DEFS_DIR.glob("*.rs")):
        if path.name == "mod.rs":
            continue
        text = path.read_text(encoding="utf-8", errors="replace")
        for m in STATIC_RE.finditer(text):
            doc_block = m.group(1) or ""
            name = m.group(2)
            authors: list[str] = []
            # Look for a `# Authors` section followed by `- Author` bullets
            authors_section = re.search(
                r"///[ \t]*# Authors\s*\n((?:[ \t]*///[ \t]*-[ \t]+[^\n]+\n)+)",
                doc_block,
            )
            if authors_section:
                for am in AUTHOR_LINE_RE.finditer(authors_section.group(1)):
                    authors.append(am.group(1).strip())
            # Count params + descriptions: find the parameters: &[ ... ] block
            block_start = m.end()
            block_end = text.find("\npub static ", block_start)
            if block_end == -1:
                block_end = len(text)
            static_block = text[block_start:block_end]
            param_total, param_desc = count_param_desc(static_block)
            defined[name] = {
                "file": path.name,
                "has_doc": bool(doc_block.strip()),
                "has_authors": bool(authors),
                "authors": authors,
                "param_total": param_total,
                "param_with_desc": param_desc,
            }

    # Registered list
    registered = extract_registered(MOD_FILE.read_text(encoding="utf-8", errors="replace"))
    registered_set = set(registered)
    defined_set = set(defined.keys())

    # ---- Output ----
    print("=" * 70)
    print("VARIATION CORPUS AUDIT")
    print("=" * 70)
    print()
    print(f"Defined statics in src/variations/defs/*.rs  : {len(defined)}")
    print(f"Registered in ALL_VARIATIONS (mod.rs)        : {len(registered)}")
    print(f"  (duplicates in registration list)           : {len(registered) - len(registered_set)}")
    documented = sum(1 for v in defined.values() if v["has_doc"])
    authored = sum(1 for v in defined.values() if v["has_authors"])
    print(f"With a `///` doc-comment                     : {documented}  ({documented * 100 // max(len(defined), 1)}%)")
    print(f"With a `# Authors` block                     : {authored}  ({authored * 100 // max(len(defined), 1)}%)")

    total_params = sum(v["param_total"] for v in defined.values())
    total_desc = sum(v["param_with_desc"] for v in defined.values())
    pct = total_desc * 100 // max(total_params, 1)
    print(f"Parameters with description                  : {total_desc} / {total_params}  ({pct}%)")

    print()
    print("-" * 70)
    print("DISCREPANCIES")
    print("-" * 70)
    defined_not_registered = sorted(defined_set - registered_set)
    registered_not_defined = sorted(registered_set - defined_set)
    if defined_not_registered:
        print(f"\nDefined but NOT registered in ALL_VARIATIONS ({len(defined_not_registered)}):")
        for n in defined_not_registered:
            print(f"  - {n}  ({defined[n]['file']})")
    else:
        print("\nDefined-but-not-registered: (none)")
    if registered_not_defined:
        print(f"\nRegistered but NOT defined ({len(registered_not_defined)}):")
        for n in registered_not_defined:
            print(f"  - {n}")
    else:
        print("Registered-but-not-defined: (none)")

    # Documentation gaps
    undocumented = sorted(name for name, v in defined.items() if not v["has_doc"])
    if undocumented:
        print(f"\nNo `///` doc-comment ({len(undocumented)}):")
        for n in undocumented:
            print(f"  - {n}  ({defined[n]['file']})")
    unauthored = sorted(
        name for name, v in defined.items()
        if v["has_doc"] and not v["has_authors"]
    )
    if unauthored:
        print(f"\nDoc-comment present but no `# Authors` block ({len(unauthored)}):")
        for n in unauthored:
            print(f"  - {n}  ({defined[n]['file']})")

    # Param-description gaps
    incomplete_params = sorted(
        (name, v["param_total"], v["param_with_desc"])
        for name, v in defined.items()
        if v["param_total"] > 0 and v["param_with_desc"] < v["param_total"]
    )
    if incomplete_params:
        print(f"\nVariations with missing param descriptions ({len(incomplete_params)}):")
        for n, total, desc in incomplete_params:
            print(f"  - {n}: {desc}/{total} described  ({defined[n]['file']})")

    print()
    print("-" * 70)
    print("AUTHOR TALLY")
    print("-" * 70)
    author_counts: Counter[str] = Counter()
    author_variations: defaultdict[str, list[str]] = defaultdict(list)
    for name, v in defined.items():
        for a in v["authors"]:
            author_counts[a] += 1
            author_variations[a].append(name)
    print(f"\n{len(author_counts)} distinct author strings, {sum(author_counts.values())} total credits\n")
    for author, count in sorted(author_counts.items(), key=lambda kv: (-kv[1], kv[0].lower())):
        print(f"  {count:4d}  {author}")

    # Suspect name variants (same name modulo case / whitespace)
    print()
    print("-" * 70)
    print("POSSIBLE NAME-VARIANT ISSUES")
    print("-" * 70)
    by_normalized: defaultdict[str, list[str]] = defaultdict(list)
    for author in author_counts:
        norm = re.sub(r"[\s_-]+", "", author).lower()
        by_normalized[norm].append(author)
    found_variants = False
    for norm, variants in sorted(by_normalized.items()):
        if len(variants) > 1:
            found_variants = True
            print(f"\n  Group `{norm}`:")
            for v in variants:
                print(f"    {author_counts[v]:4d}  {v!r}")
    if not found_variants:
        print("\n  No case-insensitive duplicates found.")

    return 0


if __name__ == "__main__":
    sys.exit(main())
