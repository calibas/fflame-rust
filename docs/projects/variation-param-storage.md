# Variation parameter storage: a general-purpose per-transform block

Status: **steps 1-4 built** (2026-11-07, branch
`variation-param-storage`; *Progress* below). The API notice is in
[v0.6.0-release.md](v0.6.0-release.md). Follows the variation twins (`rhai-variation-hooks`,
[rhai-plugins.md](rhai-plugins.md) P7 step 3).

## Why

A variation's table read at a run-time index is slow, and it is slow in
the built-ins as well as their Rhai twins. naga's SPIR-V backend has no
way to index a value array (a module `const`) at run time, so it copies
the whole array into a function-local variable, and it does the copy at
**every** such read. The code is `spill_to_internal_variable` in naga
29's `src/back/spv/block.rs`, which says it performs the store "even if
we already had a spill variable". The cost is the table's size times its
reads.

Measured on the user's GTX 1660 SUPER with `time_variation_twins`
(2026-11-06). The block was raised to 8192 floats for the test, and
twins' tables were moved into `prepare`, so they were read from the
parameter buffer (`get_param`, a storage buffer) instead of a constant:

| twin | table | constant table | table in the parameter buffer |
| --- | --- | --- | --- |
| polychoron | 4372 floats | 5.69x its built-in | **0.06x** (17x faster than the built-in) |
| su_mobius | 992 floats | 1.06 (2D) / 1.07 (3D) | 0.99 / 0.99 |
| polyhedron | 360 floats | 1.11 / 1.13 | 1.13 / 1.11 (not its table) |
| menger | 4372 floats | 1.01 | 1.00 (its default settings rarely read it) |

The built-in polychoron spends nearly all its time copying its table.
Built-ins that read the same tables (polychora.wgsl, polyhedra.wgsl,
su_mobius.wgsl) can expect the same kind of gain once their tables live
in the parameter buffer.

## What the user decided (2026-11-06)

- Raise the per-transform cap, rather than add a table buffer: the
  buffer uses little memory, and it may grow a little.
- Make it general purpose: room for a few hundred thousand floats,
  should something need that many.
- **No performance cost when the slots are not used.**
- Measure the speedup first (done, above), and include the built-ins.

## Today

- `GpuVariationParams { params: [f32; 1600] }` per transform slot, for
  `MAX_TRANSFORMS_UNIFIED` (128 + 256 subflame) slots: a fixed 2.4 MB
  (`gpu/buffers.rs`). The WGSL is `struct VariationParams { params:
  array<f32, 1600> }` at binding 5 (`shaders/core/header.wgsl`).
- `get_param` is generated per flame (`build_packed_get_param`): a
  `switch` over the local variation index, with each variation's offset
  baked in, then `variation_params[xform_id].params[offset + slot]`.
- Every transform of a flame has the same layout: each active variation
  gets `parameters + init_param_count` slots at the same offset
  (`compute_packed_layout`). Subflame transforms use the parent's local
  map (`gpu_variation_param_regions`), so one layout covers both. The
  upload writes only the regions in use (`write_regions`).
- The init pass (`build_init_shader`) declares its own copy of the
  struct, `read_write`, and fills the derived slots after the
  parameters.
- `MAX_VARIATION_PARAM_SLOTS = 1600` caps the sum over the flame's
  variations. It appears in `lang::check` (a Rhai variation's
  parameters plus `prepare`'s values), `contract.rs`
  (`variation_slots_per_flame`), `renderer/sticky.rs` and
  `probe/batch.rs`. The high-res exporter sizes its copy from the struct
  (`export/high_res.rs`).

## The design

1. **One flat array.** Binding 5 becomes `var<storage, read>
   variation_params: array<f32>` (runtime-sized). The flame's stride, the
   sum of its layout's slot counts (the sticky superset's, which the
   offsets already come from), is baked into the shader beside the
   offsets: `const PARAM_STRIDE: u32 = N;`. `get_param` reads
   `variation_params[xform_id * PARAM_STRIDE + offset + slot]`. That is
   one multiply-add, the same work as today's fixed stride of 1600.
2. **Sized to use.** The host buffer is `slots in use x stride x 4`
   bytes. It is reallocated only when that grows (power-of-two capacity,
   so a slider drag never reallocates), and only the used part is
   uploaded, as now. A flame that uses little gets a smaller buffer than
   today's fixed 2.4 MB.
3. **Caps.**
   - A transform's block: `MAX_VARIATION_PARAM_SLOTS` rises from 1600
     to 262,144 floats (1 MiB).
   - The whole buffer: the device's `max_storage_buffer_binding_size`,
     at least the WebGPU floor of 128 MiB (33.5M floats). The app's
     devices ask for the adapter's limit already (`app/export.rs`,
     `animation/export.rs`).
   - Past either cap, an error, as now past 1600.
4. **The init pass** declares the same flat array with the same baked
   stride, `read_write`.
5. **Everything sharing the layout changes together**: the subflame
   regions, the high-res exporter's buffer (`high_res.rs`), the animation
   export, the probe's batches, the escape camera lens's flame buffers,
   and the gallery modules and pyfflame (they share the library).
