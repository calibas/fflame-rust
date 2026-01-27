# Xaos (Chaos) Support

**Status:** Planning
**Priority:** Low-Medium
**Estimated Effort:** 12-16 hours
**Complexity:** High (GPU algorithm + UI matrix editor)

---

## Overview

Implement Xaos (also called "chaos") to control the probability of transitioning between specific transforms during iteration. This modifies the standard chaos game by allowing transforms to favor or avoid jumping to specific other transforms, enabling directed flows through transform space.

---

## Background

### Standard Chaos Game
Without xaos, transform selection is based purely on transform density (weight):
```
P(transform i) = weight[i] / Σ(all weights)
```

Every transform has equal access to all other transforms in the next iteration.

### With Xaos
Each transform stores a weight array controlling transition probabilities:
```
P(k → i) = (weight[i] × xaos[k][i]) / Σ(all modified weights)
```

Where:
- `k` = current transform
- `i` = next transform
- `xaos[k][i]` = xaos weight for jumping from k to i (default: 1.0)

This creates directed graphs instead of uniform random selection.

---

## Apophysis Implementation

### Data Structure (XForm.pas:96)
```pascal
modWeights: array [0..NXFORMS] of double;
```

Each transform stores an array of weights, one per transform:
- `modWeights[j]` = weight for jumping to transform j from current transform
- Default: `1.0` for all (no xaos effect)

### Building Probability Table (ControlPoint.pas:434-457)
For each transform k, build a lookup table:
```pascal
for i := 0 to n - 1 do begin
  tp[i] := xform[i].density * xform[k].modWeights[i];
  totValue := totValue + tp[i];
end;
```

This creates a `PropTable` - a lookup table of size 512 (`PROP_TABLE_SIZE`) where each entry points to a transform weighted by the modified probabilities.

### Transform Selection (RenderingImplementation.pas:306)
```pascal
xf := xf.PropTable[Random(PROP_TABLE_SIZE)];
```

Instead of choosing the next transform purely by density, it uses the current transform's `PropTable` which encodes the xaos weights.

### XML Format (XForm.pas:1419-1429)
```xml
<xform ... chaos="1.0 0.5 0.0 1.0" />
```

Lists weights for jumping to transforms 0, 1, 2, 3...
- Default: `1.0` for all transforms
- Length matches number of transforms in flame

---

## Examples

### Normal (No Xaos)
```xml
chaos="1 1 1"
```
Equal probability to all transforms (standard chaos game).

### Isolate
```xml
chaos="0 1 0"
```
Transform 0 can **only** jump to transform 1 (never to 0 or 2).

### Avoid
```xml
chaos="1 1 0.1"
```
Transform 0 **rarely** jumps to transform 2 (10% of normal probability).

### Favor
```xml
chaos="1 2 1"
```
Transform 0 is **twice as likely** to jump to transform 1.

### Cycle/Path
```xml
xform 0: chaos="0 1 0 0"  → only to xform 1
xform 1: chaos="0 0 1 0"  → only to xform 2
xform 2: chaos="0 0 0 1"  → only to xform 3
xform 3: chaos="1 0 0 0"  → only to xform 0
```
Creates a directed cycle: 0 → 1 → 2 → 3 → 0.

---

## Proposed Implementation

### Phase 1: Data Structure (2-3 hours)

**Add xaos field to Transform:**
```rust
// src/scene/transforms.rs
pub struct Transform {
    // ... existing fields ...

    /// Xaos weights for transitioning to other transforms
    /// xaos[j] = weight for jumping to transform j (default: 1.0)
    /// None = use default (all 1.0), Some(vec) = custom weights
    pub xaos: Option<Vec<f32>>,
}
```

**Default behavior:**
- `None` → All weights are 1.0 (no xaos effect)
- `Some(vec)` → Use custom weights (length = num_transforms)

