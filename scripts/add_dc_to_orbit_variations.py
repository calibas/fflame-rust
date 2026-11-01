#!/usr/bin/env python3
"""One-shot codemod: Direct Color for Iterated Dynamics' orbit maps as Rhai
variations (assets/formulas/variation-*.rhai; Chip was done by hand
first). Each gains the Color choice and its cycles, size and offset, a
helper that turns the step into a palette index, and ends its variation
with `#{ z: w, color }`. The flows' step lengths (Pickover Popcorn, the
dynamic system, Rossler) were then measured by hand against their own
units, which this does not do.

    python scripts/add_dc_to_orbit_variations.py
"""

import re
import sys

# file, helper prefix, the size one palette cycle spans by default (the
# picture's span at the defaults), the default Color, 3D
FILES = [
    ('hopalong', 'hopalong', '4.0', 1, False),
    ('martin', 'martin', '16.0', 1, False),
    ('quadruptwo', 'quadruptwo', '90.0', 1, False),
    ('threeply', 'threeply', '1400.0', 1, False),
    ('gingerbreadman', 'gingerbreadman', '11.0', 1, False),
    ('latoocarfian', 'latoocarfian', '4.0', 0, False),
    ('kamtorus', 'kamtorus', '2.0', 0, False),
    ('pickover-popcorn', 'popcorn', '22.0', 0, False),
    ('dynamic-system', 'dynamic', '14.0', 0, False),
    ('pickover-attractor', 'pickover', '5.0', 0, True),
    ('rossler-attractor', 'rossler', '30.0', 0, True),
]

HEADER = """//
// It colours as a direct-colour variation: each point's palette index
// follows its step (the Color choice), and the transform's Direct Color
// sets how much of that shows (0 gives back the transform's colour).
"""

PARAMS_2D = """    color: #{{ value: {mode}, caption: "Color", choices: ["Direction", "Step length", "Distance", "Angle"], hint: "What the palette index follows: the step's direction, its length, the new point's distance from the origin, or its angle about it" }},
    color_cycles: #{{ value: 1.0, min: 0.0, max: 10.0, caption: "Color Cycles", hint: "How many times the palette repeats: per turn (Direction, Angle), or per Color Size (Step length, Distance)" }},
    color_size: #{{ value: {size}, min: 0.0, caption: "Color Size", hint: "The length one cycle of the palette spans, for Step length and Distance (the picture spans about {span} at the defaults)" }},
    color_offset: #{{ value: 0.0, min: 0.0, max: 1.0, caption: "Color Offset", hint: "Added to the palette index" }},
"""

PARAMS_3D = """    color: #{{ value: {mode}, caption: "Color", choices: ["Direction", "Step length", "Distance", "Height"], hint: "What the palette index follows: the step's direction (its turn about the z axis), its length, the new point's distance from the origin, or its height" }},
    color_cycles: #{{ value: 1.0, min: 0.0, max: 10.0, caption: "Color Cycles", hint: "How many times the palette repeats: per turn (Direction), or per Color Size (Step length, Distance, Height)" }},
    color_size: #{{ value: {size}, min: 0.0, caption: "Color Size", hint: "The length one cycle of the palette spans, for Step length, Distance and Height (the attractor spans about {span} at the defaults)" }},
    color_offset: #{{ value: 0.0, min: 0.0, max: 1.0, caption: "Color Offset", hint: "Added to the palette index" }},
"""

HELPER_2D = """// The palette index for the step from z to w, as the Color choice says.
fn {prefix}_color(z, w, mode, cycles, size, offset) {{
    let d = w - z;
    let q = if mode == 0 {{
        atan2(d.im, d.re) / (2.0 * PI()) + 0.5
    }} else if mode == 1 {{
        d.abs() / size
    }} else if mode == 2 {{
        w.abs() / size
    }} else {{
        atan2(w.im, w.re) / (2.0 * PI()) + 0.5
    }};
    let k = offset + cycles * q;
    k - floor(k)
}}

"""

HELPER_3D = """// The palette index for the step from v to w, as the Color choice says.
fn {prefix}_color(v, w, mode, cycles, size, offset) {{
    let d = w - v;
    let q = if mode == 0 {{
        atan2(d.y, d.x) / (2.0 * PI()) + 0.5
    }} else if mode == 1 {{
        length(d) / size
    }} else if mode == 2 {{
        length(w) / size
    }} else {{
        w.z / size
    }};
    let k = offset + cycles * q;
    k - floor(k)
}}

"""


def span_text(size):
    n = int(float(size)) if float(size) >= 1 else float(size)
    return f'{n:,}'


def convert(file, prefix, size, mode, three_d):
    path = f'assets/formulas/variation-{file}.rhai'
    with open(path, encoding='utf-8', newline='') as f:
        text = f.read()
    if 'color_cycles' in text:
        return f'{path}: already done'
    nl = '\r\n' if '\r\n' in text else '\n'
    text = text.replace('\r\n', '\n')

    # The header comment's last paragraph.
    lines = text.split('\n')
    end = next(i for i, line in enumerate(lines) if not line.startswith('//'))
    lines[end:end] = HEADER.rstrip('\n').split('\n')
    text = '\n'.join(lines)

    # The parameters: appended to PARAMS, or a PARAMS of their own.
    block = (PARAMS_3D if three_d else PARAMS_2D).format(mode=mode, size=size, span=span_text(size))
    if 'const PARAMS = #{' in text:
        start = text.index('const PARAMS = #{')
        close = text.index('\n};', start)
        text = text[:close + 1] + block + text[close + 1:]
    else:
        at = text.index('const INFO')
        text = text[:at] + 'const PARAMS = #{\n' + block + '};\n\n' + text[at:]

    # The helper, before the variation; the variation ends with its colour.
    fn = 'fn variation3(v, p)' if three_d else 'fn variation(z, p)'
    at = text.index(fn)
    text = text[:at] + (HELPER_3D if three_d else HELPER_2D).format(prefix=prefix) + text[at:]
    body_open = text.index('{', text.index(fn)) + 1
    body_close = text.rstrip().rindex('}')
    body = text[body_open:body_close]
    split = max(body.rfind(';\n'), -1)
    head, last = (body[:split + 2], body[split + 2:]) if split >= 0 else ('\n', body.lstrip('\n'))
    expr = last.strip()
    point = 'v' if three_d else 'z'
    args = f'{point}, w, p.color, p.color_cycles, p.color_size, p.color_offset'
    new_last = f'    let w = {expr};\n    #{{ z: w, color: {prefix}_color({args}) }}\n'
    text = text[:body_open] + head + new_last + text[body_close:]

    with open(path, 'w', encoding='utf-8', newline='') as f:
        f.write(text.replace('\n', nl))
    return f'{path}: done'


if __name__ == '__main__':
    for spec in FILES:
        print(convert(*spec))
    sys.exit(0)
