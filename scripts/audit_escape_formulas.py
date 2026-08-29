"""Cross-check every escaping formula against an independent oracle.

The question this answers: does our WGSL iterate the map we CLAIM it
iterates? Inspection compares the shader text against a definition;
this compares the shader's BEHAVIOUR against a numpy transcription of
the canonical formula, written from the published definition rather
than from our shader (transcribing our own code would only prove it
equals itself -- the mistake that let Ducks ship with `c` outside the
log for months).

Method. Set membership is the observable: a pixel either escapes
within max_iter or it does not, and a wrong map moves that boundary
everywhere. Our render uses an all-white palette on a black
background, so "escaped" is exactly "not background" with no
dependence on palette shape or tone mapping. The oracle mirrors the
template's semantics exactly -- seed, escape metric, bailout,
convergence break, prev-z init -- because a mismatch in those is a
false alarm, not a finding.

Vacuum guard: a comparison of two uniform images passes trivially.
Any formula whose mask is more than 98% one class is reported as
UNINFORMATIVE rather than counted as agreement.

Run:  python scripts/audit_escape_formulas.py [--only name]
"""
import json
import os
import subprocess
import sys

import numpy as np
from PIL import Image

W, H = 240, 180
MAX_ITER = 256
BAILOUT = 4.0
EXE = os.path.abspath("target/release/FractalArtEditor.exe")
OUT = "output/formula_audit"

# Per-formula view: (center_re, center_im, zoom_log2). Chosen so the
# mask is informative (both classes present) -- the vacuum guard below
# reports any that are not.
VIEWS = {
    "mandelbrot": (-0.5, 0.0, -0.6),
    "multibrot": (0.0, 0.0, -0.6),
    "tricorn": (-0.2, 0.0, 0.4),
    "burning_ship": (-0.5, -0.5, -1.0),
    "mcmullen": (0.0, 0.0, 0.5),
    "phoenix": (0.0, 0.0, -0.8),
    "lambda": (1.0, 0.0, -1.2),
    "spider": (0.0, 0.0, -0.8),
    "manowar": (-0.4, 0.0, 1.0),
    "barnsley": (0.0, 0.0, -1.2),
    "cactus": (0.0, 0.0, 0.8),
    "exponential": (0.0, 0.0, -1.5),
    "trig": (0.0, 0.0, -1.5),
    "tetration": (0.0, 0.0, -1.2),
    "collatz": (0.0, 0.0, 1.6),
    "feather": (0.0, 0.0, -0.8),
    "newton": (0.0, 0.0, -0.4),
    "nova": (0.0, 0.0, -0.4),
    "magnet": (1.5, 0.0, -1.0),
    "littlewood": (0.0, 0.0, -1.0),
}

# Formulas excluded from the SET test, with the reason. Non-escaping
# maps have no set to compare (nothing ever escapes); they are audited
# by field comparison instead (Ducks was, in the Ducks fix).
# Convergent formulas need a low iteration cap: with the default they
# terminate everywhere and the mask is uniform by construction. A cap
# makes the mask "converged fast", whose boundary IS the basin
# structure, so the test exercises the same map informatively.
ITER_OVERRIDE = {"newton": 10, "nova": 14, "magnet": 12}

# Formulas audited in JULIA mode, with the constant used. McMullen's
# parameter plane escapes everywhere (its critical point is the pole),
# which is exactly what the registry doc says and what the shipped
# carpet config does -- so it is audited as it is actually used.
JULIA = {"mcmullen": (0.04, 0.0)}

# Views for the non-escaping formulas (compared by mean-|z| field).
NONESC_VIEWS = {
    "kaliset": (0.0, 0.0, -0.5),
    "ducks": (0.0, 0.0, -0.5),
    "novaretti": (0.0, 0.0, -0.5),
}

SKIP = {
    "kaliset": "non-escaping: every pixel runs to max_iter, no set boundary",
    "ducks": "non-escaping: audited by mean-|z| field comparison (2026-08-28)",
    "novaretti": "non-escaping: convergent field, no escape boundary",
}


