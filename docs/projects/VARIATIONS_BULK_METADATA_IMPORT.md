# Variations & Effects Bulk Metadata Import — Project Plan

Project to bring description, author, and per-parameter description
metadata to the ~494 variations in [src/variations/defs/](../../src/variations/defs/)
and the built-in effects in [src/effects/mod.rs](../../src/effects/mod.rs),
plus correct the ~38 int-as-bool and ~15–25+ int-as-enum parameter
misclassifications that have accumulated during the port.

Companion to [VARIATIONS_WIRE_FORMAT.md](VARIATIONS_WIRE_FORMAT.md)
which documents the wire contract between client and API. This doc
covers the *work*; the wire-format doc covers the *shape*.

---

## 1. Scope

### In scope

- **Metadata extraction** for ~494 variations:
  - Free-form variation-level `description`
  - `authors: Vec<String>` (original designer(s))
  - Free-form per-parameter `description` for every `parameters[*]`
- **Same metadata extraction** for built-in effects (count TBD by audit)
- **Type corrections** for parameters currently mis-declared:
  - ~38 confirmed `Integer` params that are semantically `Boolean`
    (`filled`, `invert`, `xaxis`, `use_cos_x`, etc.)
  - ≥8 confirmed `Integer` params that are semantically `Enum` with
    distinct branch labels (`falloff2.type`, `subflame_wf.color_mode`,
    `spirograph3D.mode`, `butterfly_fay.{outer_mode,inner_mode}`,
    `mobius_strip.{width_mode,radial_mode}`, `taurus.shape`,
    `jac_asn.jac_asn_type`); likely 15–25+ once the corpus is scanned
    exhaustively
- **Plumbing** to make all of the above renderable: Rust structural
  fields, wire format extensions, UI tooltip rendering, conversion path
- **Bulk import** script + versioned SQL migration

### Non-goals

- **i18n / translation of descriptions.** Names, display names,
  descriptions, and authors are single-locale (English) by policy.
  Variation and effect names are technical (mathematical / artistic
  jargon), descriptions are technical commentary on shader behavior,
  and author attributions are historical proper nouns. None of these
  belong in `locales/*.yml`. Translation tools should skip them.
- **Param grouping** (e.g. visual sectioning of the 13-param
  `z`/`w` variations under collapsible headers).
- **Conditional show/hide** of params based on other param values.
- **Author-as-user-account.** `authors` is free-form text, not a
  foreign key. See [VARIATIONS_WIRE_FORMAT.md §7](VARIATIONS_WIRE_FORMAT.md).

---

## 2. Plumbing prerequisites

These code changes must land **before** the bulk import script can
emit useful data. All are client-side, no API coordination required,
all are additive (no break for existing flows).

### 2.1 Structural fields

| Where | Add field | Rationale |
| ----- | --------- | --------- |
| `VariationParamDef` ([definition.rs](../../src/variations/definition.rs)) | `description: Option<&'static str>` | Static def for built-in variations carries the tooltip text. |
| `VariationParameter` ([variations/mod.rs](../../src/variations/mod.rs)) | `description: Option<String>` | Runtime form (registry-loaded), carries through from def OR API download. |
| `ApiVariationParameter` ([api/types.rs](../../src/api/types.rs)) | `description: Option<String>` (serde default) | Wire format addition. Same field name on all three layers — no rename in `from_download`. |
| `EffectParameter` ([effects/mod.rs](../../src/effects/mod.rs)) | `description: Option<String>` AND `display_name: String` | Effects today have neither. `display_name` brings effects into line with the variation convention and replaces the `translated_param_name()` i18n lookup pattern. |

### 2.2 Macro support

[`param!` macro](../../src/variations/definition.rs) currently has
arms for `float`, `unlimited_float`, `int`, `unlimited_int`, `angle`,
`bool`. **Two additions needed**:

- An `enum` arm — currently no way to declare an enum param via the
  macro. Shape: `param!(name, display, enum, default_index, [labels])`.
- Either extend every existing arm with an optional trailing
  `description` argument, or add `param_with_desc!` variants. Probably
  the former, using a token-tree pattern that makes the description
  argument optional. (Decision: pick one approach during plumbing
  work, not now.)

### 2.3 UI rendering

Wire `.on_hover_text(&description)` into every renderer in
[ui/variation_params.rs](../../src/ui/variation_params.rs):
`render_float_param`, `render_unlimited_float_param`,
`render_integer_param`, `render_unlimited_integer_param`,
`render_boolean_param`, `render_angle_param`, `render_enum_param`.