6. **No cost when unused**, held by measurement:
   - Time a set of ordinary built-in flames before and after, the same
     way each time. A/A runs bound the noise; julia's A/A reads
     0.96-1.00 in the small-dispatch tool.
   - Compare the per-frame upload size and the buffer's memory.
   - `get_param`'s code is one constant different, so the shader dumps
     show the whole change.

## The built-ins

Each built-in whose table is read at a run-time index computes that
table in its `wgsl_init` (the derived slots after its parameters), from
the same literals, and reads it with `get_param` instead of the
library's `const`:

| built-in | table | now |
| --- | --- | --- |
| polychoron, menger | `POLYCHORA_VERTS`, 1093 `vec4` (4372 floats) | `polychora.wgsl` constant |
| polyhedron, polyhedron_volume | `POLYHEDRA_FACES`, 90 `vec4` (360 floats) | `polyhedra.wgsl` constant |
| su_mobius | `SU_MOBIUS_BASE`, 248 `vec4` (992 floats) | `su_mobius.wgsl` constant |

- Only the variation's own range need be copied. menger reads 96 or
  224 vertices; polychoron's chosen shape at most 600 (the 120-cell,
  2400 floats); su_mobius's group a range.
  - A fixed `init_param_count` must hold the largest range: polychoron
    2400 floats plus parameters.
  - Or the whole table can be copied: 4372 floats, simpler.
  - Measure both.
- Each one is measured before and after with `run_benchmarks.py` and a
  flame timing on that variation. Its picture is held by the visual
  suite and its twin's gates.
- **This changes their slot counts** (`init_param_count`). The variation
  corpus the API reads changes, so tell the API repository (RELEASE.md
  §3). Their twins are retranslated with it.
- Others: any other built-in reading a `const` array at a run-time index
  gets checked the same way (mondrianomies' baked rectangle tables,
  among the specialized sources).

## The twins

- The translator moves a constant table that is read at a run-time index
  into `prepare` (`#{ tbl_x: [..] }`, read as `p.tbl_x[i]`). A helper
  that reads one takes `p`. The experiment's switch is kept in the
  scratch copy `translate_tables_in_prepare.py.keep`. Whether small
  tables move too is decided by measurement: polyhedron's 360 gained
  nothing.
- **The CPU pair judge** runs `prepare` again for each case it checks,
  and building a 4372-float table each time took polychoron's and
  menger's pairs past 10 minutes. It should run `prepare` once per
  parameter setting (`VariationRun` reused, or `prepare`'s values
  cached).
- The timing tool and the gates run as now, in 2D and in 3D.

## Order of work

1. The flat array, baked stride, buffer sized to use, and caps; the init
   pass; every sharer. Unit tests for the packing, the visual suite,
   and the twin gates.
2. The no-cost measurement, before the built-ins move: ordinary flames
   timed before and after, memory and upload size.
3. The built-ins, one table at a time, each measured.
4. The twins: the translator's tables into `prepare`, the pair judge's
   reuse, and a retranslation, gated in 2D and 3D.
5. Docs (BUFFERS.md, VARIATIONS.md, CLAUDE.md's 1600), the regenerated
   contract and shader dumps, and the API notice.

## Progress

**Steps 1 and 2** (2026-11-07):

- **Built as designed**, but for the buffer's size: one flat
  `array<f32>` at binding 5, the stride baked as `PARAM_STRIDE`
  (`packed_param_stride_of`, which the host packs by), a row of up to
  262,144 floats, only the rows in use uploaded, and every sharer (the
  high-res exporter, the escape lens, a simulation's layers, the GPU
  planner, the probe) on `PackedVariationParams`. The WGSL change is the
  binding and `get_param`'s one line (the shader dumps). The sticky
  superset keeps 1600 as its extras' budget (each extra lengthens every
  row, and so every upload); the probe keeps 1600 as its batching budget,
  a variation wider alone probed in a batch of its own.
