#!/usr/bin/env python3
"""One-shot codemod: Iterated Dynamics' orbit maps as Rhai variations
(assets/formulas/variation-*.rhai) set their flame's orbit length with an
`orbit_length` parameter (`ORBIT_LENGTH_PARAM`), saved with the flame in
place of the device's Iterations per Thread; the Iteration colour runs
over it, and Color Period goes.

    python scripts/orbit_length_for_orbit_variations.py
"""

import re
import sys

FILES = ['hopalong', 'martin', 'chip', 'quadruptwo', 'threeply', 'gingerbreadman', 'latoocarfian',
         'kamtorus', 'pickover-popcorn', 'dynamic-system', 'pickover-attractor', 'rossler-attractor']

LENGTH = ('    orbit_length: #{ value: 1024, min: 16, max: 10000, caption: "Orbit Length", hint: "How many '
          'iterations each orbit runs from a fresh start: how far it goes. Saved with the flame, in place of '
          'Iterations per Thread" },\n')

HEADER_OLD = ("// (`resets_orbits` in INFO), so each orbit is Iterations per Thread\n"
              "// long: that setting is how far an orbit goes, and Max Iterations how\n"
              "// many are drawn.")
HEADER_NEW = ("// (`resets_orbits` in INFO), each its Orbit Length long: how far an\n"
              "// orbit goes, saved with the flame in place of Iterations per Thread.\n"
              "// Max Iterations is how many are drawn.")


def convert(name):
    path = f'assets/formulas/variation-{name}.rhai'
    with open(path, encoding='utf-8', newline='') as f:
        text = f.read()
    if 'orbit_length' in text:
        return f'{path}: already done'
    nl = '\r\n' if '\r\n' in text else '\n'
    text = text.replace('\r\n', '\n')

    if HEADER_OLD in text:
        text = text.replace(HEADER_OLD, HEADER_NEW)
    else:
        # The dynamic system says it in its own words.
        old = "each Iterations per Thread long (`resets_orbits` in INFO)"
        assert old in text, path
        text = text.replace(old, "each its Orbit Length long (`resets_orbits` in INFO)")

    # The length, after the map's own parameters; no Color Period.
    text, n = re.subn(r'    color_period: #\{[^\n]*\n', '', text)
    assert n == 1, path
    at = text.index('    color: #{')
    text = text[:at] + LENGTH + text[at:]
    text = text.replace('or per Color Period (Iteration)"', 'or per orbit (Iteration)"')

    # The Iteration colour: the count over the orbit's length.
    text, n = re.subn(r'this\.n / p\.color_period', 'this.n / p.orbit_length.to_float()', text)
    assert n == 1, path

    with open(path, 'w', encoding='utf-8', newline='') as f:
        f.write(text.replace('\n', nl))
    return f'{path}: done'


if __name__ == '__main__':
    for name in FILES:
        print(convert(name))
    sys.exit(0)
