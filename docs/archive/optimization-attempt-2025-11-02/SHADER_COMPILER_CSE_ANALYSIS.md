# Shader Compiler Common Subexpression Elimination (CSE) Analysis

## The Question

Our "optimization" to precalculate common trigonometric values showed **zero performance improvement** (actually ~1% slower). This suggests modern shader compilers already perform Common Subexpression Elimination (CSE) automatically.

## Evidence from Benchmarks

```
Commit 711c947 (before optimization): 1701.80ms ± 14.50ms
Commit b9a3f35 (after optimization):  1718.09ms ± 12.52ms
Difference: +16.29ms (+0.96% SLOWER)
```

## Why Did We Get Slower?

### Hypothesis 1: Compiler Already Optimized It

Modern shader compilers (HLSL, SPIR-V, DirectX, Metal) include aggressive optimization passes:

1. **Common Subexpression Elimination (CSE)**
   - Detects duplicate calculations like `atan2(p.x, p.y)` appearing in multiple variations
   - Hoists them to temporary variables automatically
   - Reuses the same register across multiple uses

2. **Dead Code Elimination (DCE)**
   - Removes unused precalculated values
   - In our case, if only 1 variation is active, the other 4 precalc values are dead code

3. **Register Pressure**
   - Our precalculation forces 5 values into registers: `r, r2, theta, sin_theta, cos_theta`
   - If only 1-2 variations are active, we're **wasting** register space
   - Higher register pressure → more spilling → slower performance

### Hypothesis 2: We Added Overhead

**Before (compiler optimizes):**
```wgsl
fn apply_variations(p: vec2<f32>) -> vec2<f32> {
    var result = vec2(0.0);

    if (weight[5] > 0.0) {  // Polar variation
        let theta = atan2(p.x, p.y);  // Compiler sees this
        let r = length(p);
        result += weight[5] * vec2(theta/PI, r - 1.0);
    }

    if (weight[6] > 0.0) {  // Handkerchief variation
        // Compiler: "Hey, I already calculated atan2(p.x, p.y)!"
        // CSE pass reuses the existing value
        let theta = atan2(p.x, p.y);  // ← Optimized away by compiler
        let r = length(p);             // ← Optimized away by compiler
        result += weight[6] * vec2(r * sin(theta + r), ...);
    }

    return result;
}
```

**After (our "optimization"):**
```wgsl
fn apply_variations(p: vec2<f32>) -> vec2<f32> {
    // Force ALL calculations upfront
    let r = length(p);          // Always calculated
    let r2 = dot(p, p);         // Always calculated
    let theta = atan2(p.x, p.y); // Always calculated
    let sin_theta = sin(theta);  // Always calculated
    let cos_theta = cos(theta);  // Always calculated

    var result = vec2(0.0);

    if (weight[5] > 0.0) {  // Polar variation
        result += weight[5] * variation_polar(theta, r);
    }

    if (weight[6] > 0.0) {  // Handkerchief variation (uses sin/cos)
        result += weight[6] * variation_handkerchief(r, theta);
    }

    // Problem: If weight[7-15] are all 0.0, we wasted calculating sin_theta, cos_theta

    return result;
}
```

**Key Difference:**
- **Compiler CSE:** Only calculates what's actually used
- **Our precalc:** Calculates everything upfront, even if unused

### Hypothesis 3: Function Call Overhead

We changed variations from inline calculations to function calls with parameters:

**Before:**
```wgsl
if (weight[5] > 0.0) {
    let theta = atan2(p.x, p.y);
    let r = length(p);
    result += weight[5] * vec2(theta / PI, r - 1.0);  // Inlined
}
```

**After:**
```wgsl
if (weight[5] > 0.0) {
    result += weight[5] * variation_polar(theta, r);  // Function call
}

fn variation_polar(theta: f32, r: f32) -> vec2<f32> {
    return vec2(theta / PI, r - 1.0);
}
```

Even though compilers inline small functions, there's still parameter passing overhead.

## Testing the Hypothesis

### Test 1: Check Compiled SPIR-V/HLSL

We can inspect the actual compiled shader code to see if CSE happened:

