# UX Improvements Project

**Status:** In Progress
**Created:** 2025-12-28
**Updated:** 2026-01-06
**Priority:** High

## Overview

This project addresses accumulated UX debt in the fractal flame renderer. The application has powerful features but lacks discoverability, progressive complexity for different skill levels, and general polish that would make it more approachable.

## Goals

1. Make the app approachable for beginners while retaining power for experts
2. Add contextual help for cryptic parameters
3. Streamline common workflows (randomization, transform duplication)
4. Reduce UI clutter by hiding unused/advanced features
5. Better organize panels and controls

---

## Phase 1: Tooltips & Context Help ✅ COMPLETED

**Priority:** High
**Complexity:** Medium
**Status:** Implemented

### Problem
Many parameters have cryptic names (e.g., "a", "b", "c", "d", "e", "f" for affine transforms) or specialized meanings that aren't obvious to users unfamiliar with fractal flames.

### Solution
Add tooltips to all UI controls explaining:
- What the parameter does
- Typical value ranges
- Visual effect of changing it

### Implementation

1. **Create tooltip content file** (`locales/tooltips_en.yml`):
   ```yaml
   tooltips:
     affine_a: "Horizontal scaling. 1.0 = no change, 0.5 = half size"
     affine_b: "Horizontal shear. Skews the shape left/right"
     affine_c: "X translation. Moves the transform horizontally"
     affine_d: "Vertical shear. Skews the shape up/down"
     affine_e: "Vertical scaling. 1.0 = no change, 0.5 = half size"
     affine_f: "Y translation. Moves the transform vertically"
     transform_weight: "Probability of selecting this transform. Higher = more frequent"
     transform_color: "Color index (0-1) for points from this transform"
     exposure: "Overall brightness. Higher values = brighter image"
     gamma: "Tone curve adjustment. Lower = more contrast in midtones"
     # ... etc
   ```

2. **Add tooltip helper function** in `src/ui/mod.rs`:
   ```rust
   fn tooltip(ui: &mut egui::Ui, key: &str) {
       ui.label("ℹ️").on_hover_text(t!(&format!("tooltips.{}", key)));
   }
   ```

3. **Apply to all parameter controls**:
   ```rust
   ui.horizontal(|ui| {
       ui.label("a:");
       ui.add(egui::DragValue::new(&mut transform.a));
       tooltip(ui, "affine_a");
   });
   ```

### Files to Modify
- `locales/en.yml` - Add tooltip strings
- `src/ui/transforms.rs` - Transform parameters
- `src/ui/tone_mapping.rs` - Color and tonemap parameters
- `src/ui/view.rs` - Camera controls
- `src/ui/settings.rs` - Rendering settings

### Estimated Work
- Create tooltip content: 2-3 hours
- Add tooltip helper: 30 minutes
- Apply to all controls: 3-4 hours

---

## Phase 2: Progressive Disclosure

**Priority:** High
**Complexity:** High

### Problem
The full UI shows 70+ variations per transform, dozens of rendering parameters, and advanced features that overwhelm beginners.

### Solution
Implement skill-level modes that progressively reveal complexity:
- **Beginner**: Core controls only (transforms, basic colors, presets)
- **Standard**: Common parameters (affine, common variations, basic tonemap)
- **Expert**: Full access to everything

### Implementation

1. **Add skill level to SystemSettings** (`src/storage/settings.rs`):
   ```rust
   #[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Default)]
   pub enum SkillLevel {
       Beginner,
       #[default]
       Standard,
       Expert,
   }

   pub struct SystemSettings {
       // ... existing fields
       pub skill_level: SkillLevel,
   }
   ```

2. **Create visibility rules** (`src/ui/visibility.rs`):
   ```rust
   pub fn show_affine_params(level: SkillLevel) -> bool {
       matches!(level, SkillLevel::Standard | SkillLevel::Expert)
   }

   pub fn show_all_variations(level: SkillLevel) -> bool {
       matches!(level, SkillLevel::Expert)
   }

   pub fn show_accumulation_controls(level: SkillLevel) -> bool {
       matches!(level, SkillLevel::Expert)
   }
   ```

3. **Conditionally render UI elements**:
   ```rust
   if visibility::show_affine_params(skill_level) {
       // Render affine sliders
   }
   ```

### What to Hide by Skill Level

| Feature | Beginner | Standard | Expert |
|---------|----------|----------|--------|
| Preset selection | ✅ | ✅ | ✅ |
| Transform list | ✅ | ✅ | ✅ |
| Transform weights | ❌ | ✅ | ✅ |
| Affine parameters | ❌ | ✅ | ✅ |
| Triangle editor | ❌ | ✅ | ✅ |
| Basic variations (5) | ✅ | ✅ | ✅ |
| Common variations (15) | ❌ | ✅ | ✅ |
| All variations (70+) | ❌ | ❌ | ✅ |
| Color mode selection | ❌ | ✅ | ✅ |
| Palette selection | ✅ | ✅ | ✅ |
| Exposure/Gamma | ✅ | ✅ | ✅ |
| Tonemap curve | ❌ | ✅ | ✅ |
| Accumulation controls | ❌ | ❌ | ✅ |
| Iterations per thread | ❌ | ❌ | ✅ |
| Deterministic RNG | ❌ | ❌ | ✅ |
| Path editor | ❌ | ❌ | ✅ |