- **The buffer keeps its old size** (`MIN_VARIATION_PARAMS_BYTES`,
  2,457,600 bytes) and grows past it only for a flame that needs more.
  Sized to its rows (16 KiB for most flames), it moved two of the
  fourteen timed flames far past the noise -- subflame-julian 24%
  slower, init-migration 26% faster -- with the shader's work the same
  (the pictures byte-identical). Bisected: not the indexing form (the
  old struct-of-rows form at the new size moved them alike), not the
  stride (1600 floats, flat, moved them), not the init kernel; the old
  shader text and stride at a 4 MiB buffer still moved them, and exactly
  2,457,600 bytes put them back. It is where the renderer's buffers
  made after this one land: a dummy buffer after a small one, making
  the two total the old size, was fast; anything else mostly slow. So
  the timing tool now times each flame at eight placements (a buffer
  made first shifting the renderer's) and builds compare by the median
  over them. Made last, sized to its rows, the median ratio was 0.994
  but Square Tile moved +3%; at the old size and place, every flame
  lands as before.
- **No cost, measured** (`time_ordinary_flames`, eight placements, two
  runs a build, GTX 1660 SUPER): median ratio 0.995, every flame within
  0.976-1.006, inside the runs' spread. 378 of the visual suite's 390
  configs and the eight presets render byte-identical to before; the
  other 12 (the presets without a fixed seed, the solids) differ as two
  runs of the old build do.
- **The init kernel has one case a variation**, not a pair: each
  thread's run start and init from a table (`INIT_PAIRS`), each run
  copied by a loop. A case a pair copied an init's arrays for each, and
  100 transforms of an 8192-float table took the driver past ten minutes
  and 5 GB (the test process outlived its shell; stopping it was refused,
  and it ended on its own); one case, four seconds. The old kernel, a
  line a slot, would have been worse -- 1600 kept it from being tried.
- **Tests**: the rows a stride apart (`the_rows_are_a_stride_apart`),
  the shaders reading the rows the host packs, the capacity rule, and a
  GPU gate (ignored: it registers a variation) -- 100 transforms of a
  Rhai variation's 8192-float table, 3.3 MB, grow a renderer made for
  one linear transform, drawn byte for byte as a fresh renderer, its
  table's entry reaching the shader.
- **The contract moved** (`variation_slots_per_flame` 1600 to 262,144):
  tell the API repository (RELEASE.md §3).

**Step 3** (2026-11-07), measured flame by flame (`time_ordinary_flames`
with `FLAME_TIMING_VARIATIONS`, eight placements; the variation on a
turning shrink weighted 8 to two linears'), each picture byte-identical
to before in every mode, 2D and 3D:

| built-in | flame (ms per 16 dispatches) | before | after |
| --- | --- | --- | --- |
| polychoron | 16-cell, 2D / 3D | 139.5 / 228.5 | 21.9 / 23.4 |
| polychoron | 120-cell, 2D / 3D | 155.3 / 332.3 | 87.1 / 85.9 |
| menger | 4-gap, 3D | 311.0 | 81.2 |
| menger | 4-gap 4D, 3D | 321.8 | 69.2 |
| menger | 2D, any mode (the body reads no table) | 72.5-72.7 | 69.9-70.4 |

- polychoron's init copies its shape's vertices (2400 floats the most),
  menger's its 4-gap point set (896); the body reads four `get_param`s a
  vertex. The init kernel takes the name-gated libraries its variations
  use, as the main shader does. su_mobius moved likewise drew no gain
  (its 992 floats read twice a call: group 3 at 34.3 before, 34.1 / 35.7
  after), so it stays; polyhedron's 360 gained nothing in the twin
  experiment, so it stays too. Runs minutes apart drifted a few percent
  on the noisier flames (polyhedron, unchanged, 35.2 to 38.0), which is
  why each comparison was made against the old build timed then.
- The contract's `largest_shipped_variation_slots` moved 259 to 2410
  (polychoron), and polychoron's and menger's `init_param_count` with
  it: the variation corpus changes, tell the API repository.

**Step 4** (2026-11-07): the twins follow their built-ins. The
translator already makes a def's `wgsl_init` the twin's `prepare` and a
computed slot read `p.slots[e]`, so polychoron's and menger's twins,
retranslated from the new built-ins, keep their vertices in `prepare`'s
slots too. Two translator additions: a table copied whole into a
variable never written (`var verts = POLYCHORA_VERTS;`) is read as the
table, and `prepare`'s tables are written at its top (in `prepare` the
language fills a local once a call, no module constant). Every other
twin retranslates byte-identical.

- Held: compile; pairs; density in 3D (both are 4D) at the defaults and
  in each table mode -- polychoron's 5-cell, 120-cell and 600-cell,
  menger's two 4-gap modes -- with the density gate's new
  `VARIATION_TWINS_PARAMS=k=v`, which the defaults do not reach.
- Timed against the new built-ins (`time_variation_twins`, which takes
  the same knob): 1.00-1.04 in every mode (polychoron's was 5.65).
- Not needed: the CPU pair judge's `prepare` reuse. Both variations
  draw, so their pairs are judged by density and the judge never runs
  them; no twin with a large `prepare` is judged by formula today.
- The translator's general move of a table into `prepare` (for twins
  whose built-ins keep a constant table) stays unbuilt: of the measured
  ones, su_mobius's built-in gained nothing from it and polyhedron's twin
  nothing, so the twins mirror their built-ins instead.

**Found for step 4** (the twins): a Rhai array holds at most 8192
floats and a loop runs at most 4096 times, `prepare`'s included, and
`prepare` builds its result in the init as locals (three copies of each
array). A table past 8192 floats takes several arrays. The user,
2026-11-07: the cap is fine; expand it later if something needs it.

## Open questions

- The per-transform cap. 262,144 floats is a proposal; the whole-buffer
  limit binds first for a flame with many transforms using it.
- A table is the same for every transform carrying the variation, yet
  each transform keeps its own copy (the slots are per transform). A
  per-flame shared region would save that memory, at the cost of a
  second layout. Revisit if memory shows up.
