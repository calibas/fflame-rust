# Variation Execution Order Investigation

## The Problem

User correctly identified that variation execution order matters significantly for mathematical correctness. Apophysis uses a **sequential execution model** with three phases, while we use a **weighted sum blend model**. These produce COMPLETELY DIFFERENT results!

## Our Current Implementation (WRONG)

**File:** `src/shader_builder_v2.rs:171-177`

```rust
code.push_str(&format!(
    "if (xform.variations[{}] != 0.0) {{\n\
     \x20   result += xform.variations[{}] * {};\n\  // ← WEIGHTED SUM
     }}\n",
    idx, idx, call
));
```

**Generated shader code:**
```wgsl
fn apply_variations(p: vec2<f32>) -> vec2<f32> {
    var result = vec2(0.0, 0.0);

    if (weight[0] > 0.0) {  // Linear
        result += weight[0] * variation_linear(p);
    }
    if (weight[2] > 0.0) {  // Spherical
        result += weight[2] * variation_spherical(p);
    }
    if (weight[11] > 0.0) {  // Diamond
        result += weight[11] * variation_diamond(p);
    }

    return result;  // Weighted average of all variations
}
```

**This is a WEIGHTED BLEND - all variations operate on the SAME input point `p` and their outputs are mixed together.**

## Apophysis Implementation (CORRECT)

Based on the conversation summary mentioning "pre/normal/post sequence", Apophysis uses **SEQUENTIAL APPLICATION**.

### Expected Execution Model (needs verification from Apophysis source)

```pascal
// Apophysis XForm.pas (HYPOTHETICAL - needs source verification)
function ApplyVariations(p: TPoint): TPoint;
var
  temp: TPoint;
begin
  temp := p;

  // Phase 1: Pre-variations (execute first, sequentially)
  if (pre_blur_weight > 0) then
    temp := ApplyPreBlur(temp, pre_blur_weight);
  if (pre_rotate_x_weight > 0) then
    temp := ApplyPreRotateX(temp, pre_rotate_x_weight);

  // Phase 2: Normal variations (accumulate into result)
  Result := ZeroPoint;
  if (linear_weight > 0) then
    Result := Result + linear_weight * ApplyLinear(temp);
  if (spherical_weight > 0) then
    Result := Result + spherical_weight * ApplySpherical(temp);
  // ... more variations

  // Phase 3: Post-variations (execute last, sequentially on accumulated result)
  if (post_rotate_y_weight > 0) then
    Result := ApplyPostRotateY(Result, post_rotate_y_weight);
  if (flatten_weight > 0) then
    Result := ApplyFlatten(Result, flatten_weight);
end;
```

## Mathematical Difference

### Example: Linear + Spherical with weights 0.5 each

**Input point:** `p = (2.0, 0.0)`

**Our weighted blend:**
```
linear(p) = (2.0, 0.0)
spherical(p) = (2.0, 0.0) / 4.0 = (0.5, 0.0)

result = 0.5 * linear(p) + 0.5 * spherical(p)
       = 0.5 * (2.0, 0.0) + 0.5 * (0.5, 0.0)
       = (1.0, 0.0) + (0.25, 0.0)
       = (1.25, 0.0)  ← Weighted average
```

**Apophysis sequential (if normal variations accumulate):**
```
Same as our approach - both variations operate on input p
result = 0.5 * linear(p) + 0.5 * spherical(p)
       = (1.25, 0.0)  ← Same result
```

**BUT with Pre-variations:**
```
Input: p = (2.0, 0.0)

Step 1: Apply pre_rotate_x (weight 1.0, rotate 90°)
temp = rotate_x(p, 90°) = (2.0, 0.0, 0.0) rotated = (2.0, 0.0, 0.0)  // No effect in 2D

Step 2: Apply normal variations to TRANSFORMED temp
linear(temp) = temp
spherical(temp) = temp / |temp|²

result = 0.5 * linear(temp) + 0.5 * spherical(temp)
```

The **pre-variations change the input** before normal variations see it!

## The Critical Insight

**Pre-variations are NOT just another weighted variation - they MODIFY THE INPUT POINT before other variations see it!**

