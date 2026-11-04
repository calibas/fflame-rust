"""Rendering through the module: stills, the animation engine, a Python-
driven simulation sequence and video. Skipped without a GPU adapter.

The pictures themselves are checked against the native CLI by
tools/compare_native.py (a sweep, not a unit test: it needs the app's
release binary).
"""

import json
import os
import shutil
import tempfile

import pytest
import wgpu

import pyfflame as ff

ROOT = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "..")
CONFIGS = os.path.join(ROOT, "tests", "visual", "configs")

pytestmark = pytest.mark.skipif(wgpu.gpu.request_adapter_sync() is None, reason="no GPU adapter")


@pytest.fixture(scope="module")
def eng():
    return ff.Engine()


def lit(image):
    return sum(1 for i in range(0, len(image.rgba), 4) if any(image.rgba[i:i + 3]))


def test_a_flame_renders(eng):
    image = eng.render(ff.Config.load(os.path.join(CONFIGS, "2d", "plastic-sierpinski.fflame")), 160, 120,
                       iterations=5_000_000)
    assert (image.width, image.height) == (160, 120)
    assert len(image.rgba) == 160 * 120 * 4
    assert image.info["iterations"] >= 5_000_000
    assert image.info["iterations_per_thread"] == 1024, "the CLI's default"
    assert lit(image) > 1000


def test_a_png_carries_the_clis_metadata(eng):
    c = ff.Config.load(os.path.join(CONFIGS, "2d", "plastic-sierpinski.fflame"))
    png = eng.render(c, 64, 48, iterations=1_000_000).png()
    assert png.startswith(b"\x89PNG")
    for key in (b"tEXtConfig", b"tEXtIterations", b"tEXtIterationsPerThread", b"tEXtRenderTime"):
        assert key in png, key


def test_renders_are_reproducible(eng):
    """Same config, two engines (no shared state): the same bytes."""
    path = os.path.join(CONFIGS, "3d", "murl-smoke.fflame")
    a = eng.render(path, 96, 72).rgba
    b = ff.Engine().render(path, 96, 72).rgba
    assert a == b


def test_escape_and_simulation_render(eng):
    for path in ("escape/burning-ship.fflame", "sim/gray-scott-coral.fflame"):
        assert lit(eng.render(os.path.join(CONFIGS, path), 96, 72)) > 1000, path


def test_a_sequence_runs_a_simulation_on(eng):
    """A sequence frame continues the last one's field: stepping to 2N
    from a frame at N is the field a still at 2N draws from the seed."""
    cfg = json.load(open(os.path.join(CONFIGS, "sim", "gray-scott-coral.fflame"), encoding="utf-8"))
    n = 300
    cfg["sim"]["steps"] = n
    eng.render(cfg, 96, 72, sequence=True)
    cfg["sim"]["steps"] = 2 * n
    continued = eng.render(cfg, 96, 72, sequence=True).rgba
    still = ff.Engine().render(cfg, 96, 72).rgba
    assert continued == still


ANIMATED = """
script("Anim", "generator");
let t = flame.add_transform();
t.add_variation("julian", 1.0);
t.set_variation_param("julian.power", 2.0);
t.weight = 1.0;
anim.name = "Spin";
anim.duration = 8;
anim.key("rotation", 0.0, 0.0);
anim.key("rotation", 8.0, 3.0);
t.key("julian.power", 0.0, 2.0);
t.key("julian.power", 8.0, 6.0);
"""


def test_the_animation_engine_renders_a_scripts_animation(eng):
    r = ff.run_script(ANIMATED, seed=1)
    with eng.open_animation(r.animation, 64, 48, fps=2, iterations=1_000_000) as frames:
        assert len(frames) == 16, "8 s at 2 fps"
        assert frames.config(0).rotation == 0.0
        assert abs(frames.config(8).rotation - 1.5) < 1e-6, "halfway along the track"
        assert frames.config(8).get_variation_params(0)["julian.power"] == pytest.approx(4.0)
        first, later = frames.frame(0), frames.frame(8)
        assert lit(first) > 50 and later.rgba != first.rgba


@pytest.mark.skipif(shutil.which("ffmpeg") is None, reason="no FFmpeg")
def test_frames_make_a_video(eng):
    c = ff.Config.load(os.path.join(CONFIGS, "2d", "plastic-sierpinski.fflame"))
    with tempfile.TemporaryDirectory() as d:
        path = os.path.join(d, "v.mp4")
        with ff.VideoWriter(path, 64, 48, fps=10) as video:
            for n in range(5):
                c.rotation = n * 0.1
                video.write(eng.render(c, 64, 48, iterations=500_000, sequence=True))
        assert video.frames == 5
        assert os.path.getsize(path) > 0
