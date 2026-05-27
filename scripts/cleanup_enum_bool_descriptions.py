"""Clean up param descriptions on the converted Booleans and Enums.

Now that the dropdown labels (Enum) and checkbox state (Boolean) carry
the mode meaning in the UI, the "0 = X, 1 = Y, 2 = Z" enumeration in
the descriptions is redundant. Replace with prose that explains what
the param does without re-listing the modes.

Each entry is (filename, old_description, new_description, count) where
count is the number of identical instances expected. Idempotent — if
old isn't found, prints a WARN and continues; if new is already in
place, the count is 0 (also fine).
"""
from __future__ import annotations
import sys
from pathlib import Path

try:
    sys.stdout.reconfigure(encoding="utf-8")
except AttributeError:
    pass

REPO_ROOT = Path(__file__).resolve().parent.parent
DEFS_DIR = REPO_ROOT / "src" / "variations" / "defs"

# Each entry: (file, old, new, expected_count).
# Most are unique (count=1); waves_wf_family has 3 X-axis + 3 Y-axis
# instances that share descriptions.
REPLACEMENTS: list[tuple[str, str, str, int]] = [
    # ---- Enums ----
    ("singleton_misc.rs",
     '"Which axes get arctangent-transformed: 0 = Y only, 1 = X only, 2 = both."',
     '"Which axes get the arctangent transform."', 1),

    ("post_axis_symmetry_misc.rs",
     '"Axis to mirror across: 0 = X, 1 = Y, 2 = Z (3D only)."',
     '"Axis of symmetry to mirror across. Z is 3D-only."', 1),

    ("pre_wave3d_misc.rs",
     '"Wave plane / displacement axis: 0 = XY plane (displaces Z), 1 = YZ plane (displaces X), 2 = ZX plane (displaces Y), 3 = RADIAL (displaces along the unit vector from center)."',
     '"Wave plane and displacement axis. Plane modes wave one coordinate in and out of the other two; Radial waves along the unit vector from the origin."', 1),

    ("apo_misc19.rs",
     '"Y out-of-range behavior: 0 = wrap, 1 = clamp, 2 = hide, 3 = leave (pass through)."',
     '"Y out-of-range behavior. Leave passes the original point through unchanged."', 1),

    ("apo_misc19.rs",
     '"X out-of-range behavior: same 4-mode enum as `width_mode`."',
     '"X out-of-range behavior."', 1),

    ("apo_misc17.rs",
     '"Jitter pattern: 0 = single uniform on all axes, 1 = phased-sin per axis, 2 = independent uniform per axis, 3 = central-limit Gaussian per axis, 4 = ±width on X only."',
     '"Random jitter pattern applied to the width offset."', 1),

    ("klein_group_misc.rs",
     '"Generator-pair construction recipe (0-6): 0 = GRANDMA_STANDARD (parabolic commutator from *Indra\'s Pearls* Ch. 6), 1 = MASKIT_MU (single complex μ — the Maskit slice), 2 = JORGENSEN (Troels Jørgensen\'s trace-based parameterization), 3 = RILEY (Robert Riley\'s single complex c), 4 = RILEY_MODIFIED (Riley with extra b1), 5 = MASKIT_MU_MODIFIED (Maskit with extra b1), 6 = MASKIT_LEYS_MODIFIED (Jos Leys\' n-fold variant)."',
     '"Generator-pair construction recipe. Grandma is the parabolic commutator from *Indra\'s Pearls* Ch. 6. Maskit Mu is the Maskit slice (single complex μ). Jorgensen uses Troels Jørgensen\'s trace-based parameterization. Riley uses Robert Riley\'s single complex c. The `+` variants add an extra `b1`. Maskit-Leys is Jos Leys\' n-fold variant."', 1),

    ("standalone_exotics.rs",
     '"Picks one of 10 radial formulas (0-9)."',
     '"Radial-formula family. Each option applies a different geometric term inside the `sqrt(rhosq + …)` that determines the output radius."', 1),

    # butterfly_fay outer_mode + inner_mode (same shared family)
    ("butterfly_fay_misc.rs",
     '"Output mode for points outside the curve. 0-5; same enum as `inner_mode`."',
     '"Output mode for points outside the butterfly curve."', 1),
    ("butterfly_fay_misc.rs",
     '"Output mode for points inside the curve. 0-5; same enum as `outer_mode`."',
     '"Output mode for points inside the butterfly curve."', 1),

    # rhodonea inner_mode + outer_mode (shared family + asymmetric masks)
    ("rhodonea_misc.rs",
     '"Behavior when input is inside the rose curve (`|rin| ≤ |r|`). 0=normal (point on curve), 1=spread linear, 2=spread radial-mirror, 3=spread axis-abs, 4=spread half-add, 5=mask inside (hide → collapses to origin), 6=mask outside (pass-through original input)."',
     '"Behavior for points inside the rose curve (`|rin| ≤ |r|`)."', 1),
    ("rhodonea_misc.rs",
     '"Behavior when input is outside the rose curve (`|rin| > |r|`). 0-4 use the same spread formulas as `inner_mode` (with `outer_spread` instead of `inner_spread`); 5=mask inside (pass-through original input), 6=mask outside (hide → collapses to origin). Note the mask semantics on 5/6 are inverted relative to `inner_mode`."',
     '"Behavior for points outside the rose curve (`|rin| > |r|`). Modes 0-4 mirror the spread formulas of `inner_mode` (using `outer_spread` instead). The mask modes (Pass Through / Hide) have the opposite ordering from `inner_mode` — each label describes what happens at *this* side, so the underlying inversion is invisible to the user."', 1),

    # jac_asn.jac_asn_type — kept flat 8-variant enum to preserve wire format
    ("jac_asn_misc.rs",
     '"Which inverse to compute (0-7). 0/4 = invdn (elliptic F), 1/5 = inverse sn, 2/6 = inverse cn, 3/7 = inverse sc. Types > 3 swap the modulus and phi after the initial branch."',
     '"Which inverse Jacobi function to compute. The `(swap)` variants additionally swap the modulus and phi after the initial branch — same function-kind axis, different parameter roles."', 1),

    # ---- Booleans ----
    ("apo_misc13.rs",
     '"Distance formula selector: 0 = product `sqrt(x²·y²)`, 1 = Euclidean `sqrt(x² + y²)`."',
     '"When on, use Euclidean distance `sqrt(x² + y²)`. When off, the upstream-quirk product `sqrt(x²·y²)`."', 1),

    ("apo_misc18.rs",
     '"Behavior outside the sphere: 1 = hide (collapse to origin), 0 = scatter onto surface."',
     '"When on, points outside the sphere collapse to the origin. When off, they scatter onto the surface."', 1),

    ("apo_misc20.rs",
     '"1 = fill the curve interior by randomizing the radius per iteration; 0 = trace only the curve outline."',
     '"When on, fill the curve interior by randomizing the radius per iteration. When off, trace only the outline."', 1),

    ("apo_misc21.rs",
     '"1 = enable X-axis mirror branch; 0 = disable."',
     '"Enable the X-axis mirror branch."', 1),
    ("apo_misc21.rs",
     '"1 = enable Y-axis mirror branch; 0 = disable."',
     '"Enable the Y-axis mirror branch."', 1),
    ("apo_misc21.rs",
     '"1 = enable Z-axis mirror branch (3D only); 0 = disable."',
     '"Enable the Z-axis mirror branch. 3D only."', 1),

    ("bubblet3d_misc.rs",
     '"When 1, mirror the Z hole symmetrically across the XY plane (clamps `|angle_of_hole|` to < 180°). When 0, apply the hole only to one Z-pole."',
     '"When on, mirror the Z hole symmetrically across the XY plane (clamps `|angle_of_hole|` to < 180°). When off, apply the hole to only one Z-pole."', 1),

    ("bubblet3d_misc.rs",
     '"0 = hard cutout (zero out points inside stripes/hole). 1 = smooth blend via angular rotation (only effective for the Z hole when `exponent_z == 1`; otherwise falls back to hard cutout)."',
     '"When on, smooth-blend the stripe/hole interior via angular rotation. When off, hard-cutout (zero out). Only effective on the Z hole when `exponent_z == 1`; otherwise falls back to hard cutout regardless."', 1),

    ("butterfly_fay_misc.rs",
     '"1 = always use the outer mode/spread/ratio; 0 = pick based on whether the input is inside or outside the curve."',
     '"When on, always use the outer mode/spread/ratio. When off, pick based on whether the input is inside or outside the curve."', 1),

    # CIRCLECROP, PRE_CIRCLECROP, POST_CIRCLECROP all share the same desc.
    ("circlecrop_misc.rs",
     '"Behavior outside the circle: 1 = hide (collapse to origin), 0 = scatter onto boundary."',
     '"When on, points outside the circle collapse to the origin. When off, they scatter onto the boundary."', 1),
    ("circlecrop_phase.rs",
     '"Behavior outside the circle: 1 = hide (collapse to origin), 0 = scatter onto boundary."',
     '"When on, points outside the circle collapse to the origin. When off, they scatter onto the boundary."', 2),

    ("dc_misc.rs",
     '"1 = collapse out-of-triangle points to origin; 0 = scatter them by `scatter_area`."',
     '"When on, out-of-triangle points collapse to the origin. When off, they scatter by `scatter_area`."', 1),

    ("klein_group_misc.rs",
     '"When 1, the next-matrix pick excludes the inverse of the previous matrix (no `aA`, `Aa`, `bB`, or `Bb` cancellations), so the trajectory drifts through the limit set instead of bouncing back-and-forth on a small subset. When 0, all four matrices are equally likely each iteration."',
     '"When on, the next-matrix pick excludes the inverse of the previous matrix (no `aA`, `Aa`, `bB`, or `Bb` cancellations), so the trajectory drifts through the limit set instead of bouncing back-and-forth on a small subset. When off, all four matrices are equally likely each iteration."', 1),

    ("misc_extras.rs",
     '"Which side of `r = 0` to keep: 0 = keep where r ≤ 0, 1 = keep where r > 0."',
     '"When on, keep points where `r > 0`. When off, keep where `r ≤ 0`."', 1),

    ("misc_extras.rs",
     '"Whether to apply log/exp around ρ: 0 = linear radius, 1 = true log-polar."',
     '"When on, apply a log/exp transform around the polar radius ρ (true log-polar). When off, use linear radius."', 1),

    ("misc_extras.rs",
     '"Tiling axis selector: 0 = horizontal, 1 = vertical."',
     '"When on, tile along the vertical axis. When off, horizontal."', 1),

    ("rosoni_misc.rs",
     '"Inner-shape family selector: 0 = circle/square, 1 = lemniscate/angle."',
     '"When on, use the lemniscate/angle inner-shape family. When off, circle/square."', 1),

    ("singleton_misc.rs",
     '"Formula selector: 0 = `pow(x², …)`, 1 = `pow(log_base(x²·mult + 3), …) − 1.33`."',
     '"When on, use the log-base formula `pow(log_base(x²·mult + 3), …) − 1.33`. When off, plain `pow(x², …)`."', 1),

    ("stub_recoveries2.rs",
     '"Outer-boundary branch selector: 0 = swap (x↔y), 1 = pass through."',
     '"When on, pass the input through. When off, swap (x ↔ y)."', 1),

    ("supershape3d_misc.rs",
     '"0 = spherical projection (output = `r1·r2·cos·cos`, etc.). 1 = toroidal projection (output = `cos·(r1 + r2·cos)`, etc.). Both modes use `r2·sin(φ)` for Z."',
     '"When on, toroidal projection (`cos·(r1 + r2·cos)`, etc.). When off, spherical product (`r1·r2·cos·cos`, etc.). Both modes use `r2·sin(φ)` for Z."', 1),

    ("truchet_misc.rs",
     '"1 = slow iterated-LCG hash for tile-type selection (more cells produce distinct patterns); 0 = fast single-step LCG."',
     '"When on, use the slow iterated-LCG hash for tile-type selection (more cells produce distinct patterns). When off, the fast single-step LCG."', 1),

    ("truchet_misc.rs",
     '"1 = enable direct-color writes (but the color write is dropped in this port — preserved for cpp parity)."',
     '"Enable direct-color writes. (The color write is dropped in this port — preserved for cpp parity.)"', 1),

    ("truchet2_misc.rs",
     '"1 = output the complement (cells outside the arc test); 0 = standard output."',
     '"When on, output the complement (cells outside the arc test). When off, standard output."', 1),

    ("waveblur_misc.rs",
     '"1 = enable direct-color writes (dropped in this port). Preserved for cpp parity."',
     '"Enable direct-color writes. (Dropped in this port — preserved for cpp parity.)"', 1),

    # waves_wf_family: same desc on use_cos_x in WAVES2_WF, WAVES3_WF,
    # WAVES4_WF (3 instances), and same on use_cos_y (3 instances).
    ("waves_wf_family.rs",
     '"1 = use cosine on the X axis; 0 = use sine."',
     '"When on, use cosine on the X axis. When off, sine."', 3),
    ("waves_wf_family.rs",
     '"1 = use cosine on the Y axis; 0 = use sine."',
     '"When on, use cosine on the Y axis. When off, sine."', 3),
]


