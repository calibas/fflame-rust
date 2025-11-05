# Documentation Reorganization Plan

**Created:** 2025-10-28
**Status:** In Progress

## Goals

1. **Smaller, focused files** - Easier to navigate, search, and maintain
2. **Clear separation of concerns** - Each doc has single responsibility
3. **Better for AI assistance** - Lower context usage, faster searches
4. **Scalable structure** - Easy to add new topics without bloating existing docs
5. **Navigation hub pattern** - ARCHITECTURE.md as quick reference map with links to detail docs

## Proposed Structure

### Root Level
- **CLAUDE.md** - Quick reference for Claude AI
  - Build commands
  - Common tasks (how to add X)
  - File format specs (.fflame, .palette)
  - Coding guidelines
  - Cross-references to docs/main/ for details
  - Target: 400-600 lines

### docs/main/ - Core Reference Documentation
Detailed technical documentation for each major system:

- **ARCHITECTURE.md** - High-level system design (navigation hub)
  - Module organization diagram
  - Data flow overview (init, frame, event, state)
  - Key constants and limits
  - Cross-references to detailed docs
  - Target: 200-300 lines

- **UI.md** - UI organization and interactions
  - Window layout (5 windows + menu bar)
  - Panel descriptions
  - Event handling
  - Input system (keyboard, mouse, wheel)
  - UiResponse system
  - Target: 200-400 lines

- **RENDERER.md** - FlameRenderer orchestration
  - 3-pass GPU pipeline (compute, accumulate, tonemap)
  - Frame timing and performance
  - State management
  - Reset behavior
  - Target: 200-400 lines

- **PIPELINE.md** - GPU pipeline details
  - Pipeline creation and selection (2D/3D)
  - Bind group layouts
  - Shader compilation
  - Runtime pipeline switching
  - Target: 200-300 lines

- **BUFFERS.md** - GPU data structures and layouts
  - Bind group organization
  - GpuTransform, GpuParams, TonemapParams, AccumulateParams
  - Memory layouts (std140/std430)
  - Alignment rules
  - Target: 200-400 lines

- **TRANSFORMS.md** - Flame algorithm and affine math
  - IFS (Iterated Function System)
  - Affine transformations
  - Transform struct
  - CPU reference implementation
  - Point calculations (r, θ, φ)
  - Target: 200-400 lines

- **SHADERS.md** - WGSL shader system and modular compilation
  - Modular shader components (header, rng, variations, utilities, main)
  - ShaderBuilder system
  - Dynamic shader generation
  - 2D vs 3D shader selection
  - Target: 300-400 lines

- **VARIATIONS.md** - Variation registry and parameter system
  - VariationRegistry architecture
  - Core vs plugin variations
  - Two-tier ID system (0-25 core, 26-49 plugins)
  - Parameter system (Float, Integer, Angle)
  - How to add variations
  - Target: 300-400 lines

- **COLOR.md** - Color pipeline and accumulation
  - Color modes (Transform, Palette, Speed)
  - Palette system and library
  - Histogram accumulation (u32 format)
  - Encoding/decoding
  - Target: 200-400 lines

- **CONFIG.md** - Configuration and state management
  - FractalConfig structure
  - Preset system
  - Undo/redo system
  - Serialization (.fflame files)
  - State capture/restore
  - Target: 200-300 lines

- **EXPORT.md** - PNG export and headless rendering
  - Transparent vs opaque export
  - PNG metadata embedding
  - Headless rendering (CLI mode)
  - Batch export
  - Target: 200-300 lines

- **TESTING.md** - Testing and benchmarking (consolidate existing docs)
  - Unit tests
  - Regression tests
  - Benchmarks (Criterion + simple_benchmark)
  - Visual regression testing
  - Target: 200-300 lines

### docs/archive/ - Historical and Deprecated Documentation
Documents from past investigations, failed experiments, and superseded designs:

**Move here:**
- All `*_PLAN.md` files (planning docs for completed work)
- All `*_PROPOSAL.md` files
- All `*_INVESTIGATION.md` files
- `HISTOGRAM_EVOLUTION.md` (historical)
- `UI-REFACTORING.md` (completed)
- `WORKGROUP_LOCAL_HISTOGRAM_PLAN.md` (failed experiment)
- `F16_PACKED_HISTOGRAM.md` (superseded)
- `ADAPTIVE_SCALE_PLAN.md` (superseded)
- `PER_PIXEL_ADAPTIVE_SCALE_*.md` (failed approach)
- `CONVERGENCE_DIAGNOSTIC_TESTS.md` (investigation)
- `DEEPER_HISTOGRAM_PROPOSAL.md` (superseded)
- `DENSITY_AWARE_COLOR_POC_RESULTS.md` (investigation)
- `FINDINGS_PRESENTATION.md` (investigation)
- `NEURAL_NETWORK_IDEAS.md` (experimental)
- `QUALITY_INVESTIGATION.md` (investigation)
- `ZOOM_OPTIMIZATION_ANALYSIS.md` (investigation)
- `ZOOM_PERFORMANCE_ANALYSIS.md` (investigation)

### docs/projects/ - Current Active Projects
Work-in-progress documentation and recent completed features:

**Move here:**
- `ITERATIONS_PER_THREAD_QUALITY.md` (speed multiplier system)
- `HISTOGRAM_OPTIMIZATION_ATTEMPTS.md` (recent histogram work)
- `U32_HISTOGRAM_CLEANUP.md` (recent cleanup)
- `HISTOGRAM_FINAL.md` (if still relevant)
- Current `STATUS.md` (implementation status)

### docs/experimental/ - Future Plans and Ideas
Forward-looking designs and potential features:

**Keep here:**
- Future variation ideas
- Animation system proposals
- Performance optimization ideas
- Mobile platform considerations

