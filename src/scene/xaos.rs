use std::collections::HashMap;
use super::transforms::Flame;

/// A detected linked transform pair
#[derive(Debug, Clone, PartialEq)]
pub struct LinkedPair {
    /// Index of the "pre" transform (routes exclusively to post)
    pub pre: usize,
    /// Index of the "post" transform (only reachable from pre)
    pub post: usize,
}

impl Flame {
    // === XAOS (WEIGHTED TRANSFORM TRANSITIONS) ===

    /// Check if this flame has non-default xaos weights
    /// Returns false if xaos is None or all weights are 1.0
    pub fn has_xaos(&self) -> bool {
        if let Some(ref xaos) = self.xaos {
            // Check if any weight differs from 1.0
            for row in xaos {
                for &weight in row {
                    if (weight - 1.0).abs() > 1e-6 {
                        return true;
                    }
                }
            }
        }
        false
    }

    /// Get xaos weight for transition from src to dst
    /// Returns 1.0 if xaos is not enabled or indices are out of bounds
    pub fn get_xaos(&self, src: usize, dst: usize) -> f32 {
        if let Some(ref xaos) = self.xaos {
            if src < xaos.len() && dst < xaos[src].len() {
                return xaos[src][dst];
            }
        }
        1.0
    }

    /// Set xaos weight for transition from src to dst
    /// Automatically initializes xaos matrix if needed
    pub fn set_xaos(&mut self, src: usize, dst: usize, weight: f32) {
        self.ensure_xaos_size();
        if let Some(ref mut xaos) = self.xaos {
            if src < xaos.len() && dst < xaos[src].len() {
                xaos[src][dst] = weight;
            }
        }
    }

    /// Ensure xaos matrix exists and has correct size for current transform count
    /// Initializes all weights to 1.0 (default behavior)
    pub fn ensure_xaos_size(&mut self) {
        let n = self.transforms.len();
        if n == 0 {
            self.xaos = None;
            return;
        }

        match &mut self.xaos {
            None => {
                // Initialize with all 1.0 weights
                self.xaos = Some(vec![vec![1.0; n]; n]);
            }
            Some(xaos) => {
                // Resize if needed
                let current_size = xaos.len();
                if current_size != n {
                    // Resize rows
                    xaos.resize(n, vec![1.0; n]);
                    // Resize columns in each row
                    for row in xaos.iter_mut() {
                        row.resize(n, 1.0);
                    }
                }
            }
        }
    }

    /// Clear xaos (set to None, reverting to default behavior)
    pub fn clear_xaos(&mut self) {
        self.xaos = None;
    }

    /// Reset all xaos weights to 1.0 (keeping the matrix allocated)
    pub fn reset_xaos(&mut self) {
        if let Some(ref mut xaos) = self.xaos {
            for row in xaos.iter_mut() {
                for weight in row.iter_mut() {
                    *weight = 1.0;
                }
            }
        }
    }

    /// Update xaos matrix after a transform is added at the given index.
    ///
    /// Inserts a new row and column at the given index with default weight 1.0.
    /// Then preserves existing linked transforms by zeroing the new transform's
    /// weight to any existing post-transforms (transforms with exactly one
    /// non-zero incoming weight).
    pub fn on_transform_added(&mut self, index: usize) {
        // Detect linked pairs BEFORE modifying the matrix (needs immutable borrow)
        let pairs = self.detect_linked_pairs();
        let pre_indices: std::collections::HashSet<usize> = pairs.iter().map(|p| p.pre).collect();
        let post_indices: std::collections::HashSet<usize> = pairs.iter().map(|p| p.post).collect();

        if let Some(ref mut xaos) = self.xaos {
            let n = xaos.len(); // size before insertion

            // Insert new column at `index` in each existing row
            for row in xaos.iter_mut() {
                if index <= row.len() {
                    row.insert(index, 1.0);
                }
            }

            // Insert new row at `index` (length is now n+1)
            let new_row = vec![1.0; n + 1];
            if index <= xaos.len() {
                xaos.insert(index, new_row);
            }

            // Preserve linked transforms:
            // 1. New transform should not route to post-transforms (would break "sole incoming")
            for post in &post_indices {
                let adjusted_post = if *post >= index { *post + 1 } else { *post };
                if adjusted_post < xaos[index].len() {
                    xaos[index][adjusted_post] = 0.0;
                }
            }

            // 2. Pre-transforms should not route to new transform (would break "sole outgoing")
            for pre in &pre_indices {
                let adjusted_pre = if *pre >= index { *pre + 1 } else { *pre };
                if adjusted_pre < xaos.len() {
                    xaos[adjusted_pre][index] = 0.0;
                }
            }
        }
    }

