#!/usr/bin/env python3
"""Write the quality governor's reference animations (docs/projects/quality-governor.md, phase 1).

    python scripts/governor_scenes.py [OUT_DIR]     # default output/governor/scenes

One `.anim` a scene, each a visual-suite config with tracks that keep it
moving for the whole of a looped playback, for `governor_playback.py` to
play in the app with the frame log on. The configs are the repository's,
so the scenes can be made again from any checkout.
"""
import json
import os
import sys

DURATION = 12.0
VIS = 'tests/visual/configs'


def load(path):
    with open(path, encoding='utf-8') as f:
        d = json.load(f)
    return d[0] if isinstance(d, list) else d


def track(target, a, b, interpolation='Linear'):
    return {
        'target': target,
        'flame_target': 'Main',
        'source': {'type': 'Keyframes', 'keyframes': [
            {'time': 0.0, 'value': a, 'easing': 'Linear'},
            {'time': DURATION, 'value': b, 'easing': 'Linear'},
        ]},
        'interpolation': interpolation,
    }


def escape(config, **fields):
    config.setdefault('escape', {}).update(fields)
    return config


def scenes():
    """(name, config, tracks)"""
    seahorse = escape(load(f'{VIS}/escape/mandelbrot-seahorse-zoom.fflame'), supersample=4)
    yield 'escape-zoom', seahorse, [track('Escape.ZoomLog2', 7.0, 9.0), track('Escape.Rotation', 0.0, 0.5)]

    ducks = load(f'{VIS}/escape/ducks-julia.fflame')
    yield 'escape-sweep', ducks, [track('Escape.JuliaRe', 0.10, 0.20), track('Escape.JuliaIm', -0.62, -0.55)]

    solid = escape(load(f'{VIS}/escape/ifs-solid-tetrahedron-sky.fflame'), solid_tier='lit')
    yield 'escape-solid', solid, [track('Escape.CamYaw', 2.6, 3.6)]

    terrain = load(f'{VIS}/escape/terrain-seahorse-lit.fflame')
    yield 'escape-terrain', terrain, [track('Escape.CamYaw', 0.0, 0.6)]

    flame2d = load(f'{VIS}/2d/plastic-sierpinski.fflame')
    yield 'flame-2d', flame2d, [track('Rotation', 0.0, 1.0)]

    flame3d = load(f'{VIS}/3d/bubble-3d.fflame')
    yield 'flame-3d', flame3d, [track('CameraRotationY', 0.0, 1.0)]

    fixed = load(f'{VIS}/sim/gray-scott-coral.fflame')
    yield 'sim-fixed', fixed, [track('Sim.Steps', 0.0, 4000.0)]

    bound = load(f'{VIS}/sim/gray-scott-coral.fflame')
    bound['sim']['grid'] = {'viewport': {'scale': 1.0}}
    yield 'sim-bound', bound, [track('Sim.Steps', 0.0, 4000.0)]


def main():
    out = sys.argv[1] if len(sys.argv) > 1 else 'output/governor/scenes'
    os.makedirs(out, exist_ok=True)
    for name, config, tracks in scenes():
        anim = {
            'name': name,
            'base_config': config,
            'duration': DURATION,
            'tracks': tracks,
            'generators': [],
            'loop_mode': 'Loop',
        }
        path = os.path.join(out, f'{name}.anim')
        with open(path, 'w', encoding='utf-8') as f:
            json.dump(anim, f, indent=1)
        print(path)


if __name__ == '__main__':
    main()
