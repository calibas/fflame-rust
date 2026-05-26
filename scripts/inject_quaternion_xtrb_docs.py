"""Apply doc-comments + per-param descriptions to quaternion_misc.rs and xtrb_misc.rs.

One-off script for the variations-bulk-metadata project. Idempotent."""
import re
import textwrap

DOC = {
    "QUATERNION": (
        "quaternion_misc.rs",
        "Mega-variation combining 14 quaternion-extended scalar functions — 12 hyperbolic-extended trig pairs (`cosq`/`coshq`, `cotq`/`cothq`, `cscq`/`cschq`, `secq`/`sechq`, `sinq`/`sinhq`, `tanq`/`tanhq`) plus `estiq` (exp-times-trig) and `logq` (complex log). Each subfunction treats `x` as the scalar quaternion component and `|y, z|` as the imaginary magnitude, evaluates a quaternion-style identity, and accumulates into the output. Subfunctions are independently gated by their own `*pow` weight (zero = disabled) and tunable per axis via `x1, x2, y1, y2, z1, z2` mixers. Only `cosq` is active by default. At 92 user params, this is one of the most parameter-heavy variations in the registry — the port was unblocked by the packed-variation-params buffer change.",
        ["zephyrtronium", "Brad Stefanov"],
    ),
    "XTRB": (
        "xtrb_misc.rs",
        "TriBorders — triangular-grid dual tessellation in trilinear coordinates. Maps the input point to a triangular cell whose interior angles are set by `xtrb_a` and `xtrb_b` (the third follows from the angle-sum law), optionally hex-folds it through a band of width `xtrb_width`, then projects via a Julia-N style angular sampling (`xtrb_power` equally-spaced angles, radial exponent `xtrb_dist`) back to Cartesian. The body has six branches depending on the relative ordering of the three trilinear offsets, each with a hex-fold sub-branch gated by `width3 = 1 - w²`.",
        ["Xyrus02"],
    ),
}

# --- xtrb params (only 6) ---
PARAM_DOC = {
    ("XTRB", "xtrb_power"): "Julia-N branch count (integer). Each iteration picks one of `|power|` equally-spaced angles from the projected coordinate, scaling the polar angle by `1/power`.",
    ("XTRB", "xtrb_radius"): "Triangle inscribed radius — sets the overall scale of the triangular cells in the tessellation.",
    ("XTRB", "xtrb_width"): "Hex-fold band width within each cell. 0 = no hex fold (pure triangle dual); larger values push more of the cell area through the hex sub-branches before the Julia projection.",
    ("XTRB", "xtrb_dist"): "Radial exponent multiplier. Combines with `xtrb_power` as `dist / (2·power)` to set the radial blow-up power.",
    ("XTRB", "xtrb_a"): "Angle parameter A (radians). Internally `0.047 + a` becomes one of the triangle's interior angles; the third angle is derived from the angle-sum law `π - angle_b - angle_c`.",
    ("XTRB", "xtrb_b"): "Angle parameter B (radians). Internally `0.047 + b` becomes another interior angle.",
}

# --- quaternion params: generated programmatically ---
SUB_FUNCS = [
    ("cosq",  "cosine"),
    ("coshq", "hyperbolic cosine (cosh)"),
    ("cotq",  "cotangent"),
    ("cothq", "hyperbolic cotangent (coth)"),
    ("cscq",  "cosecant (csc)"),
    ("cschq", "hyperbolic cosecant (csch)"),
    ("secq",  "secant"),
    ("sechq", "hyperbolic secant (sech)"),
    ("sinq",  "sine"),
    ("sinhq", "hyperbolic sine (sinh)"),
    ("tanq",  "tangent"),
    ("tanhq", "hyperbolic tangent (tanh)"),
]

for name, human in SUB_FUNCS:
    PARAM_DOC[("QUATERNION", f"{name}pow")] = f"{name}: subfunction weight (quaternion-extended {human}). 0 disables this branch entirely; non-zero activates and scales its contribution to the accumulated output."
    PARAM_DOC[("QUATERNION", f"{name}x1")] = f"{name}: scale applied to `x` before the first trig/hyperbolic term."
    PARAM_DOC[("QUATERNION", f"{name}x2")] = f"{name}: scale applied to `x` before the second trig/hyperbolic term."
    PARAM_DOC[("QUATERNION", f"{name}y1")] = f"{name}: scale applied to the `|y, z|` magnitude before the first hyperbolic/trig term."
    PARAM_DOC[("QUATERNION", f"{name}y2")] = f"{name}: scale applied to the `|y, z|` magnitude before the second hyperbolic/trig term."
    PARAM_DOC[("QUATERNION", f"{name}z1")] = f"{name}: pre-multiplier on the `|y, z|` magnitude itself (compounds with y1/y2 axis scalings)."
    PARAM_DOC[("QUATERNION", f"{name}z2")] = f"{name}: scale applied to the Y/Z cross-term that builds the off-axis output."

