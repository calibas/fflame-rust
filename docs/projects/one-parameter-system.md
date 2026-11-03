# One parameter system

> For the parameter systems and the UI that's derived, I want to see if
> we can use a single system, and not multiple parallel systems. (The
> user, 2026-11-03.)

Branch `params-one-system`, the first of the Rhai branches in
[rhai-plugins.md](rhai-plugins.md) ("What is left, as branches"). It
replaces branch 1 there (`rhai-layouts-everywhere`): with one renderer,
conditions and grids in every panel come with it.

## Where it stands (2026-11-03, read from the code)

Five definition types, a sixth in the compiler, and four widget sets:

| Kind | Definition | Built-ins | Widget | Values |
|---|---|---|---|---|
| Variations (Transforms panel, escape transformations, the lens) | `VariationParamDef` (static), `VariationParameter` (owned: API, Rhai), `ParamType` | 2,976 `param!` uses, 175 literals | `render_variation_params` | f32 on the transform |
| Effects (Effects panel) | `EffectParameter`, `ParamType` | 84 literals | the variations', through `widget_param` (a copy into `VariationParameter`) | f32 on the instance |
| Escape formulas, colourings, layers; the lighting panel | `EscapeParamDef` | 248 literals | `escape_panel::param_control` | f32 in the escape block |
| Simulation models and colourings | `SimParamDef`, field for field `EscapeParamDef` | 280 literals | `sim_panel::param_control` ("same shape as the escape panel's") and `render_param_grid` | f32 in the simulation block |
| CPU scripts (Scripts panel, palette generator) | `ParamDecl`: float, int, bool, choice, text, colour | declared when a script runs | `script_params::render` | f64, text, colour; the panel's own, run inputs (no undo, no animation) |
| The compiler (`lang`) | `ir::Param`, `ParamKind` | -- | -- | four converters into the four GPU kinds' types |

Around them, more of the same split:
- **Slow parameters**, three lists: `variations::is_slow_param`,
  `escape::SLOW_PARAMS`, `sim::SLOW_PARAMS` and `INFO.slow`.
- **Exponential sliders** are escape's alone, by a pointer set
  (`escape::custom::is_exponential`).
- **Conditions and grids** (`param_layout`) are drawn by the simulation
  panel alone.
- **Behaviour differences**:
  - an effect's float with no range is drawn 0..1, a variation's
    −10..10;
  - simulation sliders drag at a fifth of a percent of the range,
    escape's by their own `param_slider`;
  - only the variations' renderer commits a drag's preview when the
    drag ends (`force_commit_preview`).

## The design

**One definition, `params::ParamDef`**, for every kind:

```rust
pub struct ParamDef {
    pub name: Cow<'static, str>,   // the key its value is stored under
    pub label: Cow<'static, str>,
    pub hint: Cow<'static, str>,
    pub kind: ParamKind,
    pub slow: bool,                // a change makes the next frame slow
}
pub enum ParamKind {
    Float { default: f64, min: f64, max: f64, free: bool, log: bool, angle: bool },
    Int { default: i64, min: i64, max: i64, free: bool },
    Bool { default: bool },
    Choice { default: usize, options: Options },   // Static(&'static [&'static str]) or Owned(Vec<String>)
    Text { default: Cow<'static, str>, max_len: usize },
    Color { default: [f32; 3] },
}
```

- `free` is today's unlimited float or int: a typed value may leave the
  range, and the slider keeps it.
- `log` is escape's exponential, now open to every kind.
- `angle` is degrees, drawn with the angle control.
- Text and colour are the scripts' alone: the compiler refuses them in
  a GPU kind, whose values are f32 slots.
- Built-ins stay statics, written in one const call:
  `ParamDef::new("feed", "Feed rate", "How fast.", Spec::float(0.055, 0.0, 0.1).log().slow())`.
  `Spec` is a `Copy` builder: a const function cannot drop what it
  replaces, so the owned type itself has no chained builders (checked
  with rustc). Its numbers are f32, so a GPU kind's default is the f32
  its literal names and is stored in the f64 exactly.
- A script's parameters are built owned, with no leaking: a script
  declares them again whenever it runs.
- The `param!` macro emits `ParamDef`, so the 2,976 variation uses do
  not change.

**One layout.** `param_layout` moves into the module as
`params::layout` (conditions, grids, fill, warnings). The slow flag
moves onto the definition, so the three slow lists go.

**One renderer, `ui::params`.** It draws a parameter set -- its
definitions and its layout -- through a value store:

```rust
pub trait ParamStore {
    fn value(&self, name: &str) -> Option<Value>;       // None: the target is gone
    fn set(&mut self, name: &str, v: Value, done: Done); // Done: dragging, released
    fn set_many(&mut self, values: Vec<(String, Value)>, why: &str); // a grid's fill
}
```

- `ConfigStore` writes through the `ConfigManager` by a path function,
  for every config kind: a transform's variation, an effect instance,
  the escape block's formula, colouring, layer and lens, a simulation
  layer and colour layer.
- `LocalStore` holds the Scripts panel's run inputs, as `self.values`
  does now.
- Each kind's control is one function: float (slider, typed beyond
  the range when free, logarithmic, the angle control), int, bool,
  choice (a dropdown), text, colour.
- It also does the slow commits (`slow_controls`), conditions, grids,
  tooltips, and the preview commit at a drag's end, which today is the
  variations' alone and is now every kind's.

