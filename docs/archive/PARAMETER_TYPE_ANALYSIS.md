# Parameter Type Analysis for Apophysis Variations

## Proposed ParamType Enum

```rust
pub enum ParamType {
    /// Limited floating-point value with min/max range
    Float,

    /// Unlimited floating-point value (full f32 range: -3.4E38 to +3.4E38)
    UnlimitedFloat,

    /// Integer value (stored as f32, cast for UI display)
    Integer,

    /// Boolean value (0.0 = false, non-zero = true)
    Boolean,

    /// Angle in degrees (0-360, circular wrapping)
    Angle,

    /// Enum/choice value (discrete integer choices)
    /// Example: blurtype (0=linear, 1=radial, 2=gaussian)
    Enum { choices: Vec<String> },
}
```

## Important Constants

- **Apophysis "min" value:** 1.0E-06 (used to avoid division by zero)
- **f32 range:** -3.4E38 to +3.4E38 (NOT f32::MIN which is the most negative value)
- **UnlimitedFloat range:** -3.4E38 to +3.4E38 (full practical f32 range)

## Current Parameter Usage Survey

### Parameters by Current Type

#### Float (bounded ranges)
Most parameters use Float with specific ranges that make sense for the variation:
- `julian.dist`: 0.1 to 5.0 (distance scaling)
- `blob.high`, `blob.low`: 0.0 to 2.0 (wave amplitude)
- `wedge.angle`, `wedge.hole`, `wedge.count`, `wedge.swirl`: Various ranges
- `curl.c1`, `curl.c2`: -2.0 to 2.0 (curl coefficients)
- And many more...

#### Integer (discrete values)
- `julian.power`: -10 to 10 (integer power)
- `juliascope.power`: -10 to 10
- `julia3dz.power`: -10 to 10
- `blob.waves`: -10 to 10 (wave count)
- `ngon.sides`: 3 to 20 (polygon sides)

#### Angle (0-360 degrees)
- `radial_blur.angle`: degrees
- Potentially other rotation parameters

### Parameters That Should Be Boolean

Looking through variations, these are effectively boolean flags:
- `crop.zero`: 0 or 1 (whether to zero out-of-bounds points)
- `pre_crop.zero`: 0 or 1
- `post_crop.zero`: 0 or 1
- `rectangles.x`, `rectangles.y`: 0 or 1 (enable X/Y rectangles)
- `pre_falloff2.invert`: 0 or 1 (invert falloff direction)
- `post_falloff2.invert`: 0 or 1

### Parameters That Should Be Enum

These have discrete choices with specific meanings:
- `pre_falloff2.blurtype`: 0=linear, 1=radial, 2=gaussian
- `post_falloff2.blurtype`: 0=linear, 1=radial, 2=gaussian
- Future variations may have similar mode switches

### Parameters That Could Be UnlimitedFloat

These might benefit from full f32 range (need case-by-case review):
- Multiplier parameters (`mul_x`, `mul_y`, `mul_z`, `mul_c`)
- Center/offset parameters (`x0`, `y0`, `z0`)
- Scale parameters in some contexts
- Coordinate transformation parameters

## Implementation Plan

### 1. Extend ParamType Enum
```rust
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum ParamType {
    Float,
    UnlimitedFloat,
    Integer,
    Boolean,
    Angle,
    Enum { choices: Vec<String> },
}
```

### 2. Add Enum Choices to VariationParameter
```rust
pub struct VariationParameter {
    pub name: String,
    pub display_name: String,
    pub param_type: ParamType,
    pub default_value: f32,
    pub min_value: Option<f32>,
    pub max_value: Option<f32>,

    // For Enum type: optional list of choice labels
    pub enum_labels: Option<Vec<String>>,
}
```

### 3. Update UI Rendering Functions

