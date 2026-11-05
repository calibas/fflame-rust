"""pyfflame: the Fractal Art Editor's engine in Python.

The engine -- flames, escape-time fractals, simulations, terrains, the
animation engine and Rhai scripting -- is one WebAssembly module (WASI,
built from `wasm/python`), run by wasmtime; its GPU work is done by
wgpu-py. The same module runs on every platform those two do.

Build, edit and convert flames, and run scripts -- no GPU needed::

    import pyfflame as ff
    c = ff.Config()
    i = c.add_transform()
    c.add_variation(i, "julian", 1.0)
    c.set_variation_param(i, "julian.power", 3.0)
    c.save("mine.fflame")
    r = ff.run_script(open("lsystem.rhai").read(), seed=3)

Render, animate, and write video::

    eng = ff.Engine()
    eng.render(c, 1280, 720).save("mine.png")    # the CLI's PNG, metadata and all
    with ff.VideoWriter("spin.mp4", 1280, 720, fps=30) as video:
        for n in range(90):
            c.rotation = n / 90 * 6.2832
            video.write(eng.render(c, 1280, 720, sequence=True))
    with eng.open_animation(r.animation, 1280, 720, fps=30) as frames:
        for image in frames:                     # the video export's frames
            ...
"""

import copy
import json
import os
import warnings

import wasmtime

from ._module import EngineError, Module, find_module
from .video import VideoWriter

__version__ = "0.5.0"

__all__ = [
    "Config", "Animation", "ScriptResult", "run_script", "variations", "variation_params",
    "Engine", "Image", "Frames", "VideoWriter", "EngineError", "find_module",
]


def _text(x):
    """JSON text from a Config, Animation, dict, JSON text, or a path."""
    if isinstance(x, (Config, Animation)):
        return x.to_json()
    if isinstance(x, (dict, list)):
        return json.dumps(x)
    if isinstance(x, (bytes, bytearray)):
        return x.decode()
    if isinstance(x, str) and not x.lstrip().startswith(("{", "[")) and os.path.exists(x):
        with open(x, encoding="utf-8") as f:
            return f.read()
    return x


def _write(path, text):
    # As it is: no newline translation, so a file is the app's byte for byte.
    with open(path, "w", encoding="utf-8", newline="") as f:
        f.write(text)


def _read(path):
    with open(path, encoding="utf-8") as f:
        return f.read()


# --------------------------------------------------------------- engine

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
        return f"<pyfflame.Image {self.width}x{self.height}, {self.info['iterations']:,} iterations, {self.info['ms']:.0f} ms>"


class Frames:
    """An animation opened for rendering: its frames as the video export
    renders them -- one renderer, a simulation's run carried from frame
    to frame. Render in order; going back costs a simulation a re-run."""

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
        """Frame `n`'s config, not rendered."""
        return Config.from_json(self._config_text(n))

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