Same for the effect-panel parameter loop in
[ui/effects_panel.rs](../../src/ui/effects_panel.rs) (line ~107).

When `description.is_none()`, render as today (no tooltip). Don't
hover on an empty string.

### 2.4 Conversion path

Update `VariationInfo::from_download`
([variations/mod.rs:181](../../src/variations/mod.rs)) to copy
`dl.parameters[i].description` into the runtime
`VariationParameter.description`. The variation-level `dl.description`
stays discarded by design (presentation-only, registry panel reads
from `VariationListItem` directly — see
[VARIATIONS_WIRE_FORMAT.md §7](VARIATIONS_WIRE_FORMAT.md)).

Same pattern for `EffectInfo` when an `EffectDownload` API type
eventually exists.

---

## 3. Data work (the long pole)

### 3.1 Exhaustive int→bool / int→enum classification scan

Manual pass through all 494 variations + every built-in effect.
For each `Integer` parameter:

- Inspect the WGSL body. If it branches on the param value with
  distinct-per-branch behavior, classify as `Enum` and write the
  branch labels.
- If the param's range is `[0, 1]` and the body treats it as a flag
  (`if (x > 0.5)`, multiplies into `select(...)`, etc.), classify as
  `Boolean`.
- Otherwise leave as `Integer`.

Output is a checklist (Markdown table or YAML) feeding into the
restructure pass.

### 3.2 Comment restructure

Today the corpus has inconsistent comment structure:

- Some files have a single `//!` file-level summary covering a batch
  of variations and `// =====` banners for each `pub static`.
- Some have `///` doc-comments above each static.
- Per-parameter descriptions don't exist in any form.

Goal format — every `pub static` gets `///` doc-comments with a
parseable trailer:

```rust
/// Free-form description of what this variation does, in any
/// number of lines. This becomes `variation.description` on the wire.
///
/// # Authors
/// - Original Author Name (year)
/// - Second Contributor (year)
///
/// # Source
/// jwildfire-vars/output/popcorn2_3d.cpp
pub static POPCORN2_3D: VariationDef = VariationDef { ... };
```

Same shape for `pub fn register_effects` entries in
[effects/mod.rs](../../src/effects/mod.rs).

**Per-parameter descriptions go directly in the struct**, not in a
`# Parameters` doc-section. After slice 1 of the plumbing landed,
`VariationParamDef.description` and `EffectParameter.description`
are first-class fields; populate them with `description: Some("...")`
(longhand structs) or the trailing description arg on the `param!`
macro. The Rust JSON dump (§4.1) picks them up automatically — no
comment-parsing needed for per-param prose.

### 3.3 Authoring the prose

For each of ~494 variations and ~? effects:

- **Description**: extract from existing top-of-file `//!` prose where
  available, manual writing where not. Many variations already have a
  one-sentence summary; needs cleanup to remove implementation chatter
  (e.g. "needs_transform divide-out" should not be in the
  user-facing description).
- **Authors**: extract from parentheses in existing comments
  (`(Larry Berlin, 2009)`, `(zephyrtronium / dark-beam)`). Manual
  review for ambiguous cases — multi-author splits, ports vs original
  designers, "Apophysis classic" attributions. **When attribution is
  genuinely unknown, omit the `# Authors` section entirely** — don't
  write a placeholder. The import script treats a missing section as
  empty `authors: []`.
- **Per-parameter descriptions**: this is the bulk of the work. Most
  variations have ~3–7 params; some have 13+. Many params are
  self-documenting by name (`amplitude`, `frequency`), many are not
  (`super_n3`, `_v`, `tmpVV`). Heuristic: if a param name is
  documented in the source `.cpp` comments, lift; otherwise write.

This is **~494 manual reviews + ~2000+ param-description sentences**.
The biggest single time sink in the project. Can be batched per-file,
parallelizable across people if needed.

---

## 4. Bulk import script

### 4.1 Architecture

Hybrid Rust + Python, per the analysis in earlier coordination:

