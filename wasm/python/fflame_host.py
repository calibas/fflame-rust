"""SPIKE (branch python-wasm-spike): run the engine's WASI module in
Python, its GPU calls made with wgpu-py.

The module (src/lib.rs) imports two functions, `fflame_gpu.call` and
`fflame_gpu.take`; each call is one JSON command, with a block of bytes
beside it when the command carries data. Objects are integer handles the
module allocates; `GpuHost.objs` holds the wgpu-py objects under them.
A command buffer arrives whole at submit and is replayed here on a
wgpu-py encoder.

    python wasm/python/fflame_host.py CONFIG.fflame OUT.png [WIDTH HEIGHT [ITERATIONS]]
"""

import json
import struct
import sys
import time
import zlib

import wasmtime
import wgpu

MODULE = "wasm/target-gallery/wasm32-wasip1/release/fflame_python.wasm"

# Bytes per texel, for clear_texture (which WebGPU lacks: emulated by a
# copy from a zeroed buffer).
TEXEL_BYTES = {
    "r8unorm": 1, "r16float": 2, "r32float": 4, "r32uint": 4, "r32sint": 4,
    "rg16float": 4, "rg32float": 8, "rg32uint": 8, "rgba8unorm": 4,
    "rgba8unorm-srgb": 4, "bgra8unorm": 4, "rgba16float": 8,
    "rgba32float": 16, "rgba32uint": 16,
}


def stage(s, objs):
    out = {"module": objs[s["module"]]}
    if s.get("entry_point"):
        out["entry_point"] = s["entry_point"]
    if s.get("constants"):
        out["constants"] = s["constants"]
    return out