class Engine:
    """The engine module. Loading it compiles the module (~0.4 s); the GPU
    starts on the first render (~1 s)."""

    def __init__(self, module=None):
        self._path = find_module(module)
        self._module = Module(self._path)
        self._variations = None
        self._defaults = None

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

    def info(self):
        return self._data({"fn": "info"})

    def presets(self):
        """The bundled presets, as (name, Config) pairs."""
        return [(p["name"], Config.from_json(p["config"])) for p in self._data({"fn": "presets"})]

    def variations(self):
        """{name: [parameter names]} in registration order."""
        if self._variations is None:
            self._variations = {v["name"]: v["params"] for v in self._data({"fn": "variations"})}
        return self._variations

    def defaults(self):
        if self._defaults is None:
            self._defaults = self._data({"fn": "defaults"})
        return self._defaults

    # --- rendering -------------------------------------------------------

    def render(self, config, width, height, *, iterations=None, iterations_per_thread=None,
               transparent=False, premultiplied=False, sticky=False, sequence=False):
        """A config (Config, dict, JSON, or a path) at `width` x `height`.

        `iterations`: the flame's budget (default: the config's own).
        `iterations_per_thread`: default the CLI's, 1024. `sequence`: this
        frame continues the last sequence frame -- a simulation runs on to
        this frame's step count rather than from its seed. `sticky`: fewer
        shader compiles across a batch of different flames, at the price
        of the CLI's exact picture.
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
        """An animation (Animation, `.anim` JSON or path) to render, over
        its base config: `config` if given, else the one it embeds."""
        request = {"fn": "animation_open", "animation": _text(animation), "width": width, "height": height,
                   "fps": fps, "transparent": transparent, "sticky": sticky}
        if config is not None:
            request["config"] = _text(config)
        if iterations:
            request["iterations"] = int(iterations)
        if iterations_per_thread:
            request["iterations_per_thread"] = int(iterations_per_thread)
        return Frames(self, self._data(request))

    # --- scripts ---------------------------------------------------------

    def scripts(self):
        """The bundled script library: id, name, kind, summary, params."""
        return self._data({"fn": "scripts"})

    def script_source(self, script_id):
        return self._data({"fn": "script_source", "id": script_id})

    def script_params(self, source):
        """A script's name, kind, flags and declared parameters."""
        return self._data({"fn": "script_params", "source": source})

    def run_script(self, source, seed=1, params=None, base=None):
        """Run a Rhai script (its source). See the module's `run_script`."""
        return self.run_scripts([(source, params)], seed, base)

    def run_scripts(self, stages, seed=1, base=None):
        """A generator and modifiers in one run, one seed: `stages` is a
        list of sources or (source, params) pairs."""
        stages = [s if isinstance(s, tuple) else (s, None) for s in stages]
        request = {"fn": "script_run", "seed": int(seed) & (2 ** 64 - 1),
                   "stages": [{"source": src, "params": params} for src, params in stages]}
        if base is not None:
            request["base"] = _text(base)
        return ScriptResult(self._data(request))


_default = None


def _engine():
    """The engine the module-level calls and Config's checks use."""
    global _default
    if _default is None:
        _default = Engine()
    return _default


# --------------------------------------------------------------- configs

def _blank_transform():
    return copy.deepcopy(_engine().defaults()["new_transform"])