def canonical(name, params, C, max_iter=MAX_ITER, julia_c=None):
    """The published map, in f64. Returns the escaped mask.

    Each branch cites the definition it transcribes. `C` is the pixel
    (the parameter plane's c); the seed and escape metric mirror the
    formula's registry entry.
    """
    p = params
    esc = np.zeros(C.shape, dtype=bool)
    done = np.zeros(C.shape, dtype=bool)

    # Seed (parameter plane): registry `wgsl_param_seed`.
    seeds = {
        "mcmullen": C, "manowar": C, "barnsley": C, "cactus": C,
        "tetration": C, "collatz": C, "newton": C,
        "lambda": np.full(C.shape, 0.5 + 0j),
        "nova": np.full(C.shape, 1.0 + 0j),
        "littlewood": np.full(C.shape, 1.0 + 0j),
    }
    if julia_c is not None:
        # Dynamical plane: the pixel IS z0, and c is the fixed constant.
        z = C.astype(complex).copy()
    else:
        z = seeds.get(name, np.zeros(C.shape, dtype=complex)).astype(complex).copy()

    prev = z.copy() if name == "manowar" else np.zeros(C.shape, dtype=complex)
    c = (np.full(C.shape, complex(*julia_c)) if julia_c is not None
         else C.copy())  # Spider mutates it.
    metric = {"exponential": "re", "tetration": "re",
              "trig": "absim", "collatz": "absim"}.get(name, "normsq")
    convergent = name in ("newton", "nova", "magnet")

    with np.errstate(all="ignore"):
        for _ in range(max_iter):
            zb = z
            if name == "mandelbrot":
                z = z * z + c
            elif name == "multibrot":
                z = z ** p["power"] + c
            elif name == "tricorn":
                z = np.conj(z) ** p["power"] + c
            elif name == "burning_ship":
                # Michelitsch-Rossler: (|Re z| + i|Im z|)^2 + c
                z = (np.abs(z.real) + 1j * np.abs(z.imag)) ** 2 + c
            elif name == "mcmullen":
                # z^n + c/z^m, pole guarded as the shader guards it
                pole = np.abs(z) ** 2 < 1e-20
                z = np.where(pole, 1e20 + 0j, z ** p["n"] + c / z ** p["m"])
            elif name == "phoenix":
                pp = p["p_re"] + 1j * p["p_im"]
                zn = z * z + c + pp * prev
                prev, z = z, zn
            elif name == "lambda":
                z = c * z * (1 - z)
            elif name == "spider":
                z = z * z + c
                c = c * 0.5 + z
            elif name == "manowar":
                zn = z * z + prev + c
                prev, z = z, zn
            elif name == "barnsley":  # Type 1
                z = np.where(z.real >= 0, (z - 1) * c, (z + 1) * c)
            elif name == "cactus":
                z = z ** 3 + (c - 1) * z - c
            elif name == "exponential":
                z = np.exp(z) + c
            elif name == "trig":
                z = np.sin(z) + c
            elif name == "tetration":
                z = np.exp(z * np.log(c))
            elif name == "collatz":
                z = 0.25 * (2 + 7 * z - (2 + 5 * z) * np.cos(np.pi * z))
            elif name == "feather":
                z = z ** p["power"] / (1 + z.real ** 2 - 1j * z.imag ** 2) + c
            elif name == "newton":
                pw = p["power"]
                f = z ** pw - 1
                fp = pw * z ** (pw - 1)
                z = z - (p["relax_re"] + 1j * p["relax_im"]) * (f / fp)
            elif name == "nova":
                pw = p["power"]
                f = z ** pw - 1
                fp = pw * z ** (pw - 1)
                z = z - (p["relax_re"] + 1j * p["relax_im"]) * (f / fp) + c
            elif name == "magnet":  # I
                z = ((z * z + c - 1) / (2 * z + c - 2)) ** 2
            elif name == "littlewood":
                w = c * z
                cands = [w + 1, w - 1]
                best = cands[0].copy()
                bd = np.abs(best) ** 2
                for cd in cands[1:]:
                    m = np.abs(cd) ** 2 < bd
                    best = np.where(m, cd, best)
                    bd = np.minimum(bd, np.abs(cd) ** 2)
                z = best
            else:
                raise SystemExit("no oracle for %s" % name)

            if convergent:  # template's CONVERGE_TEST, before escape
                conv = np.abs(z - zb) ** 2 < 1e-12
                esc |= conv & ~done
                done |= conv
            m = {"normsq": np.abs(z) ** 2,
                 "re": z.real,
                 "absim": np.abs(z.imag)}[metric]
            hit = np.nan_to_num(m, nan=1e30) > BAILOUT
            esc |= hit & ~done
            done |= hit
            if done.all():
                break
    return esc