class GpuHost:
    def __init__(self):
        self.objs = {}
        self.textures = {}  # id -> (format, size, mip levels)
        self.result = b""
        self.store = None
        self.memory = None
        self.device = None
        self.calls = 0
        self.call_seconds = 0.0

    # --- the two imports -------------------------------------------------

    def call(self, cmd_ptr, cmd_len, data_ptr, data_len):
        t0 = time.perf_counter()
        mem, store = self.memory, self.store
        cmd_ptr &= 0xFFFFFFFF
        data_ptr &= 0xFFFFFFFF
        cmd = json.loads(mem.read(store, cmd_ptr, cmd_ptr + cmd_len))
        data = bytes(mem.read(store, data_ptr, data_ptr + data_len)) if data_len else b""
        try:
            out = getattr(self, "op_" + cmd["op"])(cmd, data)
        except Exception as e:
            print(f"host: {cmd['op']} failed: {e!r}", file=sys.stderr)
            raise
        if out is None:
            self.result = b""
        elif isinstance(out, (bytes, bytearray, memoryview)):
            self.result = bytes(out)
        else:
            self.result = json.dumps(out).encode()
        self.calls += 1
        self.call_seconds += time.perf_counter() - t0
        return len(self.result)

    def take(self, dst):
        self.memory.write(self.store, self.result, dst & 0xFFFFFFFF)

    # --- the device ------------------------------------------------------

    def op_init(self, cmd, data):
        if self.device is None:
            adapter = wgpu.gpu.request_adapter_sync(power_preference="high-performance")
            features = [f for f in ("float32-filterable",) if f in adapter.features]
            wanted = (
                "max-storage-buffer-binding-size", "max-buffer-size",
                "max-storage-buffers-per-shader-stage", "max-texture-dimension-2d",
                "max-compute-workgroup-storage-size", "max-compute-invocations-per-workgroup",
                "max-compute-workgroup-size-x", "max-compute-workgroup-size-y",
                "max-compute-workgroups-per-dimension", "max-storage-textures-per-shader-stage",
                "max-sampled-textures-per-shader-stage", "max-uniform-buffer-binding-size",
            )
            limits = {k: adapter.limits[k] for k in wanted if k in adapter.limits}
            self.device = adapter.request_device_sync(required_features=features, required_limits=limits)
            self.adapter = adapter
        d = self.device
        info = {k: v for k, v in dict(self.adapter.info).items() if isinstance(v, (str, int))}
        return {"features": sorted(d.features), "limits": dict(d.limits), "info": info}

    def op_wait(self, cmd, data):
        # wgpuDevicePoll(wait = true). Private in wgpu-py; its public
        # queue.on_submitted_work_done_sync fails in 0.32 (a cffi callback
        # signature that no longer matches wgpu-native's).
        self.device._poll_wait()

    def op_destroy_device(self, cmd, data):
        self.device.destroy()

    def op_drop(self, cmd, data):
        self.objs.pop(cmd["id"], None)
        self.textures.pop(cmd["id"], None)

    def op_destroy(self, cmd, data):
        o = self.objs.get(cmd["id"])
        if o is not None:
            o.destroy()

    # --- resources -------------------------------------------------------

    def op_create_buffer(self, cmd, data):
        self.objs[cmd["id"]] = self.device.create_buffer(
            label=cmd["label"] or "", size=cmd["size"], usage=cmd["usage"],
            mapped_at_creation=cmd["mapped_at_creation"],
        )

    def op_create_texture(self, cmd, data):
        self.objs[cmd["id"]] = self.device.create_texture(
            label=cmd["label"] or "", size=tuple(cmd["size"]), mip_level_count=cmd["mip_level_count"],
            sample_count=cmd["sample_count"], dimension=cmd["dimension"], format=cmd["format"],
            usage=cmd["usage"], view_formats=cmd["view_formats"],
        )
        self.textures[cmd["id"]] = (cmd["format"], cmd["size"], cmd["mip_level_count"])

    def op_create_view(self, cmd, data):
        kw = {k: cmd[k] for k in ("format", "dimension", "mip_level_count", "array_layer_count") if cmd[k] is not None}
        self.objs[cmd["id"]] = self.objs[cmd["texture"]].create_view(
            label=cmd["label"] or "", aspect=cmd["aspect"], base_mip_level=cmd["base_mip_level"],
            base_array_layer=cmd["base_array_layer"], **kw,
        )

    def op_create_sampler(self, cmd, data):
        kw = {k: cmd[k] for k in (
            "address_mode_u", "address_mode_v", "address_mode_w", "mag_filter", "min_filter",
            "mipmap_filter", "lod_min_clamp", "lod_max_clamp", "max_anisotropy")}
        if cmd["compare"] is not None:
            kw["compare"] = cmd["compare"]
        self.objs[cmd["id"]] = self.device.create_sampler(label=cmd["label"] or "", **kw)

    def op_create_shader_module(self, cmd, data):
        self.objs[cmd["id"]] = self.device.create_shader_module(label=cmd["label"] or "", code=data.decode())

    def op_create_bind_group_layout(self, cmd, data):
        self.objs[cmd["id"]] = self.device.create_bind_group_layout(label=cmd["label"] or "", entries=cmd["entries"])

    def op_create_bind_group(self, cmd, data):
        entries = []
        for e in cmd["entries"]:
            r = e["resource"]
            if "buffer" in r:
                res = {"buffer": self.objs[r["buffer"]], "offset": r["offset"]}
                if r["size"] is not None:
                    res["size"] = r["size"]
            elif "sampler" in r:
                res = self.objs[r["sampler"]]
            else:
                res = self.objs[r["texture_view"]]
            entries.append({"binding": e["binding"], "resource": res})
        self.objs[cmd["id"]] = self.device.create_bind_group(
            label=cmd["label"] or "", layout=self.objs[cmd["layout"]], entries=entries)

    def op_create_pipeline_layout(self, cmd, data):
        layouts = [self.objs[i] for i in cmd["bind_group_layouts"]]
        self.objs[cmd["id"]] = self.device.create_pipeline_layout(label=cmd["label"] or "", bind_group_layouts=layouts)

    def op_create_compute_pipeline(self, cmd, data):
        layout = self.objs[cmd["layout"]] if cmd["layout"] is not None else "auto"
        self.objs[cmd["id"]] = self.device.create_compute_pipeline(
            label=cmd["label"] or "", layout=layout, compute=stage(cmd["compute"], self.objs))

    def op_create_render_pipeline(self, cmd, data):
        layout = self.objs[cmd["layout"]] if cmd["layout"] is not None else "auto"
        vertex = stage(cmd["vertex"], self.objs)
        vertex["buffers"] = []
        fragment = None
        if cmd["fragment"] is not None:
            fragment = stage(cmd["fragment"], self.objs)
            fragment["targets"] = [
                {k: v for k, v in t.items() if v is not None} for t in cmd["fragment"]["targets"]
            ]
        primitive = {k: v for k, v in cmd["primitive"].items() if v is not None}
        multisample = dict(cmd["multisample"])
        multisample["mask"] &= 0xFFFFFFFF  # wgpu's mask is a u64, WebGPU's a u32
        self.objs[cmd["id"]] = self.device.create_render_pipeline(
            label=cmd["label"] or "", layout=layout, vertex=vertex, primitive=primitive,
            multisample=multisample, fragment=fragment)

    def op_get_bind_group_layout(self, cmd, data):
        self.objs[cmd["id"]] = self.objs[cmd["pipeline"]].get_bind_group_layout(cmd["index"])

    def op_create_query_set(self, cmd, data):
        self.objs[cmd["id"]] = self.device.create_query_set(
            label=cmd["label"] or "", type=cmd["type"], count=cmd["count"])

    # --- the queue -------------------------------------------------------

    def op_write_buffer(self, cmd, data):
        self.device.queue.write_buffer(self.objs[cmd["buffer"]], cmd["offset"], data)

    def texel_texture(self, t):
        return {"texture": self.objs[t["texture"]], "mip_level": t["mip_level"],
                "origin": tuple(t["origin"]), "aspect": t["aspect"]}

    @staticmethod
    def layout(l):
        out = {"offset": l["offset"]}
        if l["bytes_per_row"] is not None:
            out["bytes_per_row"] = l["bytes_per_row"]
        if l["rows_per_image"] is not None:
            out["rows_per_image"] = l["rows_per_image"]
        return out

    def texel_buffer(self, b):
        out = self.layout(b)
        out["buffer"] = self.objs[b["buffer"]]
        return out

    def op_write_texture(self, cmd, data):
        layout = self.layout(cmd["layout"])
        if "bytes_per_row" not in layout:
            # wgpu leaves it out for a single row; wgpu-py wants it.
            fmt = self.textures[cmd["texture"]["texture"]][0]
            layout["bytes_per_row"] = cmd["size"][0] * TEXEL_BYTES[fmt]
        self.device.queue.write_texture(
            self.texel_texture(cmd["texture"]), data, layout, tuple(cmd["size"]))

    def op_submit(self, cmd, data):
        finished = []
        for cmds in cmd["buffers"]:
            enc = self.device.create_command_encoder()
            for c in cmds:
                getattr(self, "enc_" + c["op"])(enc, c)
            finished.append(enc.finish())
        self.device.queue.submit(finished)

    # --- maps ------------------------------------------------------------

    def op_map(self, cmd, data):
        mode = wgpu.MapMode.WRITE if cmd["write"] else wgpu.MapMode.READ
        self.objs[cmd["buffer"]].map_sync(mode, cmd["offset"], cmd["size"])

    def op_read_mapped(self, cmd, data):
        return self.objs[cmd["buffer"]].read_mapped(cmd["offset"], cmd["size"], copy=True)

    def op_write_mapped(self, cmd, data):
        self.objs[cmd["buffer"]].write_mapped(data, cmd["offset"])

    def op_unmap(self, cmd, data):
        self.objs[cmd["buffer"]].unmap()

    # --- encoder commands, replayed --------------------------------------

    def enc_copy_buffer_to_buffer(self, enc, c):
        src = self.objs[c["src"]]
        size = c["size"] if c["size"] is not None else src.size - c["src_offset"]
        enc.copy_buffer_to_buffer(src, c["src_offset"], self.objs[c["dst"]], c["dst_offset"], size)

    def enc_copy_buffer_to_texture(self, enc, c):
        enc.copy_buffer_to_texture(self.texel_buffer(c["src"]), self.texel_texture(c["dst"]), tuple(c["size"]))

    def enc_copy_texture_to_buffer(self, enc, c):
        enc.copy_texture_to_buffer(self.texel_texture(c["src"]), self.texel_buffer(c["dst"]), tuple(c["size"]))

    def enc_copy_texture_to_texture(self, enc, c):
        enc.copy_texture_to_texture(self.texel_texture(c["src"]), self.texel_texture(c["dst"]), tuple(c["size"]))

    def enc_clear_buffer(self, enc, c):
        buf = self.objs[c["buffer"]]
        size = c["size"] if c["size"] is not None else buf.size - c["offset"]
        enc.clear_buffer(buf, c["offset"], size)

    def enc_clear_texture(self, enc, c):
        fmt, (w, h, layers), mips = self.textures[c["texture"]]
        bpt = TEXEL_BYTES[fmt]
        first = c["base_mip_level"]
        count = c["mip_level_count"] or (mips - first)
        for mip in range(first, first + count):
            mw, mh = max(1, w >> mip), max(1, h >> mip)
            row = (mw * bpt + 255) // 256 * 256
            zeros = self.device.create_buffer(size=row * mh * layers, usage=wgpu.BufferUsage.COPY_SRC)
            enc.copy_buffer_to_texture(
                {"buffer": zeros, "offset": 0, "bytes_per_row": row, "rows_per_image": mh},
                {"texture": self.objs[c["texture"]], "mip_level": mip, "origin": (0, 0, 0)},
                (mw, mh, layers))

    def enc_resolve_query_set(self, enc, c):
        enc.resolve_query_set(self.objs[c["query_set"]], c["first"], c["count"], self.objs[c["dst"]], c["dst_offset"])

    def pass_common(self, p, k):
        if k[0] == "pipeline":
            p.set_pipeline(self.objs[k[1]])
        elif k[0] == "bind_group":
            p.set_bind_group(k[1], self.objs[k[2]] if k[2] is not None else None, k[3])
        else:
            return False
        return True

    def enc_compute_pass(self, enc, c):
        # The hot loop (a simulation's pass holds thousands of steps):
        # methods bound once, the commonest command tested first.
        p = enc.begin_compute_pass(label=c["label"] or "")
        objs = self.objs
        dispatch, set_bind_group, set_pipeline = p.dispatch_workgroups, p.set_bind_group, p.set_pipeline
        for k in c["cmds"]:
            op = k[0]
            if op == "dispatch":
                dispatch(k[1], k[2], k[3])
            elif op == "bind_group":
                set_bind_group(k[1], objs[k[2]] if k[2] is not None else None, k[3])
            elif op == "pipeline":
                set_pipeline(objs[k[1]])
            elif op == "dispatch_indirect":
                p.dispatch_workgroups_indirect(objs[k[1]], k[2])
            else:
                raise NotImplementedError(op)
        p.end()

    def enc_render_pass(self, enc, c):
        colors = []
        for a in c["color_attachments"]:
            att = {"view": self.objs[a["view"]], "load_op": a["load_op"], "store_op": a["store_op"]}
            if a["resolve_target"] is not None:
                att["resolve_target"] = self.objs[a["resolve_target"]]
            if a["clear_value"] is not None:
                att["clear_value"] = tuple(a["clear_value"])
            colors.append(att)
        p = enc.begin_render_pass(label=c["label"] or "", color_attachments=colors)
        for k in c["cmds"]:
            if self.pass_common(p, k):
                continue
            if k[0] == "draw":
                p.draw(k[1], k[2], k[3], k[4])
            elif k[0] == "viewport":
                p.set_viewport(*k[1:])
            elif k[0] == "scissor":
                p.set_scissor_rect(*k[1:])
            elif k[0] == "blend_constant":
                p.set_blend_constant(tuple(k[1]))
            elif k[0] == "stencil_reference":
                p.set_stencil_reference(k[1])
            else:
                raise NotImplementedError(k[0])
        p.end()


