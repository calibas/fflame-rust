#!/usr/bin/env python3
"""Migrate variation defs for the fx_priority feature (two passes).

See docs/projects/jwf-features.md ("<var>_fx_priority"). Two independent,
re-runnable passes over src/variations/defs/*.rs:

  Pass 1 (Any):   set `phase: VariationPhase::Normal` -> `Any` on every
                  Normal variation that is mechanically safe to move,
                  i.e. NOT NeedsAccum and state_count == 0. NeedsAccum
                  MUST be excluded (its signature carries an `accum` arg
                  the pre/post emission doesn't pass). Stateful vars are
                  excluded for safety. Excluded vars stay Normal (locked)
                  and are listed in the review queue.

  Pass 2 (Replace): classify each variation's local JWildfire source
                  (output/variation-jwf-source/*.java) by its
                  `pVarTP.{x,y,z}` write op — all `=` -> replace ->
                  add `Feature::Replace`; all `+=`/`-=` -> accumulate
                  (nothing); mixed / no source -> review queue.
                  `Replace` is inert in the normal phase, so it only
                  changes behavior once a variation is actually moved.

Dry-run by default (reports only). Pass --apply to write the files.
Idempotent: skips defs already `Any` / already carrying `Feature::Replace`.

  python scripts/migrate_fx_priority_phases.py            # report
  python scripts/migrate_fx_priority_phases.py --apply    # write
"""
import glob
import io
import os
import re
import sys

DEFS_GLOB = "src/variations/defs/*.rs"
JAVA_DIR = "output/variation-jwf-source"

# A pVarTP x/y write statement: capture coord, operator, and RHS (to the
# next `;`). We look at x/y ONLY, not z: the combine mode (replace vs
# accumulate) is uniform across x and y, while z is governed by the
# orthogonal preserve_z gate (`if (isPreserveZCoordinate()) pVarTP.z +=
# pAmount*z;`) — counting the gated z-add would mislabel the standard 3D
# pattern (x/y assigned, z gated-add) as "mixed".
#
# Crucially, a write is "accumulate" not just for `+=`/`-=` but also for
# the explicit `pVarTP.x = pVarTP.x + ...` form (JuliaN/JuliaScope write
# it this way) — i.e. an `=` whose RHS references the same coordinate.
# Only an `=` whose RHS does NOT reference `pVarTP.<coord>` is a genuine
# replace (e.g. crop's `pVarTP.x = pAmount * x;`).
JAVA_WRITE_RE = re.compile(r"pVarTP\.([xy])\s*([+\-*/]?=)(?!=)\s*([^;]*);")
NAME_CONST_RE = re.compile(r'String\s+NAME\s*=\s*"([^"]+)"')
GETNAME_RE = re.compile(r'getName\(\)\s*\{\s*return\s+"([^"]+)"', re.S)

# Within a single def block.
DEF_START_RE = re.compile(r"pub static (\w+): VariationDef = VariationDef \{")
NAME_RE = re.compile(r'name:\s*"([^"]+)",')
PHASE_RE = re.compile(r"phase:\s*VariationPhase::(\w+),")
FEATURES_RE = re.compile(r"features:\s*&\[(.*?)\]", re.S)
STATE_COUNT_RE = re.compile(r"state_count:\s*(\d+),")


class Block:
    def __init__(self, const, name, text, start, end):
        self.const = const
        self.name = name
        self.text = text          # the block's source slice
        self.start = start
        self.end = end


def split_blocks(src):
    """Yield Block objects for each `pub static X: VariationDef` in src."""
    starts = [m.start() for m in DEF_START_RE.finditer(src)]
    const_names = [m.group(1) for m in DEF_START_RE.finditer(src)]
    blocks = []
    for i, s in enumerate(starts):
        e = starts[i + 1] if i + 1 < len(starts) else len(src)
        text = src[s:e]
        nm = NAME_RE.search(text)
        blocks.append(Block(const_names[i], nm.group(1) if nm else None, text, s, e))
    return blocks


def block_meta(block):
    phase = PHASE_RE.search(block.text)
    feats = FEATURES_RE.search(block.text)
    state = STATE_COUNT_RE.search(block.text)
    aliases = re.search(r'aliases:\s*&\[(.*?)\]', block.text, re.S)
    alias_names = re.findall(r'"([^"]+)"', aliases.group(1)) if aliases else []
    return {
        "phase": phase.group(1) if phase else None,
        "features": feats.group(1) if feats else "",
        "state": int(state.group(1)) if state else 0,
        "aliases": alias_names,
    }


# ---------------------------------------------------------------------------
# JWildfire source classification (replace vs accumulate)
# ---------------------------------------------------------------------------
def extract_java_name(src):
    m = NAME_CONST_RE.search(src)
    if m:
        return m.group(1)
    m = GETNAME_RE.search(src)
    return m.group(1) if m else None


def classify_java(src):
    """'replace' | 'accumulate' | 'mixed' | 'none'."""
    votes = []  # True = replace, False = accumulate
    for coord, op, rhs in JAVA_WRITE_RE.findall(src):
        if op != "=":
            votes.append(False)                 # +=/-=/*=//= → accumulate
        else:
            # `=` is replace UNLESS the RHS reads the same coordinate
            # (`pVarTP.x = pVarTP.x + ...` is accumulate in disguise).
            votes.append(f"pVarTP.{coord}" not in rhs)
    if not votes:
        return "none"
    if all(votes):
        return "replace"
    if not any(votes):
        return "accumulate"
    return "mixed"


