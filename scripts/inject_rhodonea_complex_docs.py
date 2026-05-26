"""Apply doc-comments + per-param descriptions to rhodonea_misc.rs and complex_misc.rs.

One-off script for the variations-bulk-metadata project. Idempotent."""
import re
import textwrap

DOC = {
    "RHODONEA": (
        "rhodonea_misc.rs",
        "Rose curves (rhodonea) — `r = cos(k·θ) + radial_offset` with `k = knumer/kdenom`. Maps incoming points onto the rose with 7 inner modes (when `|rin| ≤ |r|`) and 7 outer modes (when `|rin| > |r|`), giving 49 spread/mask combinations. The init logic computes the minimum cycle count needed to close the curve, handling integer `k` (1 or ½ cycle depending on parity), rational `k` (Euclidean gcd reduction), and irrational `k` (16-cycle fallback). `metacycle_expansion` distorts amplitude across cycles for spiral-like radial growth; `fill` adds uniform random radial jitter.",
        ["CozyG"],
    ),
    "COMPLEX": (
        "complex_misc.rs",
        "2D analog of `quaternion` — combines 13 complex-plane trig/hyperbolic/exp sub-functions (`cos, cosh, cot, coth, csc, csch, exp, sec, sech, sin, sinh, tan, tanh`), each treating the input `(x, y)` as a complex number `z = x + iy` and evaluating its complex-function image. Subfunctions are independently gated by their own `*pow` weight (zero = disabled) and per-axis-tunable via `x1, x2, y1, y2` mixers. 12 subs have 5 params each; `exp` has 4 (no x2 since `e^(x·x1) · (cos, sin)` is single-axis). Default has only `cos` active. 2D-only — no 3D body.",
        ["cothe", "Brad Stefanov"],
    ),
}

# Rhodonea (15 params)
PARAM_DOC = {
    ("RHODONEA", "knumer"): "Rose-curve `k = knumer/kdenom` numerator. Integer `k` produces `k` petals (even) or `2k` petals (odd); rational `k` produces nested petal patterns with `cycles_to_close = kdenom` cycles.",
    ("RHODONEA", "kdenom"): "Rose-curve `k = knumer/kdenom` denominator.",
    ("RHODONEA", "radial_offset"): "Additive offset to the radial formula: `r = cos(k·θ) + radial_offset`. Non-zero values create an inner loop or distort the petals away from the origin.",
    ("RHODONEA", "inner_mode"): "Behavior when input is inside the rose curve (`|rin| ≤ |r|`). 0=normal (point on curve), 1=spread linear, 2=spread radial-mirror, 3=spread axis-abs, 4=spread half-add, 5=mask inside (hide → collapses to origin), 6=mask outside (pass-through original input).",
    ("RHODONEA", "outer_mode"): "Behavior when input is outside the rose curve (`|rin| > |r|`). 0-4 use the same spread formulas as `inner_mode` (with `outer_spread` instead of `inner_spread`); 5=mask inside (pass-through original input), 6=mask outside (hide → collapses to origin). Note the mask semantics on 5/6 are inverted relative to `inner_mode`.",
    ("RHODONEA", "inner_spread"): "Spread amplitude for inner modes 1-4. Scales how strongly the input position influences the displacement from the rose curve.",
    ("RHODONEA", "outer_spread"): "Spread amplitude for outer modes 1-4.",
    ("RHODONEA", "inner_spread_ratio"): "X/Y asymmetry for the inner spread. 1.0 = symmetric; other values stretch the spread along X relative to Y.",
    ("RHODONEA", "outer_spread_ratio"): "X/Y asymmetry for the outer spread.",
    ("RHODONEA", "spread_split"): "Scale factor for the input-radius comparison: `rin = spread_split · |p|`. Larger values make the curve appear smaller (more inputs classify as outside).",
    ("RHODONEA", "cycles"): "Number of θ cycles to traverse per iteration. 0 = auto-compute as `cycles_to_close × metacycles`; non-zero overrides the auto-compute.",
    ("RHODONEA", "cycle_offset"): "Phase offset for θ in fractions of 2π. Rotates the rose orientation.",
    ("RHODONEA", "metacycle_expansion"): "Per-metacycle radial expansion factor. Non-zero values cause the rose amplitude to grow linearly across subsequent metacycles, creating spiral patterns.",
    ("RHODONEA", "metacycles"): "Number of metacycles when `cycles == 0`. Each metacycle is one `cycles_to_close` worth of θ.",
    ("RHODONEA", "fill"): "Random radial jitter amplitude. Adds `fill · (rng - 0.5)` to `r` per iteration, filling in the rose shape with noise.",
}

# Complex: 13 sub-functions. 12 with 5 params (pow, x1, x2, y1, y2); exp with 4 (pow, x1, y1, y2).
COMPLEX_FULL_SUBS = [
    ("cos",  "cosine"),
    ("cosh", "hyperbolic cosine (cosh)"),
    ("cot",  "cotangent"),
    ("coth", "hyperbolic cotangent (coth)"),
    ("csc",  "cosecant (csc)"),
    ("csch", "hyperbolic cosecant (csch)"),
    ("sec",  "secant"),
    ("sech", "hyperbolic secant (sech)"),
    ("sin",  "sine"),
    ("sinh", "hyperbolic sine (sinh)"),
    ("tan",  "tangent"),
    ("tanh", "hyperbolic tangent (tanh)"),
]

for name, human in COMPLEX_FULL_SUBS:
    PARAM_DOC[("COMPLEX", f"{name}pow")] = f"{name}: subfunction weight (complex {human} of `x + iy`). 0 disables this branch entirely; non-zero activates and scales its contribution to the accumulated output."
    PARAM_DOC[("COMPLEX", f"{name}x1")] = f"{name}: scale applied to `x` before the first trig/hyperbolic term."
    PARAM_DOC[("COMPLEX", f"{name}x2")] = f"{name}: scale applied to `x` before the second trig/hyperbolic term."
    PARAM_DOC[("COMPLEX", f"{name}y1")] = f"{name}: scale applied to `y` before the first trig/hyperbolic term."
    PARAM_DOC[("COMPLEX", f"{name}y2")] = f"{name}: scale applied to `y` before the second trig/hyperbolic term."

# exp (4 params: pow, x1, y1, y2)
PARAM_DOC[("COMPLEX", "exppow")] = "exp: subfunction weight (complex exponential `e^z = e^x · (cos y, sin y)`). 0 disables; non-zero activates and scales the contribution."
PARAM_DOC[("COMPLEX", "expx1")] = "exp: scale applied to `x` before the exponential `e^(x · x1)`."
PARAM_DOC[("COMPLEX", "expy1")] = "exp: scale applied to `y` before the cosine term."
PARAM_DOC[("COMPLEX", "expy2")] = "exp: scale applied to `y` before the sine term."


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