def canonical_counts(name, params, C, max_iter, julia_c=None):
    """Iterations to escape/converge per pixel -- the field behind the
    set. Same recurrences as `canonical`, re-run to record WHEN each
    pixel terminated rather than whether it did."""
    counts = np.full(C.shape, max_iter, dtype=np.int32)
    esc = np.zeros(C.shape, dtype=bool)
    for k in range(1, max_iter + 1):
        m = canonical(name, params, C, k, julia_c)
        newly = m & ~esc
        counts[newly] = k
        esc |= m
        if esc.all():
            break
    return counts


def field_disagreement(ours_gray, oracle_counts, drop_clipped=False):
    """Tonemap-invariant comparison: both fields are monotone in the
    escape count, so thresholding each at the SAME AREA FRACTION must
    select the same region. Gamma, exposure and palette shape cannot
    affect this; a wrong map immediately does."""
    keep = np.ones(ours_gray.shape, dtype=bool)
    if drop_clipped:
        # A heavy-tailed field (novaretti spans 0.3 to 2e5) saturates
        # the 8-bit render: a fifth of the pixels land on one gray
        # level, and equal-area thresholds cannot order values the
        # image has already tied. Compare where the render still
        # carries information, and say so, rather than reporting the
        # quantization as a disagreement.
        keep = (ours_gray > ours_gray.min()) & (ours_gray < ours_gray.max())
        if keep.mean() < 0.25:
            return float("nan")
    a_src, b_src = ours_gray[keep], oracle_counts[keep]
    worst = 0.0
    for q in (0.25, 0.5, 0.75):
        a = a_src <= np.quantile(a_src, q)
        b = b_src <= np.quantile(b_src, q)
        worst = max(worst, float((a != b).mean()))
    return worst


def canonical_mean_abs(name, params, C, max_iter, julia_c=None):
    """Mean |z| over the orbit -- the observable non-escaping formulas
    are actually rendered with (`magnitude_average`), and the one that
    settled the Ducks correction. Uses the same recurrences as
    `canonical`, which is re-entered per step so the maps stay in one
    place."""
    z = C.astype(complex).copy() if julia_c is not None else None
    acc = np.zeros(C.shape, dtype=float)
    # Re-run the recurrence directly: `canonical` returns masks, so the
    # orbit is rebuilt here with the same branches it uses.
    p = params
    c = (np.full(C.shape, complex(*julia_c)) if julia_c is not None else C.copy())
    if z is None:
        z = (C.astype(complex).copy() if name == "kaliset"
             else np.zeros(C.shape, dtype=complex))
    if name == "novaretti":
        z = (C * -0.0729490168) ** (1.0 / 3.0)
    # Convergent formulas BREAK on a settled orbit, so accumulation
    # stops there: the coloring's mean is over the iterations actually
    # run, not over max_iter. Missing this reads as a formula error --
    # it showed up as 14% disagreement at four iterations, which chaos
    # cannot explain.
    convergent = name in ("novaretti",)
    live = np.ones(C.shape, dtype=bool)
    count = np.zeros(C.shape, dtype=float)
    with np.errstate(all="ignore"):
        for _ in range(max_iter):
            zb = z
            if name == "kaliset":
                r2 = (z.real ** 2 + z.imag ** 2)
                folded = np.where(r2 > 1e-30,
                                  (np.abs(z.real) + 1j * np.abs(z.imag)) / np.where(r2 > 1e-30, r2, 1),
                                  0)
                z = folded - c  # plus_c = 0 branch
            elif name == "ducks":
                z = np.log(z.real + 1j * np.abs(z.imag) + c)
            elif name == "novaretti":
                z3 = z ** 3
                den = (2 * z3 - c) ** 2
                z = np.where(np.abs(den) ** 2 < 1e-24, 1e10 + 0j,
                             -6.0 * z * (z3 + c) / np.where(np.abs(den) ** 2 < 1e-24, 1, den))
            else:
                raise SystemExit("no mean-|z| oracle for %s" % name)
            acc += np.where(live, np.nan_to_num(np.abs(z), nan=0.0, posinf=0.0), 0.0)
            count += live
            if convergent:
                live &= ~(np.abs(z - zb) ** 2 < 1e-12)
    return acc / np.maximum(count, 1.0)