**Flame-level management:**
```rust
// src/scene/transforms.rs
impl Flame {
    /// Ensure all transforms have xaos arrays matching transform count
    pub fn sync_xaos_arrays(&mut self) {
        let n = self.transforms.len();
        for xform in &mut self.transforms {
            if let Some(ref mut xaos) = xform.xaos {
                // Resize to match transform count
                xaos.resize(n, 1.0);
            }
        }
    }

    /// Enable xaos for all transforms (initialize to default 1.0)
    pub fn enable_xaos(&mut self) {
        let n = self.transforms.len();
        for xform in &mut self.transforms {
            if xform.xaos.is_none() {
                xform.xaos = Some(vec![1.0; n]);
            }
        }
    }

    /// Disable xaos for all transforms
    pub fn disable_xaos(&mut self) {
        for xform in &mut self.transforms {
            xform.xaos = None;
        }
    }
}
```

---

### Phase 2: GPU Implementation (4-5 hours)

**Challenge:** GPU needs fast random access to probability tables.

**Option A: Pre-computed Lookup Tables (Apophysis approach)**

Upload 512-entry lookup tables per transform:
```rust
// src/gpu/buffers.rs
pub struct GpuXaosTable {
    /// Pre-computed lookup table for each transform
    /// Size: MAX_TRANSFORMS × 512 entries
    /// Each entry is a u32 transform index
    pub tables: [u32; MAX_TRANSFORMS * 512],
}
```

**Shader usage:**
```wgsl
// Select next transform using current transform's table
let table_offset = current_xform_id * 512u;
let random_index = rng_next_u32(&rng) % 512u;
let next_xform_id = xaos_tables.tables[table_offset + random_index];
```

**Pros:**
- Fast lookup (O(1))
- Matches Apophysis algorithm exactly
- Simple shader code

**Cons:**
- Large GPU buffer (32 transforms × 512 entries × 4 bytes = 64 KB)
- Needs rebuild on transform weight/xaos change
- Wasted space if xaos not used

**Option B: On-the-Fly Weighted Selection (Direct approach)**

Upload xaos weight matrices:
```rust
// src/gpu/buffers.rs
pub struct GpuXaosWeights {
    /// Xaos weights for all transforms
    /// Size: MAX_TRANSFORMS × MAX_TRANSFORMS
    /// weights[k][i] = xaos weight from transform k to transform i
    pub weights: [[f32; MAX_TRANSFORMS]; MAX_TRANSFORMS],
}
```

**Shader usage:**
```wgsl
fn select_next_transform(current_id: u32, rng: ptr<function, RngState>) -> u32 {
    // Build cumulative distribution for current transform
    var cumulative = array<f32, MAX_TRANSFORMS>();
    var total = 0.0;

    for (var i = 0u; i < params.num_transforms; i++) {
        let weight = transforms[i].weight * xaos_weights.weights[current_id][i];
        total += weight;
        cumulative[i] = total;
    }

    // Sample using cumulative distribution
    let r = rng_next_f32(rng) * total;
    for (var i = 0u; i < params.num_transforms; i++) {
        if (r < cumulative[i]) {
            return i;
        }
    }
    return 0u;  // Fallback
}
```

**Pros:**
- Smaller GPU buffer (32 × 32 × 4 bytes = 4 KB)
- No rebuild needed (just upload matrix)
- Cleaner data structure

**Cons:**
- More expensive per iteration (O(n) loop)
- Different algorithm than Apophysis (may affect quality?)

**Recommendation:** Start with **Option B** (on-the-fly). It's simpler, more flexible, and performance impact is likely negligible (32 iterations << 256 variation calculations).

---

### Phase 3: XML Import/Export (2-3 hours)

**Import:**
```rust
// src/apophysis_xml.rs
fn parse_xform_xaos(xform_elem: &Element) -> Option<Vec<f32>> {
    if let Some(chaos_str) = xform_elem.get_attr("chaos") {
        let weights: Vec<f32> = chaos_str
            .split_whitespace()
            .filter_map(|s| s.parse::<f32>().ok())
            .collect();

        if !weights.is_empty() {
            return Some(weights);
        }
    }
    None
}
```

**Export:**
```rust
// src/apophysis_xml.rs
fn write_xform_xaos(xform: &Transform, writer: &mut Writer) {
    if let Some(ref xaos) = xform.xaos {
        // Check if all weights are 1.0 (default)
        let is_default = xaos.iter().all(|&w| (w - 1.0).abs() < 1e-6);

        if !is_default {
            // Format as space-separated list
            let chaos_str = xaos.iter()
                .map(|w| format!("{:.6}", w))
                .collect::<Vec<_>>()
                .join(" ");

            writer.write_attribute("chaos", &chaos_str)?;
        }
    }
}
```