class Engine:
    """The module, instantiated, with a GPU host behind it."""

    def __init__(self, path=MODULE):
        t0 = time.perf_counter()
        engine = wasmtime.Engine()
        module = wasmtime.Module.from_file(engine, path)
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
        self.instance = linker.instantiate(self.store, module)
        self.exports = self.instance.exports(self.store)
        self.gpu.store = self.store
        self.gpu.memory = self.exports["memory"]
        if "_initialize" in self.exports:
            self.exports["_initialize"](self.store)
        self.load_seconds = time.perf_counter() - t0

    def _fn(self, name, *args):
        return self.exports[name](self.store, *args)

    def _read(self, ptr_fn, len_fn):
        ptr = self._fn(ptr_fn) & 0xFFFFFFFF
        n = self._fn(len_fn)
        return bytes(self.gpu.memory.read(self.store, ptr, ptr + n))

    def render(self, config_json, width, height, iterations=0):
        data = config_json.encode()
        ptr = self._fn("fflame_alloc", len(data))
        self.gpu.memory.write(self.store, data, ptr & 0xFFFFFFFF)
        status = self._fn("fflame_render", ptr, len(data), width, height, iterations)
        self._fn("fflame_free", ptr, len(data))
        out = self._read("fflame_output_ptr", "fflame_output_len")
        if status != 0:
            raise RuntimeError(out.decode(errors="replace"))
        info = json.loads(self._read("fflame_info_ptr", "fflame_info_len"))
        return out, info


