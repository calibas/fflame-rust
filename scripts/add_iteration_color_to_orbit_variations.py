#!/usr/bin/env python3
"""One-shot codemod: Iteration colouring and fresh orbits for Iterated
Dynamics' orbit maps as Rhai variations (assets/formulas/variation-*.rhai).
Each gains `resets_orbits: true` in INFO (Dynamic System had it), a
paragraph saying what Iterations per Thread now does, the Iteration
Color mode with its Color Period, and a per-thread count of the orbit's
iterations (`fn init`, `this.n`) the colour reads.

    python scripts/add_iteration_color_to_orbit_variations.py
"""

import re
import sys

FILES = ['hopalong', 'martin', 'chip', 'quadruptwo', 'threeply', 'gingerbreadman', 'latoocarfian',
         'kamtorus', 'pickover-popcorn', 'dynamic-system', 'pickover-attractor', 'rossler-attractor']

ORBITS = """//
// A flame using it starts its orbits afresh every dispatch
// (`resets_orbits` in INFO), so each orbit is Iterations per Thread
// long: that setting is how far an orbit goes, and Max Iterations how
// many are drawn."""

PERIOD = ('    color_period: #{ value: 1024.0, min: 1.0, caption: "Color Period", hint: "The iterations one '
          'cycle of the palette spans, for Iteration (an orbit is Iterations per Thread long, 1,024 by default)" },\n')

INIT = """// Each thread's count of its orbit's iterations, for the Iteration colour.
fn init(p) {
    #{ n: 0.0 }
}

"""


def convert(name):
    path = f'assets/formulas/variation-{name}.rhai'
    with open(path, encoding='utf-8', newline='') as f:
        text = f.read()
    if 'color_period' in text:
        return f'{path}: already done'
    nl = '\r\n' if '\r\n' in text else '\n'
    text = text.replace('\r\n', '\n')
    three_d = 'fn variation3(v, p)' in text

    # The header: the colour follows the orbit's iteration too, and (but
    # where it is said already) what the orbit's length now is.
    text = text.replace(
        "// follows its step (the Color choice), and the transform's Direct Color\n"
        "// sets how much of that shows (0 gives back the transform's colour).",
        "// follows its step or its orbit's iteration (the Color choice), and the\n"
        "// transform's Direct Color sets how much of that shows (0 gives back\n"
        "// the transform's colour).")
    if 'resets_orbits' not in text:
        lines = text.split('\n')
        end = next(i for i, line in enumerate(lines) if not line.startswith('//'))
        lines[end:end] = ORBITS.split('\n')
        text = '\n'.join(lines)
        text = re.sub(r'(const INFO = #\{[^}]*?)( \};)', r'\1, resets_orbits: true\2', text, count=1)

    # The parameters: Iteration in the choice, its period, the hints.
    last = '"Height"]' if three_d else '"Angle"]'
    text = text.replace(last, last[:-1] + ', "Iteration"]', 1)
    text = re.sub(r'(color: #\{ value: \d, caption: "Color", choices: \[[^]]*\], hint: "[^"]*?), or (its (?:angle about it|height))"',
                  '\\1, \\2, or the orbit\'s iteration since it started"', text, count=1)
    text = re.sub(r'(color_cycles: #\{[^\n]*?hint: "[^"]*?), or per Color Size (\([^)]*\))"',
                  r'\1, per Color Size \2, or per Color Period (Iteration)"', text, count=1)
    at = text.index('    color_offset: #{')
    end = text.index('\n', at) + 1
    text = text[:end] + PERIOD + text[end:]

    # The count, kept per thread from the orbit's start.
    at = text.index('const INFO')
    at = text.index('\n', at) + 2
    text = text[:at] + INIT + text[at:]

    # The helper reads the count over the period as Iteration's.
    text = re.sub(r'(fn \w+_color\((?:z|v), w, mode, cycles, size, offset(?:, \w+)?)\) \{', r'\1, t) {', text, count=1)
    old_last = '    } else {\n        w.z / size\n    };' if three_d else '    } else {\n        atan2(w.im, w.re) / (2.0 * PI()) + 0.5\n    };'
    assert old_last in text, path
    new_last = old_last.replace('    } else {', '    } else if mode == 3 {', 1).replace('    };', '    } else {\n        t\n    };')
    text = text.replace(old_last, new_last, 1)

    # The variation counts, then colours.
    fn = 'fn variation3(v, p) {\n' if three_d else 'fn variation(z, p) {\n'
    text = text.replace(fn, fn + '    this.n = this.n + 1.0;\n', 1)
    text, n = re.subn(r'(color: \w+_color\([^\n]*?)\) \}\n', r'\1, this.n / p.color_period) }\n', text, count=1)
    assert n == 1, path

    with open(path, 'w', encoding='utf-8', newline='') as f:
        f.write(text.replace('\n', nl))
    return f'{path}: done'


if __name__ == '__main__':
    for name in FILES:
        print(convert(name))
    sys.exit(0)
