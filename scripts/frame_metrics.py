#!/usr/bin/env python3
"""Measurements for the quality governor (docs/archive/projects/quality-governor.md, phase 1).

    python scripts/frame_metrics.py loss [DIR]       # default output/governor
    python scripts/frame_metrics.py upscalers [DIR]  # the viewport's upscalers
    python scripts/frame_metrics.py flicker FRAMES_DIR
    python scripts/frame_metrics.py summary FRAME_LOG.csv

loss     reads DIR/calibration.csv (the `governor_calibration` test's) and
         compares every rung's picture with its scene's reference. A rung
         rendered smaller is upscaled to the reference's size first, by each
         of bilinear (what egui does today), bicubic and Lanczos. Writes
         DIR/loss.csv and DIR/table.md.
upscalers reads DIR/upscalers.csv (the `upscaler_comparison` test's: each
         rung below full size drawn up by the viewport's own pass, every
         filter, FSR included) and measures each against the reference.
         Writes DIR/upscalers.md.
flicker  compares each frame of a folder of PNGs with the one before it:
         how much a still-looking picture moves from frame to frame.
summary  reads a frame log (FFLAME_FRAME_LOG) and summarises it by mode,
         view size and whether playback ran.

The measures, on 8-bit sRGB, RGB only:
    mae    mean absolute difference, in levels of 255
    p95    the 95th percentile of the 16x16 blocks' mean absolute
           difference: where the loss concentrates, which a mean hides
    psnr   peak signal-to-noise ratio, dB
"""
import csv
import os
import sys

import numpy as np
from PIL import Image

BLOCK = 16
FILTERS = {
    'bilinear': Image.Resampling.BILINEAR,
    'bicubic': Image.Resampling.BICUBIC,
    'lanczos': Image.Resampling.LANCZOS,
}


def load_rgb(path):
    return Image.open(path).convert('RGB')


def measures(a, b):
    """mae, p95 and psnr of two same-sized RGB images."""
    x = np.asarray(a, dtype=np.float64)
    y = np.asarray(b, dtype=np.float64)
    d = np.abs(x - y).mean(axis=2)
    h, w = d.shape
    bh, bw = h // BLOCK, w // BLOCK
    blocks = d[:bh * BLOCK, :bw * BLOCK].reshape(bh, BLOCK, bw, BLOCK).mean(axis=(1, 3))
    mse = ((x - y) ** 2).mean()
    psnr = float('inf') if mse == 0 else 10 * np.log10(255.0 ** 2 / mse)
    return d.mean(), np.percentile(blocks, 95), psnr


def read_calibration(root):
    with open(os.path.join(root, 'calibration.csv'), newline='') as f:
        return list(csv.DictReader(f))


def loss(root):
    rows = read_calibration(root)
    scenes = {}
    for r in rows:
        scenes.setdefault(r['scene'], []).append(r)
    out_rows = []
    md = ['# Governor calibration', '',
          'Loss against each scene\'s reference, in levels of 255 (mae; p95 of 16x16 blocks).',
          'A rung below full size is upscaled to the reference first; each filter is shown.', '']
    for scene, rs in scenes.items():
        kind = rs[0]['kind']
        if kind == 'sim':
            md += sim_table(scene, rs)
            continue
        ref = next((r for r in rs if r['rung'] == 'ref'), None)
        if ref is None:
            continue
        ref_img = load_rgb(os.path.join(root, ref['file']))
        md += [f'## {scene}', '', f'Reference: {ref["rung"]}, {ref["width"]}x{ref["height"]}, {float(ref["ms"]):.0f} ms.', '']
        head = '| rung | size | ms |' + ''.join(f' {k} mae | {k} p95 |' for k in FILTERS)
        md += [head, '|' + '---|' * (3 + 2 * len(FILTERS))]
        for r in rs:
            if r is ref:
                continue
            img = load_rgb(os.path.join(root, r['file']))
            cells = []
            for name, flt in FILTERS.items():
                up = img if img.size == ref_img.size else img.resize(ref_img.size, flt)
                mae, p95, psnr = measures(up, ref_img)
                out_rows.append({'scene': scene, 'kind': kind, 'rung': r['rung'], 'filter': name,
                                 'width': r['width'], 'height': r['height'], 'samples': r['samples'],
                                 'ms': r['ms'], 'mae': f'{mae:.3f}', 'p95': f'{p95:.3f}', 'psnr': f'{psnr:.2f}'})
                cells.append(f' {mae:.2f} | {p95:.2f} |')
                if img.size == ref_img.size:
                    # Full size: no filter applies, so one measure stands for all.
                    cells = [cells[0]] * len(FILTERS)
                    for name2 in list(FILTERS)[1:]:
                        out_rows.append(dict(out_rows[-1], filter=name2))
                    break
            md.append(f'| {r["rung"]} | {r["width"]}x{r["height"]} | {float(r["ms"]):.1f} |' + ''.join(cells))
        md.append('')
    with open(os.path.join(root, 'loss.csv'), 'w', newline='') as f:
        if out_rows:
            w = csv.DictWriter(f, fieldnames=list(out_rows[0]))
            w.writeheader()
            w.writerows(out_rows)
    with open(os.path.join(root, 'table.md'), 'w', encoding='utf-8') as f:
        f.write('\n'.join(md) + '\n')
    print(f'{len(out_rows)} measures -> {os.path.join(root, "loss.csv")}, {os.path.join(root, "table.md")}')