**Handling transform count mismatches:**
```rust
// When loading flame with N transforms but xaos has M entries:
fn normalize_xaos(xaos: Vec<f32>, target_len: usize) -> Vec<f32> {
    let mut result = xaos;

    if result.len() < target_len {
        // Pad with 1.0 (default)
        result.resize(target_len, 1.0);
    } else if result.len() > target_len {
        // Truncate
        result.truncate(target_len);
    }

    result
}
```

---

### Phase 4: UI - Xaos Matrix Editor (4-6 hours)

**Challenge:** Editing an N×N matrix is complex UI-wise.

**UI Design: Two Views**

#### View 1: "To" View (Default)
Shows where the **selected transform** can jump TO:

```
┌─────────────────────────────────────┐
│ Xaos - Transform 0                   │
│                                      │
│ [To ●] [From ○]                      │
│                                      │
│ Transform 0 can jump to:             │
│                                      │
│ → Transform 0:  [═══════╪═══] 1.00  │
│ → Transform 1:  [═══╪═══════] 0.50  │
│ → Transform 2:  [               ] 0.00  │
│ → Transform 3:  [═══════╪═══] 1.00  │
│                                      │
│ [Reset All to 1.0] [Disable Xaos]   │
└─────────────────────────────────────┘
```

**Implementation:**
```rust
// src/ui/xaos_editor.rs
pub fn render_xaos_to_view(
    ui: &mut egui::Ui,
    config_manager: &mut ConfigManager,
    selected_xform: usize,
) -> UpdateType {
    ui.heading("Xaos - To View");
    ui.label(format!("Transform {} can jump to:", selected_xform));

    let num_xforms = config_manager.active_config().flame.transforms.len();
    let mut max_update = UpdateType::None;

    for target_id in 0..num_xforms {
        ui.horizontal(|ui| {
            ui.label(format!("→ Transform {}:", target_id));

            let mut weight = get_xaos_weight(config_manager, selected_xform, target_id);
            let response = ui.add(
                egui::Slider::new(&mut weight, 0.0..=2.0)
                    .show_value(true)
            );

            if response.changed() {
                if let Ok(update_type) = config_manager.update_param(
                    ConfigPath::TransformXaos {
                        from_index: selected_xform,
                        to_index: target_id,
                    },
                    weight.into(),
                    response.dragged()
                ) {
                    max_update = max_update.max(update_type);
                }
            }
        });
    }

    max_update
}
```

#### View 2: "From" View
Shows which transforms can jump FROM to the **selected transform**:

```
┌─────────────────────────────────────┐
│ Xaos - Transform 1                   │
│                                      │
│ [To ○] [From ●]                      │
│                                      │
│ Transforms that can jump to 1:       │
│                                      │
│ Transform 0 →:  [═══╪═══════] 0.50  │
│ Transform 1 →:  [═══════╪═══] 1.00  │
│ Transform 2 →:  [═══════════] 2.00  │
│ Transform 3 →:  [═══════╪═══] 1.00  │
│                                      │
│ [Reset All to 1.0] [Disable Xaos]   │
└─────────────────────────────────────┘
```

This view helps understand "which transforms feed into this one".

---

### Phase 5: ConfigManager Integration (1-2 hours)

**Add ConfigPath variant:**
```rust
// src/config/delta.rs
pub enum ConfigPath {
    // ... existing variants ...

    /// Xaos weight from one transform to another
    TransformXaos {
        from_index: usize,
        to_index: usize,
    },

    /// Enable/disable xaos for entire flame
    XaosEnabled,
}
```

