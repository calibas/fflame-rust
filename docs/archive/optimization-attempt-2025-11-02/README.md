# Failed Optimization Attempt: Manual Precalculation (2025-11-02)

## Summary

This directory contains documentation from a failed optimization attempt to manually precalculate common trigonometric values in GPU shaders.

**Result:** ❌ Reverted - Made performance ~1% slower instead of faster

**Root Cause:** Modern GPU shader compilers already perform Common Subexpression Elimination (CSE) automatically

## Timeline

1. **2025-11-02 Morning** - Implemented Apophysis-style precalculation
   - Commits: 471dd00, b9a3f35
   - Added precalc of r, r2, theta, sin_theta, cos_theta in `apply_variations()`
   - Updated variation signatures to accept precalculated values

2. **2025-11-02 Afternoon** - Benchmarked and discovered failure
   - Benchmark showed +16.29ms slower (+0.96%)
   - User questioned why no performance improvement
   - Investigated and discovered compiler already does CSE

3. **2025-11-02 Afternoon** - Reverted optimization
   - Commit: 5b10087
   - Restored original straightforward code
   - Documented findings

## Key Lesson

**Trust modern shader compilers.** What worked for CPU rendering in 2005 (Apophysis) doesn't apply to GPU shader compilers in 2025.

Modern compilers (SPIR-V, DXC, Metal) include:
- Common Subexpression Elimination (CSE)
- Dead Code Elimination (DCE)
- Aggressive inlining
- Platform-specific optimizations

Manual micro-optimizations often hurt performance by:
- Increasing register pressure
- Adding function call overhead
- Preventing compiler from applying platform-specific optimizations

## Documents

1. **[VARIATION_OPTIMIZATION_ANALYSIS.md](VARIATION_OPTIMIZATION_ANALYSIS.md)** - Original analysis of potential optimization
2. **[PRECALCULATION_QUALITY_ANALYSIS.md](PRECALCULATION_QUALITY_ANALYSIS.md)** - Analysis of whether optimization affects quality
3. **[SHADER_COMPILER_CSE_ANALYSIS.md](SHADER_COMPILER_CSE_ANALYSIS.md)** - Analysis of why revert was needed

## Benchmark Data

```
Before optimization (commit 711c947): 1701.80ms ± 14.50ms
After optimization (commit b9a3f35):  1718.09ms ± 12.52ms
After revert (commit 5b10087):       [expected to return to ~1700ms]
```

## See Also

- [CLAUDE.md](../../../CLAUDE.md) - Updated with guidelines about trusting shader compilers
- Git commits: 471dd00, b9a3f35 (implementation), 5b10087 (revert)