    /// Update xaos matrix after a transform is deleted at the given index.
    ///
    /// Removes the row and column at the given index. If the resulting matrix
    /// is all 1.0, clears xaos entirely.
    pub fn on_transform_deleted(&mut self, index: usize) {
        if let Some(ref mut xaos) = self.xaos {
            // Remove the row at `index`
            if index < xaos.len() {
                xaos.remove(index);
            }

            // Remove the column at `index` from each remaining row
            for row in xaos.iter_mut() {
                if index < row.len() {
                    row.remove(index);
                }
            }

            // If matrix is now empty or all 1.0, clear it
            if xaos.is_empty() {
                self.xaos = None;
            } else {
                let all_default = xaos.iter().all(|row| row.iter().all(|&w| (w - 1.0).abs() < 1e-6));
                if all_default {
                    self.xaos = None;
                }
            }
        }
    }

    /// Get xaos matrix as flat array for GPU upload
    /// Returns None if xaos is not enabled
    /// Layout: row-major, xaos_flat[src * n + dst] = weight for src→dst
    pub fn xaos_flat(&self) -> Option<Vec<f32>> {
        self.xaos.as_ref().map(|xaos| {
            let n = xaos.len();
            let mut flat = Vec::with_capacity(n * n);
            for row in xaos {
                flat.extend_from_slice(row);
            }
            flat
        })
    }

    // === LINKED TRANSFORMS (CONVENIENCE LAYER ON XAOS) ===

    /// Detect all linked transform pairs from the xaos matrix.
    ///
    /// A linked pair (pre, post) exists when:
    /// - pre has exactly one non-zero outgoing xaos weight, pointing to post
    /// - post has exactly one non-zero incoming xaos weight, coming from pre
    ///
    /// Returns pairs sorted by pre index.
    pub fn detect_linked_pairs(&self) -> Vec<LinkedPair> {
        let n = self.transforms.len();
        if n < 2 {
            return Vec::new();
        }

        // For each transform, find its sole non-zero outgoing target (if any)
        let mut sole_outgoing: Vec<Option<usize>> = Vec::with_capacity(n);
        for src in 0..n {
            let mut target = None;
            let mut count = 0;
            for dst in 0..n {
                if self.get_xaos(src, dst) > 1e-6 {
                    count += 1;
                    target = Some(dst);
                }
            }
            if count == 1 {
                sole_outgoing.push(target);
            } else {
                sole_outgoing.push(None);
            }
        }

        // For each transform, count non-zero incoming weights
        let mut incoming_count: Vec<usize> = vec![0; n];
        let mut sole_incoming_src: Vec<Option<usize>> = vec![None; n];
        for dst in 0..n {
            for src in 0..n {
                if self.get_xaos(src, dst) > 1e-6 {
                    incoming_count[dst] += 1;
                    sole_incoming_src[dst] = Some(src);
                }
            }
        }

        // A linked pair exists when pre has sole outgoing to post,
        // AND post has sole incoming from pre
        let mut pairs = Vec::new();
        for pre in 0..n {
            if let Some(post) = sole_outgoing[pre] {
                if incoming_count[post] == 1 {
                    if let Some(src) = sole_incoming_src[post] {
                        if src == pre {
                            pairs.push(LinkedPair { pre, post });
                        }
                    }
                }
            }
        }

        pairs
    }

