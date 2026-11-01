#!/usr/bin/env python3
"""Iterated Dynamics' orbit maps as escape-time formulas,
assets/formulas/orbits-*.rhai: each pixel starts an orbit of the map,
and the colouring reads it over a Size (chosen by eye per map, a point
trap at Scale 5 the measure), so a colouring's settings carry from map
to map. Each map's step is its flame variation's (`variation-*.rhai`;
`escape::iterated_dynamics` holds the two to each other).

    python scripts/gen_orbit_formulas.py            # the Sizes below
    python scripts/gen_orbit_formulas.py '{"chip": "110.0"}'   # to try others
"""
import json
import sys
import textwrap

NEVER = ("Its orbits never leave, so this is not the usual escape-time picture: "
         "the colouring reads where each pixel's orbit goes -- Orbit Trap (a point "
         "trap at Scale 5 rings the islands), the averages -- and the Radius only "
         "stops one that runs off.")
ATTRACTOR = ("Its orbits never leave: they fall onto its {onto}, so this is not the "
             "usual escape-time picture: the colouring reads how each pixel's orbit "
             "gets there -- Orbit Trap, the averages -- and the Radius only stops one "
             "that runs off.")
CHAOTIC = ("Its orbits never leave: every start falls onto the same chaotic "
           "attractor, so what the colouring reads is mostly where on it the passes "
           "leave each orbit, and the picture is grainy, with structure only where "
           "orbits take long to arrive. The Radius only stops one that runs off.")
KAM = ("Orbits outside its tori run off, so there this is an escape-time picture, "
       "the count; inside, the colouring reads where each pixel's orbit goes -- "
       "Orbit Trap, the averages.")

SIGN = """// Fractint's sign: 0 at 0.
fn {p}_sign(x) {{
    if x > 0.0 {{ 1.0 }} else if x < 0.0 {{ -1.0 }} else {{ 0.0 }}
}}

"""

