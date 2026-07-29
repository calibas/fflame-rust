"""Tests for pyfflame.

Run with `pytest`, or directly: `python tests/test_pyfflame.py`.

The point of these is that the Python path and the app agree. Anything
that could drift between them — file formats, script determinism,
parameter coercion — is checked against a fixed expectation rather than
against itself.
"""

import json
import os
import tempfile

import pyfflame as ff

HERE = os.path.dirname(os.path.abspath(__file__))
SCRIPTS = os.path.join(HERE, "..", "..", "assets", "scripts", "generators")


def _script(name):
    with open(os.path.join(SCRIPTS, name), encoding="utf-8") as fh:
        return fh.read()


def test_registry_is_loaded():
    names = ff.variations()
    assert len(names) > 500, "the whole registry should be visible"
    assert "linear" in names
    assert ff.variation_params("julian") == ["power", "dist"]


def test_build_a_flame():
    c = ff.Config()
    assert c.transform_count == 0

    i = c.add_transform()
    c.set_weight(i, 0.5)
    c.set_affine(i, 0.5, 0.0, 0.0, 0.5, 0.25, -0.1)
    c.add_variation(i, "julian", 1.0)
    c.set_variation_param(i, "julian.power", 3.0)

    # The flame model is f32 (WGSL has no f64), so a Python float is
    # stored rounded: -0.1 comes back as -0.10000000149011612. Compare
    # with tolerance, and don't expect round-tripping to be exact.
    assert all(abs(a - b) < 1e-6 for a, b in
               zip(c.get_affine(i), (0.5, 0.0, 0.0, 0.5, 0.25, -0.1)))
    assert c.get_variations(i) == {"julian": 1.0}
    assert c.get_variation_params(i) == {"julian.power": 3.0}


def test_typos_are_reported_not_swallowed():
    """A misspelled variation would otherwise render as a silently
    missing one, which is near-impossible to spot in an image."""
    c = ff.Config()
    i = c.add_transform()

    for call, expected, exc in [
        (lambda: c.add_variation(i, "linnear", 1.0), "unknown variation", ValueError),
        (lambda: c.set_variation_param(i, "julian.powr", 1.0), "no parameter", ValueError),
        (lambda: c.set_variation_param(i, "julian", 1.0), "variation.param", ValueError),
        (lambda: c.get_weight(99), "out of range", IndexError),
    ]:
        try:
            call()
        except exc as e:
            assert expected in str(e), f"{expected!r} not in {e}"
        else:
            raise AssertionError(f"expected {exc.__name__}")


def test_fflame_round_trip():
    c = ff.Config()
    i = c.add_transform()
    c.add_variation(i, "spherical", 1.0)
    c.name = "Round Trip"
    c.render_mode = "3d"
    c.zoom = 1.75
    c.preserve_z = True

    with tempfile.TemporaryDirectory() as d:
        path = os.path.join(d, "t.fflame")
        c.save(path)
        back = ff.Config.load(path)

    assert back.name == "Round Trip"
    assert back.render_mode == "3d"
    assert back.zoom == 1.75
    assert back.preserve_z is True
    assert back.get_variations(0) == {"spherical": 1.0}
    # The file the app would read, byte for byte.
    assert json.loads(back.to_json()) == json.loads(c.to_json())


def test_flame_xml_round_trip():
    c = ff.Config()
    i = c.add_transform()
    c.set_weight(i, 0.75)
    c.add_variation(i, "spherical", 1.0)

    with tempfile.TemporaryDirectory() as d:
        path = os.path.join(d, "t.flame")
        c.save_flame_xml(path)
        flames = ff.Config.load_flame_xml(path)

    # A .flame may hold several flames, so this is always a list.
    assert len(flames) == 1
    assert flames[0].get_variations(0) == {"spherical": 1.0}
    assert abs(flames[0].get_weight(0) - 0.75) < 1e-6


def test_script_is_deterministic_and_seed_dependent():
    src = _script("basic_random.rhai")
    a = ff.run_script(src, seed=7).config.to_json()
    b = ff.run_script(src, seed=7).config.to_json()
    c = ff.run_script(src, seed=8).config.to_json()

    assert json.loads(a) == json.loads(b), "same seed must reproduce the flame"
    assert json.loads(a) != json.loads(c), "a different seed must not"


def test_script_messages_are_captured():
    result = ff.run_script(_script("basic_random.rhai"), seed=7)
    assert result.messages, "print() output should reach Python"
    assert result.warnings == []


def test_choice_params_accept_a_name_or_an_index():
    """Coercion follows what the script DECLARES, matching the app's
    `generate --set`. Guessing from the Python type instead would pass a
    choice through as an int, which the script quietly ignores in favour
    of its default — a failure that looks like a broken script."""
    src = _script("lsystem.rhai")
    common = dict(axiom="X", rule_1="X=-YF+XFX+FY-", rule_2="Y=+XF-YFY-FX+", angle=90.0)

    by_name = ff.run_script(src, seed=3, params=dict(common, output="Path (finite depth)")).config
    by_index = ff.run_script(src, seed=3, params=dict(common, output=1)).config
    assert json.loads(by_name.to_json()) == json.loads(by_index.to_json())

    # One transform carries the whole finite-depth curve.
    assert by_name.transform_count == 1
    assert by_name.get_variation_params(0)["lsystem_path.iterations"] > 0

    # Index 0 is the other option, so it must build something different.
    attractor = ff.run_script(src, seed=3, params=dict(common, output=0)).config
    assert attractor.transform_count > 1


def test_bad_choice_names_the_options():
    src = _script("lsystem.rhai")
    try:
        ff.run_script(src, seed=3, params={"output": "Pathh"})
    except ValueError as e:
        assert "expects one of" in str(e)
        assert "Path (finite depth)" in str(e)
    else:
        raise AssertionError("expected ValueError")


def test_script_errors_carry_a_line_number():
    try:
        ff.run_script('script("Broken", "generator");\nlet x = ;', seed=1)
    except ValueError as e:
        assert "line" in str(e).lower(), f"no position in {e!r}"
    else:
        raise AssertionError("expected ValueError")


if __name__ == "__main__":
    failures = 0
    for name, fn in sorted(globals().items()):
        if name.startswith("test_") and callable(fn):
            try:
                fn()
                print(f"ok   {name}")
            except Exception as exc:  # noqa: BLE001 - report and continue
                failures += 1
                print(f"FAIL {name}: {exc}")
    raise SystemExit(1 if failures else 0)
