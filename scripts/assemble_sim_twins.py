"""Assemble the Rhai twins of the built-in simulation models (rhai-plugins
P4 step 7): each twin's header comment and functions, written by hand in
output/sim-twins/fn/<name>.rhai, around its generated PARAMS and INFO
(output/sim-twins/<name>.rhai, from `write_twin_skeletons`), into
assets/sim/<name>.rhai.

One-shot: the twins are kept in assets/sim/ from then on, and
`every_twin_carries_its_built_ins_settings` holds their PARAMS and INFO to
the built-ins'. Run from the repository root after the skeletons are
written:
    cargo test --release --lib write_twin_skeletons -- --ignored
    python scripts/assemble_sim_twins.py [name ...]
"""

import pathlib
import sys

SKELETONS = pathlib.Path("output/sim-twins")
FUNCTIONS = SKELETONS / "fn"
OUT = pathlib.Path("assets/sim")


def assemble(name):
    hand = (FUNCTIONS / f"{name}.rhai").read_text(encoding="utf-8")
    skeleton = (SKELETONS / f"{name}.rhai").read_text(encoding="utf-8")
    lines = hand.splitlines(keepends=True)
    split = next(i for i, line in enumerate(lines) if not line.startswith("//") and line.strip())
    header, body = "".join(lines[:split]).rstrip() + "\n\n", "".join(lines[split:])
    (OUT / f"{name}.rhai").write_text(header + skeleton + "\n" + body, encoding="utf-8", newline="\n")
    print(name)


def main():
    names = sys.argv[1:] or sorted(p.stem for p in FUNCTIONS.glob("*.rhai"))
    for name in names:
        assemble(name)
    return 0


if __name__ == "__main__":
    sys.exit(main())