# file, title, map lines, source fn, variation file, params, step body,
# behaviour, (centre, height), radius, helpers prefix, 3D
MAPS = [
    dict(file='hopalong', title='Hopalong', fn='hopalong2d_orbit',
         maps=["x' = y - sign(x) sqrt|b x - c|,", "y' = a - x,"],
         note='where sign(0) is 0',
         params=[('a', '0.4', 'a', "The constant y hops to, less x"),
                 ('b', '1.0', 'b', "The slope inside the square root"),
                 ('c', '0.0', 'c', "The offset inside the square root")],
         step="""    let x = s.z.re;
    s.z = complex(s.z.im - hopalong_sign(x) * sqrt((p.b * x - p.c).abs()), p.a - x);""",
         text=NEVER, view=((0.5, 0.5), 4.0), radius='1000.0', sign='hopalong'),
    dict(file='martin', title='Martin', fn='martin2d_orbit',
         maps=["x' = y - sin(x),   y' = a - x"], note=None,
         params=[('a', '3.14', 'a', "The constant y hops to, less x: near pi the orbits weave a lattice")],
         step="""    let x = s.z.re;
    s.z = complex(s.z.im - sin(x), p.a - x);""",
         text=NEVER, view=((1.6, 1.6), 16.0), radius='1000.0', sign=None),
    dict(file='chip', title='Chip', fn='chip2d_orbit',
         maps=["x' = y - sign(x) cos(sqr(ln|b x - c|)) atan(sqr(ln|c x - b|)),", "y' = a - x,"],
         note='where sign(0) is 0',
         params=[('a', '-15.0', 'a', "The constant y hops to, less x"),
                 ('b', '-19.0', 'b', "The first logarithm's slope, and the second's offset"),
                 ('c', '1.0', 'c', "The first logarithm's offset, and the second's slope")],
         step="""    let x = s.z.re;
    let u = ln((p.b * x - p.c).abs());
    let v = ln((p.c * x - p.b).abs());
    s.z = complex(s.z.im - chip_sign(x) * cos(u * u) * atan(v * v), p.a - x);""",
         text=NEVER, view=((-7.5, -7.5), 221.0), radius='10000.0', sign='chip'),
    dict(file='quadruptwo', title='Quadruptwo', fn='quadrup_two2d_orbit',
         maps=["x' = y - sign(x) sin(ln|b x - c|) atan(sqr(ln|c x - b|)),", "y' = a - x,"],
         note='where sign(0) is 0',
         params=[('a', '34.0', 'a', "The constant y hops to, less x"),
                 ('b', '1.0', 'b', "The first logarithm's slope, and the second's offset"),
                 ('c', '5.0', 'c', "The first logarithm's offset, and the second's slope")],
         step="""    let x = s.z.re;
    let v = ln((p.c * x - p.b).abs());
    s.z = complex(s.z.im - quadruptwo_sign(x) * sin(ln((p.b * x - p.c).abs())) * atan(v * v), p.a - x);""",
         text=NEVER, view=((17.0, 17.0), 88.0), radius='10000.0', sign='quadruptwo'),
    dict(file='threeply', title='Threeply', fn='three_ply2d_orbit',
         maps=["x' = y - sign(x) |sin(x) cos(b) + c - x sin(a + b + c)|,", "y' = a - x,"],
         note='where sign(0) is 0',
         params=[('a', '-55.0', 'a', "The constant y hops to, less x"),
                 ('b', '-1.0', 'b', "The angle whose cosine weighs sin(x)"),
                 ('c', '-42.0', 'c', "The offset inside the absolute value")],
         step="""    let x = s.z.re;
    let k = (sin(x) * cos(p.b) + p.c - x * sin(p.a + p.b + p.c)).abs();
    s.z = complex(s.z.im - threeply_sign(x) * k, p.a - x);""",
         text=NEVER, view=((-35.0, -17.5), 1390.0), radius='100000.0', sign='threeply'),
    dict(file='gingerbreadman', title='Gingerbread Man', fn='ginger_bread_orbit',
         maps=["x' = 1 - y + |x|,   y' = x"], note=None, params=[],
         step="""    s.z = complex(1.0 - s.z.im + s.z.re.abs(), s.z.re);""",
         text=NEVER, view=((2.5, 2.5), 11.0), radius='1000.0', sign=None),
    dict(file='latoocarfian', title='Latoocarfian', fn='latoo_orbit',
         maps=["x' = sin(b y) + c sin(b x),", "y' = sin(a x) + d sin(a y)"], note=None,
         params=[('a', '-0.966918', 'a', "The frequency of y's sines"),
                 ('b', '2.879879', 'b', "The frequency of x's sines"),
                 ('c', '0.765145', 'c', "How much sin(b x) adds to x"),
                 ('d', '0.744728', 'd', "How much sin(a y) adds to y")],
         step="""    let x = s.z.re;
    let y = s.z.im;
    s.z = complex(sin(y * p.b) + p.c * sin(x * p.b), sin(x * p.a) + p.d * sin(y * p.a));""",
         text=CHAOTIC, view=((0.0, 0.0), 3.0), radius='1000.0', sign=None),
    dict(file='kamtorus', title='KAM Torus', fn='kam_torus_orbit',
         maps=["(x, y) -> (x, y - x^2) turned by Angle"], note=None,
         params=[('angle', '1.3', 'Angle', "The turn each pass, in radians (Fractint's first parameter)")],
         step="""    let x = s.z.re;
    let t = s.z.im - x * x;
    let sa = sin(p.angle);
    let ca = cos(p.angle);
    s.z = complex(x * ca - t * sa, x * sa + t * ca);""",
         text=KAM, view=((0.0, 0.0), 1.5), radius='1000.0', sign=None),
    dict(file='pickover-popcorn', title='Pickover Popcorn', fn='popcorn_fractal (the generalized step, with sin and tan)',
         maps=["x' = x - h sin(y + tan(C y)),", "y' = y - h sin(x + tan(C x))"], note=None,
         params=[('h', '0.05', 'Step', "The step h each pass takes (Fractint's real step size)"),
                 ('c', '3.0', 'Constant', "The frequency inside the tangent (Fractint's real constant C)")],
         step="""    let x = s.z.re;
    let y = s.z.im;
    s.z = complex(x - p.h * sin(y + tan(p.c * y)), y - p.h * sin(x + tan(p.c * x)));""",
         text=NEVER, view=((0.0, 0.0), 4.5), radius='1000.0', sign=None),
    dict(file='dynamic-system', title='Dynamic System', fn='dynamic_orbit',
         maps=["dx/dt = -sin(y + a sin(b y)),   dy/dt = sin(x + a sin(b x)),", "stepped by dt each pass"], note=None,
         params=[('dt', '0.1', 'Time step', "How far each pass flows"),
                 ('a', '1.0', 'a', "How much the inner sine bends the flow"),
                 ('b', '3.0', 'b', "The inner sine's frequency")],
         extra='    update: #{ value: 0, caption: "Update", choices: ["Both from the old point", "y first (Euler)"], hint: "Fractint\'s two orders: both parts from the old point, or y first and x from the new y (its negative time step)" },\n',
         step="""    let x = s.z.re;
    let y = s.z.im;
    let ny = y + p.dt * sin(x + p.a * sin(p.b * x));
    let yy = if p.update == 1 { ny } else { y };
    s.z = complex(x - p.dt * sin(yy + p.a * sin(p.b * yy)), ny);""",
         text=ATTRACTOR.format(onto='loops'), view=((0.0, 0.0), 13.6), radius='1000.0', sign=None),
    dict(file='pickover-attractor', title='Pickover Attractor', fn='pickover_orbit',
         maps=["x' = sin(a y) - z cos(b x),", "y' = z sin(c x) - cos(d y),", "z' = sin(x)"], note=None,
         params=[('a', '2.24', 'a', "The frequency of y in x's sine"),
                 ('b', '0.43', 'b', "The frequency of x in x's cosine"),
                 ('c', '-0.65', 'c', "The frequency of x in y's sine"),
                 ('d', '-2.43', 'd', "The frequency of y in y's cosine"),
                 ('start_z', '0.0', 'Start z', "The third coordinate every pixel's orbit starts at")],
         step="""    let x = s.z.re;
    let y = s.z.im;
    let h = s.h;
    s.z = complex(sin(p.a * y) - h * cos(p.b * x), h * sin(p.c * x) - cos(p.d * y));
    s.h = sin(x);""",
         text=CHAOTIC + " It is a 3D map: each pixel is a start's (x, y), its z the Start z, and the colouring reads the orbit's (x, y).",
         view=((0.0, 0.0), 4.0), radius='1000.0', sign=None, three=True),
    dict(file='rossler-attractor', title='Rossler Attractor', fn='rossler_orbit',
         maps=["x' = x - (y + z) dt,", "y' = y + (x + a y) dt,", "z' = z + (b + z (x - c)) dt"], note=None,
         params=[('dt', '0.04', 'Time step', "How far each pass flows"),
                 ('a', '0.2', 'a', "How much y feeds its own growth"),
                 ('b', '0.2', 'b', "z's constant growth"),
                 ('c', '5.7', 'c', "How far x must climb before z grows"),
                 ('start_z', '0.0', 'Start z', "The third coordinate every pixel's orbit starts at")],
         step="""    let x = s.z.re;
    let y = s.z.im;
    let h = s.h;
    s.z = complex(x - (y + h) * p.dt, y + (x + p.a * y) * p.dt);
    s.h = h + (p.b + h * (x - p.c)) * p.dt;""",
         text=ATTRACTOR.format(onto='attractor') + " It is a 3D map: each pixel is a start's (x, y), its z the Start z, and the colouring reads the orbit's (x, y).",
         view=((0.0, 10.0), 60.0), radius='1000.0', sign=None, three=True),
]


