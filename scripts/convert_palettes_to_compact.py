#!/usr/bin/env python3
"""
Convert palette pack JSON files from verbose stop format to compact indexed_colors format.

The compact format uses a comma-separated hex string for 256-color palettes:
  {"name": "fire", "indexed_colors": "FF0000,FF1100,..."}

Gradient palettes with few stops are kept in the original stops format:
  {"name": "simple", "stops": [{"position": 0.0, "color": [1,0,0]}, ...]}

Usage:
  python convert_palettes_to_compact.py input.json [output.json]

If output is not specified, prints to stdout.
"""

import json
import sys
from pathlib import Path


def is_indexed_256(stops: list) -> bool:
    """Check if palette has exactly 256 stops at evenly-spaced positions."""
    if len(stops) != 256:
        return False

    for i, stop in enumerate(stops):
        expected_pos = i / 255.0
        if abs(stop["position"] - expected_pos) > 0.001:
            return False

    return True


def stops_to_hex_string(stops: list) -> str:
    """Convert stops to compact hex string format."""
    hex_colors = []
    for stop in stops:
        r = int(round(stop["color"][0] * 255))
        g = int(round(stop["color"][1] * 255))
        b = int(round(stop["color"][2] * 255))
        hex_colors.append(f"{r:02X}{g:02X}{b:02X}")
    return ",".join(hex_colors)


def convert_palette(palette: dict) -> dict:
    """Convert a single palette to compact format if applicable."""
    if "stops" not in palette:
        # Already in compact format or invalid
        return palette

    stops = palette["stops"]

    if is_indexed_256(stops):
        # Convert to compact format
        result = {
            "name": palette["name"],
            "indexed_colors": stops_to_hex_string(stops)
        }
        if palette.get("locked", False):
            result["locked"] = True
        return result
    else:
        # Keep as gradient format (few stops)
        return palette


def convert_pack(pack: dict) -> dict:
    """Convert all palettes in a pack to compact format where applicable."""
    converted_palettes = [convert_palette(p) for p in pack["palettes"]]

    return {
        "pack_name": pack["pack_name"],
        "description": pack["description"],
        "enabled_by_default": pack.get("enabled_by_default", False),
        "palettes": converted_palettes
    }


def main():
    if len(sys.argv) < 2:
        print(__doc__)
        sys.exit(1)

    input_path = Path(sys.argv[1])
    output_path = Path(sys.argv[2]) if len(sys.argv) > 2 else None

    # Load input
    with open(input_path, 'r', encoding='utf-8') as f:
        pack = json.load(f)

    # Count stats before conversion
    total = len(pack["palettes"])
    indexed_count = sum(1 for p in pack["palettes"]
                       if "stops" in p and is_indexed_256(p["stops"]))

    # Convert
    converted = convert_pack(pack)

    # Output
    output_json = json.dumps(converted, indent=2)

    if output_path:
        with open(output_path, 'w', encoding='utf-8') as f:
            f.write(output_json)
        print(f"Converted {input_path} -> {output_path}")
        print(f"  {total} palettes total")
        print(f"  {indexed_count} converted to compact format")
        print(f"  {total - indexed_count} kept as gradient format")

        # Size comparison
        original_size = input_path.stat().st_size
        output_size = output_path.stat().st_size
        reduction = (1 - output_size / original_size) * 100
        print(f"  Size: {original_size:,} -> {output_size:,} bytes ({reduction:.1f}% reduction)")
    else:
        print(output_json)


if __name__ == "__main__":
    main()
