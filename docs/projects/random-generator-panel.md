# Random Fractal Generator Panel

**Status:** Planning
**Created:** 2025-12-29
**Priority:** Medium
**Related:** Random Flame menu option (already implemented)

## Overview

A dedicated panel for generating random fractal flames with full control over the randomization parameters. Extends the simple "Random Flame" menu option into a powerful exploration tool.

## Goals

1. Give users full control over randomization parameters
2. Enable batch generation for exploration
3. Integrate with existing gallery UI for browsing generated fractals
4. Save/load randomization presets for reproducible workflows

---

## UI Design

### Panel Layout

```
┌─────────────────────────────────────────────┐
│  Random Generator                        ≡  │
├─────────────────────────────────────────────┤
│  ┌─────────────────────────────────────────┐│
│  │ Preset: [Default          ▼] [Save][Load]│
│  └─────────────────────────────────────────┘│
│                                             │
│  ▼ Transforms ─────────────────────────────│
│    Count:        [2 ▼] to [5 ▼]            │
│    □ Include Final Transform               │
│                                             │
│  ▼ Variations ─────────────────────────────│
│    Per Transform: [1 ▼] to [3 ▼]           │
│    Weight Range:  [0.2    ] to [1.0    ]   │
│    ☑ Always include Linear (first xform)   │
│                                             │
│    Available Variations:                    │
│    ┌───────────────────────────────────────┐│
│    │ ☑ linear      ☑ sinusoidal  ☑ spherical│
│    │ ☑ swirl       ☑ horseshoe   ☑ polar   │
│    │ ☑ handkerchief☑ heart       ☑ disc    │
│    │ ☑ spiral      ☑ hyperbolic  ☑ diamond │
│    │ ☑ julia       ☑ bent        ☑ waves   │
│    │ ☐ ex          ☐ zcone       ☐ flatten │
│    │ [Select All] [Select None] [2D Only]  │
│    └───────────────────────────────────────┘│
│                                             │
│  ▼ Affine Parameters ──────────────────────│
│    Scale (a,d):   [-1.5  ] to [1.5   ]     │
│    Shear (b,c):   [-0.8  ] to [0.8   ]     │
│    Translate (e,f):[-1.0 ] to [1.0   ]     │
│    □ Allow negative scale (flips)          │
│                                             │
│  ▼ Color & Weight ─────────────────────────│
│    Weight Range:  [0.5   ] to [1.5   ]     │
│    □ Distribute colors evenly              │
│    □ Random palette from library           │
│                                             │
│  ▼ Symmetry ─────────────────────────────────│
│    Type: [None ▼]                          │
│      • None                                │
│      • Bilateral (Horizontal)              │
│      • Bilateral (Vertical)                │
│      • Rotational...  Order: [3 ▼]         │
│      • Dihedral...    Order: [3 ▼]         │
│                                             │
│  ▼ 3D Options (optional) ──────────────────│
│    □ Enable 3D mode                        │
│    □ Include 3D variations                 │
│    Perspective:   [0.0   ] to [5.0   ]     │
│                                             │
│  ─────────────────────────────────────────  │
│  [🎲 Generate Single]  [📦 Generate Batch] │
│                                             │
│  Batch Options:                             │
│    Count: [10    ]  Seed: [        ] [🎲]  │
│                                             │
└─────────────────────────────────────────────┘
```

### Batch Generation Gallery

When "Generate Batch" is clicked, opens a gallery view (using `FractalConfigGallery`) showing all generated fractals with thumbnails. User can:
- Browse generated fractals
- Click to load one
- Delete unwanted ones
- Save favorites to preset library

---

## Data Structures

### RandomGeneratorSettings

```rust
/// Settings for random fractal generation
#[derive(Clone, Serialize, Deserialize)]
pub struct RandomGeneratorSettings {
    /// Name for this preset
    pub name: String,

    // Transform settings
    pub transform_count_min: usize,
    pub transform_count_max: usize,
    pub include_final_transform: bool,

    // Variation settings
    pub variations_per_transform_min: usize,
    pub variations_per_transform_max: usize,
    pub variation_weight_min: f32,
    pub variation_weight_max: f32,
    pub always_include_linear: bool,
    pub enabled_variations: HashSet<String>,

    // Affine ranges
    pub scale_min: f32,
    pub scale_max: f32,
    pub shear_min: f32,
    pub shear_max: f32,
    pub translate_min: f32,
    pub translate_max: f32,
    pub allow_negative_scale: bool,

    // Color & weight
    pub weight_min: f32,
    pub weight_max: f32,
    pub distribute_colors_evenly: bool,
    pub random_palette: bool,

    // Symmetry options
    pub symmetry: SymmetryType,

    // 3D options
    pub enable_3d: bool,
    pub include_3d_variations: bool,
    pub perspective_min: f32,
    pub perspective_max: f32,

    // Batch options
    pub batch_count: usize,
    pub seed: Option<u64>,
}

/// Symmetry type for generated flames
#[derive(Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum SymmetryType {
    #[default]
    None,
    /// Mirror across Y axis (flip X). Adds 1 transform with A=-1.
    BilateralHorizontal,
    /// Mirror across X axis (flip Y). Adds 1 transform with D=-1.
    BilateralVertical,
    /// N-fold rotational symmetry. Adds N-1 transforms rotated by k×(360°/N).
    Rotational(u8),
    /// Dihedral symmetry (rotation + reflection). Adds N transforms total.
    Dihedral(u8),
}

impl Default for RandomGeneratorSettings {
    fn default() -> Self {
        Self {
            name: "Default".to_string(),
            transform_count_min: 2,
            transform_count_max: 5,
            include_final_transform: false,
            variations_per_transform_min: 1,
            variations_per_transform_max: 3,
            variation_weight_min: 0.2,
            variation_weight_max: 1.0,
            always_include_linear: true,
            enabled_variations: default_variations(),
            scale_min: 0.3,
            scale_max: 1.5,
            shear_min: -0.8,
            shear_max: 0.8,
            translate_min: -1.0,
            translate_max: 1.0,
            allow_negative_scale: true,
            weight_min: 0.5,
            weight_max: 1.5,
            distribute_colors_evenly: true,
            random_palette: true,
            symmetry: SymmetryType::None,
            enable_3d: false,
            include_3d_variations: false,
            perspective_min: 0.0,
            perspective_max: 5.0,
            batch_count: 10,
            seed: None,
        }
    }
}
```