**Implement get_value and apply_delta:**
```rust
// src/config/manager.rs
impl ConfigManager {
    fn get_value(&self, path: &ConfigPath) -> Result<ConfigValue> {
        match path {
            ConfigPath::TransformXaos { from_index, to_index } => {
                let xform = &config.flame.transforms[*from_index];
                let weight = xform.xaos
                    .as_ref()
                    .and_then(|xaos| xaos.get(*to_index))
                    .copied()
                    .unwrap_or(1.0);  // Default
                Ok(weight.into())
            }
            ConfigPath::XaosEnabled => {
                let enabled = config.flame.transforms
                    .iter()
                    .any(|xform| xform.xaos.is_some());
                Ok(enabled.into())
            }
            // ...
        }
    }

    fn apply_delta_commit(&mut self, delta: &ConfigDelta) -> Result<UpdateType> {
        match &delta.path {
            ConfigPath::TransformXaos { from_index, to_index } => {
                let xform = &mut self.current.flame.transforms[*from_index];

                // Ensure xaos array exists
                if xform.xaos.is_none() {
                    let n = self.current.flame.transforms.len();
                    xform.xaos = Some(vec![1.0; n]);
                }

                // Set weight
                if let Some(ref mut xaos) = xform.xaos {
                    if let Some(slot) = xaos.get_mut(*to_index) {
                        *slot = delta.new_value.try_into()?;
                    }
                }

                Ok(UpdateType::Flame)  // Need to rebuild xaos tables
            }
            ConfigPath::XaosEnabled => {
                let enabled: bool = delta.new_value.try_into()?;
                if enabled {
                    self.current.flame.enable_xaos();
                } else {
                    self.current.flame.disable_xaos();
                }
                Ok(UpdateType::Flame)
            }
            // ...
        }
    }
}
```

**Undo/redo support:**
Xaos changes are automatically tracked by ConfigManager's delta system.

---

## Use Cases

### 1. Create Paths
Force specific transform sequences:
```
xform 0 → xform 1 → xform 2 → xform 0 (cycle)
```

Set:
- `xform[0].xaos = [0, 1, 0]` → only to xform 1
- `xform[1].xaos = [0, 0, 1]` → only to xform 2
- `xform[2].xaos = [1, 0, 0]` → only to xform 0

### 2. Isolate Transforms
Prevent certain transforms from interacting:
```
xform 0 and xform 1 never connect to xform 2
```

Set:
- `xform[0].xaos[2] = 0.0`
- `xform[1].xaos[2] = 0.0`

### 3. Favor Connections
Make transform 0 prefer jumping to transform 1:
```
xform 0 → xform 1 (2× more likely than others)
```

Set:
- `xform[0].xaos = [1, 2, 1, 1]`

### 4. Advanced Structures
Create complex directed graphs for sophisticated flame design:
- Tree structures (root → branches, never reverse)
- Layered flows (background → midground → foreground)
- Symmetry breaking (different paths for different regions)

---

## Implementation Files

### New Files
- `src/ui/xaos_editor.rs` - Xaos matrix editor UI

### Modified Files
- `src/scene/transforms.rs` - Add `xaos: Option<Vec<f32>>` field
- `src/config/delta.rs` - Add `TransformXaos` and `XaosEnabled` variants
- `src/config/manager.rs` - Implement xaos get/set logic
- `src/gpu/buffers.rs` - Add `GpuXaosWeights` struct (Option B)
- `shaders/core/header.wgsl` - Add xaos weights binding
- `shaders/core/main_2d.wgsl` - Add `select_next_transform()` function
- `shaders/core/main_3d.wgsl` - Add `select_next_transform()` function
- `src/apophysis_xml.rs` - Parse and write `chaos` attribute
- `src/ui/mod.rs` - Add Xaos tab to transform editor

---

## Testing Strategy

### Unit Tests
```rust
#[test]
fn test_xaos_default() {
    // All weights 1.0 → standard chaos game
}

#[test]
fn test_xaos_isolate() {
    // Weight 0 → transform never selected
}

#[test]
fn test_xaos_favor() {
    // Weight 2 → transform selected 2× more often
}

#[test]
fn test_xaos_sync_on_transform_add() {
    // Adding transform resizes all xaos arrays
}

#[test]
fn test_xaos_xml_roundtrip() {
    // Import → export → import preserves xaos
}
```

### Visual Tests
- Create test flame with known xaos pattern
- Verify visual output matches Apophysis
- Test with various xaos configurations (isolate, favor, cycle)

### Performance Tests
- Measure iteration speed with/without xaos
- Ensure on-the-fly selection doesn't slow rendering significantly
- Target: < 5% performance impact

---

## Technical Considerations