- **Rust binary** `cargo run --bin export_variations_json` walks the
  loaded `VariationRegistry` and dumps every variation's structural
  fields (name, display_name, category, phase, flags, parameters with
  types and defaults, WGSL bodies, version) as JSON. Avoids
  reimplementing Rust parsing for `r#"..."#` raw strings and macro
  expansion.

  **Built 2026-07-31** ([src/bin/export_variations_json.rs](../../src/bin/export_variations_json.rs)).
  646 variations, 3.2 MB — generated on demand into the gitignored
  `output/`, not committed, since it carries every WGSL body.

  Two things it does beyond the original sketch:

  * **Vocabularies come from `to_api_str`**, the same source as
    [`docs/generated/engine-contract.json`](../generated/engine-contract.json),
    so the dump cannot disagree with the contract about what a category
    or feature is called. It also emits the newer wire fields the
    sketch predates: `features[]`, `state_count`, `shader_state_init`,
    `plot_emits`, `aliases`.
  * **It embeds `contract_shape`.** A dump merged against a mismatched
    vocabulary is exactly the failure this whole line of work exists to
    prevent, so the fingerprint travels with the data and the merge can
    refuse rather than silently produce rows nobody can read.

- ~~**Python script** reads the JSON, then opens each `.rs` file in
  `defs/` and extracts description + authors from the structured `///`
  comments. Merges structural data + extracted metadata.~~

  **Superseded 2026-08-22: the Rust binary does this itself.** The
  split was the wrong shape, and the way it failed is the argument
  against it. The Python half was never written, so the exporter's
  "explicit nulls as a visible merge target" were simply nulls: every
  corpus it produced carried 647 of them, and since prose reaches the
  app *only* through the API catalog
  ([`storage::variation_catalog`](../../src/storage/variation_catalog.rs)),
  that is the whole reason variation descriptions were missing
  downstream. A two-step pipeline whose second step does not exist
  looks exactly like a one-step pipeline that works.

  [`variations::docs`](../../src/variations/docs.rs) now parses the
  `///` blocks — description, `# Authors` list, and a markdown-stripped
  `description_plain` — and the exporter fills the three fields
  directly. `cargo run --bin export_variations_json` produces a
  complete corpus in one command, and **refuses to write one with a
  missing description** rather than emitting a null. A `--lib` test
  asserts every registered built-in has prose, so the gap cannot
  silently reopen.

  Per-parameter descriptions never needed comment parsing: they are
  struct fields, emitted straight from the registry.

Effects still need the equivalent, and it is NOT the same job: effect
prose lives in `//` headers in the `.wgsl` files, not in Rust `///`
comments, and `EffectParameter::description` is unpopulated at the
source. [`export_effects_json`](../../src/bin/export_effects_json.rs)
documents both gaps in its own header.

### 4.2 SQL emission shape

The app and API ship in lockstep, so the initial population is a
single fresh import — no conflict resolution, no version-gating.
Either `TRUNCATE variations; INSERT ...;` or just a plain
`INSERT INTO variations ...` against an empty table. Every row goes
in at `version = 1`.

```sql
INSERT INTO variations (
  name, version, display_name, category, phase, description, authors,
  needs_rng, needs_transform, writes_color, init_param_count,
  shader_2d, shader_3d, shader_init,
  parameters  -- JSONB array; each entry carries description
) VALUES (
  'popcorn2_3D', 1, 'Popcorn2 3D', 'full_3d', 'normal',
  'Free-form description...',
  ARRAY['Larry Berlin (2009)'],
  false, true, false, 0,
  $shader_2d$ ... WGSL ... $shader_2d$,
  $shader_3d$ ... WGSL ... $shader_3d$,
  NULL,
  '[
    {"name":"popcorn2_3D_x", "display_name":"X", "param_type":"unlimited_float",
     "default_value":0.1, "min_value":-10.0, "max_value":10.0,
     "description":"Horizontal modulation strength."},
    ...
  ]'::jsonb
);
```

Dollar-quoted strings for WGSL bodies (no escaping).

### 4.3 Version assignment

Trivial for v1: every variation is `version = 1`. The `version` field
exists in the wire format and on the cache for future use (see
[VARIATIONS_WIRE_FORMAT.md §8](VARIATIONS_WIRE_FORMAT.md)) but the
import script doesn't need to compute or bump anything.

Future incremental updates — if they ever happen — would re-introduce
`ON CONFLICT (name) DO UPDATE ... WHERE EXCLUDED.version >
variations.version` and follow the forward-looking bump policy from
the wire-format spec. Not Day-1 work.

---

## 5. Sequencing

Phases that can land independently or in parallel.

