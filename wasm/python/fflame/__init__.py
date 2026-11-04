"""fflame: the Fractal Art Editor's engine in Python (SPIKE, branch
python-wasm-spike).

The engine -- flames, escape-time fractals, simulations, the animation
engine and Rhai scripting -- is one WebAssembly module (WASI), run by
wasmtime; its GPU work is done by wgpu-py. Nothing here is compiled
for a platform.

A config is a dict: a `.fflame` file's JSON. Change it like any dict
and render it::

    import fflame
    eng = fflame.Engine()
    cfg = fflame.load_config("flame.fflame")
    cfg["zoom"] *= 2
    eng.render(cfg, 1280, 720).save("flame.png")

Animate it from Python, frame by frame (`sequence=True` carries a
simulation's run from one frame to the next, as a video does)::

    with fflame.VideoWriter("out.mp4", 1280, 720, fps=30) as video:
        for i in range(90):
            cfg["rotation"] = i / 90 * 6.2832
            video.write(eng.render(cfg, 1280, 720, sequence=True))

Or let the animation engine do it, from a `.anim` file or the animation
a Rhai script defines::

    result = eng.run_script(open("turntable.rhai").read(), base=cfg)
    with eng.open_animation(result.animation, 1280, 720, fps=30) as anim:
        for image in anim:
            ...
"""

import json
import os

import wasmtime

from ._module import EngineError, Module, find_module
from .video import VideoWriter

__all__ = [
    "Engine", "Image", "Animation", "ScriptResult", "EngineError", "VideoWriter",
    "load_config", "save_config", "find_module",
]


def load_config(path):
    """A `.fflame` file as a dict."""
    with open(path, encoding="utf-8") as f:
        return json.load(f)


def save_config(config, path):
    """A config (dict or JSON) as a `.fflame` file. The text is written
    as it is (no newline translation), so a script's `config_text` makes
    the CLI's file byte for byte."""
    with open(path, "w", encoding="utf-8", newline="") as f:
        f.write(_text(config))


def _text(x):
    """JSON text from a dict, JSON text, or a path to a JSON file."""
    if isinstance(x, (dict, list)):
        return json.dumps(x)
    if isinstance(x, (bytes, bytearray)):
        return x.decode()
    if isinstance(x, str) and not x.lstrip().startswith(("{", "[")) and os.path.exists(x):
        with open(x, encoding="utf-8") as f:
            return f.read()
    return x


class Image:
    """A rendered frame: RGBA bytes, `width` x `height`, and its numbers
    (`info`: iterations, ms, iterations_per_thread)."""

    def __init__(self, engine, rgba, info, config):
        self._engine = engine
        self._config = config  # JSON text, or a callable giving it
        self.rgba = rgba
        self.info = info
        self.width = info["width"]
        self.height = info["height"]

    def png(self):
        """The PNG the CLI writes for this frame, its config and numbers
        in the metadata (so it reloads in the app)."""
        config = self._config() if callable(self._config) else self._config
        out, _ = self._engine._call({
            "fn": "encode_png", "config": config, "width": self.width, "height": self.height,
            "iterations": self.info["iterations"], "ms": self.info["ms"],
            "iterations_per_thread": self.info["iterations_per_thread"],
        }, self.rgba)
        return out

    def save(self, path):
        with open(path, "wb") as f:
            f.write(self.png())

    def to_numpy(self):
        """An (height, width, 4) uint8 array."""
        import numpy
        return numpy.frombuffer(self.rgba, numpy.uint8).reshape(self.height, self.width, 4)

    def to_pil(self):
        from PIL import Image as PILImage
        return PILImage.frombytes("RGBA", (self.width, self.height), self.rgba)

    def __repr__(self):
        return f"<fflame.Image {self.width}x{self.height}, {self.info['iterations']:,} iterations, {self.info['ms']:.0f} ms>"


class Animation:
    """An animation, opened: its frames as the video export renders them
    -- one renderer, a simulation's run carried from frame to frame.
    Render frames in order; going back costs a simulation a re-run."""

    def __init__(self, engine, info):
        self._engine = engine
        self.id = info["id"]
        self.frames = info["frames"]
        self.fps = info["fps"]
        self.duration = info["duration"]
        self.warnings = info["warnings"]

    def __len__(self):
        return self.frames

    def frame(self, n):
        out, info = self._engine._call({"fn": "animation_frame", "id": self.id, "frame": n})
        return Image(self._engine, out, info, lambda: self._config_text(n))

    def _config_text(self, n):
        return self._engine._data({"fn": "animation_config", "id": self.id, "frame": n})

    def config(self, n):
        """Frame `n`'s config (a dict), not rendered."""
        return json.loads(self._config_text(n))

    def __iter__(self):
        for n in range(self.frames):
            yield self.frame(n)

    def close(self):
        if self.id is not None:
            self._engine._call({"fn": "animation_close", "id": self.id})
            self.id = None

    def __enter__(self):
        return self

    def __exit__(self, *exc):
        self.close()


