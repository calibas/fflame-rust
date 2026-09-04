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


def stabilise(grains, size, log=None):
    """Drop `grains` on the centre of a size*size grid and topple in bulk.

    Returns (rounds, height field, seconds).

    Three things keep this tractable at 2^20, none of which changes the
    round count -- that is a property of the parallel schedule, and the
    schedule is identical:

    - int32. After round one the seed cell holds N mod 4 and each
      neighbour N/4, so 2^20 grains peak at 262,144 and never approach
      the type's range.
    - A PILE WINDOW. Sites with h >= 4 can only exist inside the bounding
      box of h > 0, which grows by at most one cell per round. It is
      recomputed every K rounds and padded by K + 1, so the full-grid
      pass happens once per K rounds rather than once per round.
    - An ACTIVE WINDOW inside that: the bounding box of this round's
      topplings, padded by one. Late in a pile the rounds are small
      avalanches, and this is what makes them cheap.

    The first version rolled four full-grid int64 temporaries per round
    on a grid six pile-radii wide -- nine times the cells the pile needs
    -- and 2^18 took seventeen minutes."""
    h = np.zeros((size, size), dtype=np.int32)
    c = size // 2
    h[c, c] = grains
    t = np.zeros_like(h)
    rounds = 0
    t0 = time.time()
    K = 256
    y0 = x0 = c
    y1 = x1 = c + 1
    while True:
        if rounds % K == 0:
            occ_r = np.flatnonzero(h.any(axis=1))
            occ_c = np.flatnonzero(h.any(axis=0))
            y0 = max(int(occ_r[0]) - K - 1, 0)
            y1 = min(int(occ_r[-1]) + K + 2, size)
            x0 = max(int(occ_c[0]) - K - 1, 0)
            x1 = min(int(occ_c[-1]) + K + 2, size)
            if log and rounds % (K * 40) == 0 and rounds:
                log(f"      round {rounds:,}  pile window {y1 - y0}x{x1 - x0}  {time.time() - t0:.0f}s")
        hw = h[y0:y1, x0:x1]
        tw = t[y0:y1, x0:x1]
        np.right_shift(hw, 2, out=tw)     # floor(h / 4) per site
        rows = np.flatnonzero(tw.any(axis=1))
        if rows.size == 0:
            break
        cols = np.flatnonzero(tw.any(axis=0))
        a0 = max(int(rows[0]) - 1, 0)
        a1 = min(int(rows[-1]) + 2, hw.shape[0])
        b0 = max(int(cols[0]) - 1, 0)
        b1 = min(int(cols[-1]) + 2, hw.shape[1])
        ha = hw[a0:a1, b0:b1]
        ta = tw[a0:a1, b0:b1]
        ha -= ta << 2
        # The ring of the active window holds no topplings, so slice
        # shifts inside it are exact; grains that would leave the GRID
        # are simply never added anywhere, which is the edge sink.
        ha[:-1, :] += ta[1:, :]
        ha[1:, :] += ta[:-1, :]
        ha[:, :-1] += ta[:, 1:]
        ha[:, 1:] += ta[:, :-1]
        rounds += 1
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
    logf = open(os.path.join(OUT, "sandpile_progress.log"), "w")

    def log(msg):
        print(msg, flush=True)
        logf.write(msg + chr(10))
        logf.flush()

    rows = []
    # Pile radius from the known scaling: area ~ N / (mean stable height
    # ~2.1), so r ~ sqrt(N / 6.6). Measured at 2^18 that estimate is
    # within 5%, so 1.3x it each side is margin enough, and the edge
    # assertion is what actually guarantees nothing was lost.
    for p in (12, 14, 16, 18, 20):
        n = 1 << p
        r_est = (n / 6.6) ** 0.5
        size = int(max(64, 2.6 * r_est)) | 1
        log(f"  2^{p} = {n:,} grains on {size}x{size} ...")
        rounds, h, secs = stabilise(n, size, log)
        edge = int(h[0, :].sum() + h[-1, :].sum() + h[:, 0].sum() + h[:, -1].sum())
        assert edge == 0, "pile reached the edge (mass lost); grow the grid"
        assert int(h.sum()) == n, f"mass not conserved: {int(h.sum())} of {n}"
        r = radius_of(h)
        rows.append({"grains": n, "exp": p, "grid": size, "rounds": rounds,
                     "radius_cells": round(r, 1),
                     "rounds_per_radius": round(rounds / max(r, 1), 2),
                     "seconds": round(secs, 1)})
        save(f"sandpile_2e{p}.png", h)
        log(f"      {rounds:,} rounds, radius {r:.0f}, {secs:.1f}s")
        # Written after EVERY size so a killed run keeps what it measured.
        with open(os.path.join(OUT, "sandpile_rounds.json"), "w") as f:
            json.dump(rows, f, indent=1)

    log("")
    log(f"{'grains':>12}{'grid':>8}{'rounds':>10}{'radius':>9}{'rounds/r':>10}{'sec':>8}")
    for r in rows:
        log(f"{r['grains']:>12,}{r['grid']:>8}{r['rounds']:>10,}"
            f"{r['radius_cells']:>9}{r['rounds_per_radius']:>10}{r['seconds']:>8}")
    n = np.array([r["grains"] for r in rows], dtype=float)
    rd = np.array([r["rounds"] for r in rows], dtype=float)
    ra = np.array([r["radius_cells"] for r in rows], dtype=float)
    b = np.polyfit(np.log(n), np.log(rd), 1)[0]
    d = np.polyfit(np.log(n), np.log(ra), 1)[0]
    log(f"rounds  ~ N^{b:.3f}   (N^0.5 would be one round per cell of radius)")
    log(f"radius  ~ N^{d:.3f}   (theory: 0.5)")
    logf.close()


if __name__ == "__main__":
    main()
