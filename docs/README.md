# Documentation Structure

Complete guide to the fflame-rust documentation organization.

## Quick Navigation

**For AI Assistants:** Start with [../CLAUDE.md](../CLAUDE.md) for quick reference, then dive into topic docs below.

**For Developers:** Start with [ARCHITECTURE.md](ARCHITECTURE.md) for system overview and navigation.

**For Testing:** See [TESTING-GUIDE.md](TESTING-GUIDE.md) for all testing commands.

---

## Documentation Hierarchy

```
docs/
├── ARCHITECTURE.md          # System overview and navigation hub
├── TESTING-GUIDE.md         # Complete testing and profiling guide
├── STATUS.md                # Implementation status vs original design
├── WASM.md                  # WebAssembly build guide
├── outline.md               # Original design goals
│
├── main/                    # Current implementation (detailed topic docs)
│   ├── UI.md                # Windows, panels, input handling
│   ├── BUFFERS.md           # GPU layouts, bind groups, data structures
│   ├── TRANSFORMS.md        # Flame algorithm, IFS, thread isolation
│   ├── RENDERER.md          # 3-pass pipeline, FlameRenderer
│   ├── SHADERS.md           # WGSL modular system, ShaderBuilder
│   ├── VARIATIONS.md        # All 26 variations, registry
│   ├── COLOR.md             # Color modes, palette, histogram
│   ├── CONFIG.md            # FractalConfig, presets, undo/redo
│   └── EXPORT.md            # PNG export, metadata, CLI batch mode
│
├── projects/                # Active development work
│   └── README.md            # What goes here and how to organize
│
├── experimental/            # Future ideas and proposals
│   └── README.md            # Future features, experimental designs
│
└── archive/                 # Historical documentation
    ├── README.md            # Archive organization and purpose
    └── histogram/           # Histogram evolution history (15 docs)
        └── README.md        # Complete histogram investigation timeline
```

---

## Documentation Categories

### 1. Core Navigation (docs/)

**[ARCHITECTURE.md](ARCHITECTURE.md)** - START HERE
- System overview and module organization
- Data flow diagrams
- Quick reference tables
- Links to all detailed documentation
- Critical code paths and hot paths

**[TESTING-GUIDE.md](TESTING-GUIDE.md)** - Testing Reference
- Unit tests, regression tests, benchmarks
- Command reference with examples
- What's tested and how to run

**[STATUS.md](STATUS.md)** - Implementation Status
- What's implemented vs original design
- Current features and limitations
- Priority breakdown

**[WASM.md](WASM.md)** - WebAssembly Guide
- Build commands for web
- Platform-specific details
- Limitations and workarounds

**[outline.md](outline.md)** - Original Design
- Original goals and vision
- Design decisions and rationale

### 2. Detailed Implementation (docs/main/)

**Current system documentation - organized by topic:**

**[UI.md](main/UI.md)** (542 lines)
- Window layout and panels
- Input handling (keyboard, mouse, wheel)
- UiResponse system
- State management patterns

**[BUFFERS.md](main/BUFFERS.md)** (565 lines)
- GPU buffer layouts and bind groups
- Data structures (GpuTransform, GpuParams, etc.)
- Memory layout rules (std140 vs std430)
- Buffer update patterns

**[TRANSFORMS.md](main/TRANSFORMS.md)** (640 lines)
- Flame algorithm and IFS explanation
- Transform structure (affine + variations)
- CPU and GPU implementation
- Thread isolation and parallelism
- Point iteration mechanics

**[RENDERER.md](main/RENDERER.md)** (607 lines)
- 3-pass rendering pipeline
- FlameRenderer architecture
- Per-frame render flow
- PNG export (transparent/opaque paths)

**[SHADERS.md](main/SHADERS.md)** (671 lines)
- WGSL modular shader system
- ShaderBuilder dynamic compilation
- Shader component organization
- Variation code generation

**[VARIATIONS.md](main/VARIATIONS.md)** (659 lines)
- All 26 core variations
- Variation registry architecture
- Parameter system
- Adding new variations

**[COLOR.md](main/COLOR.md)** (605 lines)
- Color modes (Transform, Palette, Speed)
- Palette system and interpolation
- Histogram accumulation (u32 format)
- Accumulation controls

**[CONFIG.md](main/CONFIG.md)** (795 lines)
- FractalConfig structure
- Preset system
- Undo/redo architecture
- JSON serialization
- Asset loading