    /// Detect linked chains from linked pairs.
    ///
    /// A chain is a sequence [A, B, C, ...] where A->B and B->C are linked pairs.
    /// Returns chains sorted by first element. Each transform appears in at most one chain.
    pub fn detect_linked_chains(&self) -> Vec<Vec<usize>> {
        let pairs = self.detect_linked_pairs();
        if pairs.is_empty() {
            return Vec::new();
        }

        // Build lookup: pre -> post
        let mut pre_to_post: HashMap<usize, usize> = HashMap::new();
        let mut is_post: std::collections::HashSet<usize> = std::collections::HashSet::new();
        for pair in &pairs {
            pre_to_post.insert(pair.pre, pair.post);
            is_post.insert(pair.post);
        }

        // Find chain starts: transforms that are a pre but not a post in any pair
        let mut chains = Vec::new();
        for pair in &pairs {
            if !is_post.contains(&pair.pre) {
                // This is a chain start
                let mut chain = vec![pair.pre];
                let mut current = pair.pre;
                while let Some(&next) = pre_to_post.get(&current) {
                    chain.push(next);
                    current = next;
                }
                chains.push(chain);
            }
        }

        chains
    }

    /// Generate xaos changes to link pre -> post.
    ///
    /// This modifies the xaos matrix so that:
    /// - pre routes exclusively to post (all other outgoing weights = 0)
    /// - post is only reachable from pre (all other incoming weights to post = 0)
    /// - post inherits pre's original outgoing weights
    /// - post does not route to itself
    ///
    /// Returns the batch of (ConfigPath, ConfigValue) changes to apply via ConfigManager.
    pub fn link_transforms_changes(&self, pre: usize, post: usize) -> Vec<(crate::config::ConfigPath, crate::config::ConfigValue)> {
        use crate::config::{ConfigPath, ConfigValue};

        let n = self.transforms.len();
        if pre >= n || post >= n || pre == post {
            return Vec::new();
        }

        let mut changes = Vec::new();

        // 1. Copy pre's current outgoing weights to post's row
        for dst in 0..n {
            let weight = self.get_xaos(pre, dst);
            changes.push((
                ConfigPath::Xaos { src: post, dst },
                ConfigValue::Float(weight),
            ));
        }

        // 2. Set pre's row: all zeros except post = 1.0
        for dst in 0..n {
            let weight = if dst == post { 1.0 } else { 0.0 };
            changes.push((
                ConfigPath::Xaos { src: pre, dst },
                ConfigValue::Float(weight),
            ));
        }

        // 3. Set all other transforms' weight to post = 0 (post only reachable from pre)
        for src in 0..n {
            if src != pre {
                changes.push((
                    ConfigPath::Xaos { src, dst: post },
                    ConfigValue::Float(0.0),
                ));
            }
        }

        // 4. Post does not route to itself
        changes.push((
            ConfigPath::Xaos { src: post, dst: post },
            ConfigValue::Float(0.0),
        ));

        changes
    }