def make_config(name, params, view, field=False, mean_abs=False,
                mean_scale=1.0, mean_offset=0.0, max_iter=None):
    base = json.load(open("tests/visual/configs/escape/ducks-param.fflame"))
    base["flame"]["name"] = "audit-" + name
    # All-white palette: "escaped" becomes "not background", with no
    # dependence on where the palette happens to be dark.
    ramp = field or mean_abs
    base["palette"] = ({"name": "audit-ramp",
                        "stops": [{"position": 0.0, "color": [0.0, 0.0, 0.0]},
                                  {"position": 1.0, "color": [1.0, 1.0, 1.0]}],
                        "locked": False} if ramp else
                       {"name": "audit-white",
                        "stops": [{"position": 0.0, "color": [1.0, 1.0, 1.0]},
                                  {"position": 1.0, "color": [1.0, 1.0, 1.0]}],
                        "locked": False})
    base["background_color"] = [0.0, 0.0, 0.0]
    base["exposure"] = 1.0
    base["gamma"] = 1.0
    base["escape"] = {
        "formula": name,
        "coloring": ("magnitude_average" if mean_abs
                     else "escape_count" if field else "smooth"),
        "max_iter": max_iter or ITER_OVERRIDE.get(name, MAX_ITER),
        "center_re": repr(view[0]), "center_im": repr(view[1]),
        "zoom_log2": view[2], "bailout": BAILOUT,
        "formula_params": params,
        "coloring_params": ({"scale": mean_scale, "offset": mean_offset} if mean_abs
                            else {"scale": 1.0 / ITER_OVERRIDE.get(name, MAX_ITER)}
                            if field else {}),
        **({"julia": True, "julia_re": JULIA[name][0], "julia_im": JULIA[name][1]}
           if name in JULIA else {}),
    }
    suffix = "-mean" if mean_abs else "-field" if field else ""
    path = "%s/%s%s.fflame" % (OUT, name, suffix)
    json.dump(base, open(path, "w"))
    return path


