# Linked Transforms (Pre/Post Transforms)

## Overview

Linked transforms are a convenience feature that creates deterministic transform chains via xaos routing. A "linked" pair means transform C always routes to transform D, and D is only reachable from C. This creates a guaranteed two-step pipeline: the "pre" transform fires first, then the "post" transform follows.

This is a **UI-only feature** built entirely on top of the existing xaos system. No shader, GPU, or core algorithm changes are needed.

## Background

### How It Works in Apophysis/JWildfire

In Apophysis, "linked transforms" are not stored as a separate field. The linked status is **inferred from the xaos matrix pattern**:

- Transform C (pre) has exactly one non-zero outgoing weight (pointing to D)
- Transform D (post) has exactly one non-zero incoming weight (coming from C)
- D routes to the rest of the flame as C originally did

The Fr0st tutorial describes it as: "Creating a linked transform basically splits your existing transform in two."

### Key Properties

- **Opacity is orthogonal**: A transform with opacity 0 still participates in iteration (processes the point, routes via xaos), it just doesn't plot to the histogram. This allows invisible intermediate steps.
- **Color speed is preserved**: No automatic color speed changes when linking. Users can manually set color_speed=1.0 on the pre-transform if they want color to pass through unchanged.
- **Chains are possible**: A->B->C naturally falls out of the pattern (B is both a "post" of A and a "pre" of B->C).

## Design

### Detection Algorithm

A linked pair (pre=C, post=D) is detected when:

1. **C has exactly one non-zero outgoing xaos weight**, and it points to D
2. **D has exactly one non-zero incoming xaos weight**, and it comes from C

This is a scan of the xaos matrix. For N transforms, detection is O(N^2).

**Chain detection**: After finding all pairs, chains are assembled by following links:
- If (A->B) and (B->C) are both detected pairs, they form chain A->B->C
- The UI displays chains as a single entry rather than separate pairs

### UI Location: Xaos Editor Panel

The linked transforms section lives at the **bottom of the xaos editor panel** (`src/ui/xaos_editor.rs`), below the existing grid.

#### Layout

```
[Existing xaos grid...]

--- Linked Transforms ---

  Chain: T1 -> T3 -> T5           [Unlink]
  Link:  T2 -> T4                 [Unlink]

  [Link Transforms...]
```

#### "Link Transforms" Action

When the user clicks "Link Transforms":
1. A dropdown/combo appears to select the **Pre** transform
2. A second dropdown appears to select the **Post** transform
3. On confirm, the xaos matrix is modified:
   - Pre's outgoing row: set all to 0, except Post = 1.0
   - All other transforms' weight to Post: set to 0 (Post only reachable from Pre)
   - Post's outgoing row: copy Pre's original outgoing weights (Post routes where Pre used to)
   - Post's self-weight: set to 0 (Post doesn't route to itself)

This is a batch xaos update through ConfigManager.

#### "Unlink" Action

Each detected link/chain has an "Unlink" button. Unlinking a pair (C, D):
1. Restore C's outgoing weights: copy D's current outgoing weights back to C
2. Restore D's incoming weights: set all other transforms' weight to D back to 1.0
3. Restore C's self-weight to 1.0
4. Restore D's self-weight to 1.0

This effectively merges D's routing back into C and makes D freely reachable again.

For chains (A->B->C), the Unlink button removes the entire chain, restoring the first transform's routing from the last transform in the chain.

#### Visual Indicators

- In the xaos grid, linked cells are highlighted with a distinct border or icon
- Linked transform pairs show a link icon in the transform list panel (optional, future)

### Adding New Transforms

When a new transform is added, existing links must be preserved:

**Current behavior**: `ensure_xaos_size()` fills new rows/columns with 1.0 (default weight).

**Problem**: If transform D is a "post" (only reachable from C), adding transform E gives E a default weight of 1.0 to D, breaking the "only reachable from C" constraint.

**Solution**: When adding a transform, after `ensure_xaos_size()`:
1. Detect existing linked pairs before the add
2. After the add, restore the link constraints:
   - For each detected post-transform D: set new transform's weight to D back to 0

This happens in the transform add handler (structural change in ConfigManager).

### Deleting Transforms

When a transform involved in a link is deleted:
- If the **pre** is deleted: the post becomes a normal transform (all incoming weights reset to 1.0)
- If the **post** is deleted: the pre's outgoing weights need restoration (copy from post's row before deletion)
- If a **middle** in a chain is deleted: the chain breaks into separate parts

This happens in the transform delete handler.

### No New Data Model Fields

Linked status is **not stored** in the Flame or Transform structs. It is always inferred from the xaos matrix at display time. This matches how Apophysis handles it and avoids data model complexity.

## Implementation Steps

### Step 1: Detection Functions

Add to `src/scene/transforms.rs` on the `Flame` impl:

```rust
/// A detected linked pair: pre_idx routes exclusively to post_idx
pub struct LinkedPair {
    pub pre: usize,
    pub post: usize,
}

/// Detect all linked transform pairs from the xaos matrix
pub fn detect_linked_pairs(&self) -> Vec<LinkedPair>

/// Detect chains from linked pairs (e.g., A->B->C)
pub fn detect_linked_chains(&self) -> Vec<Vec<usize>>
```

Unit tests for detection with various xaos configurations.

### Step 2: Link Action

Add to `src/scene/transforms.rs`:

```rust
/// Generate xaos changes to link pre -> post
pub fn link_transforms_changes(pre: usize, post: usize) -> Vec<(ConfigPath, ConfigValue)>
```

This returns the batch of ConfigPath changes to apply through ConfigManager.

### Step 3: Unlink Action

```rust
/// Generate xaos changes to unlink a pair or chain
pub fn unlink_transforms_changes(chain: &[usize]) -> Vec<(ConfigPath, ConfigValue)>
```

### Step 4: UI - Linked Transforms Section

Add the linked transforms section to the bottom of `render_xaos_editor_content()` in `src/ui/xaos_editor.rs`:

- Display detected chains/pairs
- "Link Transforms" button with transform selection
- "Unlink" buttons per chain/pair

### Step 5: Transform Add/Delete Preservation

Modify the transform add/delete handlers to preserve link constraints:
- After adding: zero out new transform's weight to any existing post-transforms
- Before deleting: if deleting a post, copy its routing to its pre; if deleting a pre, free its post

### Step 6: i18n

Add translation keys for linked transform UI elements in `locales/en.yml`.

### Step 7: Testing

- Unit tests for detect_linked_pairs with various matrix configurations
- Unit tests for link/unlink change generation
- Unit tests for chain detection
- Manual testing of UI interactions

## References

- [Fr0st Linked Transform Tutorial](https://fr0st.wordpress.com/2008/08/23/linked-transform-tutorial/)
- [Fr0st Xaos Tutorial](https://fr0st.wordpress.com/2008/08/23/xaos-tutorial/)
- [Learning Apophysis Post Transforms](https://learningapophysis.wordpress.com/2010/03/03/post-transforms/)
- [Fractorium Pre/Post Documentation](https://fractorium.com)
- [chronologicaldot blog - Transform Processing](https://chronologicaldot.wordpress.com/2013/10/15/understanding-how-fractal-transforms-are-processed/)