    /// Generate xaos changes to unlink a chain.
    ///
    /// For a chain [A, B, C]:
    /// - A gets C's outgoing weights (the last element's routing)
    /// - All intermediate and final elements become freely reachable (incoming weights = 1.0)
    /// - All elements get self-weight restored to 1.0
    ///
    /// For a simple pair [A, B]:
    /// - A gets B's outgoing weights
    /// - B becomes freely reachable
    ///
    /// Returns the batch of (ConfigPath, ConfigValue) changes to apply via ConfigManager.
    pub fn unlink_chain_changes(&self, chain: &[usize]) -> Vec<(crate::config::ConfigPath, crate::config::ConfigValue)> {
        use crate::config::{ConfigPath, ConfigValue};

        let n = self.transforms.len();
        if chain.len() < 2 {
            return Vec::new();
        }

        let mut changes = Vec::new();

        // Restore all chain members to normal routing (all weights = 1.0).
        // The old algorithm copied last's outgoing to first's row, but that
        // produced duplicate keys (first→last = 0.0 from copy, then 1.0 from
        // restore) and the first write won in the batch delta, leaving a stale 0.
        for &idx in chain {
            for dst in 0..n {
                changes.push((
                    ConfigPath::Xaos { src: idx, dst },
                    ConfigValue::Float(1.0),
                ));
            }
            for src in 0..n {
                changes.push((
                    ConfigPath::Xaos { src, dst: idx },
                    ConfigValue::Float(1.0),
                ));
            }
        }

        changes
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::transforms::Transform;

    fn make_flame_with_xaos(num_transforms: usize, xaos: Vec<Vec<f32>>) -> Flame {
        let mut flame = Flame::new();
        for _ in 0..num_transforms {
            let mut t = Transform::new();
            t.set_variation("linear", 1.0);
            flame.transforms.push(t);
        }
        flame.xaos = Some(xaos);
        flame
    }

    #[test]
    fn test_detect_no_links() {
        // All weights 1.0 - no links
        let flame = make_flame_with_xaos(3, vec![
            vec![1.0, 1.0, 1.0],
            vec![1.0, 1.0, 1.0],
            vec![1.0, 1.0, 1.0],
        ]);
        assert!(flame.detect_linked_pairs().is_empty());
        assert!(flame.detect_linked_chains().is_empty());
    }

    #[test]
    fn test_detect_no_links_without_xaos() {
        // No xaos at all (None) - no links
        let mut flame = Flame::new();
        for _ in 0..3 {
            let mut t = Transform::new();
            t.set_variation("linear", 1.0);
            flame.transforms.push(t);
        }
        assert!(flame.detect_linked_pairs().is_empty());
        assert!(flame.detect_linked_chains().is_empty());
    }

    #[test]
    fn test_detect_simple_link() {
        // T0 -> T1 link: T0 routes only to T1, T1 only reachable from T0
        let flame = make_flame_with_xaos(3, vec![
            vec![0.0, 1.0, 0.0], // T0: only goes to T1
            vec![1.0, 0.0, 1.0], // T1: goes to T0 and T2
            vec![1.0, 0.0, 1.0], // T2: goes to T0 and T2 (NOT T1)
        ]);

        let pairs = flame.detect_linked_pairs();
        assert_eq!(pairs.len(), 1);
        assert_eq!(pairs[0].pre, 0);
        assert_eq!(pairs[0].post, 1);

        let chains = flame.detect_linked_chains();
        assert_eq!(chains.len(), 1);
        assert_eq!(chains[0], vec![0, 1]);
    }

    #[test]
    fn test_detect_chain() {
        // T0 -> T1 -> T2 chain
        let flame = make_flame_with_xaos(4, vec![
            vec![0.0, 1.0, 0.0, 0.0], // T0: only to T1
            vec![0.0, 0.0, 1.0, 0.0], // T1: only to T2
            vec![1.0, 0.0, 0.0, 1.0], // T2: to T0 and T3
            vec![1.0, 0.0, 0.0, 1.0], // T3: to T0 and T3 (NOT T1 or T2)
        ]);

        let pairs = flame.detect_linked_pairs();
        assert_eq!(pairs.len(), 2);
        assert_eq!(pairs[0], LinkedPair { pre: 0, post: 1 });
        assert_eq!(pairs[1], LinkedPair { pre: 1, post: 2 });

        let chains = flame.detect_linked_chains();
        assert_eq!(chains.len(), 1);
        assert_eq!(chains[0], vec![0, 1, 2]);
    }

    #[test]
    fn test_detect_multiple_links() {
        // Two independent links: T0->T1 and T2->T3
        // T1 only reachable from T0, T3 only reachable from T2
        let flame = make_flame_with_xaos(4, vec![
            vec![0.0, 1.0, 0.0, 0.0], // T0: only to T1
            vec![1.0, 0.0, 1.0, 0.0], // T1: to T0 and T2 (NOT T3)
            vec![0.0, 0.0, 0.0, 1.0], // T2: only to T3
            vec![1.0, 0.0, 1.0, 0.0], // T3: to T0 and T2 (NOT T1)
        ]);

        let pairs = flame.detect_linked_pairs();
        assert_eq!(pairs.len(), 2);
        assert_eq!(pairs[0], LinkedPair { pre: 0, post: 1 });
        assert_eq!(pairs[1], LinkedPair { pre: 2, post: 3 });

        let chains = flame.detect_linked_chains();
        assert_eq!(chains.len(), 2);
        assert_eq!(chains[0], vec![0, 1]);
        assert_eq!(chains[1], vec![2, 3]);
    }

    #[test]
    fn test_link_transforms_changes() {
        // Start with 3 transforms, all default xaos (1.0)
        let flame = make_flame_with_xaos(3, vec![
            vec![1.0, 1.0, 1.0],
            vec![1.0, 1.0, 1.0],
            vec![1.0, 1.0, 1.0],
        ]);

        let changes = flame.link_transforms_changes(0, 1);
        assert!(!changes.is_empty());

        // Apply changes manually to verify
        let mut test_flame = flame.clone();
        for (path, value) in &changes {
            if let crate::config::ConfigPath::Xaos { src, dst } = path {
                let weight: f32 = match value {
                    crate::config::ConfigValue::Float(v) => *v,
                    _ => panic!("Expected float"),
                };
                test_flame.set_xaos(*src, *dst, weight);
            }
        }

        // Verify the link was created
        let pairs = test_flame.detect_linked_pairs();
        assert_eq!(pairs.len(), 1);
        assert_eq!(pairs[0].pre, 0);
        assert_eq!(pairs[0].post, 1);

        // T0 should route only to T1
        assert!(test_flame.get_xaos(0, 0) < 1e-6);
        assert!(test_flame.get_xaos(0, 1) > 0.9);
        assert!(test_flame.get_xaos(0, 2) < 1e-6);

        // T1 should not be reachable from T2
        assert!(test_flame.get_xaos(2, 1) < 1e-6);
    }

    #[test]
    fn test_unlink_chain() {
        // Set up a linked pair T0 -> T1
        let flame = make_flame_with_xaos(3, vec![
            vec![0.0, 1.0, 0.0], // T0: only to T1
            vec![1.0, 0.0, 1.0], // T1: to T0 and T2
            vec![1.0, 0.0, 1.0], // T2: to T0 and T2
        ]);

        let chains = flame.detect_linked_chains();
        assert_eq!(chains.len(), 1);

        let changes = flame.unlink_chain_changes(&chains[0]);
        assert!(!changes.is_empty());

        // Apply changes
        let mut test_flame = flame.clone();
        for (path, value) in &changes {
            if let crate::config::ConfigPath::Xaos { src, dst } = path {
                let weight: f32 = match value {
                    crate::config::ConfigValue::Float(v) => *v,
                    _ => panic!("Expected float"),
                };
                test_flame.set_xaos(*src, *dst, weight);
            }
        }

        // After unlinking, no linked pairs should exist
        let pairs = test_flame.detect_linked_pairs();
        assert!(pairs.is_empty(), "Expected no linked pairs after unlink, got: {:?}", pairs);

        // Verify all chain members have all weights restored to 1.0
        for &idx in &chains[0] {
            for dst in 0..3 {
                assert!((test_flame.get_xaos(idx, dst) - 1.0).abs() < 1e-6,
                    "Expected weight 1.0 for xaos[{}][{}], got {}", idx, dst, test_flame.get_xaos(idx, dst));
            }
        }
    }

    #[test]
    fn test_unlink_restores_pre_to_post_weight() {
        // Regression: linking T1->T2 then unlinking left xaos[T1][T2] = 0.0
        let mut flame = make_flame_with_xaos(3, vec![
            vec![1.0, 1.0, 1.0],
            vec![1.0, 1.0, 1.0],
            vec![1.0, 1.0, 1.0],
        ]);

        // Link T1 -> T2 (indices 1 -> 2)
        let link_changes = flame.link_transforms_changes(1, 2);
        for (path, value) in &link_changes {
            if let crate::config::ConfigPath::Xaos { src, dst } = path {
                let weight: f32 = match value {
                    crate::config::ConfigValue::Float(v) => *v,
                    _ => panic!("Expected float"),
                };
                flame.set_xaos(*src, *dst, weight);
            }
        }

        // Verify link exists
        let pairs = flame.detect_linked_pairs();
        assert_eq!(pairs.len(), 1);

        // Unlink
        let chains = flame.detect_linked_chains();
        let unlink_changes = flame.unlink_chain_changes(&chains[0]);
        for (path, value) in &unlink_changes {
            if let crate::config::ConfigPath::Xaos { src, dst } = path {
                let weight: f32 = match value {
                    crate::config::ConfigValue::Float(v) => *v,
                    _ => panic!("Expected float"),
                };
                flame.set_xaos(*src, *dst, weight);
            }
        }

        // THE BUG: xaos[1][2] was 0.0 instead of 1.0
        assert!((flame.get_xaos(1, 2) - 1.0).abs() < 1e-6,
            "xaos[1][2] should be 1.0 after unlink, got {}", flame.get_xaos(1, 2));

        // All weights should be 1.0
        for src in 0..3 {
            for dst in 0..3 {
                assert!((flame.get_xaos(src, dst) - 1.0).abs() < 1e-6,
                    "xaos[{}][{}] should be 1.0, got {}", src, dst, flame.get_xaos(src, dst));
            }
        }
    }

    #[test]
    fn test_on_transform_added_preserves_links() {
        // Set up linked pair T0 -> T1
        let mut flame = make_flame_with_xaos(3, vec![
            vec![0.0, 1.0, 0.0], // T0: only to T1
            vec![1.0, 0.0, 1.0], // T1: to T0 and T2
            vec![1.0, 0.0, 1.0], // T2: to T0 and T2
        ]);

        // Verify link exists
        let pairs = flame.detect_linked_pairs();
        assert_eq!(pairs.len(), 1);

        // Add a new transform at index 3 (end)
        flame.on_transform_added(3);
        flame.transforms.push(Transform::new());

        // Matrix should be 4x4 now
        let xaos = flame.xaos.as_ref().unwrap();
        assert_eq!(xaos.len(), 4);
        assert_eq!(xaos[0].len(), 4);

        // The link should still be detected (T0 -> T1)
        let pairs_after = flame.detect_linked_pairs();
        assert_eq!(pairs_after.len(), 1, "Link should be preserved after adding transform");
        assert_eq!(pairs_after[0].pre, 0);
        assert_eq!(pairs_after[0].post, 1);

        // New transform (T3) should have weight 0 to T1 (post-transform)
        assert!(flame.get_xaos(3, 1) < 1e-6, "New transform should not route to post-transform");
    }

    #[test]
    fn test_on_transform_deleted_updates_xaos() {
        let mut flame = make_flame_with_xaos(3, vec![
            vec![0.5, 1.0, 0.0],
            vec![1.0, 0.5, 1.0],
            vec![0.0, 1.0, 0.5],
        ]);

        // Delete transform at index 1
        flame.transforms.remove(1);
        flame.on_transform_deleted(1);

        // Matrix should be 2x2 now with correct indices
        let xaos = flame.xaos.as_ref().unwrap();
        assert_eq!(xaos.len(), 2);
        assert_eq!(xaos[0].len(), 2);

        // Row 0 (was T0): kept columns 0 and 2 (now 0 and 1)
        assert_eq!(flame.get_xaos(0, 0), 0.5); // T0->T0
        assert_eq!(flame.get_xaos(0, 1), 0.0); // T0->T2 (was column 2)

        // Row 1 (was T2): kept columns 0 and 2 (now 0 and 1)
        assert_eq!(flame.get_xaos(1, 0), 0.0); // T2->T0
        assert_eq!(flame.get_xaos(1, 1), 0.5); // T2->T2 (was column 2)
    }

    #[test]
    fn test_too_few_transforms_for_links() {
        // 0 or 1 transforms - can't have links
        let flame = Flame::new();
        assert!(flame.detect_linked_pairs().is_empty());

        let mut flame = Flame::new();
        flame.transforms.push(Transform::new());
        assert!(flame.detect_linked_pairs().is_empty());
    }

    #[test]
    fn test_link_same_transform_returns_empty() {
        let flame = make_flame_with_xaos(2, vec![
            vec![1.0, 1.0],
            vec![1.0, 1.0],
        ]);
        let changes = flame.link_transforms_changes(0, 0);
        assert!(changes.is_empty());
    }
}