def main():
    os.makedirs(OUT, exist_ok=True)
    meta = json.load(open("output/formula_meta.json"))
    only = sys.argv[2] if len(sys.argv) > 2 and sys.argv[1] == "--only" else None

    print("%-14s %-9s %s" % ("formula", "disagree", "verdict"))
    rows = []
    for name, info in meta.items():
        if only and name != only:
            continue
        params = info["params"]
        view = VIEWS.get(name) or NONESC_VIEWS.get(name)
        if view is None:
            rows.append((name, None, "no view configured"))
            continue

        if name in SKIP:
            # Non-escaping: compare the mean-|z| field, equal-area.
            it = int(os.environ.get("AUDIT_NONESC_ITERS", "60"))
            span_y = 4.0 / (2.0 ** view[2])
            span_x = span_y * W / H
            xs = ((np.arange(W) + 0.5) / W - 0.5) * span_x + view[0]
            ys = -(((np.arange(H) + 0.5) / H - 0.5) * span_y) + view[1]
            X, Y = np.meshgrid(xs, ys)
            oracle_f = canonical_mean_abs(name, params, X + 1j * Y, it,
                                          JULIA.get(name))
            # Percentile scaling spreads the BODY of the field over
            # the 8-bit range, at the cost of the top tail exceeding
            # 1.0 -- which the template's fract() wraps, drawing halo
            # rings that destroy rank ordering exactly where they
            # appear. True-range scaling avoids the wrap but crushes a
            # 10^5-range field onto one gray level. Neither carries the
            # whole field, so: scale to the body, and EXCLUDE the tail
            # that wraps. 0.5% of pixels are unmeasurable this way and
            # are reported as such rather than counted as disagreement.
            lo = float(np.nanpercentile(oracle_f, 0.5))
            hi = float(np.nanpercentile(oracle_f, 99.5))
            span = max(hi - lo, 1e-6)
            cfg = make_config(name, params, view, mean_abs=True,
                              mean_scale=0.98 / span, mean_offset=lo,
                              max_iter=it)
            png = "%s/%s-mean.png" % (OUT, name)
            r = subprocess.run([EXE, "export", "-i", cfg, "-o", png,
                                "--width", str(W), "--height", str(H)],
                               capture_output=True, text=True)
            if r.returncode != 0 or not os.path.exists(png):
                rows.append((name, None, "MEAN RENDER FAILED"))
                continue
            g = np.array(Image.open(png).convert("L")).astype(np.float64)
            unwrapped = oracle_f <= hi
            d = field_disagreement(g[unwrapped], oracle_f[unwrapped])
            rows.append((name, d, "match (mean-|z| field, tail excluded)" if d < 0.03
                         else "FIELD MISMATCH - investigate"))
            continue

        cfg = make_config(name, params, view)
        png = "%s/%s.png" % (OUT, name)
        r = subprocess.run([EXE, "export", "-i", cfg, "-o", png,
                            "--width", str(W), "--height", str(H)],
                           capture_output=True, text=True)
        if r.returncode != 0 or not os.path.exists(png):
            rows.append((name, None, "RENDER FAILED"))
            continue
        img = np.array(Image.open(png).convert("RGB")).astype(np.int32)
        ours = img.sum(axis=2) > 24  # not background

        # Same pixel->plane mapping as the template.
        span_y = 4.0 / (2.0 ** view[2])
        span_x = span_y * W / H
        xs = ((np.arange(W) + 0.5) / W - 0.5) * span_x + view[0]
        ys = -(((np.arange(H) + 0.5) / H - 0.5) * span_y) + view[1]
        X, Y = np.meshgrid(xs, ys)
        oracle = canonical(name, params, X + 1j * Y,
                           ITER_OVERRIDE.get(name, MAX_ITER),
                           JULIA.get(name))

        frac_true = oracle.mean()
        disagree = (ours != oracle).mean()
        if frac_true > 0.98 or frac_true < 0.02:
            # The set cannot discriminate here (everything escapes, or
            # nothing does). Compare the escape-TIME field instead.
            it = ITER_OVERRIDE.get(name, MAX_ITER)
            cfg2 = make_config(name, params, view, field=True)
            png2 = "%s/%s-field.png" % (OUT, name)
            r2 = subprocess.run([EXE, "export", "-i", cfg2, "-o", png2,
                                 "--width", str(W), "--height", str(H)],
                                capture_output=True, text=True)
            if r2.returncode != 0 or not os.path.exists(png2):
                rows.append((name, None, "FIELD RENDER FAILED"))
                continue
            g = np.array(Image.open(png2).convert("L")).astype(np.float64)
            counts = canonical_counts(name, params, X + 1j * Y, it, JULIA.get(name))
            d2 = field_disagreement(g, counts)
            disagree = d2
            verdict = ("match (escape-time field; set is uniform here)"
                       if d2 < 0.02 else
                       "FIELD MISMATCH - investigate")
        elif disagree < 0.005:
            verdict = "match"
        elif disagree < 0.05:
            verdict = "CLOSE - inspect (boundary/precision?)"
        else:
            verdict = "MISMATCH - investigate"
        rows.append((name, disagree, verdict))

    for name, d, v in rows:
        print("%-14s %-9s %s" % (name, "-" if d is None else "%.3f%%" % (100 * d), v))


if __name__ == "__main__":
    main()
