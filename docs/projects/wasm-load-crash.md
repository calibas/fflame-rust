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

**`--symbols` used to destroy step 2's artifact.** wasm-bindgen writes
a fixed filename into `--out-dir`, so the names build overwrote
`pkg/fractal_flame_wgpu_bg.wasm` — the only module that reproduces the
crash — and left a pairing of one build against itself. Both build
scripts now park the shipped module and restore it, writing the names
build to `…_bg.names.wasm`, so the served bundle is unchanged and the
two are always available together.

**Read the pairing line before the name.** The script prints both
modules' geometry first. **Function-count drift is the check that
works**; code-size drift is not enough on its own. Measured on a stale
pair: 0.129% code-size drift — inside any sane threshold — while
differing by **628 function bodies**, and reporting a reassuring 92%
alignment, *better* than a true pair's 82%. Over 1% count drift means
rebuild; do not read the name.

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
