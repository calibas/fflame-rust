"""Wolfram elementary cellular automata: verify the rule, not the cost.

Phase-0 measurement for simulation mode. This is the one Tier-1 model
whose step count needs no measuring -- the field IS the space-time
diagram, so `steps` = grid height, exactly, by construction. One
`update` dispatch writes row t from row t-1; there is no ping-pong
because each row is written once.

What DOES need checking is the bit convention, because it is easy to
get backwards and the result still looks like a cellular automaton.
The next state is bit (4*left + 2*self + right) of the rule number, so
rule 90 must be left XOR right and must draw Sierpinski's triangle from
a single seed. That is a falsifiable check, and it is the whole point
of running this before writing the shader.

Writes PNGs and a JSON row set into output/sim_proto/.
"""
import json
import os

import numpy as np
from PIL import Image

OUT = "output/sim_proto"
os.makedirs(OUT, exist_ok=True)
W = H = 512


def run(rule, seed_kind="single", density=0.5, seed=1):
    """Row t from row t-1; periodic in x."""
    rg = np.random.default_rng(seed)
    grid = np.zeros((H, W), dtype=np.uint8)
    if seed_kind == "single":
        grid[0, W // 2] = 1
    else:
        grid[0] = (rg.random(W) < density).astype(np.uint8)
    bits = np.array([(rule >> k) & 1 for k in range(8)], dtype=np.uint8)
    for t in range(1, H):
        prev = grid[t - 1]
        idx = (np.roll(prev, 1) << 2) | (prev << 1) | np.roll(prev, -1)
        grid[t] = bits[idx]
    Image.fromarray(grid * 255).save(os.path.join(OUT, f"wolfram_{rule}_{seed_kind}.png"))
    return grid


def sierpinski_check(grid):
    """Rule 90 from a single seed is Pascal's triangle mod 2.

    Comparing against binomials computed independently is what makes
    this a check rather than a screenshot."""
    ok = 0
    total = 0
    for t in range(1, min(64, H)):
        # cell at offset d from centre is C(t, (t+d)/2) mod 2 when the
        # parities agree, else 0.
        row = grid[t]
        for d in range(-t, t + 1):
            if (t + d) % 2:
                continue
            k = (t + d) // 2
            c = 1
            for j in range(k):
                c = c * (t - j) // (j + 1)
            total += 1
            if row[(W // 2 + d) % W] == (c & 1):
                ok += 1
    return ok, total


def main():
    rows = []
    for rule in (30, 90, 110, 184, 54, 22, 126, 150):
        g = run(rule, "single")
        rows.append({"model": "wolfram_eca", "rule": rule, "seed": "single",
                     "steps": H, "live_fraction": round(float(g.mean()), 4)})
    for rule in (30, 110):
        g = run(rule, "random")
        rows.append({"model": "wolfram_eca", "rule": rule, "seed": "random",
                     "steps": H, "live_fraction": round(float(g.mean()), 4)})

    ok, total = sierpinski_check(run(90, "single"))
    rows.append({"model": "wolfram_eca", "rule": 90, "check": "sierpinski",
                 "cells_matching_binomial_mod_2": f"{ok}/{total}"})

    with open(os.path.join(OUT, "wolfram_step_costs.json"), "w") as f:
        json.dump(rows, f, indent=1)
    print(f"{'rule':>6}{'seed':>9}{'steps':>8}{'live':>9}")
    for r in rows:
        if "check" in r:
            continue
        print(f"{r['rule']:>6}{r['seed']:>9}{r['steps']:>8}{r['live_fraction']:>9}")
    print(f"\nrule 90 vs binomial mod 2: {ok}/{total} cells match "
          f"({'PASS' if ok == total else 'FAIL'})")
    print("steps = grid height, exactly, by construction -- nothing to measure")


if __name__ == "__main__":
    main()
