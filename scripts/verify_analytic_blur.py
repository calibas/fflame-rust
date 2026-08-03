#!/usr/bin/env python3
"""Golden diff test for the analytic-blur buffer feature (gates merge).

Renders the SAME flame three ways and checks that the analytic-blur path
reproduces the stochastic blur — smooth, unbiased, energy-preserving:

  * analytic@N    — one transform uses `analytic_blur` (deterministic kernel
                    convolution; smooth from few samples)
  * stochastic@N  — the same transform uses the original `blur` (noisy at N)
  * reference     — stochastic at a high sample count (converged ground truth)

Assertions (see docs/projects/analytic-blur-buffer.md "The golden test"):
  1. Energy preserved: mean brightness of analytic@N within 5% of reference.
  2. No spatial bias: denoised (structure-only) RMSE of analytic@N vs the
     reference is small — a shifted/wrong kernel would blow this up.
  3. The point of the feature: analytic@N is markedly CLOSER to the converged
     reference than stochastic@N (it's smooth, not grainy).

Usage:  python scripts/verify_analytic_blur.py [--width 500] [--iters 80_000_000]
Exits non-zero on failure. Artifacts land in output/ (gitignored).
"""
import argparse
import json
import os
import subprocess
import sys

import numpy as np
from PIL import Image, ImageFilter

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
BASE = os.path.join(ROOT, "tests", "visual", "configs", "2d", "simple-linear.fflame")
OUT = os.path.join(ROOT, "output")


def find_binary():
    for profile in ("release", "debug"):
        for ext in (".exe", ""):
            p = os.path.join(ROOT, "target", profile, "FractalArtEditor" + ext)
            if os.path.exists(p):
                return p
    sys.exit("error: build the binary first (cargo build --release)")


def make_variant(base, blur_name, iters):
    """Return a config dict using `blur_name` (analytic_blur or blur) on the
    first transform, plus a linear companion, at the given iteration count."""
    d = json.loads(json.dumps(base))  # deep copy
    d["max_iterations"] = iters
    t0 = d["flame"]["transforms"][0]
    t0["variations"] = {"linear": 1.0, blur_name: 0.5}
    return d


def render(binary, config, path, width):
    cfg_path = os.path.splitext(path)[0] + ".fflame"
    with open(cfg_path, "w") as f:
        json.dump(config, f)
    subprocess.run(
        [binary, "export", "-i", cfg_path, "-o", path,
         "--width", str(width), "--height", str(width)],
        check=True, cwd=ROOT,
        stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL,
    )
    return np.asarray(Image.open(path).convert("RGB"), dtype=float)


def denoise(path):
    im = Image.open(path).convert("RGB").filter(ImageFilter.GaussianBlur(2))
    return np.asarray(im, dtype=float)


def rmse(a, b):
    return float(np.sqrt(((a - b) ** 2).mean()))


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--width", type=int, default=500)
    ap.add_argument("--iters", type=int, default=80_000_000)
    ap.add_argument("--ref-iters", type=int, default=2_000_000_000)
    args = ap.parse_args()

    binary = find_binary()
    os.makedirs(OUT, exist_ok=True)
    with open(BASE) as f:
        base = json.load(f)

    p_ana = os.path.join(OUT, "verify_ab_analytic.png")
    p_sto = os.path.join(OUT, "verify_ab_stochastic.png")
    p_ref = os.path.join(OUT, "verify_ab_reference.png")

    print(f"Rendering analytic@{args.iters:,} ...")
    a = render(binary, make_variant(base, "analytic_blur", args.iters), p_ana, args.width)
    print(f"Rendering stochastic@{args.iters:,} ...")
    s = render(binary, make_variant(base, "blur", args.iters), p_sto, args.width)
    print(f"Rendering converged reference@{args.ref_iters:,} ...")
    r = render(binary, make_variant(base, "blur", args.ref_iters), p_ref, args.width)

    bright_ana, bright_ref = a.mean(), r.mean()
    bright_err = abs(bright_ana - bright_ref) / max(bright_ref, 1e-6)
    rmse_ana_raw = rmse(a, r)
    rmse_sto_raw = rmse(s, r)
    rmse_ana_struct = rmse(denoise(p_ana), denoise(p_ref))

    print("\n--- results ---")
    print(f"brightness analytic={bright_ana:.3f} ref={bright_ref:.3f} (err {bright_err*100:.2f}%)")
    print(f"RAW RMSE vs reference:   analytic={rmse_ana_raw:.3f}  stochastic={rmse_sto_raw:.3f}")
    print(f"STRUCTURE RMSE (analytic vs reference, denoised): {rmse_ana_struct:.3f}")

    ok = True
    # 1. Energy preserved.
    if bright_err > 0.05:
        print(f"FAIL: brightness error {bright_err*100:.2f}% > 5%")
        ok = False
    # 2. No spatial bias (a wrong/shifted kernel inflates this).
    if rmse_ana_struct > 5.0:
        print(f"FAIL: structure RMSE {rmse_ana_struct:.3f} > 5.0 (kernel bias?)")
        ok = False
    # 3. Analytic is markedly closer to converged than stochastic at the same N.
    if rmse_ana_raw > 0.7 * rmse_sto_raw:
        print(f"FAIL: analytic ({rmse_ana_raw:.3f}) not clearly smoother than "
              f"stochastic ({rmse_sto_raw:.3f}) — expected < 0.7x")
        ok = False

    print("\nPASS" if ok else "\nFAILED")
    sys.exit(0 if ok else 1)


if __name__ == "__main__":
    main()
