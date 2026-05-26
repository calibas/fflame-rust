"""Convert Integer [0, 1] params to Boolean across the variation corpus.

One-off script for the variations-param-type-cleanup branch.

Per-static-name list of param names to convert from `int` to `bool`.
The script reads each param!() macro, extracts the existing description
(if present), and rewrites the macro using the `bool` arm.

Old form: param!("name", "Display", int, 0.0, 0.0, 1.0, "desc")
New form: param!("name", "Display", bool, false, "desc")

Default-value mapping: 0.0 → false, 1.0 → true.
Idempotent — checks for existing `bool` type and skips.
"""
from __future__ import annotations
import re
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent
DEFS_DIR = REPO_ROOT / "src" / "variations" / "defs"

# (file, static_name, param_name) tuples — int [0, 1] → bool.
CONVERSIONS = [
    ("standalone_exotics.rs", "HOLE2", "inside"),
    ("parametric_curves.rs", "CROP3D", "zero"),
    ("misc_extras.rs", "CHUNK", "mode"),
    ("misc_extras.rs", "PTRANSFORM", "use_log"),
    ("misc_extras.rs", "TILE_REVERSE", "vertical"),
    ("singleton_misc.rs", "CORNERS", "logmode"),
    ("stub_recoveries2.rs", "TQMIRROR", "type"),
    ("apo_misc13.rs", "RIPPLE", "fixed_dist_calc"),
    ("rosoni_misc.rs", "ROSONI", "altshapes"),
    ("circlecrop_misc.rs", "CIRCLECROP", "zero"),
    ("circlecrop_phase.rs", "PRE_CIRCLECROP", "zero"),
    ("circlecrop_phase.rs", "POST_CIRCLECROP", "zero"),
    ("apo_misc18.rs", "SPHERECROP", "zero"),
    ("apo_misc21.rs", "POST_MIRROR_WF", "xaxis"),
    ("apo_misc21.rs", "POST_MIRROR_WF", "yaxis"),
    ("apo_misc21.rs", "POST_MIRROR_WF", "zaxis"),
    ("apo_misc20.rs", "CANNABISCURVE_WF", "filled"),
    ("wf_curves.rs", "CLOVERLEAF_WF", "filled"),
    ("wf_curves.rs", "ROSE_WF", "filled"),
    ("waves_wf_family.rs", "WAVES2_WF", "use_cos_x"),
    ("waves_wf_family.rs", "WAVES2_WF", "use_cos_y"),
    ("waves_wf_family.rs", "WAVES3_WF", "use_cos_x"),
    ("waves_wf_family.rs", "WAVES3_WF", "use_cos_y"),
    ("waves_wf_family.rs", "WAVES4_WF", "use_cos_x"),
    ("waves_wf_family.rs", "WAVES4_WF", "use_cos_y"),
    ("dc_misc.rs", "DC_TRIANGLE", "zero_edges"),
    ("truchet_misc.rs", "TRUCHET", "extended"),
    ("truchet_misc.rs", "TRUCHET", "direct_color"),
    ("truchet2_misc.rs", "TRUCHET2", "inverse"),
    ("waveblur_misc.rs", "WAVEBLUR_WF", "direct_color"),
    ("butterfly_fay_misc.rs", "BUTTERFLY_FAY", "unified_inner_outer"),
    ("bubblet3d_misc.rs", "BUBBLE_T3D", "symmetry_z"),
    ("bubblet3d_misc.rs", "BUBBLE_T3D", "modus_blur"),
    ("supershape3d_misc.rs", "SUPERSHAPE_3D", "toroidmap"),
    ("klein_group_misc.rs", "KLEIN_GROUP", "avoid_reversal"),
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


def convert_one(src: str, static_name: str, param_name: str) -> tuple[str, str]:
    """Returns (new_src, status). Status one of: 'converted', 'already-bool', 'skipped-not-int', 'not-found'."""
    static_pat = f"pub static {static_name}: VariationDef"
    static_idx = src.find(static_pat)
    if static_idx == -1:
        return src, f"not-found: {static_name}"
    next_static = src.find("\npub static ", static_idx + 1)
    end_idx = next_static if next_static != -1 else len(src)

    # Find the param!("name", ...) macro start within this static block
    block = src[static_idx:end_idx]
    head = f'param!("{param_name}"'
    head_idx = block.find(head)
    if head_idx == -1:
        return src, f"not-found: {static_name}.{param_name}"

    open_idx = block.find("(", head_idx)
    close_idx = find_macro_close(block, open_idx)
    if close_idx == -1:
        return src, f"unbalanced-paren: {static_name}.{param_name}"

    inner = block[open_idx + 1:close_idx]
    args = split_top_level(inner)
    if len(args) < 4:
        return src, f"unexpected-arg-count {len(args)}: {static_name}.{param_name}"

    # args: [name, display, type, default, min, max, [desc]] for int
    # or:   [name, display, bool, default, [desc]] for already-converted
    type_token = args[2]
    if type_token == "bool":
        return src, f"already-bool: {static_name}.{param_name}"
    if type_token not in ("int",):
        return src, f"skipped-not-int (type={type_token}): {static_name}.{param_name}"

    name_arg = args[0]
    display_arg = args[1]
    default_arg = args[3]
    desc_arg = args[6] if len(args) >= 7 else None

    # Map default 0.0 → false, 1.0 → true (and tolerate "0", "1")
    default_clean = default_arg.replace(" ", "")
    if default_clean in ("0", "0.0", "0.", "0.0f32"):
        new_default = "false"
    elif default_clean in ("1", "1.0", "1.", "1.0f32"):
        new_default = "true"
    else:
        return src, f"unexpected-default {default_arg}: {static_name}.{param_name}"

    # Build new macro
    if desc_arg is not None:
        new_call = f"param!({name_arg}, {display_arg}, bool, {new_default}, {desc_arg})"
    else:
        new_call = f"param!({name_arg}, {display_arg}, bool, {new_default})"

    old_call = block[head_idx:close_idx + 1]
    new_block = block[:head_idx] + new_call + block[close_idx + 1:]
    new_src = src[:static_idx] + new_block + src[end_idx:]
    return new_src, f"converted: {static_name}.{param_name}"


def main() -> int:
    # Group by file
    by_file: dict[str, list[tuple[str, str]]] = {}
    for f, s, p in CONVERSIONS:
        by_file.setdefault(f, []).append((s, p))

    total_converted = 0
    total_already = 0
    for filename, conversions in by_file.items():
        path = DEFS_DIR / filename
        src = path.read_text(encoding="utf-8")
        for static_name, param_name in conversions:
            src, status = convert_one(src, static_name, param_name)
            print(f"  {status}")
            if status.startswith("converted:"):
                total_converted += 1
            elif status.startswith("already-bool:"):
                total_already += 1
        path.write_text(src, encoding="utf-8")
    print()
    print(f"Converted {total_converted}, already-bool {total_already}, total candidates {len(CONVERSIONS)}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
