#!/usr/bin/env python3
"""Generate the sym_ng1..sym_ng17 variation defs from JWildfire's Java.

The "Network Symmetry Group" variations (Jesus Sosa) all share one shape:
pick one of N baked 2x2 symmetry affines at random, apply it to the input
(optionally pre-offset), and accumulate. They differ only in their param
set, the matrix table, and the `init()` formulas that fill each matrix's
translation (c, f).

We parse the **Java** source (JWildfire's default/CPU renderer, the gold
standard) for each `SymNetGN Func.java`:
  - params + defaults (paramNames order + `private double x = d;` fields)
  - the `transfs` literal (a, b, d, e per row; c/f are placeholders that
    init() overwrites)
  - the `init()` body: derived locals (`double s = stepx/2;`), field
    overwrites (`spacex = sqrt(...)`), and `transfs[i][2|5] = expr`
  - the pre-offset in `transform()` (`z.plus(new vec2(ox, oy))`)

…and emit `src/variations/defs/sym_ng.rs` with all 17 defs. Re-runnable.

Java→WGSL transpile is small: floatify int literals, `Math.sqrt`→`sqrt`,
names resolve to the emitted `var` (params, possibly overwritten) / `let`
(derived) locals.
"""

import os
import re

SRC = os.path.join("output", "variation-jwf-source")
OUT = os.path.join("src", "variations", "defs", "sym_ng.rs")
N_VARS = 17


def read(path):
    with open(path, encoding="utf-8") as f:
        return f.read()


def strip_block_comments(s):
    # Java // line comments only appear at line ends in these files.
    return re.sub(r"//[^\n]*", "", s)


def floatify(expr):
    # Append .0 to integer literals not already part of a float/identifier.
    # Java writes `2.` (trailing dot) too -> normalize to `2.0`.
    expr = re.sub(r"(?<![\w.])(\d+)\.(?![\w\d])", r"\1.0", expr)        # 2. -> 2.0
    expr = re.sub(r"(?<![\w.])(\d+)(?![\w.\d])", r"\1.0", expr)         # 2 -> 2.0
    return expr


def transpile(expr):
    e = expr.strip()
    e = e.replace("Math.sqrt", "sqrt")
    e = floatify(e)
    e = re.sub(r"\s+", " ", e).strip()
    # WGSL has no unary `+`. Java writes `+sx`, `+spacex + stepx/2` etc. —
    # strip only the LEADING plus (interior `a + b` stays a binary op).
    e = re.sub(r"^\+\s*", "", e)
    return e


def parse_params(src):
    # PARAM_FOO = "foo"
    const_to_name = dict(re.findall(r'PARAM_(\w+)\s*=\s*"([^"]+)"', src))
    # paramNames = {PARAM_A, PARAM_B, ...}
    m = re.search(r"paramNames\s*=\s*\{([^}]*)\}", src)
    order = []
    if m:
        for c in m.group(1).split(","):
            c = c.strip()
            cm = re.match(r"PARAM_(\w+)", c)
            if cm and cm.group(1) in const_to_name:
                order.append(const_to_name[cm.group(1)])
    # private double foo = 0.5;
    defaults = {}
    for name, val in re.findall(r"private\s+double\s+(\w+)\s*=\s*([\d.+-]+)", src):
        defaults[name] = val
    return [(n, defaults.get(n, "0.0")) for n in order]


def parse_transfs(src):
    s = strip_block_comments(src)
    m = re.search(r"transfs\s*=\s*\{(.*?)\}\s*;", s, re.S)
    body = m.group(1)
    rows = re.findall(r"\{([^{}]*)\}", body)
    mats = []
    for row in rows:
        nums = [v.strip() for v in row.split(",") if v.strip() != ""]
        mats.append([floatify(v) for v in nums])  # [a,b,c,d,e,f]
    return mats