**Storage stays where it is.** Each value keeps its config place and
its `ConfigPath`, and the GPU packing, undo and animation are
unchanged. Only the definitions and the drawing unify.

**One converter.** The compiler gives `Vec<ParamDef>` (`lang::params`)
and its layout, so each GPU kind registers a script's parameters the
same way.

**The wire stays as it is.** The contract's `ParamType` names are
`ParamKind`'s spelling: float, unlimited_float (`Float { free }`),
integer, unlimited_integer, boolean, angle (`Float { angle }`), enum
(`Choice`). `log` and `slow` are presentation and are not published.
Text and colour are no variation's or effect's, so they are not in the
contract. The `shape` should not move; if it does, the API repository is
told (RELEASE.md §3).

## Behaviour, kept or unified

- **Kept, as data:** each migrated parameter keeps its range, so an
  effect's float the panel drew 0..1 says 0..1.
- **Unified:**
  - every drag commits its preview when released;
  - every slider drags at one speed rule;
  - a slow parameter behaves one way in every panel: a dropdown over a
    small whole-number range, else a commit on release, as all three do
    now.
- **Opened to every kind:** `log`, conditions, grids, and a cell at a
  computed row (each kind's dynamic slot read, as `Host::GridParam` is
  the simulations').

## Steps

Each a commit, each behaviour-preserving and held by tests:
1. **A snapshot first**, on `main`'s types: every built-in parameter of
   every kind -- name, label, hint, kind, default (f32 bits), range,
   choices, slow -- written to `output/params-snapshot.json` (an ignored
   test). Each migration step compares its kind's dump with it, so a
   default or a range that moved is a failure, not a surprise. **Done**
   (`params::snapshot`: 3,946 parameters, each with the control and the
   range it is drawn with, and escape's five-decade log sliders); the
   API corpora too (`export_variations_json`, `export_effects_json`,
   `export_scripts_json` into `output/params-baseline/`), to diff after
   the variations, effects and scripts steps. Read from them:
   - every bound a variation or an effect leaves out is a boolean's,
     which the corpus publishes as null: `ParamKind::bounds` gives none
     for a switch, so required bounds leave the corpus as it is;
   - every escape and simulation choice's min and max are its indices,
     so `Choice` carries no range;
   - no integer has a fractional default or bound.
2. **`params`**: `ParamDef`, `ParamKind`, `Spec`, `Options`, values,
   and the layout moved in. **Done**: `params::Value` is the scripts'
   `ParamValue` (re-exported under that name until step 8), and a GPU
   kind's f32 slot is read and written by `ParamKind::from_slot` /
   `to_slot` (a choice rounded and clamped, a switch above one half).
3. **`ui::params`**: the renderer and its two stores, with layout tests
   for every kind of control. **Done**: `control` (one per kind, every
   slider a `VkbSlider`, `Edits` clamping, or `Never` for a free number;
   a linear float's value field drags at a 500th of its range, the
   simulations' rule; a choice is a dropdown labelled on its left, as
   escape's, the simulations' and the scripts' are), and `list` /
   `list_with` (conditions, fills as one undo step, grids, a `hide`
   predicate, an `after` hook for escape's polar row). Grid cells no
   longer clamp a stored value just by being drawn
   (`clamp_existing_to_range(false)`).
4. **Simulations**: 280 `SimParamDef` literals by a codemod in
   `scripts/`; `sim::custom` gives `ParamDef`; `SLOW_PARAMS` and
   `INFO.slow` become flags; the simulation panel draws through the
   renderer. The twins' settings test compares definitions.
5. **Escape**: 248 `EscapeParamDef` literals; `is_exponential`'s pointer
   set becomes `log`; `escape::SLOW_PARAMS`; the escape panel's formula,
   colouring and layer sections, the lighting panel, the animation
   target selector.
6. **Effects**: 84 `EffectParameter` literals, Rhai effects, the
   Effects panel (`widget_param` goes), `export_effects_json` (its
   output unchanged).
7. **Variations**:
   - `param!` emits `ParamDef`, and 175 `VariationParamDef` literals
     move by codemod;
   - `VariationParameter` goes;
   - the API conversion (`ApiVariationParameter`);
   - the registry, the probe, the census, the docs corpus;
   - the Transforms panel, escape transformations and the lens through
     the renderer;
   - `variations::is_slow_param` becomes flags.
8. **Scripts**:
   - `ParamDecl` goes: the CPU API's `param_*` functions build owned
     `ParamDef`s;
   - one parse of a value from text or JSON (`params::parse`) serves
     the CLI's `--set` and the script module's parameters;
   - the Scripts panel and the palette generator draw through the
     renderer with a `LocalStore`.
9. **The compiler**: `ir::Param` gives `ParamDef` directly, and the four
   converters go.
10. **Everywhere**: conditions and grids in every panel (falls out of
    4-9), and a computed grid row in every kind.
11. **Docs and gates**: VARIATIONS.md, EFFECTS.md, ESCAPE.md,
    SIMULATION.md, SCRIPTING.md and UI.md; the contract's `shape`
    checked; the visual suite.

## Out of it

- Each kind's values stay in their own config places: a single store of
  values would be a config format change, with nothing gained.
- The API's own types (`ApiVariationParameter`) stay its wire format;
  only their conversion changes.
