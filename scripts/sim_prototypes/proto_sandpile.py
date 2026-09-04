"""Abelian sandpile: how many PARALLEL bulk-toppling rounds does it take.

Phase-0 measurement for simulation mode. The catalogue (section 14)
calls this "the open cost question" and the only entry it could not
estimate at all:

    mass must propagate radially, so rounds >= radius (~ sqrt(N)c) and
    in practice far more; the plan measures it in a NumPy prototype
    before committing to a step budget (a 2^20-grain pile is the target
    picture).

The rule is Abelian -- the stable configuration does not depend on
toppling order -- so a GPU can topple every over-full site at once:
each site with h >= 4 sends floor(h/4) to each von Neumann neighbour,
which is just floor(h/4) consecutive single topplings. One round is one
dispatch: read own h and the four neighbours' floor(h/4). No atomics.

What this measures, per pile size:
  - rounds to a stable configuration
  - the pile radius, so rounds can be stated per radius
  - how both scale, which is what sizes the step budget for 2^20

Edges are sinks. The grid is sized so nothing actually reaches them,
and that is asserted rather than assumed -- a pile that spills off the
edge has lost mass and its round count means nothing.

Writes PNGs and a JSON row set into output/sim_proto/.
"""
import json
import os
import time

import numpy as np
from PIL import Image

OUT = "output/sim_proto"
os.makedirs(OUT, exist_ok=True)

# The classic four-colour picture: h in {0,1,2,3}.
PALETTE = np.array([(0, 0, 40), (0, 110, 160), (60, 200, 160), (255, 240, 120)],
                   dtype=np.uint8)


def stabilise(grains, size, report_every=2000):
    """Drop `grains` on the centre of a size*size grid and topple in bulk.

    Returns (rounds, height field, seconds)."""
    # np.roll was the first version and it is too slow to reach 2^20:
    # every round rolled four full-grid temporaries and then masked the
    # wrap back off, and 2^16 alone took two minutes. SLICE assignment
    # shifts without ever creating the wrap the sink boundary has to
    # undo, which is 34x faster here -- 2^16 in 3.5 s. (The dtype stays
    # int64: the seed cell holds all N grains before the first round.)
    h = np.zeros((size, size), dtype=np.int64)
    h[size // 2, size // 2] = grains
    rounds = 0
    t0 = time.time()
    t = np.empty_like(h)
    while True:
        np.right_shift(h, 2, out=t)     # floor(h / 4)
        if not t.any():
            break
        h -= t << 2
        # Grains leaving the grid are simply not added anywhere: that is
        # the catalogue's `sink: edges`, and it needs no masking when the
        # shift is a slice rather than a roll.
        h[:-1, :] += t[1:, :]
        h[1:, :] += t[:-1, :]
        h[:, :-1] += t[:, 1:]
        h[:, 1:] += t[:, :-1]
        rounds += 1
        if report_every and rounds % report_every == 0:
            print(f"      ... round {rounds:,}, {int(t.sum()):,} topplings", flush=True)
    return rounds, h, time.time() - t0


def radius_of(h):
    """Half-width of the occupied region, in cells."""
    occ = np.argwhere(h > 0)
    if occ.size == 0:
        return 0
    (y0, x0), (y1, x1) = occ.min(0), occ.max(0)
    return max(y1 - y0, x1 - x0) / 2.0


def save(name, h):
    img = PALETTE[np.clip(h, 0, 3).astype(np.uint8)]
    Image.fromarray(img).save(os.path.join(OUT, name))


def main():
    rows = []
    # Grid sized from the known scaling: the pile's area is about
    # N / (mean stable height ~2.1), so r ~ sqrt(N / 6.6). Take 3x that
    # radius of margin and assert nothing reached the edge.
    for p in (12, 14, 16, 18, 20):
        n = 1 << p
        r_est = (n / 6.6) ** 0.5
        size = int(max(64, 6 * r_est))
        size += size % 2 ^ 1 & 1        # keep it odd so there is a centre
        if size % 2 == 0:
            size += 1
        print(f"  2^{p} = {n:,} grains on {size}x{size} ...", flush=True)
        rounds, h, secs = stabilise(n, size)
        edge = int(h[0, :].sum() + h[-1, :].sum() + h[:, 0].sum() + h[:, -1].sum())
        assert edge == 0, f"pile reached the edge (mass lost); grow the grid"
        r = radius_of(h)
        rows.append({"grains": n, "exp": p, "grid": size, "rounds": rounds,
                     "radius_cells": round(r, 1),
                     "rounds_per_radius": round(rounds / max(r, 1), 2),
                     "seconds": round(secs, 1)})
        save(f"sandpile_2e{p}.png", h)
        print(f"      {rounds:,} rounds, radius {r:.0f}, {secs:.1f}s", flush=True)

    with open(os.path.join(OUT, "sandpile_rounds.json"), "w") as f:
        json.dump(rows, f, indent=1)

    print()
    print(f"{'grains':>12}{'grid':>8}{'rounds':>10}{'radius':>9}"
          f"{'rounds/r':>10}{'sec':>8}")
    for r in rows:
        print(f"{r['grains']:>12,}{r['grid']:>8}{r['rounds']:>10,}"
              f"{r['radius_cells']:>9}{r['rounds_per_radius']:>10}{r['seconds']:>8}")

    # Scaling exponents: rounds ~ a * N^b, radius ~ c * N^d.
    n = np.array([r["grains"] for r in rows], dtype=float)
    rd = np.array([r["rounds"] for r in rows], dtype=float)
    ra = np.array([r["radius_cells"] for r in rows], dtype=float)
    b = np.polyfit(np.log(n), np.log(rd), 1)[0]
    d = np.polyfit(np.log(n), np.log(ra), 1)[0]
    print(f"\nrounds  ~ N^{b:.3f}   (N^0.5 would be 'one round per cell of radius')")
    print(f"radius  ~ N^{d:.3f}   (theory: 0.5)")


if __name__ == "__main__":
    main()