---

## Symmetry Transform Generation

Symmetry transforms are added **after** the random transforms. They all use:
- Variation: Linear only (weight 1.0)
- Transform weight: 1.0
- Colors: Distributed evenly from 0.0 to 1.0 among symmetry transforms only

### Bilateral (Horizontal) - Mirror across Y axis
Adds 1 transform:
```
A = -1, B = 0, C = 0, D = 1, E = 0, F = 0
```

### Bilateral (Vertical) - Mirror across X axis
Adds 1 transform:
```
A = 1, B = 0, C = 0, D = -1, E = 0, F = 0
```

### Rotational (N) - N-fold rotational symmetry
Adds N-1 transforms, each rotated by k × (360°/N) for k = 1 to N-1:
```
angle = k × (360° / N)
A = cos(angle), B = -sin(angle), C = sin(angle), D = cos(angle), E = 0, F = 0
```

Example for 3-fold:
- Transform 1: 120° → A=-0.5, B=-0.866, C=0.866, D=-0.5
- Transform 2: 240° → A=-0.5, B=0.866, C=-0.866, D=-0.5

### Dihedral (N) - Rotation + Reflection
Adds N transforms total:
1. First: Bilateral (Horizontal) transform
2. Then: N-1 Rotational transforms

This creates the mathematical dihedral group D_n with n rotations and n reflections.

---

## Implementation Plan

### Phase 1: Core Generator with Settings

**Files:**
- `src/scene/randomize.rs` - Extend with `RandomGeneratorSettings` parameter
- `src/ui/random_generator.rs` - New panel UI

**Tasks:**
1. Add `RandomGeneratorSettings` struct to `src/scene/randomize.rs`
2. Update `generate_random_flame()` to accept settings parameter
3. Create `generate_random_flame_with_settings(settings: &RandomGeneratorSettings) -> Flame`
4. Keep existing `generate_random_flame()` as convenience wrapper using defaults

### Phase 2: Panel UI

**Tasks:**
1. Create `src/ui/random_generator.rs` with panel implementation
2. Add collapsible sections for each category
3. Implement variation checkbox grid
4. Wire up "Generate Single" to load result directly
5. Add panel to workspace and menu (Window > Random Generator)

### Phase 3: Batch Generation

**Tasks:**
1. Add batch generation function: `generate_batch(settings: &RandomGeneratorSettings) -> Vec<FractalConfig>`
2. Create temporary gallery state to hold batch results
3. Open gallery in modal/popup or dedicated area when batch generated
4. Integrate with `FractalConfigGallery` for display
5. Add "Save to Presets" action for favorites

### Phase 4: Settings Presets

**Tasks:**
1. Add save/load for `RandomGeneratorSettings`
2. Store in `assets/random_presets/` or user data directory
3. Built-in presets: "Default", "Simple", "Complex", "Symmetrical", "Organic"
4. Dropdown to select preset, buttons to save/load custom

### Phase 5: Seed Control

**Tasks:**
1. Add optional seed field for reproducible generation
2. Display/copy seed after generation for sharing
3. "Re-roll" button to generate new random seed

---

## File Structure

```
src/
├── scene/
│   └── randomize.rs          # Extended with settings-based generation
└── ui/
    ├── random_generator.rs   # New panel
    └── mod.rs                # Export new panel

assets/
└── random_presets/           # Built-in generator presets
    ├── default.json
    ├── simple.json
    └── complex.json
```

---

## Integration Points

### Menu Bar
- File > Random Flame (existing, uses defaults)
- Window > Random Generator (opens panel)

### Workspace
- Add `PanelType::RandomGenerator` to workspace system
- Default: Not shown (opened via menu)

### Gallery Reuse
- Batch results displayed using `FractalConfigGallery`
- Same thumbnail generation, search, and selection as Preset Library

---

## UI/UX Considerations

1. **Collapsible Sections**: Advanced options hidden by default
2. **Tooltips**: Explain what each parameter affects
3. **Quick Presets**: Buttons for common configurations (Simple, Complex, Weird)
4. **Live Preview**: Optional toggle to auto-regenerate on setting change
5. **History**: Keep last N generated fractals for comparison

---

## Future Enhancements

- **Random Palette Generation**: Generate random palettes alongside flames
  - Completely random colors
  - Harmonious/complementary color schemes
  - Variations on current palette
  - Seed-based for reproducibility
- **Symmetry Options**: Generate flames with rotational/reflective symmetry
- **Style Transfer**: Use an existing flame as a "template" for randomization ranges
- **Genetic Algorithm**: Evolve fractals by selecting favorites from batches
- **Parameter Locking**: Lock specific parameters while randomizing others
- **Undo in Batch**: Regenerate single items in batch without losing others
