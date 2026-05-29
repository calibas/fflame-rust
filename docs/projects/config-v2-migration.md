# Config v1 → v2 migration tracker

Running list of changes accumulating for the eventual `CURRENT_CONFIG_VERSION`
bump to 2. **This document is a tracker, not an active project plan** — the
migration code isn't being written yet. We're collecting the changes that
will need handling when v2 lands.

## Why we haven't bumped yet

The user is manually tracking the fairly small set of v1 `.fflame` files
in the repo, and has another feature in flight that will also need to be
part of v2. Bumping prematurely would force migration code for each
change individually; batching minimizes the number of migration steps
to write + test.

## What's accumulated so far

### 1. Tonemap default value shifts

Several `defaults.rs` constants were recalibrated alongside the Levels
scale-invariance change. v1 `.fflame` files that didn't serialize these
fields rely on the (now-different) deserialize-time default.

| Constant | Old default | New default | Notes |
|---|---|---|---|
| `DEFAULT_EXPOSURE` | `1.0` | `0.5` | Recalibrated against scale-invariant Levels |
| `DEFAULT_GAMMA` | `1.0` (`2.2` via function) | `1.5` | `default_gamma()` was hardcoded `2.2` pre-bump, now reads the constant |
| `DEFAULT_GAMMA_THRESHOLD` | `0.0025` | `5.0` | Recalibration |
| `DEFAULT_LEVELS_HIGH` | `1000.0` (raw density) → `1.0` (× mean, post-P1) → `10.0` | `10.0` | Two unit changes already happened; see "Levels units" below |

**v2 migration**: The intent is for old flames to *pick up* the new
defaults (they're an improvement, not a regression). So the simplest
migration is "do nothing — let serde's default-on-missing take effect."
The only field that needs *value translation* is `levels_high`.

### 2. Levels units change (already shipped on v1 silently)

The `levels-scale-invariance` PR changed `levels_high` / `levels_low`
units from raw cumulative density to multiples of `sample_density`,
without bumping the version. So:

- A v1 `.fflame` saved *before* that PR has `levels_high` in raw units
  (e.g. `1000` was the default; `20000` or higher for explicit overrides).
- A v1 `.fflame` saved *after* that PR has `levels_high` in × mean units
  (e.g. `5.0`, `0.14`, etc.).
- Both look like `version: 1` to the parser.

**v2 migration heuristic**: divide explicit `levels_high` values by ~1000
if greater than ~100 (above any realistic × mean value). Skip if absent
(default-on-missing takes care of it). The 100× safety threshold cleanly
separates old units (typically 100s–100,000s) from new units (typically
0.1–50).

User decision: divide by 1000 specifically, with the threshold as the
guard against double-applying.

### 3. DOF Focus Distance default shift

| Constant | Old default | New default | Notes |
|---|---|---|---|
| `DEFAULT_DOF_FOCUS_DISTANCE` | `1.0` | `0.0` | Apophysis hardcodes `0.0`; v1 default mismatched. |

Invisible when DOF is off (`DEFAULT_DOF_BLUR_STRENGTH = 0.0`), so v1
files that omit the field stay visually identical on load. When blur
is enabled the change is visible, but no v1 files in the repo enable
blur by default — the user-personal `output/*.fflame` set is the only
risk and the user is tracking those manually.

**v2 migration**: same "do nothing" pattern as section 1 — let
serde's default-on-missing apply the new value.

### 4. (Pending feature) — placeholder

A separate in-flight feature will land before v2 ships. Add notes here
once it's clearer what fields/semantics change.

## v1 files in the repo

These need manual review/update at v2 bump (none should silently shift):

- `assets/presets.fflame` (9 entries, manually updated during P7 of the
  tonemap-and-palette branch with new tonemap values)
- `tests/visual/configs/**/*.fflame` (visual regression baselines —
  expected to render the same image before/after defaults shift since
  Levels is mostly at defaults across the corpus)
- `output/*.fflame` (user's personal flame collection — user is tracking
  these manually)

## Out of scope for v2 migration itself

- **API contract changes**: the bigger lift here is the API/integration
  layer — most v1 → v2 work is on that side, not in the local file format.
  Tracked separately.
- **Tone curve shape**: not touched by v2; the curve `points` field
  shape is stable.
- **Effect chain definitions**: stable.

## When to bump

Bump `CURRENT_CONFIG_VERSION: u32 = 1 → 2` in
`src/config/fractal_config.rs` once:

1. The currently-pending feature has landed and we know its v2 semantics.
2. We've decided the migration code can live in
   `migrate_v1_to_v2(config)` (post-deserialize, operating on the
   `Self` value) vs. needing JSON-level pre-processing (to distinguish
   "present in JSON" from "default").
3. The deserialize-time migration is implemented and unit-tested.

At that point this doc becomes the implementation checklist.
