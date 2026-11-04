"""The engine's WASI module, instantiated, with a GPU host behind it.

One call crosses into it: `fflame_call`, a JSON request and a block of
bytes beside it; back come a status, an output of bytes and a JSON note
(src/lib.rs lists the calls).
"""

import json
import os

import wasmtime

from ._gpu import GpuHost

_HERE = os.path.dirname(os.path.abspath(__file__))


class EngineError(Exception):
    """A call the engine refused or failed, with its reason."""


def find_module(path=None):
    """The module's file: `path`, $FFLAME_WASM, one beside this package,
    or the spike's build output."""
    candidates = [
        path,
        os.environ.get("FFLAME_WASM"),
        os.path.join(_HERE, "fflame.wasm"),
        os.path.join(_HERE, "..", "..", "target-gallery", "wasm32-wasip1", "release", "fflame_python.wasm"),
    ]
    for c in candidates:
        if c and os.path.exists(c):
            return os.path.abspath(c)
    raise FileNotFoundError(
        "no engine module: build wasm/python for wasm32-wasip1, or set FFLAME_WASM")


# Compiling the module is most of loading it; a module made again after
# a trap reuses the compilation.
_COMPILED = {}


class Module:
    def __init__(self, path=None):
        path = find_module(path)
        if path not in _COMPILED:
            engine = wasmtime.Engine()
            _COMPILED[path] = (engine, wasmtime.Module.from_file(engine, path))
        engine, module = _COMPILED[path]
        self.store = wasmtime.Store(engine)
        wasi = wasmtime.WasiConfig()
        wasi.inherit_stdout()
        wasi.inherit_stderr()
        self.store.set_wasi(wasi)
        linker = wasmtime.Linker(engine)
        linker.define_wasi()
        self.gpu = GpuHost()
        i32 = wasmtime.ValType.i32()
        linker.define_func("fflame_gpu", "call", wasmtime.FuncType([i32] * 4, [i32]), self.gpu.call)
        linker.define_func("fflame_gpu", "take", wasmtime.FuncType([i32], []), self.gpu.take)
        instance = linker.instantiate(self.store, module)
        self.exports = instance.exports(self.store)
        self.memory = self.exports["memory"]
        self.gpu.store = self.store
        self.gpu.memory = self.memory
        if "_initialize" in self.exports:
            self.exports["_initialize"](self.store)

    def _fn(self, name, *args):
        return self.exports[name](self.store, *args)

    def _put(self, data):
        if not data:
            return 0
        ptr = self._fn("fflame_alloc", len(data)) & 0xFFFFFFFF
        self.memory.write(self.store, data, ptr)
        return ptr

    def _get(self, ptr_fn, len_fn):
        ptr = self._fn(ptr_fn) & 0xFFFFFFFF
        n = self._fn(len_fn)
        return bytes(self.memory.read(self.store, ptr, ptr + n)) if n else b""

    def call(self, request, data=b""):
        """One call: `(output bytes, note)`, or EngineError."""
        req = json.dumps(request).encode()
        rp, dp = self._put(req), self._put(data)
        try:
            status = self._fn("fflame_call", rp, len(req), dp, len(data))
        finally:
            self._fn("fflame_free", rp, len(req))
            if dp:
                self._fn("fflame_free", dp, len(data))
        out = self._get("fflame_output_ptr", "fflame_output_len")
        if status != 0:
            raise EngineError(out.decode(errors="replace"))
        info = self._get("fflame_info_ptr", "fflame_info_len")
        return out, (json.loads(info) if info else None)