### Files to Modify
- `src/storage/settings.rs` - Add SkillLevel enum
- `src/ui/mod.rs` - Pass skill level to all panels
- `src/ui/settings.rs` - Add skill level selector
- All panel files - Conditional rendering

### Estimated Work
- Design visibility rules: 1 hour
- Implement SkillLevel system: 2 hours
- Apply to all UI panels: 4-6 hours

---

## Phase 3: Randomize Button ✅ COMPLETED

**Priority:** High (listed in STATUS.md)
**Complexity:** Medium
**Status:** Implemented

### Problem
Users have no easy way to explore fractal space. Creating interesting flames from scratch requires expertise.

### Solution
Add a "Randomize" button that generates random but visually interesting flames.

### Implementation

1. **Create randomization module** (`src/scene/randomize.rs`):
   ```rust
   pub fn generate_random_flame(rng: &mut impl Rng) -> Flame {
       let num_transforms = rng.gen_range(2..=5);
       let mut transforms = Vec::new();

       for _ in 0..num_transforms {
           transforms.push(random_transform(rng));
       }

       // Ensure at least one transform has linear variation
       // Add some probability for each variation category
       // Balance weights to sum to 1.0

       Flame { transforms, ..Default::default() }
   }

   fn random_transform(rng: &mut impl Rng) -> Transform {
       // Random affine with reasonable ranges
       // Random selection of 1-3 variations
       // Random but balanced weight
   }
   ```

2. **Add button to Settings panel**:
   ```rust
   if ui.button("🎲 Randomize").clicked() {
       let flame = randomize::generate_random_flame(&mut rand::thread_rng());
       config_manager.load_flame(flame);
   }
   ```

3. **Optional: Seed input for reproducibility**:
   ```rust
   ui.horizontal(|ui| {
       if ui.button("🎲").clicked() { /* randomize */ }
       ui.text_edit_singleline(&mut seed_string);
   });
   ```

### Randomization Strategy
- 2-5 transforms (more transforms = more complex)
- Each transform gets 1-3 variations from "visually interesting" subset
- Affine parameters in reasonable ranges (-2 to 2 for scale/shear, -1 to 1 for translation)
- Color values spread across palette (0, 0.25, 0.5, 0.75, 1.0)
- Weights balanced but not uniform

### Files to Create/Modify
- `src/scene/randomize.rs` - New file
- `src/scene/mod.rs` - Export module
- `src/ui/settings.rs` - Add button

### Estimated Work
- Design randomization algorithm: 2 hours
- Implement and test: 3-4 hours

---

## Phase 4: Transform Clone ✅ COMPLETED

**Priority:** Medium
**Complexity:** Low
**Status:** Implemented

### Problem
No way to duplicate a transform. Users must manually recreate similar transforms.

### Solution
Add a "Clone" button next to each transform that creates an exact copy.

### Implementation

1. **Add clone button to transform list** (`src/ui/transforms.rs`):
   ```rust
   ui.horizontal(|ui| {
       // Existing transform controls
       if ui.button("📋").on_hover_text("Clone transform").clicked() {
           clone_index = Some(index);
       }
   });

   // After the loop:
   if let Some(idx) = clone_index {
       let cloned = flame.transforms[idx].clone();
       flame.transforms.insert(idx + 1, cloned);
       // Rebalance weights
   }
   ```

2. **Handle weight rebalancing**:
   - Option A: Split cloned transform's weight in half
   - Option B: Proportionally reduce all weights
   - Option C: Let user adjust manually (simplest)

### Files to Modify
- `src/ui/transforms.rs` - Add clone button

### Estimated Work
- Implementation: 1 hour

---

## Phase 5: Transform Panel Improvements ✅ COMPLETED

**Priority:** High
**Complexity:** High
**Status:** Implemented

### Problem
1. All 70+ variations shown for each transform (overwhelming)
2. No easy way to add/remove variations
3. Transform summary shows too much information

### Solution

#### 5.1: Show Only Active Variations
Display only variations that have non-zero weights, with an "Add Variation" button.

```rust
// Instead of showing all variations:
for (variation_name, weight) in transform.active_variations() {
    ui.horizontal(|ui| {
        ui.label(variation_name);
        ui.add(egui::DragValue::new(weight));
        if ui.button("❌").clicked() {
            remove_variation = Some(variation_name.clone());
        }
    });
}

if ui.button("➕ Add Variation").clicked() {
    show_variation_picker = true;
}
```