def parse_init(src):
    """Return (statements, c_expr[i], f_expr[i]) from init().
    statements: list of ('let'|'set', name, wgsl_expr) for derived locals and
    field overwrites, in order. c_expr/f_expr: dict idx -> wgsl_expr."""
    s = strip_block_comments(src)
    m = re.search(r"public\s+void\s+init\s*\([^)]*\)\s*\{(.*?)\n\s*\}", s, re.S)
    body = m.group(1)
    stmts = []
    c_expr, f_expr = {}, {}
    for raw in body.split(";"):
        st = raw.strip()
        if not st:
            continue
        mt = re.match(r"transfs\[(\d+)\]\[(\d+)\]\s*=\s*(.+)", st, re.S)
        if mt:
            idx, col, rhs = int(mt.group(1)), int(mt.group(2)), mt.group(3)
            if col == 2:
                c_expr[idx] = transpile(rhs)
            elif col == 5:
                f_expr[idx] = transpile(rhs)
            continue
        md = re.match(r"double\s+(\w+)\s*=\s*(.+)", st, re.S)
        if md:
            stmts.append(("let", md.group(1), transpile(md.group(2))))
            continue
        ms = re.match(r"(\w+)\s*=\s*(.+)", st, re.S)
        if ms:
            stmts.append(("set", ms.group(1), transpile(ms.group(2))))
            continue
    return stmts, c_expr, f_expr


def parse_offset(src):
    s = strip_block_comments(src)
    m = re.search(r"z\.plus\(new\s+vec2\(([^,]+),([^)]+)\)\)", s)
    if not m:
        return None
    return (transpile(m.group(1)), transpile(m.group(2)))


def _idents(*exprs):
    ids = set()
    for ex in exprs:
        if ex is None:
            continue
        ids |= set(re.findall(r"[A-Za-z_]\w*", ex))
    return ids


def emit_body(name, params, mats, stmts, c_expr, f_expr, offset, dim):
    """Emit one variation_<name> WGSL function body (2D or 3D)."""
    slot = {p[0]: i for i, p in enumerate(params)}
    param_names = {p[0] for p in params}
    # Every identifier referenced anywhere in the math.
    refs = _idents(*( [offset[0], offset[1]] if offset else [] ),
                   *(ex for _, _, ex in stmts),
                   *c_expr.values(), *f_expr.values())
    # Params reassigned in init() ("set" to a param name) need `var`.
    overwritten = {nm for k, nm, _ in stmts if k == "set" and nm in param_names}

    pt = "vec2<f32>" if dim == 2 else "vec3<f32>"
    sig = (f"fn variation_{name}(p: {pt}, xform_id: u32, variation_id: u32, "
           f"rng: ptr<function, RngState>) -> {pt} {{")
    L = [sig]
    # Only emit get_param for params actually referenced by the math
    # (others still exist in the def's `parameters` for round-trip).
    declared = set()
    for p, _ in params:
        if p in refs or p in overwritten:
            kw = "var" if p in overwritten else "let"
            L.append(f"    {kw} {p} = get_param(xform_id, variation_id, {slot[p]}u);")
            declared.add(p)
    # init body statements in order
    for kind, nm, ex in stmts:
        if kind == "let":
            L.append(f"    let {nm} = {ex};")
            declared.add(nm)
        elif nm in param_names:        # set: reassign param var
            L.append(f"    {nm} = {ex};")
        elif nm in declared:           # set: reassign a derived field
            L.append(f"    {nm} = {ex};")
        else:                          # set: first decl of a non-param field
            L.append(f"    let {nm} = {ex};")
            declared.add(nm)
    # pre-offset
    if offset:
        L.append(f"    let zx = p.x + ({offset[0]});")
        L.append(f"    let zy = p.y + ({offset[1]});")
    else:
        L.append("    let zx = p.x;")
        L.append("    let zy = p.y;")
    # pick matrix
    n = len(mats)
    L.append(f"    let idx = i32(floor(rng_nextf(rng) * {float(n)}));")
    L.append("    var a: f32; var b: f32; var c: f32; var d: f32; var e: f32; var ff: f32;")
    L.append("    switch (idx) {")
    for i, mrow in enumerate(mats):
        a, b, _, d, e, _ = mrow
        c = c_expr.get(i, mrow[2])
        ff = f_expr.get(i, mrow[5])
        last = "default: " if i == n - 1 else f"case {i}: "
        L.append(f"        {last}{{ a = {a}; b = {b}; c = {c}; d = {d}; e = {e}; ff = {ff}; }}")
    L.append("    }")
    L.append("    let ox = a * zx + b * zy + c;")
    L.append("    let oy = d * zx + e * zy + ff;")
    if dim == 2:
        L.append("    return vec2<f32>(ox, oy);")
    else:
        L.append("    return vec3<f32>(ox, oy, p.z);")
    L.append("}")
    return "\n".join(L)