# estiq (exp-times-trig): 6 params (no x2)
PARAM_DOC[("QUATERNION", "estiqpow")] = "estiq: subfunction weight (exp-times-trig: `e^x · (cos, sin)`). 0 disables; non-zero activates and scales the contribution."
PARAM_DOC[("QUATERNION", "estiqx1")] = "estiq: scale applied to `x` before the exponential `e^(x · x1)`."
PARAM_DOC[("QUATERNION", "estiqy1")] = "estiq: scale applied to the `|y, z|` magnitude before the sine term."
PARAM_DOC[("QUATERNION", "estiqy2")] = "estiq: scale applied to the `|y, z|` magnitude before the cosine term."
PARAM_DOC[("QUATERNION", "estiqz1")] = "estiq: pre-multiplier on the `|y, z|` magnitude."
PARAM_DOC[("QUATERNION", "estiqz2")] = "estiq: scale applied to the Y/Z cross-term."

# logq (complex log): 2 params
PARAM_DOC[("QUATERNION", "logqpow")] = "logq: subfunction weight (complex logarithm: `(½·log|z|², atan2(|y,z|, x))`). 0 disables; non-zero activates and scales the contribution. X uses the pre-computed `_denom = 0.5 / ln(base)`."
PARAM_DOC[("QUATERNION", "logqbase")] = "logq: logarithm base. Pre-computed at init into `_denom = 0.5 / ln(base)`. Default `e` (~2.718) gives the natural log; 10 gives common log; 2 gives binary log."


def find_macro_close(text: str, open_paren_idx: int) -> int:
    assert text[open_paren_idx] == "("
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


files = {}
for static_name, (path_rel, body, authors) in DOC.items():
    files.setdefault(path_rel, {})[static_name] = (body, authors)

for path_rel, doc_map in files.items():
    full_path = f"src/variations/defs/{path_rel}"
    with open(full_path, "rb") as f:
        src = f.read().decode("utf-8")

    inserted = 0
    already = 0
    for name, (body, authors) in doc_map.items():
        target = f"\npub static {name}: VariationDef = VariationDef {{"
        if target not in src:
            print(f"  WARN: no match for {name} in {path_rel}")
            continue
        idx = src.find(target)
        prefix = src[:idx]
        last_nl = prefix.rfind("\n")
        if last_nl != -1:
            prev_line = src[last_nl + 1:idx + 1].strip()
            if prev_line.startswith("///"):
                already += 1
                continue
        lines = []
        for paragraph in body.split("\n"):
            wrapped = textwrap.fill(paragraph, width=72) if paragraph.strip() else ""
            for line in wrapped.split("\n"):
                lines.append(f"/// {line}".rstrip())
        if authors:
            lines.append("///")
            lines.append("/// # Authors")
            for author in authors:
                lines.append(f"/// - {author}")
        doc = "\n".join(lines)
        src = src.replace(target, f"\n{doc}\npub static {name}: VariationDef = VariationDef {{", 1)
        inserted += 1
    print(f"  {path_rel} pass1: inserted {inserted}, already {already}")

    pinserted = 0
    palready = 0
    for (static_name, param_name), desc in PARAM_DOC.items():
        if static_name not in doc_map:
            continue
        start_pattern = f"pub static {static_name}: VariationDef"
        start_idx = src.find(start_pattern)
        if start_idx == -1:
            continue
        next_static = src.find("\npub static ", start_idx + 1)
        end_idx = next_static if next_static != -1 else len(src)
        block = src[start_idx:end_idx]

        head = f'param!("{param_name}"'
        head_idx = block.find(head)
        if head_idx == -1:
            print(f"  WARN: no param {static_name}.{param_name}")
            continue
        open_idx = block.find("(", head_idx)
        close_idx = find_macro_close(block, open_idx)
        if close_idx == -1:
            print(f"  WARN: unbalanced param {static_name}.{param_name}")
            continue
        inner = block[open_idx + 1:close_idx]
        depth = 0
        in_str = False
        commas = 0
        j = 0
        while j < len(inner):
            ch = inner[j]
            if in_str:
                if ch == "\\":
                    j += 2
                    continue
                if ch == '"':
                    in_str = False
            else:
                if ch == '"':
                    in_str = True
                elif ch == "(":
                    depth += 1
                elif ch == ")":
                    depth -= 1
                elif ch == "," and depth == 0:
                    commas += 1
            j += 1
        if commas >= 6:
            palready += 1
            continue

        new_block = (
            block[:close_idx]
            + f', "{desc}"'
            + block[close_idx:]
        )
        src = src[:start_idx] + new_block + src[end_idx:]
        pinserted += 1
    print(f"  {path_rel} pass2: injected {pinserted}, already {palready}")

    with open(full_path, "wb") as f:
        f.write(src.encode("utf-8"))