class ScriptResult:
    """What a script run made: `config` (a dict) and `config_text` (its
    exact JSON: the bytes the CLI's `generate` writes for the same script,
    seed and parameters), `animation` (the `.anim` JSON text a script that
    keyed `anim` defined, else None), and each stage's `messages` and
    `warnings`."""

    def __init__(self, result):
        self.config_text = result["config"]
        self.config = json.loads(self.config_text)
        self.animation = result["animation"]
        self.stages = result["stages"]
        self.messages = [m for s in self.stages for m in s["messages"]]
        self.warnings = [w for s in self.stages for w in s["warnings"]]


class Engine:
    """The engine. The GPU starts on the first render."""

    def __init__(self, module=None):
        self._path = find_module(module)
        self._module = Module(self._path)

    def _call(self, request, data=b""):
        try:
            return self._module.call(request, data)
        except wasmtime.WasmtimeError as e:
            # A panic traps the instance and leaves it unusable: start a
            # new one (its open animations are gone with the old).
            self._module = Module(self._path)
            raise EngineError(f"the engine panicked in `{request.get('fn')}` (see stderr); it was restarted") from e

    def _data(self, request, data=b""):
        return self._call(request, data)[1]["result"]

    # --- configs ---------------------------------------------------------

    def info(self):
        return self._data({"fn": "info"})

    def default_config(self):
        return json.loads(self._data({"fn": "default_config"}))

    def presets(self):
        """The bundled presets, as (name, config) pairs."""
        return [(p["name"], json.loads(p["config"])) for p in self._data({"fn": "presets"})]

    def import_flame(self, xml):
        """An Apophysis/JWildfire `.flame` (text or path): its configs."""
        return [json.loads(c) for c in self._data({"fn": "flame_import", "xml": _text(xml)})]

    def export_flame(self, config):
        """A config as `.flame` XML."""
        return self._data({"fn": "flame_export", "config": _text(config)})

    # --- rendering -------------------------------------------------------

    def render(self, config, width, height, *, iterations=None, iterations_per_thread=None,
               transparent=False, premultiplied=False, sticky=False, sequence=False):
        """A config (dict, JSON, or a path) at `width` x `height`.

        `iterations`: the flame's budget (default: the config's own).
        `sequence`: this frame continues the last sequence frame -- a
        simulation runs on to this frame's step count rather than from
        its seed. `sticky`: fewer shader compiles across a batch of
        different flames, at the price of the CLI's exact picture.
        """
        text = _text(config)
        request = {"fn": "render", "config": text, "width": width, "height": height,
                   "transparent": transparent, "premultiplied": premultiplied,
                   "sticky": sticky, "sequence": sequence}
        if iterations:
            request["iterations"] = int(iterations)
        if iterations_per_thread:
            request["iterations_per_thread"] = int(iterations_per_thread)
        out, info = self._call(request)
        return Image(self, out, info, text)

    def open_animation(self, animation, width, height, fps=30, config=None, *,
                       iterations=None, iterations_per_thread=None, transparent=False, sticky=False):
        """An animation (`.anim` dict, JSON or path) over its base config:
        `config` if given, else the one the animation embeds."""
        request = {"fn": "animation_open", "animation": _text(animation), "width": width, "height": height,
                   "fps": fps, "transparent": transparent, "sticky": sticky}
        if config is not None:
            request["config"] = _text(config)
        if iterations:
            request["iterations"] = int(iterations)
        if iterations_per_thread:
            request["iterations_per_thread"] = int(iterations_per_thread)
        return Animation(self, self._data(request))

    # --- scripts ---------------------------------------------------------

    def scripts(self):
        """The bundled script library: id, name, kind, summary, params."""
        return self._data({"fn": "scripts"})

    def script_source(self, script_id):
        return self._data({"fn": "script_source", "id": script_id})

    def script_params(self, source):
        """A script's name, kind, flags and declared parameters."""
        return self._data({"fn": "script_params", "source": source})

    def run_script(self, source, seed=0, params=None, base=None):
        """Run a Rhai script (its source text). A generator starts from the
        default config; a modifier from `base`. Same script and seed, same
        config, on every platform."""
        return self.run_scripts([(source, params)], seed, base)

    def run_scripts(self, stages, seed=0, base=None):
        """A generator and modifiers in one run, one seed: `stages` is a
        list of sources or (source, params) pairs."""
        stages = [s if isinstance(s, tuple) else (s, None) for s in stages]
        request = {"fn": "script_run", "seed": int(seed),
                   "stages": [{"source": src, "params": params} for src, params in stages]}
        if base is not None:
            request["base"] = _text(base)
        return ScriptResult(self._data(request))