### Transform Count Changes
When adding/removing transforms, xaos arrays need updating:
```rust
impl Flame {
    pub fn add_transform(&mut self, xform: Transform) {
        self.transforms.push(xform);
        self.sync_xaos_arrays();  // Resize all xaos arrays
    }

    pub fn remove_transform(&mut self, index: usize) {
        self.transforms.remove(index);

        // Remove column from all xaos arrays
        for xform in &mut self.transforms {
            if let Some(ref mut xaos) = xform.xaos {
                xaos.remove(index);
            }
        }
    }
}
```

### Default Behavior
- **No xaos:** Standard chaos game (weight-based selection)
- **All 1.0:** Equivalent to no xaos (no effect)
- **Some 0.0:** Creates directed graph (certain paths blocked)

### Memory Usage
- CPU: `MAX_TRANSFORMS × MAX_TRANSFORMS × 4 bytes = 4 KB` (negligible)
- GPU: Same (small buffer)
- No significant memory impact

### GPU Performance
On-the-fly selection loop:
```wgsl
for (var i = 0u; i < num_transforms; i++) {
    // O(n) per iteration
}
```

With 32 transforms:
- 32 iterations × 2 ops (multiply + add) = 64 ops
- Compared to variation calculations (100+ ops), this is minor
- Expected impact: < 5%

---

## Success Criteria

### Functionality
- [ ] Xaos weights stored per transform
- [ ] GPU respects xaos during iteration
- [ ] XML import/export preserves xaos
- [ ] UI editor for xaos matrix (To/From views)
- [ ] Undo/redo works for xaos changes
- [ ] Transform add/remove updates xaos arrays

### Compatibility
- [ ] Flames without xaos render identically
- [ ] Apophysis flames with xaos import correctly
- [ ] Visual output matches Apophysis (within tolerance)

### Performance
- [ ] < 5% performance impact when xaos enabled
- [ ] No impact when xaos disabled (default)

### Usability
- [ ] Clear UI labels ("To" vs "From" views)
- [ ] Easy to reset xaos (all to 1.0)
- [ ] Easy to disable xaos entirely
- [ ] Preview mode works while editing xaos

---

## Risks and Mitigations

### Risk: GPU Algorithm Complexity
**Impact:** Medium
**Mitigation:** Start with on-the-fly approach (Option B). If performance is an issue, switch to pre-computed tables (Option A).

### Risk: UI Complexity (N×N Matrix)
**Impact:** High
**Mitigation:** Use To/From views instead of full matrix. Most users only edit one row at a time.

### Risk: Visual Differences from Apophysis
**Impact:** Medium (if Option B differs)
**Mitigation:** Test with known flames. If differences exist, implement Option A (exact Apophysis algorithm).

### Risk: Transform Count Changes
**Impact:** Low
**Mitigation:** Auto-sync xaos arrays on add/remove. Clear documentation of behavior.

---

## Future Enhancements

### Xaos Visualization
- Graph view showing transform connections
- Arrow thickness = weight strength
- Highlight blocked paths (weight 0)

### Xaos Presets
- "Cycle" - Create circular path
- "Isolate" - Disconnect specific transforms
- "Favor" - Boost specific connections
- "Reset" - All weights to 1.0

### Advanced Editing
- Copy/paste xaos rows
- Mirror xaos (make symmetric)
- Invert xaos (swap high/low weights)

### Xaos Templates
- Save xaos patterns for reuse
- Apply pattern to new flames
- Library of common xaos structures

---

## Related Documentation

- `docs/main/TRANSFORMS.md` - Transform system reference
- `docs/projects/apophysis-remaining-features.md` - Feature #6 (Xaos)
- Apophysis Source: `XForm.pas:96`, `ControlPoint.pas:434-457`, `RenderingImplementation.pas:306`

---

## Priority Justification

**Low-Medium Priority** because:
- Rarely used feature (advanced technique)
- Most flames work fine without xaos
- Complex UI and GPU implementation
- Other features (XML export, final transform) more important

**Should implement after:**
1. XML Export (#3 in apophysis-remaining-features.md)
2. Final Transform (#7)
3. UI improvements (docking system)

**Reasoning:** Xaos is a power-user feature. Focus on basics first (export, final transform), then advanced features.

---

**Created:** 2025-11-08
**Status:** Planning
**Next Steps:** Review design, prioritize after higher-priority features