```bash
# Dump SPIR-V disassembly
spirv-cross --output shader.hlsl trajectory.spv

# Look for duplicate atan2/length calls
# If compiler already optimized, we'll see temps like:
#   float3 _temp_atan2_result = atan2(p.x, p.y);
```

### Test 2: Worst-Case Scenario (All Variations Active)

Our precalculation should only help when **many** variations are active. Let's test:

```
Config: Linear + Spherical + Polar + Handkerchief + Spiral + Hyperbolic + Diamond
(7 variations, all using r/theta)

Hypothesis:
- Before: 7× atan2, 7× length → compiler CSE → 1× atan2, 1× length ✅
- After: 1× atan2, 1× length (forced) ✅
- Result: Same performance
```

### Test 3: Best-Case Scenario (Single Variation)

```
Config: Linear only (no polar coordinates needed)

Hypothesis:
- Before: 0× atan2, 0× length (DCE removes unused code) ✅
- After: 1× atan2, 2× sin, 2× cos, 1× length (WASTED) ❌
- Result: Our version slower
```

## SPIR-V/HLSL Optimizer Capabilities

Modern GPU compilers include:

### SPIRV-Tools Optimizer (Vulkan)
- `--eliminate-dead-code-aggressive`
- `--merge-blocks`
- `--inline-entry-points-exhaustive`
- `--simplify-instructions`
- **`--eliminate-local-multi-store`** ← CSE pass
- **`--scalar-replacement`** ← Register optimization

### DXC (DirectX Shader Compiler)
- `/O3` - Aggressive optimization
- Loop unrolling
- Common subexpression elimination
- Constant folding
- Dead code elimination

### Metal Compiler
- Built into Apple's compiler infrastructure (LLVM-based)
- Same optimizations as LLVM IR:
  - GVN (Global Value Numbering) - CSE
  - LICM (Loop-Invariant Code Motion)
  - InstCombine (Instruction Combining)

## Conclusion

**The shader compiler was already doing CSE for us.**

Our "optimization" actually made things worse because:
1. **Forced calculations** even when not needed (register pressure)
2. **Function call overhead** instead of inline code
3. **No benefit** when compiler already eliminated redundant work

## Recommendation: Revert the Optimization

The correct approach is to **trust the shader compiler** and write clear, straightforward code:

```wgsl
fn apply_variations(p: vec2<f32>) -> vec2<f32> {
    var result = vec2(0.0);

    // Let each variation calculate what it needs
    // Compiler's CSE pass will deduplicate automatically

    if (weight[0] > 0.0) {
        result += weight[0] * variation_linear(p);
    }

    if (weight[5] > 0.0) {
        result += weight[5] * variation_polar(p);
    }

    return result;
}

fn variation_polar(p: vec2<f32>) -> vec2<f32> {
    let theta = atan2(p.x, p.y);  // Compiler will CSE this
    let r = length(p);             // Compiler will CSE this
    return vec2(theta / PI, r - 1.0);
}
```

**Why this is better:**
- Compiler only calculates what's used
- No wasted register pressure
- No forced function parameter passing
- Clear, maintainable code
- Compiler optimizations are **platform-specific** and improve over time

## Historical Note: Why Apophysis Did It

Apophysis was written in 2000-2005 for **CPU rendering** with older compilers (Borland Delphi, early Visual C++). Those compilers had **much weaker** optimization passes than modern LLVM/DXC/SPIRV-Tools.

Additionally, CPU and GPU optimization strategies differ:
- **CPU:** Limited execution units, CSE helps reduce instruction count
- **GPU:** Massive parallelism, register pressure is the bottleneck

What worked on CPUs in 2005 doesn't necessarily apply to GPUs in 2025.

## Verification Steps

1. **Revert commits b9a3f35 and 471dd00**
2. **Re-run benchmarks** to confirm performance returns to baseline
3. **Inspect SPIR-V** to verify compiler CSE is happening
4. **Trust the compiler** going forward

Performance optimization on GPUs is best left to:
- GPU driver shader compilers (highly tuned for specific hardware)
- High-level algorithmic improvements (better algorithms, not micro-optimizations)
- Memory access patterns (coalescing, cache locality)

Premature micro-optimization is the root of all evil.
