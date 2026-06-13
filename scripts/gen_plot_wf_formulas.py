#!/usr/bin/env python3
"""Generate WGSL formula-switch functions for the JWF plot-family ports.

Parses JWildfire's plot preset files (output/variation-jwf-source/plot/
*_wf_presets.txt) and emits one WGSL function per variation that maps
`preset_id` to the transpiled formula(s). The output files land in
output/generated/<name>_formula.wgsl and are pasted into the
corresponding src/variations/defs/<name>.rs WGSL bodies (the def files
note they are generated — re-run this script if the preset corpus ever
changes and re-splice).

Transpile rules (Java math expression -> WGSL):
  - integer literals floatified (12 -> 12.0), with lookarounds so 1.2
    and exponents survive
  - fabs -> abs; sqr(x) -> <name>_sqr(x) helper (per-def prefix to
    avoid cross-variation collisions in the concatenated shader)
  - `pi` resolved by a local `let pi = ...;` in the emitted function
  - param_a..param_f become function arguments (live param values)
  - the single ternary in the corpus (yplot2d #7) is special-cased

Out-of-range ids (including -1 = custom JWF formula) return the JWF
default-preset formula value 0.0.
"""

import os
import re
import sys

SRC = os.path.join("output", "variation-jwf-source", "plot")
DST = os.path.join("output", "generated")

# (name, preset file, formula keys, formula arg vars)
VARIATIONS = [
    ("yplot2d_wf", "yplot2d_wf_presets.txt", ["formula"], ["x"]),
    ("parplot2d_wf", "parplot2d_wf_presets.txt", ["xformula", "yformula", "zformula"], ["u", "v"]),
    ("polarplot2d_wf", "polarplot2d_wf_presets.txt", ["formula"], ["t"]),
    ("polarplot3d_wf", "polarplot3d_wf_presets.txt", ["formula"], ["t", "u"]),
]

# Hand-translated ternaries (Java `c?a:b` has no direct regex-safe
# transpile; the corpus has exactly one in the ported set).
TERNARY_OVERRIDES = {
    "x>0?pow(x,param_a):-pow(-x,param_a)":
        "select(-pow(-x, param_a), pow(x, param_a), x > 0.0)",
}


def parse_presets(path, keys):
    presets = {}
    cur_id = None
    with open(path, encoding="utf-8") as f:
        for line in f:
            line = line.strip()
            m = re.match(r"^##(\d+)", line)
            if m:
                cur_id = int(m.group(1))
                presets[cur_id] = {}
                continue
            if cur_id is None or "=" not in line:
                continue
            key, _, value = line.partition("=")
            key = key.strip()
            if key in keys:
                # JWF's WFFuncPresets.parseToken strips trailing
                # `---` annotations ("--- change the 0 value for...").
                cut = value.find("---")
                if cut > 0:
                    value = value[:cut]
                presets[cur_id][key] = value.strip()
    return presets


def floatify(expr):
    # Integer literal not part of an identifier, not adjacent to a dot
    # or another digit -> append .0
    return re.sub(r"(?<![\w.])(\d+)(?![\w.])", r"\1.0", expr)


def transpile(expr, name):
    if expr in TERNARY_OVERRIDES:
        return TERNARY_OVERRIDES[expr]
    if "?" in expr:
        raise SystemExit(f"unhandled ternary in {name}: {expr}")
    e = floatify(expr)
    e = e.replace("fabs(", "abs(")
    e = re.sub(r"\bsqr\(", f"{name}_sqr(", e)
    return e


def emit(name, presets, keys, args):
    out = []
    params = ", ".join(f"{a}: f32" for a in args)
    out.append(f"fn {name}_sqr(v: f32) -> f32 {{ return v * v; }}\n")
    if len(keys) == 1:
        ret = "f32"
        default = "return 0.0;"
    else:
        ret = "vec3<f32>"
        default = "return vec3<f32>(0.0, 0.0, 0.0);"
    out.append(
        f"fn {name}_formula(id: i32, {params}, param_a: f32, param_b: f32, "
        f"param_c: f32, param_d: f32, param_e: f32, param_f: f32) -> {ret} {{"
    )
    out.append("    let pi = 3.14159265358979;")
    for pid in sorted(presets):
        body = presets[pid]
        if any(k not in body for k in keys):
            raise SystemExit(f"{name} preset {pid} missing a formula key")
        if len(keys) == 1:
            expr = transpile(body[keys[0]], name)
            out.append(f"    if (id == {pid}) {{ return {expr}; }}")
        else:
            exprs = [transpile(body[k], name) for k in keys]
            out.append(f"    if (id == {pid}) {{")
            out.append(f"        return vec3<f32>(")
            out.append(f"            {exprs[0]},")
            out.append(f"            {exprs[1]},")
            out.append(f"            {exprs[2]});")
            out.append("    }")
    out.append(f"    {default}")
    out.append("}")
    return "\n".join(out) + "\n"


BEGIN_MARK = "// BEGIN GENERATED FORMULAS (scripts/gen_plot_wf_formulas.py)"
END_MARK = "// END GENERATED FORMULAS"


def splice(name, wgsl):
    """Replace every marker region in the def file with the generated
    formulas (the 2D and 3D bodies each carry one region)."""
    path = os.path.join("src", "variations", "defs", f"{name}.rs")
    if not os.path.exists(path):
        print(f"  (no {path} yet - skipping splice)")
        return
    with open(path, encoding="utf-8") as f:
        src = f.read()
    pattern = re.compile(
        re.escape(BEGIN_MARK) + r".*?" + re.escape(END_MARK), re.S
    )
    if not pattern.search(src):
        raise SystemExit(f"{path}: missing formula marker region")
    replacement = BEGIN_MARK + "\n" + wgsl + END_MARK
    src, n = pattern.subn(replacement.replace("\\", r"\\"), src)
    with open(path, "w", encoding="utf-8", newline="\n") as f:
        f.write(src)
    print(f"  spliced into {path} ({n} regions)")


def main():
    os.makedirs(DST, exist_ok=True)
    for name, fname, keys, args in VARIATIONS:
        presets = parse_presets(os.path.join(SRC, fname), keys)
        wgsl = emit(name, presets, keys, args)
        dst = os.path.join(DST, f"{name}_formula.wgsl")
        with open(dst, "w", encoding="utf-8") as f:
            f.write(wgsl)
        print(f"{name}: {len(presets)} presets -> {dst}")
        splice(name, wgsl)


if __name__ == "__main__":
    main()