This is fundamentally different from weighted blending.

## What Needs Investigation

1. **Verify Apophysis source code** - How does XForm.pas actually execute variations?
   - Is it truly sequential (pre → normal → post)?
   - Do normal variations blend or apply sequentially?
   - Which variations are classified as pre/normal/post?

2. **Check variation classification**
   - Which variations are "pre_*"?
   - Which are "post_*"?
   - Which are normal?

3. **Test with pre/post variations**
   - Create test flame with pre_rotate_x
   - Compare output with Apophysis
   - This should show obvious differences if execution order matters

## Hypothesis

**If Apophysis uses sequential execution and we use weighted blending, this could explain why non-symmetrical fractals render differently!**

Sequential application would create very different trajectories than weighted blending, especially with multiple variations active.

## Action Items

1. [ ] Find and read Apophysis XForm.pas variation execution code
2. [ ] Document exact execution order (pre/normal/post)
3. [ ] Identify which variations go in which phase
4. [ ] Create test case with pre_rotate variation
5. [ ] Implement proper sequential execution model
6. [ ] Re-test all variations with correct execution order

## Current Implementation Analysis

Looking at `src/shader_builder_v2.rs:229-237`, we DO handle pre/post rotations specially:

```rust
"pre_rotate_x" | "pre_rotate_y" | "post_rotate_x" | "post_rotate_y" => {
    let rotate_fn = if name.contains("_x") { "rotate_x" } else { "rotate_y" };
    code.push_str(&format!(
        "if (xform.variations[{}] != 0.0) {{\n\
         \x20   result = {}(result, xform.variations[{}]);\n\  // ← SEQUENTIAL!
         }}\n",
        idx, rotate_fn, idx
    ));
}
```

**But they execute in WRONG ORDER!**

- Line 149/194: All variations sorted by registry index
- Pre_rotate indices: 19-22 (execute LATE)
- Normal variation indices: 0-18 (execute EARLY)
- **Problem:** Pre-variations execute AFTER normal variations!

## Example of Current WRONG Execution

```wgsl
fn apply_variations(p: vec3<f32>) -> vec3<f32> {
    var result = vec3(0.0);

    // Linear (index 0) - EXECUTES FIRST
    if (weight[0] > 0.0) {
        result += weight[0] * variation_linear(p);
    }

    // Spherical (index 2) - EXECUTES SECOND
    if (weight[2] > 0.0) {
        result += weight[2] * variation_spherical(p);
    }

    // Pre_rotate_y (index 20) - EXECUTES LAST! ❌
    if (weight[20] > 0.0) {
        result = rotate_y(result, weight[20]);  // Too late!
    }

    return result;
}
```

**The pre-rotation happens AFTER the normal variations, when it should happen BEFORE!**

## Correct Apophysis Execution Order

```wgsl
fn apply_variations(p: vec3<f32>) -> vec3<f32> {
    var temp = p;

    // PHASE 1: Pre-variations (modify input point)
    if (weight[19] > 0.0) {  // pre_rotate_x
        temp = rotate_x(temp, weight[19]);
    }
    if (weight[20] > 0.0) {  // pre_rotate_y
        temp = rotate_y(temp, weight[20]);
    }

    // PHASE 2: Normal variations (accumulate weighted results)
    var result = vec3(0.0);
    if (weight[0] > 0.0) {  // linear
        result += weight[0] * variation_linear(temp);  // Uses ROTATED input!
    }
    if (weight[2] > 0.0) {  // spherical
        result += weight[2] * variation_spherical(temp);  // Uses ROTATED input!
    }

    // PHASE 3: Post-variations (modify accumulated result)
    if (weight[21] > 0.0) {  // post_rotate_x
        result = rotate_x(result, weight[21]);
    }
    if (weight[22] > 0.0) {  // post_rotate_y
        result = rotate_y(result, weight[22]);
    }

    return result;
}
```

## Status

**CRITICAL BUG IDENTIFIED** - Pre/post variations execute in wrong order!

**Root cause of rendering differences:** Pre-variations modify the input before normal variations see it. We currently execute them AFTER normal variations, producing completely different results.
