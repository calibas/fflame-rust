"""Audit JWF z-write semantics and apply Feature::AlwaysZ to our defs.

JWildfire has three z-write classes for variations:

  - gated (the default): `if (pContext.isPreserveZCoordinate())
    pVarTP.z += pAmount * pAffineTP.z;` — z passthrough only when the
    flame's preserve_z is on.
  - always: unconditional `pVarTP.z` writes (true-3D variations like
    Julia3DFunc, ZConeFunc). These let z compound across iterations.
  - never: the transform never touches pVarTP.z.

Our 3D dispatch (shader_builder_v2::build_apply_variations_3d) zeroes
the z component of gated variations' contributions under
preserve_z = false; variations carrying Feature::AlwaysZ keep z. This
script classifies every local JWF source in
output/variation-jwf-source/*.java and ADDS `Feature::AlwaysZ` to the
matching defs in src/variations/defs/*.rs for the "always" class.

"never" candidates are REPORTED but not applied (Feature::NeverZ
exists, but our hand-written 3D bodies sometimes extend 2D variations
deliberately, so enforcement needs per-def review). "mixed" files
(some writes gated, some not) are reported for manual review and left
at the gated default — conservative, identical to the old blanket
flatten for them.

Run from the repo root:  python scripts/audit_z_write_semantics.py
Re-runnable: skips defs that already carry Feature::AlwaysZ.
"""
import io
import re
import glob
import os

JAVA_DIR = "output/variation-jwf-source"
DEFS_GLOB = "src/variations/defs/*.rs"

WRITE_RE = re.compile(r"pVarTP\.z\s*(\+=|-=|\*=|/=|=[^=])")
GATE_RE = re.compile(r"isPreserveZCoordinate")
NAME_CONST_RE = re.compile(r'String\s+NAME\s*=\s*"([^"]+)"')
GETNAME_RE = re.compile(r'getName\(\)\s*\{\s*return\s+"([^"]+)"', re.S)


def extract_name(src):
    m = NAME_CONST_RE.search(src)
    if m:
        return m.group(1)
    m = GETNAME_RE.search(src)
    if m:
        return m.group(1)
    return None


def classify(src):
    """Return 'always' | 'gated' | 'mixed' | 'never'."""
    lines = src.splitlines()
    writes = []
    for i, line in enumerate(lines):
        if WRITE_RE.search(line):
            # A write is "gated" if isPreserveZCoordinate appears on
            # the same line or within the 4 lines above (the standard
            # JWF pattern is the write directly inside the if-block).
            ctx = "\n".join(lines[max(0, i - 4): i + 1])
            writes.append(bool(GATE_RE.search(ctx)))
    if not writes:
        return "never"
    if all(writes):
        return "gated"
    if not any(writes):
        return "always"
    return "mixed"


def load_defs():
    """Map canonical variation name -> (path, file content)."""
    defs = {}
    for path in glob.glob(DEFS_GLOB):
        s = io.open(path, encoding="utf-8").read()
        for m in re.finditer(r'name:\s*"([^"]+)",', s):
            defs[m.group(1)] = path
    return defs


def ensure_feature_import(s, path):
    """Make sure `Feature` is importable as a bare name. Returns
    (new_content, ok)."""
    if re.search(r"definition::\{[^}]*\bFeature\b", s, re.S):
        return s, True
    m = re.search(r"definition::\{", s)
    if m:
        return s[: m.end()] + "Feature, " + s[m.end():], True
    # `definition::VariationDef` style (no brace group)
    m = re.search(r"definition::(VariationDef\b)", s)
    if m:
        return s[: m.start()] + "definition::{Feature, VariationDef}" + s[m.end():], True
    return s, False


def add_always_z(s, name):
    """Add Feature::AlwaysZ to the features slice of def `name`.
    Returns (new_content, status) where status is 'applied',
    'already', or 'failed'."""
    anchor = s.find(f'name: "{name}",')
    if anchor == -1:
        return s, "failed"
    feat = s.find("features: &[", anchor)
    end = s.find("]", feat)
    if feat == -1 or end == -1:
        return s, "failed"
    inner = s[feat + len("features: &["): end]
    if "AlwaysZ" in inner:
        return s, "already"
    if "NeverZ" in inner:
        return s, "failed"  # contradiction — manual review
    new_inner = "Feature::AlwaysZ" if not inner.strip() else inner + ", Feature::AlwaysZ"
    return s[: feat + len("features: &[")] + new_inner + s[end:], "applied"


def main():
    defs = load_defs()
    report = {"applied": [], "already": [], "mixed": [], "never": [],
              "unported": [], "no_name": [], "failed": []}

    for jpath in sorted(glob.glob(os.path.join(JAVA_DIR, "*.java"))):
        base = os.path.basename(jpath)
        if base.startswith("Abstract"):
            continue
        src = io.open(jpath, encoding="utf-8", errors="replace").read()
        name = extract_name(src)
        if not name:
            report["no_name"].append(base)
            continue
        cls = classify(src)
        if cls == "mixed":
            report["mixed"].append(f"{name} ({base})")
            continue
        if cls == "never":
            if name in defs:
                report["never"].append(f"{name} ({base})")
            continue
        if cls == "gated":
            continue  # default — nothing to do
        # cls == "always"
        if name not in defs:
            report["unported"].append(f"{name} ({base})")
            continue
        path = defs[name]
        s = io.open(path, encoding="utf-8").read()
        s2, status = add_always_z(s, name)
        if status == "applied":
            s2, ok = ensure_feature_import(s2, path)
            if not ok:
                report["failed"].append(f"{name} ({path}): import")
                continue
            io.open(path, "w", encoding="utf-8", newline="").write(s2)
            report["applied"].append(f"{name} -> {path}")
        elif status == "already":
            report["already"].append(name)
        else:
            report["failed"].append(f"{name} ({path})")

    for key in ("applied", "already", "mixed", "never", "unported", "no_name", "failed"):
        items = report[key]
        print(f"\n=== {key} ({len(items)})")
        for it in items[:200]:
            print(f"  {it}")


if __name__ == "__main__":
    main()