def upscalers(root):
    """The viewport's own upscalers against each reference: the pictures
    `upscaler_comparison` drew (renderer/calibration.rs), one per rung below
    full size and filter. Writes DIR/upscalers.md."""
    with open(os.path.join(root, 'upscalers.csv'), newline='') as f:
        rows = list(csv.DictReader(f))
    refs = {}
    table = {}
    filters = []
    for r in rows:
        ref = refs.get(r['ref']) or refs.setdefault(r['ref'], load_rgb(os.path.join(root, r['ref'])))
        mae, p95, _ = measures(load_rgb(os.path.join(root, r['file'])), ref)
        table.setdefault(r['scene'], {}).setdefault(r['rung'], {})[r['filter']] = (mae, p95)
        if r['filter'] not in filters:
            filters.append(r['filter'])
    md = ['# The viewport\'s upscalers', '',
          'Each rung below full size drawn up to its reference\'s size by the viewport\'s pass',
          '(renderer/upscale.rs), against the reference: mae, p95 of 16x16 blocks, in levels of 255.', '']
    for scene, rungs in table.items():
        md += [f'## {scene}', '', '| rung |' + ''.join(f' {f} mae | {f} p95 |' for f in filters),
               '|' + '---|' * (1 + 2 * len(filters))]
        for rung, by in rungs.items():
            md.append(f'| {rung} |' + ''.join(f' {by[f][0]:.2f} | {by[f][1]:.2f} |' if f in by else ' - | - |' for f in filters))
        md.append('')
    with open(os.path.join(root, 'upscalers.md'), 'w', encoding='utf-8') as f:
        f.write('\n'.join(md) + '\n')
    print('\n'.join(md))


def sim_table(scene, rs):
    """A step's cost by field scale, and its cost a million cells: flat if
    cost goes with cells, as the governor's prediction assumes."""
    md = [f'## {scene}', '', '| scale | grid | ms a step | ms a Mcell-step |', '|---|---|---|---|']
    for r in rs:
        cells = int(r['width']) * int(r['height'])
        ms = float(r['ms'])
        md.append(f'| {r["scale"]} | {r["width"]}x{r["height"]} | {ms:.3f} | {ms / (cells / 1e6):.3f} |')
    return md + ['']


def flicker(folder):
    frames = sorted(f for f in os.listdir(folder) if f.lower().endswith('.png'))
    if len(frames) < 2:
        sys.exit(f'{folder}: fewer than two PNGs')
    prev = load_rgb(os.path.join(folder, frames[0]))
    maes, p95s = [], []
    for name in frames[1:]:
        img = load_rgb(os.path.join(folder, name))
        mae, p95, _ = measures(img, prev)
        maes.append(mae)
        p95s.append(p95)
        print(f'{name}: mae {mae:.3f}  p95 {p95:.3f}')
        prev = img
    print(f'{len(maes)} pairs: mae mean {np.mean(maes):.3f}, p95 mean {np.mean(p95s):.3f}, p95 max {np.max(p95s):.3f}')


def summary(path):
    with open(path, newline='') as f:
        rows = list(csv.DictReader(f))
    groups = {}
    for r in rows:
        key = (r['mode'], f'{r["view_w"]}x{r["view_h"]}', 'playing' if r['playing'] == '1' else 'idle',
               'interacting' if r['interacting'] == '1' else '')
        groups.setdefault(key, []).append(r)

    def col(rs, name):
        return np.array([float(r[name]) for r in rs if r.get(name, '') != ''])

    # fps is frames over the time they took: a median interval hides
    # frames that alternate long and short. cpu is the frame's own work,
    # its waits for the surface (acquire, present) taken out; `part` is
    # the share of frames that showed a part-drawn escape picture.
    print(f'{"mode":<15}{"view":<11}{"state":<20}{"frames":>7}{"fps":>7}{"int p95":>9}'
          f'{"cpu ms":>8}{"acquire":>9}{"gpu ui":>8}{"gpu eng":>9}{"eng p95":>9}{"part":>6}')
    for key, rs in groups.items():
        interval = col(rs, 'interval_ms')
        ui, eng = col(rs, 'gpu_ui_ms'), col(rs, 'gpu_engine_ms')
        acquire = col(rs, 'acquire_ms')
        cpu = col(rs, 'cpu_ms')
        if len(acquire) == len(cpu):
            cpu = cpu - acquire - col(rs, 'present_ms')
        span = (float(rs[-1]['t_ms']) - float(rs[0]['t_ms'])) if len(rs) > 1 else 0.0
        fps = 1000.0 * (len(rs) - 1) / span if span > 0 else 0.0
        med = lambda a: f'{np.median(a):.2f}' if len(a) else '-'
        p95 = lambda a: f'{np.percentile(a, 95):.2f}' if len(a) else '-'
        part = col(rs, 'escape_partial')
        part = f'{100 * part.mean():.0f}%' if len(part) else '-'
        state = ' '.join(k for k in key[2:] if k)
        print(f'{key[0]:<15}{key[1]:<11}{state:<20}{len(rs):>7}{fps:>7.1f}{p95(interval):>9}'
              f'{med(cpu):>8}{med(acquire):>9}{med(ui):>8}{med(eng):>9}{p95(eng):>9}{part:>6}')


def main():
    if len(sys.argv) < 2 or sys.argv[1] not in ('loss', 'upscalers', 'flicker', 'summary'):
        sys.exit(__doc__)
    cmd = sys.argv[1]
    if cmd == 'loss':
        loss(sys.argv[2] if len(sys.argv) > 2 else 'output/governor')
    elif cmd == 'upscalers':
        upscalers(sys.argv[2] if len(sys.argv) > 2 else 'output/governor')
    elif len(sys.argv) < 3:
        sys.exit(__doc__)
    elif cmd == 'flicker':
        flicker(sys.argv[2])
    else:
        summary(sys.argv[2])


if __name__ == '__main__':
    main()
