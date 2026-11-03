"""Rewrite the McCabe twin's PARAMS with grids (rhai-plugins P7): the
per-scale table's 42 cells become the `table` grid and the per-scale
warps' 30 the `warps` grid (`drawn: false`: the Warp section draws them),
every caption, hint, range and default carried over; the ladder's own
parameters show only under the ladder.

One-shot, run from the repository root on the flat twin
(assets/sim/mccabe.rhai as P4 step 9c wrote it):
    python scripts/mccabe_twin_grids.py
"""

import re
import sys

PATH = "assets/sim/mccabe.rhai"
COLUMNS = [
    ("radius", "Radius", False, None),
    ("ratio", "Ratio", False, None),
    ("amount", "Step", False, None),
    ("weight", "Weight", False, None),
    ("symmetry", "Sym", True, None),
    ("stretch", "Stretch", False, "averaging: 2"),
    ("angle", "Angle", False, "averaging: 2"),
]
WARP_COLUMNS = ["zoom", "rotation", "pan_x", "pan_y", "flow"]
LADDER = ("base_radius", "ratio", "amount", "amount_min", "symmetry")
SETTINGS = re.compile(
    r'\s*value: ([^,]+), (?:min: ([^,]+), max: ([^,]+), )?(?:choices: (\[.*?\]), )?caption: "([^"]*)",\n\s*hint: "((?:[^"\\]|\\.)*)",',
    re.S,
)


def main():
    s = open(PATH, encoding="utf-8").read()
    start = s.index("const PARAMS = #{\n")
    end = s.index("\n};\n", start)
    body = s[start + len("const PARAMS = #{\n"):end]
    blocks = re.findall(r"    (\w+): #\{\n(.*?)\n    \},", body + "\n", re.S)
    info = {}
    for name, block in blocks:
        m = SETTINGS.match(block)
        if not m:
            sys.exit(f"{name}: settings not read")
        info[name] = dict(value=m.group(1), min=m.group(2), max=m.group(3), caption=m.group(5), hint=m.group(6))

    def column(key, cell, header=None, whole=False, show=None):
        first = info[cell.format(r=0)]
        caption = first["caption"].replace("Scale 1 ", "", 1)
        values = [info[cell.format(r=r)]["value"] for r in range(6)]
        for r in range(1, 6):
            o = info[cell.format(r=r)]
            if (o["min"], o["max"], o["hint"], o["caption"].split(" ", 2)[2]) != (first["min"], first["max"], first["hint"], caption):
                sys.exit(f"{cell.format(r=r)}: unlike its column")
        value = values[0] if len(set(values)) == 1 else "[" + ", ".join(values) + "]"
        parts = [f"value: {value}", f"min: {first['min']}", f"max: {first['max']}", f'caption: "{caption}"']
        if header:
            parts.append(f'header: "{header}"')
        if whole:
            parts.append("whole: true")
        if show:
            parts.append(f"show_if: #{{ {show} }}")
        parts.append(f'hint: "{first["hint"]}"')
        return f"            {key}: #{{ " + ", ".join(parts) + " },"

    table = "\n".join(
        [
            "    table: #{",
            '        type: "grid",',
            "        rows: 6,",
            '        live_rows: "scales",',
            '        cell: "s{row}_{column}",',
            '        row: "Scale {n}",',
            "        show_if: #{ layout: 1 },",
            '        fill: #{ when: #{ layout: 1 }, from: "ladder" },',
            '        warn: "past_reach",',
            "        columns: #{",
            *[column(k, "s{r}_" + k, h, w, sh) for k, h, w, sh in COLUMNS],
            "        },",
            "    },",
        ]
    )
    warps = "\n".join(
        [
            "    warps: #{",
            '        type: "grid",',
            "        rows: 6,",
            '        live_rows: "scales",',
            '        cell: "s{row}_warp_{column}",',
            '        row: "Scale {n}",',
            "        drawn: false,",
            "        columns: #{",
            *[column(k, "s{r}_warp_" + k) for k in WARP_COLUMNS],
            "        },",
            "    },",
        ]
    )
    cells = {f"s{r}_{k}" for r in range(6) for k, *_ in COLUMNS} | {f"s{r}_warp_{k}" for r in range(6) for k in WARP_COLUMNS}
    out, placed = [], set()
    for name, block in blocks:
        if name in cells:
            grid = "warps" if "_warp_" in name else "table"
            if grid not in placed:
                out.append(warps if grid == "warps" else table)
                placed.add(grid)
            continue
        if name in LADDER:
            block = block.replace("\n        hint:", " show_if: #{ layout: 0 },\n        hint:", 1)
        out.append(f"    {name}: #{{\n{block}\n    }},")
    if placed != {"table", "warps"}:
        sys.exit("the cells were not found")
    s = s[:start] + "const PARAMS = #{\n" + "\n".join(out) + "\n};\n" + s[end + len("\n};\n"):]
    open(PATH, "w", encoding="utf-8", newline="\n").write(s)
    print(f"{PATH}: {len(blocks)} parameters, {len(cells)} of them now grid cells")


if __name__ == "__main__":
    main()
