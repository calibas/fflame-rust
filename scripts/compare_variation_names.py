"""Compare variation names: our registry vs upstream Apophysis/JWildfire.

Extracts:
  - Apophysis canonical name from `APO_PLUGIN("name")` in each
    `output/jwildfire-vars/output/*.cpp`.
  - JWildfire canonical name from the embedded `getName()` body in
    the same `.cpp` files (commented-out Java reference). JWildfire's
    `getName()` is authoritative; cpp ports occasionally diverge.
  - Our canonical name from `name: "..."` in each
    `pub static …: VariationDef` literal under
    `src/variations/defs/`.

Categories reported:
  1. EXACT — name matches all three sides. Quiet (just counted).
  2. CASE-ONLY — same letters, different casing. Strong rename
     candidate (the `curl3d` / `curl3D` family).
  3. SUFFIX / SEPARATOR — same root, different `_3d` vs `3D` vs `3d`
     handling. Often a casing fix too.
  4. CPP-VS-JAVA — Apophysis cpp and embedded Java disagree on the
     name. Useful for picking which canonical name to match.
  5. UPSTREAM-ONLY — name exists upstream but not in our registry.
     Cross-check with `variation-bulk-port.md` to confirm whether it's
     a missing port or an alias / synonym we already handle elsewhere.
  6. OURS-ONLY — name exists in our registry but not in the upstream
     cpp corpus. Could be Apo-7X-Pascal-only, our own additions, or a
     stale rename. Worth auditing.

Run from repo root: `python scripts/compare_variation_names.py`
Read-only — no files modified.
"""
from __future__ import annotations
import os
import re
import sys
from collections import defaultdict

DEFS_DIR = os.path.join("src", "variations", "defs")
CPP_DIR = os.path.join("output", "jwildfire-vars", "output")

APO_PLUGIN_RE = re.compile(r'APO_PLUGIN\("([^"]+)"\)')
JAVA_GETNAME_RE = re.compile(
    r'//\s*public\s+String\s+getName\(\)\s*\{\s*\n//\s*return\s+"([^"]+)"\s*;',
    re.MULTILINE,
)
OUR_NAME_RE = re.compile(r'^\s+name:\s*"([^"]+)",\s*$', re.MULTILINE)
VARIATION_DEF_OPEN_RE = re.compile(
    r'pub\s+static\s+\w+\s*:\s*VariationDef\s*=\s*VariationDef\s*\{'
)


def collect_upstream() -> dict[str, dict[str, str | None]]:
    """Walk cpp corpus, return {file_stem: {apo: name, java: name|None}}."""
    out: dict[str, dict[str, str | None]] = {}
    if not os.path.isdir(CPP_DIR):
        sys.exit(f"upstream dir missing: {CPP_DIR}")
    for fname in sorted(os.listdir(CPP_DIR)):
        if not fname.endswith(".cpp"):
            continue
        path = os.path.join(CPP_DIR, fname)
        with open(path, encoding="utf-8", errors="replace") as f:
            content = f.read()
        apo_match = APO_PLUGIN_RE.search(content)
        if not apo_match:
            continue
        apo_name = apo_match.group(1)
        java_match = JAVA_GETNAME_RE.search(content)
        java_name = java_match.group(1) if java_match else None
        out[apo_name] = {
            "apo": apo_name,
            "java": java_name,
            "file": fname,
        }
    return out


def collect_ours() -> dict[str, str]:
    """Walk our defs, return {variation_name: source_file_path}."""
    out: dict[str, str] = {}
    if not os.path.isdir(DEFS_DIR):
        sys.exit(f"defs dir missing: {DEFS_DIR}")
    for root, _dirs, files in os.walk(DEFS_DIR):
        for fname in files:
            if not fname.endswith(".rs"):
                continue
            path = os.path.join(root, fname)
            with open(path, encoding="utf-8") as f:
                content = f.read()
            # Only count `name:` fields that are inside a VariationDef literal —
            # walk the file linearly, tracking when we're inside a VariationDef.
            inside = False
            depth = 0
            for i, line in enumerate(content.split("\n")):
                if VARIATION_DEF_OPEN_RE.search(line):
                    inside = True
                    depth = 1
                    continue
                if inside:
                    depth += line.count("{") - line.count("}")
                    if depth <= 0:
                        inside = False
                        continue
                    # Only match `name:` at the immediate struct-literal level
                    # (depth == 1), not inside a VariationParamDef literal
                    # (which has its own `name:`).
                    if depth == 1:
                        m = OUR_NAME_RE.match(line)
                        if m:
                            out[m.group(1)] = path
    return out