# The colouring's unit for each map, by eye (a quarter, one and four times
# a quarter of the view tried, under a point trap at Scale 5); others from
# the command line as JSON.
SIZES = {
    'hopalong': '1.0', 'martin': '4.0', 'chip': '220.0', 'quadruptwo': '22.0', 'threeply': '1400.0',
    'gingerbreadman': '2.75', 'latoocarfian': '0.75', 'kamtorus': '1.5', 'pickover-popcorn': '4.5',
    'dynamic-system': '3.4', 'pickover-attractor': '1.0', 'rossler-attractor': '15.0',
}
SIZES.update(json.loads(sys.argv[1]) if len(sys.argv) > 1 else {})


def wrap(text, first='// '):
    return '\n'.join('// ' + line for line in textwrap.wrap(text, 71, break_on_hyphens=False))


for m in MAPS:
    name = m['file']
    lead = f"{m['title']}'s orbits as an escape-time picture: each pixel starts an orbit of the map"
    head = wrap(lead) + '\n' + ''.join(f'//   {line}\n' for line in m['maps'])
    tail = (f"{m['note'] + ' ' if m['note'] else ''}(Iterated Dynamics' `{m['fn']}`; the flame variation "
            f"`variation-{name}.rhai` draws the orbits themselves). {m['text']} The colouring reads the "
            f"orbit divided by Size, so its settings carry from map to map.")
    tail = tail[0].upper() + tail[1:] if not m['note'] else tail
    head += wrap(tail) + '\n'
    params = ''.join(f'    {k}: #{{ value: {v}, caption: "{cap}", hint: "{hint}" }},\n' for k, v, cap, hint in m['params'])
    params += m.get('extra', '')
    params += (f'    size: #{{ value: {SIZES[name]}, min: 0.0, caption: "Size", hint: "The length the colouring reads as 1: '
               f'the orbit, in the map\'s units, is divided by it before the colouring sees it" }},\n')
    params += (f'    radius: #{{ value: {m["radius"]}, min: 0.0, caption: "Radius", hint: "An orbit that goes this far from '
               f'the origin stops" }},\n')
    (cx, cy), height = m['view']
    magn = 4.0 / height
    info = f'const INFO = #{{ title: "{m["title"]} (Orbits)", center: complex({cx!r}, {cy!r}), magn: {magn!r}, maxiter: 200 }};\n'
    start = '#{ z: pixel / p.size, m: pixel, h: p.start_z }' if m.get('three') else '#{ z: pixel / p.size, m: pixel }'
    step = m['step'].replace('s.z', 's.m') + '\n    s.z = s.m / p.size;'
    body = (f"{head}\nconst PARAMS = #{{\n{params}}};\n\n{info}\n"
            + (SIGN.format(p=m['sign']) if m['sign'] else '')
            + "// `m` is the orbit in the map's units; `z`, what the colouring reads,\n"
            + "// is it over Size.\n"
            + f"fn start(pixel, p, ctx) {{\n    {start}\n}}\n\n"
            + f"fn step(s, p) {{\n{step}\n    s\n}}\n\n"
            + "fn escaped(s, p) {\n    s.m.norm_sqr() >= p.radius * p.radius\n}\n")
    with open(f'assets/formulas/orbits-{name}.rhai', 'w', encoding='utf-8', newline='\n') as f:
        f.write(body)
    print(name)
