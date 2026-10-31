#!/usr/bin/env python3
"""Play the quality governor's reference animations in the app, logging every frame
(docs/projects/quality-governor.md, phase 1).

    python scripts/governor_scenes.py                   # once: write the scenes
    python scripts/governor_playback.py [--seconds N] [--only a,b] [--warm] [--tag T] [--render-scale S]

Each scene in output/governor/scenes/ is opened in the release app with
the frame log on (FFLAME_FRAME_LOG, FFLAME_FRAME_LOG_OPEN,
FFLAME_FRAME_LOG_SECONDS; src/app/frame_log.rs), played for N seconds
(default 20) and closed; the logs go to output/governor/playback[-T]/.
`--warm` first runs each scene for a few seconds and throws the log
away: the first run after a rebuild pays the driver's cold shader
compiles. The app opens a window each run: leave the machine alone
while it plays, or the times are someone else's.

It does not build. Run `cargo build --release` first; when the app is
open, the release binary is locked and its copy in target/release/deps
is run instead.
"""
import argparse
import csv
import os
import subprocess
import sys

sys.path.insert(0, os.path.dirname(__file__))
import frame_metrics  # noqa: E402

SCENES = 'output/governor/scenes'
EXPECTED_MODE = {'escape': 'escape', 'flame-2d': '2d', 'flame-3d': '3d', 'sim': 'sim'}


def binary():
    for p in ('target/release/FractalArtEditor.exe', 'target/release/FractalArtEditor',
              'target/release/deps/FractalArtEditor.exe'):
        if os.path.exists(p):
            return os.path.abspath(p)  # Windows' CreateProcess does not take a relative path
    sys.exit('no release binary: cargo build --release')


def run(exe, scene, log, seconds, render_scale=None, target_fps=None):
    env = dict(os.environ, FFLAME_FRAME_LOG=log, FFLAME_FRAME_LOG_OPEN=scene,
               FFLAME_FRAME_LOG_SECONDS=str(seconds))
    if render_scale is not None:
        # The run's render scale, in place of the saved setting (untouched).
        env['FFLAME_FRAME_LOG_RENDER_SCALE'] = str(render_scale)
    if target_fps is not None:
        # The run's Target Frame Rate ('display' or fps), likewise.
        env['FFLAME_FRAME_LOG_TARGET_FPS'] = str(target_fps)
    try:
        subprocess.run([exe], env=env, timeout=seconds + 120, check=False,
                       stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    except subprocess.TimeoutExpired:
        print(f'  {os.path.basename(scene)}: did not quit within {seconds + 120} s')


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument('--seconds', type=float, default=20.0)
    ap.add_argument('--only', default='')
    ap.add_argument('--warm', action='store_true')
    ap.add_argument('--tag', default='')
    ap.add_argument('--render-scale', type=float, default=None,
                    help='render at this scale for the run (the saved setting is not touched)')
    ap.add_argument('--target-fps', default=None,
                    help="hold this frame rate for the run, 'display' or fps (the saved setting is not touched)")
    ap.add_argument('--exe', default=None,
                    help='the binary to run (e.g. target/release/deps/FractalArtEditor.exe, built while a '
                         'running app locks target/release/)')
    args = ap.parse_args()
    if not os.path.isdir(SCENES):
        sys.exit(f'{SCENES}: run scripts/governor_scenes.py first')
    only = [n for n in args.only.split(',') if n]
    scenes = sorted(f for f in os.listdir(SCENES) if f.endswith('.anim')
                    and (not only or f[:-5] in only))
    out = 'output/governor/playback' + (f'-{args.tag}' if args.tag else '')
    os.makedirs(out, exist_ok=True)
    exe = os.path.abspath(args.exe) if args.exe else binary()
    print(f'{exe}, {len(scenes)} scenes, {args.seconds:.0f} s each -> {out}/')
    if args.warm:
        for f in scenes:
            run(exe, os.path.join(SCENES, f), os.path.join(out, 'warm.csv'), 5)
    for f in scenes:
        name = f[:-5]
        log = os.path.join(out, f'{name}.csv')
        print(f'\n== {name}')
        run(exe, os.path.join(SCENES, f), log, args.seconds, args.render_scale, args.target_fps)
        if not os.path.exists(log):
            print('  no log written')
            continue
        frame_metrics.summary(log)
        want = next((m for k, m in EXPECTED_MODE.items() if name.startswith(k)), None)
        with open(log, newline='') as fh:
            modes = {r['mode'] for r in csv.DictReader(fh)}
        if want and not any(m.startswith(want) for m in modes):
            print(f'  WARNING: expected a {want} mode, logged {sorted(modes)}: the scene did not open')


if __name__ == '__main__':
    main()
