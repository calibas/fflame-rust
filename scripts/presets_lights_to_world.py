"""One-shot (2026-10-19): fix the shipped solid presets' lights in the world.

Config v6 made a solid's directional lights the fractal's (fixed in its
space) rather than the camera's. A v5 preset loads with
`lights_follow_camera` on, to render as it did; the shipped presets should
show the new default instead. `ui::render_mode::lights_to_world` turns each
light's angles into the fractal's space at the preset's own camera, so the
picture is unchanged where the preset stands.

Input: the JSON a temporary test wrote (preset index -> its four lights
converted by the real code), removed after the run. It was, in
`ui::render_mode`'s tests:

    for (i, v) in presets.into_iter().enumerate() {
        let c = FractalConfig::from_json_value(v)?;      // v5: follows the camera
        if !c.solid_shading.lights_follow_camera { continue; }
        let mut after = c.clone();
        for (path, value) in lights_to_world(&c) { apply_value_detached(&mut after, ..) }
        out[i] = serde_json::to_value(after.solid_shading.lights);
    }
 This patches `assets/presets.fflame` in place:
each converted preset's lights (azimuth and elevation rounded to 1e-4
degrees; a preset with no lights stored takes the four whole), and its
version to 6. Everything else in the file is left byte for byte.

Usage: python scripts/presets_lights_to_world.py <converted.json>
"""
import io
import json
import sys

PRESETS = "assets/presets.fflame"


def main(path):
    converted = json.load(open(path, encoding="utf-8"))
    raw = io.open(PRESETS, encoding="utf-8", newline="").read()
    presets = json.loads(raw)
    assert json.dumps(presets, indent=2, ensure_ascii=False) + "\n" == raw, "the file no longer round-trips"
    for key, lights in converted.items():
        p = presets[int(key)]
        assert p["version"] == 5, (key, p["version"])
        for light in lights:
            light["azimuth"] = round(light["azimuth"], 4)
            light["elevation"] = round(light["elevation"], 4)
            light["intensity"] = round(light["intensity"], 6)
        shading = p.setdefault("solid_shading", {})
        if "lights" in shading:
            for stored, new in zip(shading["lights"], lights):
                stored["azimuth"] = new["azimuth"]
                stored["elevation"] = new["elevation"]
        else:
            shading["lights"] = lights
        p["version"] = 6
    io.open(PRESETS, "w", encoding="utf-8", newline="").write(json.dumps(presets, indent=2, ensure_ascii=False) + "\n")
    print(f"{len(converted)} presets' lights fixed in the world")


if __name__ == "__main__":
    main(sys.argv[1])