**[EXPORT.md](main/EXPORT.md)** (817 lines)
- PNG export (interactive and CLI)
- Dual export paths (transparent/opaque)
- Metadata embedding (tEXt chunks)
- Batch export mode

**Total:** 5,901 lines of focused topic documentation

### 3. Active Work (docs/projects/)

**Current development projects:**
- (Empty - add active project docs here as work begins)

**Examples of what goes here:**
- Feature implementation plans
- Refactoring project documentation
- Active investigation reports
- Work-in-progress designs

**See [projects/README.md](projects/README.md)** for organization guidelines.

### 4. Future Ideas (docs/experimental/)

**Experimental features and future proposals:**
- (Empty - add experimental designs here)

**Examples of what goes here:**
- Tiled high-resolution export proposal
- Animation system design
- CUDA backend exploration
- Adaptive sampling research
- Alternative architecture proposals

**See [experimental/README.md](experimental/README.md)** for organization guidelines.

### 5. Historical Archive (docs/archive/)

**Completed investigations and historical context:**

**histogram/** (15 documents)
- Complete evolution: textureStore → u16 packed → u32 unpacked
- Failed optimization attempts
- Design decision documentation
- See [archive/histogram/README.md](archive/histogram/README.md)

**See [archive/README.md](archive/README.md)** for archive organization and purpose.

---

## Documentation Principles

### 1. Single Source of Truth

Each topic has ONE authoritative document:
- ✅ TRANSFORMS.md is THE flame algorithm reference
- ✅ COLOR.md is THE color system reference
- ❌ Don't duplicate content across multiple docs

Other docs should **link** to the authoritative source, not duplicate content.

### 2. Appropriate Detail Level

**CLAUDE.md** - Quick reference only (quick facts + links)
**ARCHITECTURE.md** - Navigation hub (summaries + links)
**docs/main/*.md** - Complete detailed documentation

### 3. Clear Cross-References

Every doc should link to related docs:
- "See also:" section at the top
- Inline links to related topics
- "Related:" section at the bottom

### 4. Living Documentation

Documentation should evolve with the code:
- Update docs when implementation changes
- Archive old docs when systems evolve
- Move completed projects to archive
- Keep experimental ideas separate from implementation docs

### 5. Self-Sufficient

Each topic doc should be readable standalone:
- Doesn't require reading other docs first
- Includes enough context to understand the topic
- Links to related topics for deeper understanding

---

## Documentation Workflow

### When Starting New Work

1. Check **docs/experimental/** for existing proposals
2. Create **docs/projects/feature-name-plan.md** for active work
3. Track progress in the project doc

### When Completing Work

1. Update **docs/main/*.md** with implementation details
2. Move project doc to **docs/archive/** (preserve for history)
3. Update **ARCHITECTURE.md** if structure changed

### When Investigating Alternatives

1. Document exploration in **docs/projects/**
2. If approach fails, move to **docs/archive/** with "what we learned"
3. If approach succeeds, update **docs/main/** and archive investigation

### When Proposing Future Features

1. Create doc in **docs/experimental/**
2. Include rationale, design ideas, open questions
3. When work begins, move to **docs/projects/**

---

## File Naming Conventions

**Topic docs (main/):** `TOPIC.md` (all caps, e.g., `TRANSFORMS.md`)

**Project docs (projects/):** `feature-name-plan.md`, `investigation-topic.md`

**Experimental docs (experimental/):** `PROPOSAL-feature.md`, `EXPERIMENT-approach.md`

**Archive docs (archive/):** Preserve original naming, organize in topic folders

---

## Statistics

**Current Documentation (2025-10-28):**
- CLAUDE.md: 661 lines (quick reference)
- ARCHITECTURE.md: 703 lines (navigation hub)
- docs/main/: 5,901 lines across 9 topic files
- Total active docs: ~7,265 lines
- Archive: 15 histogram docs in archive/histogram/

**Documentation Reorganization (2025-10-28):**
- Extracted 9 focused topic files from ARCHITECTURE.md
- ARCHITECTURE.md reduced from 1,060 → 703 lines (-34%)
- Archived 15 histogram investigation docs
- Created organized folder structure (main/, projects/, experimental/, archive/)

---

**For questions about documentation organization, see this README or ask in git commit history.**