#### 5.2: Variation Picker Dialog
Modal or dropdown showing available variations by category:
- Basic 2D (Linear, Sinusoidal, Spherical, Swirl, Horseshoe)
- Advanced 2D (Polar, Heart, Disc, Spiral, Julia, etc.)
- 3D Depth (Zcone, Flatten, ZScale)
- 3D Rotation (PreRotateX/Y, PostRotateX/Y)
- Parameterized (JuliaN, Blob)

#### 5.3: Collapsible Transform Details
Use collapsing headers to hide advanced information:

```rust
ui.collapsing("Affine Transform", |ui| {
    // a, b, c, d, e, f sliders
});

ui.collapsing("Post Transform", |ui| {
    // Post-affine if used
});

// Always visible: variations, weight, color
```

#### 5.4: Connect to Shader Builder
When variations change, trigger shader rebuild:
- Already happens through ConfigManager
- Ensure variation picker updates correctly

### Files to Modify
- `src/ui/transforms.rs` - Major refactor
- `src/ui/mod.rs` - Add variation picker state

### Estimated Work
- Active variations display: 2 hours
- Variation picker: 3-4 hours
- Collapsible sections: 1-2 hours
- Testing shader integration: 1 hour

---

## Phase 6: Remove Unused Elements ✅ COMPLETED

**Priority:** Medium
**Complexity:** Low
**Status:** Implemented

### Problem
Some UI elements may not be functional or useful (e.g., "Density Scale").

### Resolution
- Investigated `density_scale` - it IS used in accumulate shader for density normalization
- Reviewed other parameters for actual usage
- All current parameters are functional and documented with tooltips
- Added i18n support for all UI strings

---

## Phase 7: Label Experimental Features ✅ COMPLETED

**Priority:** Low
**Complexity:** Low
**Status:** Implemented

### Problem
Experimental features (like path editor) aren't marked as such, potentially confusing users.

### Solution
Add "(Experimental)" suffix or badge to experimental features:

```rust
ui.collapsing("🧪 Path Editor (Experimental)", |ui| {
    ui.label("This feature is under development and may change.");
    // Path editor UI
});
```

### Experimental Features to Label
- Path editor
- Any other in-progress features

### Files to Modify
- Relevant UI files with experimental features

### Estimated Work
- 30 minutes

---

## Phase 8: Panel Reorganization

**Priority:** Medium
**Complexity:** Medium

### Problem
Too many separate panels for related features:
- Colors, View, and Rendering could be combined
- Transform and Triangle Editor are naturally connected

### Solution

#### Option A: Combine Related Panels
1. **Transform Editor** (merge Transform + Triangle Editor)
   - Tabbed interface: "Parameters" | "Visual Editor"
   - Single panel for all transform editing

2. **Display Settings** (merge View + Colors + Rendering)
   - Sections: Camera, Colors, Tone Mapping, Performance
   - Or tabbed: "View" | "Colors" | "Render"

#### Option B: Smart Docking Presets
Create workspace presets that group related panels:
- "Editing" layout: Transform + Triangle side by side
- "Color" layout: Colors + Palette + Tone Mapping together
- Users can switch between or customize

### Implementation Approach

Prefer **Option A** for cleaner UX:

1. Create combined panel components
2. Use egui tabs or collapsing headers for organization
3. Remove old separate panels
4. Update workspace defaults

### Files to Modify
- `src/ui/mod.rs` - Panel structure
- `src/ui/workspace.rs` - Default layouts
- Create new combined panel files or merge existing

### Estimated Work
- Design new panel structure: 2 hours
- Implement combined panels: 4-6 hours
- Testing and polish: 2 hours

---

## Implementation Order

Recommended order based on impact and dependencies:

1. ~~**Phase 4: Transform Clone**~~ ✅ Completed
2. ~~**Phase 7: Label Experimental**~~ ✅ Completed
3. ~~**Phase 1: Tooltips**~~ ✅ Completed
4. ~~**Phase 3: Randomize Button**~~ ✅ Completed
5. ~~**Phase 5: Transform Panel**~~ ✅ Completed
6. ~~**Phase 6: Remove Unused**~~ ✅ Completed
7. **Phase 2: Progressive Disclosure** (Benefits from Phase 5) - **NEXT**
8. **Phase 8: Panel Reorganization** (Major refactor, do last)

### Remaining Work
- **Phase 2**: Skill level modes (Beginner/Standard/Expert) (~6-8 hours)
- **Phase 8**: Panel reorganization (~8-10 hours)

---

## Success Metrics

- Beginners can create interesting flames within 5 minutes
- All parameters have explanatory tooltips
- Transform editing requires fewer clicks
- UI clutter reduced by 50% in Beginner mode
- No confusion about experimental features

---

## Related Documentation

- [UI.md](../main/UI.md) - Current UI architecture
- [ui-improvements-docking.md](../archive/projects/ui-improvements-docking.md) - Previous UX work
- [STATUS.md](../STATUS.md) - Project status and priorities