### Platform-Specific (stay in docs/)
- `WASM.md` - WebAssembly build guide
- Mobile platform docs (if created)

## Benefits

### For Claude (AI Assistant)
- ✅ **Faster searches** - Grep/Glob finds relevant content quickly
- ✅ **Lower context usage** - Only read what's needed
- ✅ **Better comprehension** - 200-line focused doc vs 1000-line mega-doc
- ✅ **Precise references** - "See VARIATIONS.md section 3" vs "See ARCHITECTURE.md line 890"

### For Humans
- ✅ **Easier to navigate** - Know exactly where to look
- ✅ **Faster to update** - Change one focused file vs hunting through huge doc
- ✅ **Better git diffs** - Changes to UI don't show up when reviewing shader changes
- ✅ **Clearer ownership** - Each doc has a single responsibility

### For the Project
- ✅ **Easier onboarding** - "Start with ARCHITECTURE.md overview, then read RENDERER.md"
- ✅ **Better cross-references** - Explicit links between related concepts
- ✅ **Scalable** - Add new topics without bloating existing docs

## Current Problems

### ARCHITECTURE.md (1060 lines) - Too Large
Currently covers:
- ✅ Module organization (keep)
- ✅ Data flow (keep)
- ❌ GPU buffers (extract to BUFFERS.md)
- ❌ GPU data structures (extract to BUFFERS.md)
- ❌ Flame algorithm (extract to TRANSFORMS.md)
- ❌ Histogram system (extract to COLOR.md)
- ❌ Speed multiplier (already in ITERATIONS_PER_THREAD_QUALITY.md, just reference)
- ❌ UI organization (extract to UI.md)
- ❌ Modification guide (move to CLAUDE.md)

### CLAUDE.md (698 lines) - Mixed Concerns
Contains both quick reference AND detailed implementation notes. Should focus on quick reference only.

## Implementation Order

1. ✅ **Create this plan document** (DOCUMENTATION_REORGANIZATION_PLAN.md)
2. ✅ **Extract UI.md** from ARCHITECTURE.md (397 lines)
3. 🔄 **Refactor ARCHITECTURE.md to navigation hub**
   - Add "See also" section at top with links to all detail docs
   - Add "Quick Reference" table for fast lookup
   - Remove duplicate UI content (now in UI.md)
   - Keep unique content (module tree, data flow, constants)
   - Target: ~300 lines
4. ⏳ Extract BUFFERS.md from ARCHITECTURE.md
4. ⏳ Extract TRANSFORMS.md from ARCHITECTURE.md
5. ⏳ Extract RENDERER.md from ARCHITECTURE.md
6. ⏳ Extract SHADERS.md from ARCHITECTURE.md + CLAUDE.md
7. ⏳ Extract VARIATIONS.md from CLAUDE.md
8. ⏳ Create COLOR.md (histogram, palettes, color modes)
9. ⏳ Create CONFIG.md (presets, undo/redo, serialization)
10. ⏳ Create EXPORT.md (PNG export, metadata, CLI)
11. ⏳ Consolidate TESTING.md from TESTING-GUIDE.md + TESTING-PLAN.md
12. ⏳ Create PIPELINE.md (GPU pipelines)
13. ⏳ Trim ARCHITECTURE.md to navigation hub (~250 lines)
14. ⏳ Trim CLAUDE.md to quick reference (~500 lines)
15. ⏳ Move archive docs to docs/archive/
16. ⏳ Move project docs to docs/projects/
17. ⏳ Update all cross-references

## Target File Sizes

| Doc Type | Target Lines | Purpose |
|----------|-------------|---------|
| Overview (ARCHITECTURE.md) | 200-300 | Navigation hub, high-level concepts |
| Core reference (BUFFERS.md, SHADERS.md, etc.) | 200-400 | Deep dive on one topic |
| Quick reference (CLAUDE.md) | 400-600 | Essential info + recipes |
| Specialized (WASM.md, TESTING.md) | 200-300 | Platform/process specific |

## Cross-Reference Strategy

### ARCHITECTURE.md Pattern (Navigation Hub)
```markdown
# Architecture Overview

**Detailed Documentation:**
- [UI.md](main/UI.md) - Windows, panels, input handling, UiResponse system
- [BUFFERS.md](main/BUFFERS.md) - GPU layouts and data structures
- [RENDERER.md](main/RENDERER.md) - 3-pass pipeline orchestration
... (all detail docs listed with brief description)

## Quick Reference: Where to Find Things
| I need to... | Read this | Key files |
|--------------|-----------|-----------|
| Understand UI | [UI.md](main/UI.md) | src/ui/mod.rs |
... (task-oriented lookup table)

[Unique content: module tree, data flow diagrams, constants]
```

### Detail Doc Pattern (e.g., RENDERER.md)
```markdown
# Renderer Architecture

**See also:**
- [ARCHITECTURE.md](../ARCHITECTURE.md) - Overall system navigation
- [BUFFERS.md](BUFFERS.md) - GPU buffer layouts (related)
- [SHADERS.md](SHADERS.md) - Shader system (related)

**Code locations:**
- [src/renderer/compute_kernel.rs](../../src/renderer/compute_kernel.rs)
- [src/gpu/pipelines.rs](../../src/gpu/pipelines.rs)

[Detailed content for this topic only]
```

## Success Criteria

- ✅ No single doc exceeds 600 lines (except CLAUDE.md quick ref)
- ✅ Each doc has single clear purpose
- ✅ Easy to find relevant info (clear naming + cross-refs)
- ✅ All archive docs moved out of main docs/
- ✅ ARCHITECTURE.md serves as navigation hub (~250 lines)

---

**Status Legend:**
- ✅ Done
- 🔄 In Progress
- ⏳ Not Started