Add new renderers in `src/ui/variation_params.rs`:
```rust
fn render_boolean_param(ui: &mut Ui, param: &VariationParameter, value: &mut f32) -> Response {
    let mut checked = *value > 0.5;
    let response = ui.checkbox(&mut checked, &param.display_name);
    *value = if checked { 1.0 } else { 0.0 };
    response
}

fn render_enum_param(ui: &mut Ui, param: &VariationParameter, value: &mut f32, choices: &[String]) -> Response {
    let current_idx = (*value as usize).min(choices.len().saturating_sub(1));
    let mut selected = current_idx;

    let response = egui::ComboBox::from_label(&param.display_name)
        .selected_text(&choices[selected])
        .show_ui(ui, |ui| {
            for (idx, choice) in choices.iter().enumerate() {
                ui.selectable_value(&mut selected, idx, choice);
            }
        })
        .inner;

    *value = selected as f32;
    response.unwrap_or_else(|| ui.label("").interact(...))
}

fn render_unlimited_float_param(ui: &mut Ui, param: &VariationParameter, value: &mut f32) -> Response {
    ui.add(
        egui::DragValue::new(value)
            .speed(0.01)
            .prefix(format!("{}: ", param.display_name))
            .clamp_range(-3.4e38..=3.4e38)  // Full f32 range
    )
}
```

### 4. Migration Strategy

**Do NOT auto-migrate!** Review each parameter individually:

1. Create a spreadsheet/document listing all current parameters
2. For each parameter, decide:
   - Keep as Float with current range?
   - Change to Boolean?
   - Change to Enum?
   - Change to UnlimitedFloat?
   - Change to Integer with different range?
3. Update parameter definitions one variation at a time
4. Test each variation after updating

## Questions to Answer Per Parameter

1. **What values does Apophysis allow?**
   - Check Pascal source code
   - Check actual usage in flame files

2. **What is the semantic meaning?**
   - Is it truly continuous (Float)?
   - Is it a flag/switch (Boolean)?
   - Is it a mode selection (Enum)?
   - Is it unbounded (UnlimitedFloat)?

3. **What makes sense for users?**
   - Slider vs. checkbox vs. dropdown?
   - What's the most intuitive control?

4. **Are there special constraints?**
   - Should angles wrap around?
   - Should integers be clamped?
   - Can negative values make sense?

## Example Parameter Reviews Needed

### pre_falloff2 / post_falloff2 (11 parameters)
- `scatter`: Float (0.0-5.0) or UnlimitedFloat? → Review usage
- `mindist`: Float (0.0-5.0) or UnlimitedFloat? → Review usage
- `mul_x`, `mul_y`, `mul_z`, `mul_c`: UnlimitedFloat? → Multipliers often unbounded
- `x0`, `y0`, `z0`: UnlimitedFloat? → Coordinates can be anywhere
- `invert`: **Boolean** → Clearly 0 or 1
- `blurtype`: **Enum** → 0=linear, 1=radial, 2=gaussian

### crop / pre_crop / post_crop (6 parameters)
- `left`, `top`, `right`, `bottom`: UnlimitedFloat? → Coordinates
- `scatter_area`: Float (current: -1.0 to 1.0) → Review if this makes sense
- `zero`: **Boolean** → 0 or 1 flag

### ngon (4 parameters)
- `sides`: Integer (3-20) → Keep range
- `power`: Float or UnlimitedFloat? → Review
- `circle`: Float or UnlimitedFloat? → Review
- `corners`: Float or UnlimitedFloat? → Review

### rectangles (2 parameters)
- `x`: **Boolean** → 0 or 1 flag
- `y`: **Boolean** → 0 or 1 flag

## Next Steps

1. **You review each variation's parameters** (don't start changes yet)
2. Create a decision table with your choices
3. I'll implement the ParamType extensions
4. We'll migrate parameters in batches, testing each batch
5. Update documentation with semantic meanings

## Benefits of This Approach

✅ **Better UX** - Checkboxes for booleans, dropdowns for enums
✅ **Clearer intent** - Parameter types document what they represent
✅ **Type safety** - Prevent invalid values (e.g., blurtype=1.5)
✅ **Apophysis compatibility** - Match semantic meanings, not just ranges
✅ **Future-proof** - Easy to add new parameter types as needed

---

**Status:** Planning phase - awaiting parameter-by-parameter review decisions