class Config:
    """A complete flame: the model plus its camera, colour and render
    settings -- what a `.fflame` file holds. `data` is that file's JSON,
    as a dict, for anything the properties below do not name."""

    def __init__(self, data=None):
        if data is None:
            data = json.loads(_engine()._data({"fn": "default_config"}))
        self.data = data

    # --- files -----------------------------------------------------------

    @staticmethod
    def load(path):
        """Read a `.fflame` file."""
        return Config.from_json(_read(path))

    @staticmethod
    def from_json(text):
        """A `.fflame` from a JSON string. Checked: a config the engine
        cannot read is refused here, not at the render."""
        return Config(json.loads(_engine()._data({"fn": "config_normalize", "config": text})))

    @staticmethod
    def load_flame_xml(path):
        """An Apophysis/JWildfire `.flame` file. One file may hold several
        flames, so this always returns a list."""
        return Config.parse_flame_xml(_read(path))

    @staticmethod
    def parse_flame_xml(xml):
        return [Config(json.loads(c)) for c in _engine()._data({"fn": "flame_import", "xml": xml})]

    def to_json(self):
        """The `.fflame` JSON, exactly as the app writes it."""
        return _engine()._data({"fn": "config_normalize", "config": json.dumps(self.data)})

    def save(self, path):
        _write(path, self.to_json())

    def to_flame_xml(self):
        return _engine()._data({"fn": "flame_export", "config": json.dumps(self.data)})

    def save_flame_xml(self, path):
        _write(path, self.to_flame_xml())

    def copy(self):
        return Config(copy.deepcopy(self.data))

    # --- flame-level properties -----------------------------------------

    def _get(self, key):
        # An absent field reads as the engine reads it (`defaults`).
        if key in self.data:
            return self.data[key]
        return _engine().defaults()["config"][key]

    @property
    def name(self):
        return self.data["flame"].get("name", "")

    @name.setter
    def name(self, value):
        self.data["flame"]["name"] = value

    @property
    def render_mode(self):
        """"2d", "3d", "escape" or "simulation"."""
        return self._get("render_mode")

    @render_mode.setter
    def render_mode(self, value):
        mode = str(value).lower()
        if mode not in ("2d", "3d", "escape", "simulation"):
            raise ValueError(f'render_mode must be "2d", "3d", "escape" or "simulation", got {value!r}')
        self.data["render_mode"] = mode

    def set(self, key, value):
        """Set a setting by name -- the file's (`"sim.terrain.height"`) or
        its track's (`"Sim.Terrain.Height"`) -- by the app's rules, as a
        script's `config.set` sets it: the panels' clamps, and what a
        change implies (a simulation model starts at its defaults). A
        field no panel edits as one value is written as given. The flame
        is refused: use the transform calls."""
        out = _engine()._data({"fn": "config_set", "config": json.dumps(self.data), "key": key, "value": value})
        self.data = json.loads(out["config"])
        if not out["changed"]:
            warnings.warn(f"Config.set({key!r}, ...) changed nothing -- check the setting name", stacklevel=2)

    def _prop(key, doc=""):
        def get(self):
            return self._get(key)

        def put(self, value):
            self.set(key, value)
        return property(get, put, doc=doc)

    zoom = _prop("zoom")
    pan_x = _prop("pan_x")
    pan_y = _prop("pan_y")
    rotation = _prop("rotation", "2D rotation, radians")
    camera_pitch = _prop("camera_rotation_x", "3D camera pitch")
    camera_yaw = _prop("camera_rotation_y", "3D camera yaw")
    perspective_strength = _prop("perspective_strength")
    preserve_z = _prop("preserve_z")
    max_iterations = _prop("max_iterations")
    del _prop

    # --- transforms ------------------------------------------------------

    @property
    def _transforms(self):
        return self.data["flame"].setdefault("transforms", [])

    def _xform(self, index):
        ts = self._transforms
        if not 0 <= index < len(ts):
            raise IndexError(f"transform {index} out of range (flame has {len(ts)})")
        return ts[index]

    @property
    def transform_count(self):
        return len(self._transforms)

    def add_transform(self):
        """Append a transform (the app's new transform) and return its index."""
        self._transforms.append(_blank_transform())
        return len(self._transforms) - 1

    def remove_transform(self, index):
        self._xform(index)
        del self._transforms[index]

    def _field(self, index, key):
        t = self._xform(index)
        return t[key] if key in t else _engine().defaults()["transform"][key]

    def get_weight(self, index):
        return self._field(index, "weight")

    def set_weight(self, index, value):
        self._xform(index)["weight"] = value

    def get_color(self, index):
        return self._field(index, "color")

    def set_color(self, index, value):
        self._xform(index)["color"] = value

    def get_color_speed(self, index):
        return self._field(index, "color_speed")

    def set_color_speed(self, index, value):
        self._xform(index)["color_speed"] = value

    def get_opacity(self, index):
        return self._field(index, "opacity")

    def set_opacity(self, index, value):
        self._xform(index)["opacity"] = value

    def get_affine(self, index):
        """(a, b, c, d, e, f): x' = a*x + b*y + e, y' = c*x + d*y + f."""
        return tuple(self._field(index, k) for k in "abcdef")

    def set_affine(self, index, a, b, c, d, e, f):
        t = self._xform(index)
        t.update(a=a, b=b, c=c, d=d, e=e, f=f)

    # --- variations ------------------------------------------------------

    def get_variations(self, index):
        """{name: weight} for the transform's variations."""
        return dict(self._field(index, "variations"))

    def add_variation(self, index, name, weight):
        """Add (or re-weight) a variation. Checked against the registry: an
        unknown name is a typo that would render as a missing variation."""
        if name not in _engine().variations():
            raise ValueError(f"unknown variation {name!r} (see pyfflame.variations())")
        self._xform(index).setdefault("variations", {})[name] = weight

    def remove_variation(self, index, name):
        self._xform(index).get("variations", {}).pop(name, None)

    def set_variation_param(self, index, key, value):
        """A variation parameter, keyed "variation.param" as `.fflame`
        files store it."""
        var, dot, param = key.partition(".")
        if not dot:
            raise ValueError(f'parameter key must be "variation.param", got {key!r}')
        known = _engine().variations()
        if var not in known:
            raise ValueError(f"unknown variation {var!r}")
        if param not in known[var]:
            raise ValueError(f"variation {var!r} has no parameter {param!r}; it has: {', '.join(known[var])}")
        self._xform(index).setdefault("variation_params", {})[key] = value

    def get_variation_params(self, index):
        return dict(self._field(index, "variation_params"))

    def __repr__(self):
        return f"<pyfflame.Config {self.name!r} {self.transform_count} transform(s) {self.render_mode}>"