```
Phase 1: Client plumbing (no API dependency)
  - §2.1 structural fields
  - §2.2 macro arms
  - §2.3 UI tooltips
  - §2.4 from_download conversion
  → Ships with empty descriptions / no enums. Harmless.

Phase 2: Type-correction audit (no API dependency)
  - §3.1 exhaustive scan
  - Output: classification spreadsheet
  → Can run in parallel with Phase 1.

Phase 3: Type corrections in defs/ (depends on Phase 1 + 2)
  - Apply int→bool and int→enum migrations to defs/
  → Client-side only; built-in variations still ship correctly.

Phase 4: Comment restructure + description writing (depends on Phase 1)
  - §3.2 and §3.3
  - Batched per-file; pause-and-resume friendly
  → Can run in parallel with Phase 3.

Phase 5: Import script (depends on Phases 1, 3, 4 being complete)
  - §4 build the Rust + Python pair
  - Generate first SQL output for review

Phase 6: API team — schema migration (depends on
                       VARIATIONS_WIRE_FORMAT.md additions landing)
  - Add description/authors columns
  - Add per-param description to JSONB parameter spec
  - Wire fields into VariationDownload + VariationListItem responses
  → Coordination work, independent of client work above.

Phase 7: Run the import + client adoption
  - API team runs the SQL migration (fresh INSERTs, all at version 1)
  - Client deploys a version that reads the new fields from API
  - Lockstep release — no cache invalidation choreography needed

Phase 8: Effects parity
  - Repeat Phases 1–7 for effects
  - First effects need an API home at all (no EffectDownload type today)
  - Defer until variations migration ships and proves the pattern
```

Phases 1–4 can land *now*, before API team commits to anything.
Phase 5 needs Phases 1+3+4. Phase 6 is the API team's. Phase 7 is the
join point. Phase 8 is a separate project.

---

## 6. API team coordination

Confirmed in [VARIATIONS_WIRE_FORMAT.md](VARIATIONS_WIRE_FORMAT.md):

- Add `description: Option<String>` to `VariationListItem`
- Add `description: Option<String>` to `ApiVariationParameter`
- Add `authors: Vec<String>` to both `VariationListItem` and
  `VariationDownload`
- All wire additions are serde-default — older clients unaffected
- Names / descriptions / authors are English-only by policy
- Initial population is `version = 1` for the whole corpus; app/API
  ship in lockstep, so no version-gating logic at import time. The
  bump-vs-no-bump policy is forward-looking guidance, not Day-1 work.

Still open with the API team:

1. **Mutation endpoint shape** for description / authors / param
   description edits — generic upsert through the import migration
   only, or a dedicated `PATCH /api/variations/{name}/metadata` for
   admin tooling? The version-bump exemption is easier to enforce via
   a dedicated endpoint.
2. **Author handling for non-bulk-imported variations** — when the
   API gains admin tooling to create variations directly (no client
   `defs/` ancestry), what's the source of the author field? Same
   free-form contract, just filled in by the creator.
3. **Effects API entirely** — when effects move to the API, the wire
   format should mirror variations one-for-one, with the additions
   from this project: per-param `description`, `display_name` on
   `EffectParameter`. Decide once, apply twice (per Phase 8).

---

## 7. Risks

- **Comment-parsing brittleness.** If contributors drift from the
  `# Authors` / `# Parameters` heading convention, the script
  silently drops fields. Mitigation: a `cargo check`–time lint that
  verifies every `pub static VariationDef` has a `///` block with the
  required headings. Cheap to add.
- **The ~494 manual reviews are the long pole.** Realistic estimate:
  weeks of focused work, not days. Suggest batching by sub-directory
  (`apo_misc*.rs` together, `*_3d_misc.rs` together) so reviewers
  build up cpp/Java context once per batch.
- **Type-correction false positives.** Some `int [0, 1]` params may
  genuinely be integer-valued (not boolean) — e.g. a "ring count"
  that just happens to default to 0–1. The audit must inspect the
  WGSL body, not the range. Same for `int [0, N]` — only convert to
  enum when the body has distinct per-value branches.

---

## 8. Definition of done

- All ~494 variations in DB carry `description`, `authors`, and
  per-param `description` (some may have empty `authors` where
  attribution is genuinely unknown).
- All int-as-bool / int-as-enum corrections applied; clients render
  checkboxes / dropdowns where appropriate.
- Variation registry panel shows description and authors per entry.
- Hovering a parameter slider shows its description (variations + effects).
- The `param!` macro supports `enum` and per-param `description`.
- Effects equivalent metadata is captured in `EffectParameter` and
  ready for the eventual API rollout (Phase 8).
- VARIATIONS_WIRE_FORMAT.md compatibility-status section flips from
  "in flight" to "shipped" for all three additions.