def load_java_classes():
    """name -> class for every local JWF source."""
    out = {}
    for jpath in glob.glob(os.path.join(JAVA_DIR, "*.java")):
        base = os.path.basename(jpath)
        if base.startswith("Abstract"):
            continue
        src = io.open(jpath, encoding="utf-8", errors="replace").read()
        name = extract_java_name(src)
        if name:
            out[name] = classify_java(src)
    return out


# ---------------------------------------------------------------------------
# Edits
# ---------------------------------------------------------------------------
def set_phase_any(text):
    return text.replace("phase: VariationPhase::Normal,",
                        "phase: VariationPhase::Any,", 1)


def ensure_feature_import(s):
    """Make sure `Feature` is importable as a bare name in a def file.
    Returns (new_content, changed)."""
    if re.search(r"definition::\{[^}]*\bFeature\b", s, re.S):
        return s, False
    m = re.search(r"definition::\{", s)
    if m:
        return s[: m.end()] + "Feature, " + s[m.end():], True
    m = re.search(r"definition::(VariationDef\b)", s)
    if m:
        return s[: m.start()] + "definition::{Feature, VariationDef}" + s[m.end():], True
    return s, False


def add_replace_feature(text):
    """Add Feature::Replace to the features slice of a def block."""
    m = FEATURES_RE.search(text)
    if not m:
        return text, False
    inner = m.group(1)
    if "Replace" in inner:
        return text, False  # already
    stripped = inner.strip().rstrip(",").strip()
    new_inner = "Feature::Replace" if not stripped else inner.rstrip() + ", Feature::Replace"
    return text[:m.start(1)] + new_inner + text[m.end(1):], True


def main():
    apply = "--apply" in sys.argv
    java = load_java_classes()

    any_applied, any_skip_accum, any_skip_state, any_already = [], [], [], []
    repl_applied, repl_mixed, repl_nosource, repl_already = [], [], [], []

    for path in sorted(glob.glob(DEFS_GLOB)):
        src = io.open(path, encoding="utf-8").read()
        blocks = split_blocks(src)
        # Rebuild file from block edits (process right-to-left to keep
        # offsets valid).
        new_src = src
        for block in sorted(blocks, key=lambda b: b.start, reverse=True):
            if not block.name:
                continue
            meta = block_meta(block)
            text = block.text
            changed = False

            # --- Pass 1: Any ---
            if meta["phase"] == "Normal":
                has_accum = "NeedsAccum" in meta["features"]
                if has_accum:
                    any_skip_accum.append(block.name)
                elif meta["state"] > 0:
                    any_skip_state.append(block.name)
                else:
                    text = set_phase_any(text)
                    changed = True
                    any_applied.append(block.name)
            elif meta["phase"] == "Any":
                any_already.append(block.name)

            # --- Pass 2: Replace --- (only for vars that could be moved;
            # i.e. Any now, or just-flipped to Any this run). Inert
            # otherwise, but we scope it to keep the change meaningful.
            is_any = meta["phase"] == "Any" or "VariationPhase::Any," in text
            if is_any:
                names = [block.name] + meta["aliases"]
                cls = next((java[n] for n in names if n in java), None)
                if "Replace" in meta["features"]:
                    repl_already.append(block.name)
                elif cls == "replace":
                    text, ok = add_replace_feature(text)
                    if ok:
                        changed = True
                        repl_applied.append(block.name)
                elif cls == "mixed":
                    repl_mixed.append(block.name)
                elif cls is None:
                    repl_nosource.append(block.name)

            if changed:
                new_src = new_src[:block.start] + text + new_src[block.end:]

        # If we added Feature::Replace, make sure the file imports Feature.
        if "Feature::Replace" in new_src:
            new_src, _ = ensure_feature_import(new_src)

        if apply and new_src != src:
            io.open(path, "w", encoding="utf-8", newline="\n").write(new_src)

    def report(title, items):
        print(f"\n=== {title} ({len(items)})")
        for it in sorted(items):
            print(f"  {it}")

    print("fx_priority phase migration", "(APPLIED)" if apply else "(dry-run)")
    report("Pass1 Any APPLIED (Normal -> Any)", any_applied)
    report("Pass1 review queue: NeedsAccum (stay Normal)", any_skip_accum)
    report("Pass1 review queue: stateful (stay Normal)", any_skip_state)
    report("Pass1 already Any", any_already)
    report("Pass2 Replace APPLIED", repl_applied)
    report("Pass2 review: mixed JWF source (stay accumulate)", repl_mixed)
    report("Pass2 review: no local JWF source (stay accumulate)", repl_nosource)
    report("Pass2 already Replace", repl_already)
    print(f"\nSummary: {len(any_applied)} -> Any, "
          f"{len(repl_applied)} -> Replace, "
          f"{len(any_skip_accum) + len(any_skip_state)} Any review-queue, "
          f"{len(repl_mixed) + len(repl_nosource)} Replace review-queue")


if __name__ == "__main__":
    main()