class Animation:
    """An animation (a `.anim`): a duration and tracks, and the flame it
    was made with. Render it with `Engine.open_animation`."""

    def __init__(self, text):
        self._text = text
        self.data = json.loads(text)

    @staticmethod
    def load(path):
        """Read a `.anim` file."""
        return Animation(_read(path))

    @staticmethod
    def from_json(text):
        return Animation(text)

    def to_json(self):
        return self._text

    def save(self, path):
        _write(path, self._text)

    @property
    def name(self):
        return self.data.get("name", "")

    @property
    def duration(self):
        """Seconds."""
        return float(self.data.get("duration", 0.0))

    @property
    def targets(self):
        """The setting each track animates, e.g. "Rotation" or
        "Transform.0.VariationParam.julian.power"."""
        return [t["target"] for t in self.data.get("tracks", [])]

    @property
    def config(self):
        """The flame the animation was made with, carried inside it so the
        `.anim` stands alone; None if it carries none."""
        base = self.data.get("base_config")
        return Config.from_json(json.dumps(base)) if base is not None else None

    def __repr__(self):
        return f"<pyfflame.Animation {self.name!r} {self.duration:.3f}s {len(self.targets)} track(s)>"


class ScriptResult:
    """What a script made: `config` (a Config) and `config_text` (its
    exact JSON -- the bytes the CLI's `generate` writes for the same
    script, seed and parameters), `animation` (an Animation, if the
    script keyed `anim`; else None), and the `messages` (`print()`) and
    `warnings` of every stage."""

    def __init__(self, result):
        self.config_text = result["config"]
        self.config = Config(json.loads(self.config_text))
        self.animation = Animation(result["animation"]) if result["animation"] else None
        self.stages = result["stages"]
        self.messages = [m for s in self.stages for m in s["messages"]]
        self.warnings = [w for s in self.stages for w in s["warnings"]]

    def __repr__(self):
        return f"<pyfflame.ScriptResult {len(self.messages)} message(s) {len(self.warnings)} warning(s)>"


# --------------------------------------------------------------- the module's calls

def run_script(source, seed=1, params=None, base=None):
    """Run a Rhai flame script -- the app's sandboxed engine and seeded RNG,
    so (script, seed, params) gives the app's flame, byte for byte.

    `params` sets the script's declared parameters by name (a choice by
    its text or index, a colour as "#rrggbb" or an (r, g, b) triple); an
    undeclared name is an error. `base` is the flame a modifier starts
    from. `seed` is taken modulo 2**64: -1 is the last seed. Errors
    (ValueError) lead with the script's line.
    """
    return _engine().run_script(source, seed, params, base)


def variations():
    """Every variation's name, in registration order."""
    return list(_engine().variations())


def variation_params(name):
    """A variation's parameter names (without the "name." prefix)."""
    known = _engine().variations()
    if name not in known:
        raise ValueError(f"unknown variation {name!r}")
    return list(known[name])
