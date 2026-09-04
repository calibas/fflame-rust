# The WASM load crash

**Status:** OPEN, on `main`. Not caused by the escape-time work.

A browser trap while loading fractal config files. Either fractal
type, usually by the second file, sometimes the first. The app freezes;
the console reports one of

    Uncaught RuntimeError: memory access out of bounds      (Chrome)
    Uncaught RuntimeError: index out of bounds              (Firefox)

Firefox uses the second wording for *table* faults as well as memory
ones, which sent one round of this investigation chasing dangling
closures that do not exist.

The investigation ran on the `escape-time` branch and its full log is
in [escape-time-fractals.md](escape-time-fractals.md) (search "wasm
load freeze"). This file is the standing summary, kept where the bug
lives.

---

## What is established

**It is on `main`.** One build answered that, before a 198-commit
bisect of the wrong range started.

**It is not the escape engine.** Both render modes crash; the trace
frames are event-loop frames, not render frames.

**It is not stack depth.** The shadow stack was raised 16 -> 64 MiB
and verified in the binary. It then crashed on the *first* load, where
nothing has accumulated and 64 MiB needs unbounded recursion to
exhaust.

**It is not accumulation.** Same first-load reproduction. A real
closure leak was found on this path and fixed anyway (both browser file
pickers built handlers with `Closure::wrap(..).forget()` under a
comment claiming they would be "cleaned up when the reader is done" --
`forget` never cleans anything up; four sites, now
`Closure::once_into_js`, which JS owns until it fires and then drops).
Four leaked table entries were never enough to explain this, and the
fix did not stop it.

**No reachable `unsafe` in this crate explains it.** Safe Rust cannot
produce a linear-memory trap. The one raw-pointer site in the escape
engine (the simd128 column multiply in `fixedpoint.rs`) is not on the
flame path, which also crashes.

**It requires the stripped link, and that closes off instrumentation.**
Three builds were tried and none reproduces it:

| build | module | CODE vs dist | reproduces? |
|---|---|---|---|
| `dist` (shipped) | 16.0 MiB | — | yes, 1st-2nd load |
| `dist-debug` | — | **+50.6%** | no |
| `dist-symbols`, `debug = true` | 206 MB | +0.03% | no, 12+ loads |
| `dist-symbols`, `debug = false` | 18.9 MiB | -0.007% | no, dozens |
| names build, name section deleted **by byte edit** | 16.8 MiB | **0** (identical) | **no** |

The last row is the decisive one and was the point of building
`scripts/wasm-strip-names.py`. It removes the name and DWARF sections
from the names build without relinking, so the code section is byte-for-byte
untouched (offset shift measured at exactly 0) while the module drops
to essentially the shipped size. It does not crash. Module size and
load timing are therefore **not** the variable: the crash needs the
stripped LINK itself, and no symbol-preserving build will ever observe
it in the act.

---

## Two tooling errors, both of which produced confident wrong answers

Worth reading before trusting any offset-to-name result, because each
looked like a real answer at the time.

**1. The wrong file.** `scripts/wasm-symbolize.py` (now deleted) read
`target/wasm32-unknown-unknown/<profile>/fractal_flame_wgpu.wasm`. The
browser loads `pkg/fractal_flame_wgpu_bg.wasm`, which wasm-bindgen
*rewrites* from that file — 11,443 function bodies against the raw
module's 13,214. Every offset was being looked up in a module the
browser never ran.

**2. Index alignment across `strip`.** The same script shifted offsets
by the code-section delta and assumed function *i* is function *i* in
both builds. `strip` renumbers. Measured on one pair:

| | |
|---|---|
| bodies, dist / dist-symbols | 13,214 / 13,205 |
| index-aligned bodies identical | 15.4% |
| body **sizes** matching as a multiset | **98.7%** |
| bodies identical as an unordered multiset | 22.3% |

An earlier note in the escape log reported "0 bodies matching by
content hash". That figure was wrong — it is 22.3% — and the correction
matters, because "not one body is identical" was read as "a different
program". The truth is in between: the sizes say the same code is
mostly there, the content says it is not merely renumbered.

Wrong pairings produced three plausible names across the investigation
(`rhai::FuncRegistration::set_into_module_raw`,
`script::api::register_builtins`, and one more from a stale build).
None meant anything.

---

## The tool that replaces them

`scripts/wasm-locate.py`. It reads the `pkg/` modules the browser
actually loads, aligns the two builds' body-size sequences with a diff
instead of assuming index equality, and prints a confidence line for
every answer. Measured alignment on a real pair: **81.6%** of bodies
land in a matching run.

That 81.6% is the honest ceiling. Richer alignment keys were tried and
every one did worse — body ends 61%, size plus 12 bytes at each end
45%, exact content hash 19% — which says the two links differ in more
than numbering, so no cheap key identifies a body across them. An
offset in an unaligned region therefore gets **no name**, rather than a
guess.

Two things it can say with no cross-build assumption at all: the
containing function index and body size (read from the shipped module
alone), and the function's **export name** where it has one, since the
export section survives `strip`.

### Protocol

1. `./build-wasm.sh` and `./build-wasm.sh --symbols` from the **same
   commit**, no `src/` change between. Order does not matter.
2. Reproduce with the **shipped** bundle; copy the offset.
3. `python scripts/wasm-locate.py <offset>`

**Capture the whole wasm stack, not just the top frame.** Chrome shows
the frames as `wasm-function[N]`, which can be passed straight to the
locator as `#N` and skips the offset arithmetic entirely. Several
frames give several independent anchors, so one unaligned frame no
longer wastes the round.

**`--symbols` used to destroy step 2's artifact, twice over.**
wasm-bindgen writes fixed filenames into `--out-dir`, so the names
build overwrote `pkg/fractal_flame_wgpu_bg.wasm` — the only module that
reproduces the crash — leaving a pairing of one build against itself.
The first fix parked and restored that file, and broke the app: the JS
glue is emitted *alongside* the module and the two are a matched pair
(the import object is generated from the specific module processed), so
restoring only the `.wasm` left glue that would not link —
`LinkError: import object field '__wbindgen_object_drop_ref' is not a
Function`. `--symbols` now writes to `./pkg-names/` and copies just the
module out as `…_bg.names.wasm`. `./pkg` is never written to at all,
in any order, which is the property that was actually wanted.

**Read the pairing line before the name.** The script prints both
modules' geometry first. **Function-count drift is the check that
works**; code-size drift is not enough on its own. Measured on a stale
pair: 0.129% code-size drift — inside any sane threshold — while
differing by **628 function bodies**, and reporting a reassuring 92%
alignment, *better* than a true pair's 82%. Over 1% count drift means
rebuild; do not read the name.

---

## The Firefox trace, and what it identifies (2026-09-03)

The first stack that names anything. **Firefox only — Chrome could not
reproduce it at all**, which is itself a result: it moves a GC- or
scheduling-sensitive mechanism to the front.

```
Uncaught RuntimeError: index out of bounds
    __wasm_bindgen_func_elem_10607_13   fractal_flame_wgpu.js:2773
    real                                fractal_flame_wgpu.js:3094
fractal_flame_wgpu_bg.wasm:3534022
```

Both JS frames are wasm-bindgen's closure machinery. Line 3094 is
inside `makeMutClosure`'s `real`:

```js
const a = state.a;
state.a = 0;
try { return f(a, state.b, ...args); }
```

`state.a` is the Rust closure environment pointer. So a **JS-held
callback is being invoked, and the wasm side traps immediately** —
there is exactly one wasm frame, meaning the fault is in the trampoline
or its first callee, not deep in application code.

**The closure is identified exactly.** wasm-bindgen leaves the Rust
type in a comment at the cast site:

```
Closure { owned: true, function: Function {
    arguments: [NamedExternref("PointerEvent")],
    shim_idx: 4150, ret: Unit }, mutable: true }
```

`Closure<dyn FnMut(web_sys::PointerEvent)>`, and there is **exactly one
such adapter in the entire module** — 18 closure cast intrinsics, one
of which takes a `PointerEvent`. Every instance of that type shares it,
so the adapter names the *type* and not the instance.

**Instances of that type, and which can be freed:**

| owner | site | lifetime |
|---|---|---|
| ours | `lib.rs` `touch_fix` (pointerdown) | `forget()` — never dropped |
| ours | `lib.rs` `touch_up_fix` (pointerup/cancel) | `forget()` — never dropped |
| winit | `web_sys/pointer.rs`, `canvas.rs`, `event_loop/runner.rs` | `EventListenerHandle<dyn FnMut(PointerEvent)>` — **dropped on teardown and re-registration** |

That asymmetry is the lead. Ours cannot be freed; winit's are freed
whenever a handle is dropped. A trap *immediately* inside the
trampoline is what a freed environment looks like: a garbage vtable
pointer read from reused memory, then `call_indirect` on it — which is
precisely the fault Firefox words as "index out of bounds".

Both of ours also return immediately unless `pointer_type() == "touch"`,
so on a desktop mouse they do nothing at all, while winit's handle
every mouse event.

**The experiment that partitions this** is one build: remove the two
`lib.rs` touch closures and reproduce. If it still crashes, the closure
is winit's and the question becomes what drops a pointer listener
during a config load (a canvas resize or surface reconfigure would).
If it stops, ours are implicated despite `forget()`, and
`touch_up_fix`'s synchronous `dispatch_event` re-entrancy is the
suspect.

### Two more parser bugs found while reading this

Both had produced confident wrong readings, and both are now fixed:

- `export_names` keyed a dict by function index, but **several exports
  share one index** — `__wasm_bindgen_func_elem_10607_11/_12/_13/_14`
  are all one trampoline. The dict kept the last, so the module looked
  like it exported 9 of the 19 shims its own JS calls, i.e. like a
  mismatched bundle. It was not mismatched.
- The function-count pairing guard **rejects the correct pairing**.
  `--keep-debug` changes wasm-bindgen's closure strategy — the shipped
  module gets those trampoline exports and the names build gets none —
  so a genuine same-commit pair is 10,815 bodies against 11,443, 5.8%
  apart. The guard now compares DATA sections instead: the constant
  pool is what the same source produces regardless of link. Measured on
  this pair, **0.002%**.

---

## The function is identified, and alignment is not how (2026-09-03)

The `?no-touch-fix` build crashed **with and without the flag**, which
retires our two `PointerEvent` closures, and it came with a much fuller
Firefox stack. Read as Firefox's async causality chain it is:

```
run -> __wbg_init -> finalize_init -> queueMicrotask
    -> addEventListener -> requestAnimationFrame -> [trap]
```

A **requestAnimationFrame callback**, which is what the very first
report said before three rounds of tooling went past it.

**Alignment was measured and it does not work.** Before reading any
name out of it: exports named in BOTH modules are ground truth, so map
each shipped index through the alignment and compare. Of 22 testable,
**9 correct, 6 WRONG, 7 honestly unaligned** — and the wrong ones miss
by thousands of indices (`wasmapi_get_config_json` mapped to 4,752
against a truth of 8,971). A tool that is wrong 40% of the time it
answers is worse than one that does not answer. The `pkg`-level
alignment is dead; the guards stay, but the technique does not earn a
name on its own.

**What identified it instead: the module's own string literals.** Scan
the trapping body for `i32.const <addr>, i32.const <len>` pairs landing
in a data segment and read the bytes. Both offsets (3,534,022 and
3,534,521) are inside shipped body #257, a 5,905-byte function, and it
references exactly two strings:

```
'handler woken up without user event'
'internal error: entered unreachable code'
```

The first is winit's, and appears in exactly two places in winit
0.30.13 — `platform_impl/web/event_loop/mod.rs`, in `run()` and in
`spawn()`. Together with the `unreachable!()` beside it, that is the
web event-loop handler closure, with our own `event_handler` inlined
into it by LTO. **No cross-build assumption at any step**: the shipped
module names itself.

### `run()` on the web borrows a stack frame it then abandons

```rust
// SAFETY: Don't use `move` to make sure we leak the `event_handler` and `target`.
let handler: Box<dyn FnMut(Event<()>)> = Box::new(|event| { ... });
let handler = unsafe { std::mem::transmute::<..., ... + 'static>(handler) };
self.elw.p.run(handler, false);
backend::throw("Using exceptions for control flow, ...");
```

`self`, `target` and `event_handler` are **locals of `run`**. The
closure borrows them, is transmuted to `'static`, and the frame is kept
alive only by never returning — `throw` leaves via a JS exception. Our
`src/app/mod.rs` called this on every platform, `#[allow(deprecated)]`
and all.

`EventLoopExtWebSys::spawn()` is the same handler built with `move`,
owning its captures on the heap, and it "returns immediately, and
doesn't throw an exception". It is the documented web entry point.

So the wasm build now uses `spawn` and the desktop keeps `run`. This is
not proof of the mechanism — whether the abandoned frame is genuinely
reused is not established, and the shadow-stack pointer is plausibly
never rewound, which would make the trick sound. It removes the
dependency instead of arguing about it, and it moves us onto the API
winit documents for this platform. If the crash survives it, the
handler closure is still the place to look and the next question is
what inside it holds a stale reference.

---

## Next moves, in order of cost

1. **One paired run of the protocol above.** It has never actually been
   executed with a correct pairing on the correct files — every prior
   attempt failed one of those two conditions. This is one build and
   one reproduction.
2. If the offset lands unaligned, **bisect `main`**. The report says
   "introduced recently", so start shallow. `scripts/bisect-wasm.bat`
   builds a step with the stack size pinned.
3. If a name lands in a dependency, that dependency's async or
   event-handler path is the suspect; `winit`'s web event loop and
   `wasm-bindgen`'s closure machinery are the two on this path.
