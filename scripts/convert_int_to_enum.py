"""Convert Integer [0, N-1] params to Enum across the variation corpus.

One-off script for the variations-param-type-cleanup branch.

Note: the falloff2 family (FALLOFF2.type, PRE_FALLOFF2.type,
POST_FALLOFF2.type) is intentionally not in CONVERSIONS — those three
were declared with raw `VariationParamDef { ... }` struct literals
rather than the param!() macro, and were hand-edited directly.

Per-static-name list of (param, choices) tuples. The script reads each
param!() macro, extracts the existing description (if present), and
rewrites the macro using the `enum` arm.

Old form: param!("name", "Display", int, 0.0, 0.0, 2.0, "desc")
New form: param!("name", "Display", enum, 0, &["Mode A", "Mode B", "Mode C"], "desc")

Default-value mapping: old `int` default rounded to nearest integer
becomes the new `enum` default index. Idempotent — checks for existing
`enum` type and skips.

Only includes enums with unambiguous semantic labels. Deferred:
- butterfly_fay.outer_mode/inner_mode (same spread-formula family as
  rhodonea; do both together for consistent labels)
- rhodonea.inner_mode/outer_mode (asymmetric mask semantics on 5/6)
- iconattractor_js.preset_id (17 modes, no obvious labels)
- jac_asn.jac_asn_type (two orthogonal toggles, needs design decision)
- subflame_wf.color_mode (-1 baseline + partial implementation)
"""
from __future__ import annotations
import re
import sys
from pathlib import Path

# Force UTF-8 output so unicode in choice labels (e.g. "XY → Z") prints
# without UnicodeEncodeError on Windows consoles defaulting to cp1252.
try:
    sys.stdout.reconfigure(encoding="utf-8")
except AttributeError:
    pass

REPO_ROOT = Path(__file__).resolve().parent.parent
DEFS_DIR = REPO_ROOT / "src" / "variations" / "defs"

# (file, static_name, param_name, choices_list)
# Default value is preserved from the existing macro.
CONVERSIONS = [
    # atan axis selector (3 modes — Y only / X only / both)
    ("singleton_misc.rs", "ATAN_VAR", "mode", ["Y Only", "X Only", "Both"]),

    # post_axis_symmetry_wf axis (3 modes — X / Y / Z)
    ("post_axis_symmetry_misc.rs", "POST_AXIS_SYMMETRY_WF", "axis", ["X", "Y", "Z"]),

    # pre_wave3D_wf displacement axis (4 modes)
    ("pre_wave3d_misc.rs", "PRE_WAVE3D_WF", "axis",
     ["XY → Z", "YZ → X", "ZX → Y", "Radial"]),

    # mobius_strip out-of-range behavior (4 modes — wrap/clamp/hide/leave)
    ("apo_misc19.rs", "MOBIUS_STRIP", "width_mode",
     ["Wrap", "Clamp", "Hide", "Leave"]),
    ("apo_misc19.rs", "MOBIUS_STRIP", "radial_mode",
     ["Wrap", "Clamp", "Hide", "Leave"]),

    # spirograph3D width-jitter pattern (5 modes)
    ("apo_misc17.rs", "SPIROGRAPH3D", "mode",
     ["Uniform", "Phased Sin", "Independent", "Gaussian", "X Only"]),

    # klein_group Kleinian generator recipe (7 modes)
    ("klein_group_misc.rs", "KLEIN_GROUP", "recipe",
     ["Grandma", "Maskit Mu", "Jorgensen", "Riley", "Riley+", "Maskit+", "Maskit-Leys"]),

    # hole2 radial-formula shape selector (10 modes; labels derived from the
    # WGSL formula structure — Linear is the only one with the delta added
    # outside the sqrt; others wrap the delta inside it).
    ("standalone_exotics.rs", "HOLE2", "shape",
     ["Linear", "Constant", "Petals", "Dipolar", "Quadratic", "Spiked",
      "Cardioid", "Half-Petals", "Nested Sin", "Harmonic"]),

    # butterfly_fay outer_mode = inner_mode (6 modes; shared spread-formula
    # family with rhodonea modes 0-4, plus a "Linear Input" mode 5 unique
    # to butterfly_fay).
    ("butterfly_fay_misc.rs", "BUTTERFLY_FAY", "outer_mode",
     ["On Curve", "Radial Stretch", "Mirror Blend", "Mirror Add",
      "Half + Offset", "Linear Input"]),
    ("butterfly_fay_misc.rs", "BUTTERFLY_FAY", "inner_mode",
     ["On Curve", "Radial Stretch", "Mirror Blend", "Mirror Add",
      "Half + Offset", "Linear Input"]),

    # rhodonea outer_mode (7 modes; modes 0-4 shared with butterfly_fay,
    # mode 5 = pass-through, mode 6 = hide).
    ("rhodonea_misc.rs", "RHODONEA", "outer_mode",
     ["On Curve", "Radial Stretch", "Mirror Blend", "Mirror Add",
      "Half + Offset", "Pass Through", "Hide"]),

    # rhodonea inner_mode (7 modes; mode 5/6 swapped vs outer_mode because
    # the underlying mask semantics are inverted on the inner side).
    ("rhodonea_misc.rs", "RHODONEA", "inner_mode",
     ["On Curve", "Radial Stretch", "Mirror Blend", "Mirror Add",
      "Half + Offset", "Hide", "Pass Through"]),

    # jac_asn function-kind × swap-modulus selector (8 modes: 4 inverse
    # Jacobi functions × 2 swap modes). Flat 8-variant enum to preserve
    # wire-format compatibility — could split into two separate params
    # (4-enum + bool) but that requires a legacy_import shim.
    ("jac_asn_misc.rs", "JAC_ASN", "jac_asn_type",
     ["Inverse DN", "Inverse SN", "Inverse CN", "Inverse SC",
      "Inverse DN (swap)", "Inverse SN (swap)", "Inverse CN (swap)",
      "Inverse SC (swap)"]),
]


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


