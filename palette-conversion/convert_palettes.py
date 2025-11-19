import re
import json
import sys

PALETTE_HEADER_RE = re.compile(r"//\s*(\d+)\s+([A-Za-z0-9\-\_]+)")
RGB_TUPLE_RE = re.compile(r"\((\d+),\s*(\d+),\s*(\d+)\)")


def parse_palettes(text):
    lines = text.splitlines()

    palettes = []
    current_name = None
    rgb_values = []

    collecting = False

    for line in lines:
        # Detect palette header line (example: "// 0 south-sea-bather")
        header_match = PALETTE_HEADER_RE.search(line)
        if header_match:
            # If previously collecting, save previous palette
            if current_name and rgb_values:
                palettes.append((current_name, rgb_values))
                rgb_values = []

            current_name = header_match.group(2)
            collecting = True
            continue

        # If in a palette block, extract RGB tuples
        if collecting:
            for r, g, b in RGB_TUPLE_RE.findall(line):
                rgb_values.append((int(r), int(g), int(b)))

            # Palette ends when we hit ")," (closing bracket of the block)
            if line.strip().endswith("),") or line.strip().endswith(")"):
                # BUT only end if we already have 256 tuples
                if len(rgb_values) >= 256:
                    palettes.append((current_name, rgb_values[:256]))
                    rgb_values = []
                    collecting = False

    # If file ends without trailing comma
    if current_name and rgb_values:
        palettes.append((current_name, rgb_values[:256]))

    return palettes


def make_json(palettes):
    json_list = []

    for name, rgb_list in palettes:
        if len(rgb_list) != 256:
            print(f"Warning: palette '{name}' has {len(rgb_list)} values (expected 256). Skipping.")
            continue

        stops = []
        for i, (r, g, b) in enumerate(rgb_list):
            stops.append({
                "position": i / 255.0,
                "color": [r / 255.0, g / 255.0, b / 255.0]
            })

        json_list.append({
            "name": name,
            "stops": stops
        })

    return json_list


def main():
    if len(sys.argv) != 3:
        print("Usage: python convert_palettes.py input.txt output.json")
        return

    input_path = sys.argv[1]
    output_path = sys.argv[2]

    with open(input_path, "r", encoding="utf-8") as f:
        text = f.read()

    palettes = parse_palettes(text)
    json_data = make_json(palettes)

    with open(output_path, "w", encoding="utf-8") as f:
        json.dump(json_data, f, indent=2)

    print(f"Wrote {len(json_data)} palettes to {output_path}")


if __name__ == "__main__":
    main()