def main() -> int:
    by_file: dict[str, list[tuple[str, str, int]]] = {}
    for filename, old, new, count in REPLACEMENTS:
        by_file.setdefault(filename, []).append((old, new, count))

    total_replaced = 0
    total_already = 0
    total_warnings = 0
    for filename, repls in by_file.items():
        path = DEFS_DIR / filename
        if not path.exists():
            print(f"  WARN: file not found: {filename}")
            total_warnings += 1
            continue
        src = path.read_text(encoding="utf-8")
        for old, new, expected in repls:
            count_in_src = src.count(old)
            count_new_in_src = src.count(new)
            if count_in_src == 0 and count_new_in_src >= expected:
                print(f"  already-applied [{expected}x] in {filename}")
                total_already += expected
                continue
            if count_in_src != expected:
                snippet = old[:60].replace("\n", " ")
                print(f"  WARN [{filename}]: found {count_in_src} of expected {expected}: {snippet!r}…")
                total_warnings += 1
                continue
            src = src.replace(old, new, expected)
            print(f"  replaced [{expected}x] in {filename}")
            total_replaced += expected
        path.write_text(src, encoding="utf-8")

    print()
    print(f"Replaced {total_replaced}, already-applied {total_already}, warnings {total_warnings}, total {sum(r[3] for r in REPLACEMENTS)}")
    return 0 if total_warnings == 0 else 1


if __name__ == "__main__":
    raise SystemExit(main())