def split_top_level(inner: str) -> list[str]:
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


def format_choices(choices: list[str]) -> str:
    quoted = ", ".join(f'"{c}"' for c in choices)
    return f"&[{quoted}]"


def convert_one(src: str, static_name: str, param_name: str, choices: list[str]) -> tuple[str, str]:
    """Returns (new_src, status)."""
    static_pat = f"pub static {static_name}: VariationDef"
    static_idx = src.find(static_pat)
    if static_idx == -1:
        return src, f"not-found static: {static_name}"
    next_static = src.find("\npub static ", static_idx + 1)
    end_idx = next_static if next_static != -1 else len(src)

    block = src[static_idx:end_idx]
    head = f'param!("{param_name}"'
    head_idx = block.find(head)
    if head_idx == -1:
        return src, f"not-found param: {static_name}.{param_name}"

    open_idx = block.find("(", head_idx)
    close_idx = find_macro_close(block, open_idx)
    if close_idx == -1:
        return src, f"unbalanced-paren: {static_name}.{param_name}"

    inner = block[open_idx + 1:close_idx]
    args = split_top_level(inner)
    if len(args) < 4:
        return src, f"unexpected-arg-count {len(args)}: {static_name}.{param_name}"

    type_token = args[2]
    if type_token == "enum":
        return src, f"already-enum: {static_name}.{param_name}"
    if type_token != "int":
        return src, f"skipped-not-int (type={type_token}): {static_name}.{param_name}"

    name_arg = args[0]
    display_arg = args[1]
    default_arg = args[3]
    desc_arg = args[6] if len(args) >= 7 else None

    # Parse default and clamp to a valid enum index
    try:
        default_f = float(default_arg.rstrip("f32").rstrip("f"))
    except ValueError:
        return src, f"unparseable-default {default_arg!r}: {static_name}.{param_name}"
    default_i = int(round(default_f))
    if default_i < 0 or default_i >= len(choices):
        return src, f"default-out-of-range {default_i} (choices={len(choices)}): {static_name}.{param_name}"

    choices_str = format_choices(choices)
    if desc_arg is not None:
        new_call = f"param!({name_arg}, {display_arg}, enum, {default_i}, {choices_str}, {desc_arg})"
    else:
        new_call = f"param!({name_arg}, {display_arg}, enum, {default_i}, {choices_str})"

    new_block = block[:head_idx] + new_call + block[close_idx + 1:]
    new_src = src[:static_idx] + new_block + src[end_idx:]
    return new_src, f"converted: {static_name}.{param_name} → {default_i}/{len(choices)}"


def main() -> int:
    by_file: dict[str, list[tuple[str, str, list[str]]]] = {}
    for f, s, p, choices in CONVERSIONS:
        by_file.setdefault(f, []).append((s, p, choices))

    total_converted = 0
    total_already = 0
    for filename, conversions in by_file.items():
        path = DEFS_DIR / filename
        src = path.read_text(encoding="utf-8")
        for static_name, param_name, choices in conversions:
            src, status = convert_one(src, static_name, param_name, choices)
            print(f"  {status}")
            if status.startswith("converted:"):
                total_converted += 1
            elif status.startswith("already-enum:"):
                total_already += 1
        path.write_text(src, encoding="utf-8")
    print()
    print(f"Converted {total_converted}, already-enum {total_already}, total candidates {len(CONVERSIONS)}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