def categorize(
    upstream: dict[str, dict],
    ours: dict[str, str],
) -> dict[str, list]:
    """Produce categorization buckets."""
    categories: dict[str, list] = defaultdict(list)

    upstream_names = set(upstream.keys())  # Apophysis-side names
    java_names = {
        rec["java"] for rec in upstream.values() if rec["java"]
    }
    all_upstream = upstream_names | java_names
    our_names = set(ours.keys())

    # 1. Apo vs Java disagreement (interesting per-cpp-file).
    for apo_name, rec in upstream.items():
        if rec["java"] and rec["java"] != apo_name:
            categories["cpp-vs-java"].append(
                (apo_name, rec["java"], rec["file"])
            )

    # 2. Exact match (ours == apo OR ours == java).
    exact = set()
    for name in our_names:
        if name in upstream_names or name in java_names:
            exact.add(name)
    categories["exact"] = sorted(exact)

    # 3. Case-only differences: lowercase name matches but exact form doesn't.
    upstream_lower = {n.lower(): n for n in all_upstream}
    case_only = []
    for our_name in sorted(our_names - exact):
        ul = our_name.lower()
        if ul in upstream_lower:
            up = upstream_lower[ul]
            # Also expose Java spelling if different from Apo spelling.
            apo_match = next((n for n in upstream_names if n.lower() == ul), None)
            java_match = next((n for n in java_names if n.lower() == ul), None)
            case_only.append({
                "ours": our_name,
                "apo": apo_match,
                "java": java_match,
                "file": ours[our_name],
            })
    categories["case-only"] = case_only

    # 4. Separator differences (strip underscores + lowercase):
    #    catches `julia_n` vs `julian`, `pre_blur3d` vs `preBlur3D`, etc.
    def norm(s: str) -> str:
        return s.replace("_", "").lower()

    handled = set(exact) | {c["ours"] for c in case_only}
    upstream_norm: dict[str, list[str]] = defaultdict(list)
    for n in all_upstream:
        upstream_norm[norm(n)].append(n)
    separator = []
    for our_name in sorted(our_names - handled):
        n = norm(our_name)
        if n in upstream_norm:
            separator.append({
                "ours": our_name,
                "upstream": sorted(set(upstream_norm[n])),
                "file": ours[our_name],
            })
    categories["separator"] = separator

    # 5. Upstream-only (in upstream cpp, not in ours by any of the above).
    matched_upstream = set()
    for n in our_names:
        if n in upstream_names:
            matched_upstream.add(n)
        if n in java_names:
            # Map back to apo-name to mark the cpp file as covered.
            for apo, rec in upstream.items():
                if rec["java"] == n:
                    matched_upstream.add(apo)
        nl = n.lower()
        if nl in upstream_lower:
            matched_upstream.add(upstream_lower[nl])
        nn = norm(n)
        if nn in upstream_norm:
            for un in upstream_norm[nn]:
                matched_upstream.add(un)
    upstream_only = sorted(upstream_names - matched_upstream)
    categories["upstream-only"] = [
        {"apo": n, "java": upstream[n]["java"], "file": upstream[n]["file"]}
        for n in upstream_only
    ]

    # 6. Ours-only (in our registry, no upstream match by any norm).
    matched_ours = set(exact) | {c["ours"] for c in case_only} | {
        c["ours"] for c in separator
    }
    categories["ours-only"] = [
        {"ours": n, "file": ours[n]}
        for n in sorted(our_names - matched_ours)
    ]

    return categories


def print_report(cats: dict[str, list]) -> None:
    """Format the report. Loud on actionable categories; quiet on EXACT."""
    print(f"=== Variation name comparison ===\n")

    print(f"  EXACT match (no action):                   {len(cats['exact']):>4}")
    print(f"  CASE-ONLY differences (rename candidates): {len(cats['case-only']):>4}")
    print(f"  SEPARATOR / underscore differences:        {len(cats['separator']):>4}")
    print(f"  CPP-VS-JAVA upstream disagreement:         {len(cats['cpp-vs-java']):>4}")
    print(f"  UPSTREAM-ONLY (not in our registry):       {len(cats['upstream-only']):>4}")
    print(f"  OURS-ONLY (not in upstream cpp):           {len(cats['ours-only']):>4}\n")

    if cats["case-only"]:
        print("--- CASE-ONLY differences (rename candidates) ---")
        for c in cats["case-only"]:
            up_strs = []
            if c["apo"] and c["java"] and c["apo"] == c["java"]:
                up_strs.append(f"upstream={c['apo']}")
            else:
                if c["apo"]:
                    up_strs.append(f"apo={c['apo']}")
                if c["java"]:
                    up_strs.append(f"java={c['java']}")
            short_file = c['file'].replace('src/variations/defs/', '').replace('\\', '/')
            print(f"  ours={c['ours']!r}  {'  '.join(up_strs)}  ({short_file})")
        print()

    if cats["separator"]:
        print("--- SEPARATOR / underscore differences ---")
        for c in cats["separator"]:
            up_strs = ", ".join(repr(u) for u in c["upstream"])
            short_file = c['file'].replace('src/variations/defs/', '').replace('\\', '/')
            print(f"  ours={c['ours']!r}  upstream=[{up_strs}]  ({short_file})")
        print()

    if cats["cpp-vs-java"]:
        print("--- CPP-VS-JAVA upstream disagreement (pick Java) ---")
        for apo, java, fname in cats["cpp-vs-java"]:
            print(f"  cpp APO_PLUGIN={apo!r}  java getName={java!r}  ({fname})")
        print()

    if cats["upstream-only"]:
        print(f"--- UPSTREAM-ONLY (top 30 of {len(cats['upstream-only'])}) ---")
        for c in cats["upstream-only"][:30]:
            java_str = f" java={c['java']!r}" if c['java'] and c['java'] != c['apo'] else ""
            print(f"  {c['apo']!r}{java_str}  ({c['file']})")
        if len(cats["upstream-only"]) > 30:
            print(f"  ... + {len(cats['upstream-only']) - 30} more")
        print()

    if cats["ours-only"]:
        print(f"--- OURS-ONLY (top 30 of {len(cats['ours-only'])}) ---")
        for c in cats["ours-only"][:30]:
            short_file = c['file'].replace('src/variations/defs/', '').replace('\\', '/')
            print(f"  {c['ours']!r}  ({short_file})")
        if len(cats["ours-only"]) > 30:
            print(f"  … + {len(cats['ours-only']) - 30} more")
        print()


def main() -> None:
    upstream = collect_upstream()
    ours = collect_ours()
    cats = categorize(upstream, ours)
    print(f"upstream cpp variations: {len(upstream)}")
    print(f"our registered variations: {len(ours)}\n")
    print_report(cats)


if __name__ == "__main__":
    main()