def write_png(path, rgba, width, height):
    raw = b"".join(b"\x00" + rgba[y * width * 4:(y + 1) * width * 4] for y in range(height))
    def chunk(kind, body):
        c = struct.pack(">I", len(body)) + kind + body
        return c + struct.pack(">I", zlib.crc32(kind + body) & 0xFFFFFFFF)
    with open(path, "wb") as f:
        f.write(b"\x89PNG\r\n\x1a\n")
        f.write(chunk(b"IHDR", struct.pack(">IIBBBBB", width, height, 8, 6, 0, 0, 0)))
        f.write(chunk(b"IDAT", zlib.compress(raw, 6)))
        f.write(chunk(b"IEND", b""))


def main():
    cfg_path, out_path = sys.argv[1], sys.argv[2]
    width = int(sys.argv[3]) if len(sys.argv) > 3 else 512
    height = int(sys.argv[4]) if len(sys.argv) > 4 else 512
    iterations = int(sys.argv[5]) if len(sys.argv) > 5 else 0
    config = open(cfg_path, encoding="utf-8").read()
    eng = Engine()
    print(f"module loaded in {eng.load_seconds:.2f}s")
    for attempt in ("first", "second"):
        t0 = time.perf_counter()
        calls0, secs0 = eng.gpu.calls, eng.gpu.call_seconds
        pixels, info = eng.render(config, width, height, iterations)
        wall = time.perf_counter() - t0
        print(f"{attempt} render: {wall:.2f}s wall, engine says {info['ms']:.0f} ms, "
              f"{info['iterations']:,} iterations, {eng.gpu.calls - calls0} host calls "
              f"({eng.gpu.call_seconds - secs0:.2f}s in them)")
    write_png(out_path, pixels, info["width"], info["height"])
    print(f"wrote {out_path}")


if __name__ == "__main__":
    main()