def emit_def(num, name, params, body2d, body3d):
    const = name.upper()
    pretty = f"Sym NG{num}"
    plist = []
    for p, default in params:
        # all are unlimited_float sliders; tooltip is generic
        plist.append(
            f'        param!("{p}", "{p}", unlimited_float, {floatify(default)}, '
            f'-100.0, 100.0, "Network-symmetry parameter (see JWildfire SymNetG{num})."),'
        )
    plist_s = "\n".join(plist) if plist else ""
    params_field = f"    parameters: &[\n{plist_s}\n    ]," if plist else "    parameters: &[],"
    return f'''pub static {const}: VariationDef = VariationDef {{
    name: "{name}",
    aliases: &[],
    display_name: "{pretty}",
    category: VariationCategory::Plugin,
    phase: VariationPhase::Normal,
    features: &[Feature::NeedsRng],
{params_field}
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    wgsl_2d: r#"
{body2d}
"#,
    wgsl_3d: r#"
{body3d}
"#,
}};
'''


def main():
    defs = []
    consts = []
    for num in range(1, N_VARS + 1):
        name = f"sym_ng{num}"
        src = read(os.path.join(SRC, f"SymNetG{num}Func.java"))
        params = parse_params(src)
        mats = parse_transfs(src)
        stmts, c_expr, f_expr = parse_init(src)
        offset = parse_offset(src)
        b2 = emit_body(name, params, mats, stmts, c_expr, f_expr, offset, 2)
        b3 = emit_body(name, params, mats, stmts, c_expr, f_expr, offset, 3)
        defs.append(emit_def(num, name, params, b2, b3))
        consts.append(name.upper())
        print(f"{name}: {len(mats)} mats, {len(params)} params"
              + (", offset" if offset else ""))

    header = '''//! Network Symmetry Group variations sym_ng1..sym_ng17 (Jesus Sosa).
//!
//! GENERATED by scripts/gen_sym_ng.py from
//! `output/variation-jwf-source/SymNetG*Func.java`. Do not edit by hand —
//! re-run the generator instead.
//!
//! Each is a band/frieze symmetry base shape: pick one of N baked 2x2
//! symmetry affines at random, apply it (optionally after a fixed offset),
//! and accumulate. The matrices and the init() translation formulas are
//! transcribed from JWildfire's Java (its default/CPU renderer). Reference:
//! McGregor & Watt, "The Art of Graphics for the IBM PC", pp. 162-205.

use crate::variations::{
    definition::{Feature, VariationDef, VariationParamDef},
    ParamType, VariationCategory, VariationPhase,
};
use crate::param;

'''
    with open(OUT, "w", encoding="utf-8", newline="\n") as f:
        f.write(header)
        f.write("\n".join(defs))
    print(f"\nwrote {OUT}")
    print("\n// mod.rs registration:")
    print("\n".join(f"    &{c}," for c in consts))


if __name__ == "__main__":
    main()
