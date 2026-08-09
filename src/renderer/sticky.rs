//! The sticky variation superset — Layer B of
//! `docs/projects/sticky-shader-compilation.md`.
//!
//! The renderer remembers which variations recent flames used and
//! compiles every flame against one **canonical map**: the sorted union
//! of the flame's variations and the retained extras, the extras at
//! weight 0. The dispatcher already gates each variation on `w != 0.0`,
//! so a dead entry is a skipped branch — and because the map depends
//! only on the *set* (not on which transform introduced which name), a
//! batch of random flames drawing from one pool converges to a single
//! compiled pipeline. Measured before this existed: 91% of a 20-seed
//! random batch was shader compilation (cold driver cache; 54% warm).
//!
//! This is **renderer state, never serialized** — a `.fflame` describes
//! a flame, not the compilation strategy that happened to render it.
//!
//! # Ordering — the correctness kernel, amended by measurement
//!
//! The plan first tried `flame order ++ extras`, which keeps sticky
//! renders bit-identical to specialized ones. The convergence test
//! killed it: the union's order across transforms differs per flame
//! (whichever transform introduces a name first places it), so two
//! flames with the SAME variation set still compiled two shaders and
//! Layer B never converged. Canonical (sorted) maps are what converge —
//! the Phase 0 proxy demonstrated exactly this.
//!
//! The price, stated precisely:
//!
//! - **Normal-phase variations**: map order only changes f32 summation
//!   order — a different trajectory through the same attractor, the ULP
//!   class. Accepted.
//! - **Chained phases (pre/post)**: order IS semantics — each chained
//!   variation transforms the previous output. Any flame where canonical
//!   order could disagree with flame order in a chained phase — a
//!   transform with two or more Pre variations, two or more Post
//!   variations, or any `fx_priority` override — **falls back to a
//!   specialized compile** for that flame. Detection is cheap and
//!   conservative.
//!
//! What remains provable and proven by test: extras alone never change
//! pixels. A flame whose own order already matches canonical renders
//! **byte-identical** with any number of dead extras compiled in — dead
//! entries are branch-skipped, never summed.
//!
//! # Who opts out
//!
//! One-shot paths get no benefit and must not pay the reorder: the
//! unified `render()` disables sticky on its throwaway renderer (CLI
//! export and the visual suite keep rendering exactly the specialized
//! shaders their baselines were made with), and the probe and census
//! harnesses disable it explicitly — they compile their own shaders
//! from the raw flame and pack buffers through the renderer, so an
//! augmented map would misalign their `get_param` offsets.
//!
//! # Eviction is the clearing mechanism — and a throughput guardrail
//!
//! No clear triggers. When capacity runs out, the extras least recently
//! *live* are dropped. The cap is not hygiene: the Phase 0 dead-cost
//! curve measured dead variations free to ~15, −8% throughput at 30,
//! −40% at 60 — an unbounded set would silently halve render speed.

use crate::scene::transforms::Flame;
use std::collections::HashMap;

/// Ceiling on the compiled set, `|flame ∪ extras|`. From the Phase 0
/// dead-cost curve: free to ~15 dead entries, −8% at 30, a cliff past
/// that. 32 holds the default generator pool (15) with room while
/// keeping worst-case throughput cost under ~10%.
pub const STICKY_CAP: usize = 32;

/// `MAX_VARIATIONS_PER_FLAME` — the shader builder's hard ceiling.
const HARD_VARIATION_CAP: usize = 100;

/// `MAX_VARIATION_PARAM_SLOTS` — the packed param buffer's hard ceiling.
const HARD_SLOT_CAP: usize = 1600;

pub struct StickyVariations {
    enabled: bool,
    /// Bumped per adopt; the recency clock.
    generation: u64,
    /// Every retained name → the last generation it was *live* (present
    /// in a loaded flame, not merely compiled in as an extra).
    last_live: HashMap<String, u64>,
    /// The full canonical map the most recent adopt compiled against,
    /// or `None` when that flame fell back to a specialized compile.
    /// Repack paths (`set_variation_params`) must reproduce the map the
    /// compiled shader was built with, so they re-apply THIS rather than
    /// recomputing.
    current_map: Option<Vec<String>>,
    /// How many of `current_map` were extras (dead), for stats.
    current_extra_count: usize,
}

impl StickyVariations {
    pub fn new() -> Self {
        Self {
            enabled: true,
            generation: 0,
            last_live: HashMap::new(),
            current_map: None,
            current_extra_count: 0,
        }
    }

    /// Turn the superset off (and forget everything) or back on.
    /// Off means every flame compiles specialized — the pre-Layer-B
    /// behavior, kept for one-shot paths, harnesses, and benchmarking.
    pub fn set_enabled(&mut self, on: bool) {
        self.enabled = on;
        if !on {
            self.last_live.clear();
            self.current_map = None;
            self.current_extra_count = 0;
        }
    }

    pub fn enabled(&self) -> bool {
        self.enabled
    }

    /// Names currently retained.
    pub fn retained(&self) -> usize {
        self.last_live.len()
    }

    /// Extras the last adopt actually compiled in (0 after a fallback).
    pub fn extras(&self) -> usize {
        self.current_extra_count
    }

    /// Canonical reordering is only safe when no transform carries a
    /// chained-phase sequence whose order the reorder could change:
    /// pre/post variations chain (each transforms the previous output),
    /// and `fx_priority` overrides move Any-phase variations into those
    /// chains. Conservative: two-or-more same-chained-phase variations
    /// on one transform, or any priority override, means fall back.
    fn chain_order_safe(flame: &Flame) -> bool {
        use crate::variations::VariationPhase;
        let registry = crate::variations::global_registry();
        for t in &flame.transforms {
            if !t.variation_priorities.is_empty() {
                return false;
            }
            let mut pre = 0usize;
            let mut post = 0usize;
            for name in t.variations.keys() {
                match registry.get(name).map(|i| i.phase.clone()) {
                    Some(VariationPhase::Pre) => pre += 1,
                    Some(VariationPhase::Post) => post += 1,
                    _ => {}
                }
            }
            if pre >= 2 || post >= 2 {
                return false;
            }
        }
        true
    }

    /// Record this flame's variations as live, refresh recency, prune to
    /// capacity, and return the flame to compile and pack: augmented to
    /// the canonical map, or a plain clone on fallback.
    pub fn adopt(&mut self, flame: &Flame) -> Flame {
        if !self.enabled || flame.transforms.is_empty() {
            self.current_map = None;
            self.current_extra_count = 0;
            return flame.clone();
        }

        let registry = crate::variations::global_registry();
        let union = flame.active_variation_names_ordered(&registry);

        self.generation += 1;
        for name in &union {
            self.last_live.insert(name.clone(), self.generation);
        }

        if !Self::chain_order_safe(flame) {
            // Chained-phase order could change under the canonical map:
            // compile this flame specialized. Retention still updated
            // above — its variations stay warm for later flames.
            self.current_map = None;
            self.current_extra_count = 0;
            return flame.clone();
        }

        // Extras: retained names not live here, newest first (name as a
        // deterministic tiebreak), admitted while both ceilings hold.
        let mut candidates: Vec<(&String, u64)> = self
            .last_live
            .iter()
            .filter(|(n, _)| !union.contains(n))
            .map(|(n, g)| (n, *g))
            .collect();
        candidates.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(b.0)));

        let mut slots: usize = union
            .iter()
            .filter_map(|n| registry.get(n))
            .map(|i| i.slot_count())
            .sum();
        let mut vars = union.len();
        let cap = STICKY_CAP.min(HARD_VARIATION_CAP);

        let mut keep: Vec<String> = Vec::new();
        for (name, _) in candidates {
            if vars >= cap {
                break;
            }
            let Some(info) = registry.get(name) else { continue };
            if slots + info.slot_count() > HARD_SLOT_CAP {
                // Skip the fat one; a smaller candidate may still fit.
                continue;
            }
            vars += 1;
            slots += info.slot_count();
            keep.push(name.clone());
        }

        // Prune: retention IS live ∪ kept. Everything else ages out here.
        self.last_live
            .retain(|n, _| union.contains(n) || keep.contains(n));
        drop(registry);

        // The canonical map: one sorted list of everything compiled in.
        // Depending only on the SET is what makes two flames with the
        // same pool share one shader.
        let mut map: Vec<String> = union.iter().cloned().chain(keep.iter().cloned()).collect();
        map.sort();
        self.current_extra_count = keep.len();
        self.current_map = Some(map);

        self.augmented(flame)
    }

    /// Rewrite a flame onto the last adopt's canonical map — for the
    /// compile-and-pack path and for repacks that must match it. A plain
    /// clone when the last adopt fell back or sticky is off.
    ///
    /// Transform 0 carries the whole map (its own weights preserved,
    /// everything else at weight 0) and its `variation_order` IS the
    /// map, which pins the union order for every consumer that derives
    /// the local index map from the flame. Other transforms keep their
    /// entries untouched — their order no longer contributes new names.
    pub fn augmented(&self, flame: &Flame) -> Flame {
        let mut out = flame.clone();
        let Some(map) = (self.enabled).then_some(self.current_map.as_ref()).flatten() else {
            return out;
        };
        if out.transforms.is_empty() {
            return out;
        }
        let t0 = &mut out.transforms[0];
        for name in map {
            t0.variations.entry(name.clone()).or_insert(0.0);
        }
        t0.variation_order = map.clone();
        out
    }
}

impl Default for StickyVariations {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod sticky_tests {
    use super::*;
    use crate::config::FractalConfig;
    use crate::renderer::compute_kernel::FlameRenderer;
    use crate::renderer::render::{render_with, NoProgress, RenderJob};
    use crate::scene::randomize::{generate_random_flame_with_rng, RandomGeneratorSettings};
    use crate::scene::transforms::Transform;
    use rand::SeedableRng;

    // ---- unit-level, no GPU ----

    fn flame_of(names_per_transform: &[&[&str]]) -> Flame {
        let mut flame = Flame::new();
        flame.transforms.clear();
        for names in names_per_transform {
            let mut t = Transform::new();
            t.a = 0.5;
            t.e = 0.5;
            t.weight = 1.0;
            for n in *names {
                t.set_variation(n, 1.0);
            }
            flame.transforms.push(t);
        }
        flame
    }

    fn union_of(flame: &Flame) -> Vec<String> {
        let reg = crate::variations::global_registry();
        flame.active_variation_names_ordered(&reg)
    }

    /// THE convergence property: two flames with the same variation set
    /// but different per-transform placement produce the same map — the
    /// thing flame-order maps could not do, which is why the design was
    /// amended.
    #[test]
    fn same_set_different_placement_yields_one_map() {
        let mut sticky = StickyVariations::new();
        let a = sticky.adopt(&flame_of(&[&["swirl", "linear"], &["polar"]]));
        let map_a = union_of(&a);
        let b = sticky.adopt(&flame_of(&[&["polar"], &["linear", "swirl"]]));
        let map_b = union_of(&b);
        assert_eq!(map_a, map_b, "same set must mean same map, or nothing converges");
        assert_eq!(map_a, ["linear", "polar", "swirl"], "and it is the sorted set");
    }

    /// A transform with two chained-phase (pre) variations must fall
    /// back to a specialized compile: canonical reorder would change
    /// which pre feeds which, which is a different image, not rounding.
    #[test]
    fn chained_phases_fall_back_to_specialized() {
        let reg = crate::variations::global_registry();
        let pres: Vec<String> = reg
            .names()
            .iter()
            .filter(|n| {
                reg.get(n)
                    .map(|i| i.phase == crate::variations::VariationPhase::Pre)
                    .unwrap_or(false)
            })
            .take(2)
            .cloned()
            .collect();
        drop(reg);
        assert_eq!(pres.len(), 2, "registry should have two pre-phase variations");

        let mut sticky = StickyVariations::new();
        // Warm the sticky set so a fallback visibly refuses extras.
        sticky.adopt(&flame_of(&[&["swirl"]]));

        let hazard = flame_of(&[&[pres[0].as_str(), pres[1].as_str()]]);
        let out = sticky.adopt(&hazard);
        assert_eq!(sticky.extras(), 0, "fallback must compile specialized");
        assert_eq!(
            union_of(&out),
            union_of(&hazard),
            "fallback must leave the flame's own order untouched"
        );
    }

    #[test]
    fn disabled_means_a_plain_clone_and_no_retention() {
        let mut sticky = StickyVariations::new();
        sticky.adopt(&flame_of(&[&["swirl"]]));
        sticky.set_enabled(false);
        let f = flame_of(&[&["linear"]]);
        let out = sticky.adopt(&f);
        assert_eq!(out.transforms[0].variation_order, vec!["linear"]);
        assert_eq!(sticky.retained(), 0);
    }

    #[test]
    fn capacity_is_enforced_and_evicts_oldest() {
        let mut sticky = StickyVariations::new();
        let reg = crate::variations::global_registry();
        // Exclude "linear": the final probe flame below uses it live, and
        // a live name is legitimately present regardless of eviction.
        let names: Vec<String> = reg
            .names()
            .iter()
            .filter(|n| {
                n.as_str() != "linear"
                    && reg.get(n).map(|i| i.slot_count() <= 2).unwrap_or(false)
            })
            .take(STICKY_CAP + 10)
            .cloned()
            .collect();
        drop(reg);
        for n in &names {
            sticky.adopt(&flame_of(&[&[n.as_str()]]));
        }
        assert!(
            sticky.retained() <= STICKY_CAP,
            "retention must stay under the cap: {} > {STICKY_CAP}",
            sticky.retained()
        );

        let last = names.last().unwrap();
        let augmented = sticky.adopt(&flame_of(&[&["linear"]]));
        let map = union_of(&augmented);
        assert!(
            map.iter().any(|n| n == last),
            "most recent live name should survive as an extra"
        );
        assert!(
            !map.iter().any(|n| n == &names[0]),
            "the oldest name should have been evicted"
        );
    }

    // ---- GPU, skip without an adapter ----

    fn device() -> Option<(wgpu::Device, wgpu::Queue)> {
        pollster::block_on(async {
            let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
                backends: wgpu::Backends::all(),
                ..wgpu::InstanceDescriptor::new_without_display_handle()
            });
            let adapter = instance
                .request_adapter(&wgpu::RequestAdapterOptions::default())
                .await
                .ok()?;
            let al = adapter.limits();
            let mut limits = wgpu::Limits::default();
            limits.max_storage_buffer_binding_size = al.max_storage_buffer_binding_size;
            limits.max_buffer_size = al.max_buffer_size;
            limits.max_storage_buffers_per_shader_stage = al.max_storage_buffers_per_shader_stage;
            adapter
                .request_device(&wgpu::DeviceDescriptor {
                    label: Some("sticky test"),
                    required_features: wgpu::Features::CLEAR_TEXTURE,
                    required_limits: limits,
                    memory_hints: wgpu::MemoryHints::Performance,
                    experimental_features: Default::default(),
                    trace: Default::default(),
                })
                .await
                .ok()
        })
    }

    fn seed_config(seed: u64) -> FractalConfig {
        let mut settings = RandomGeneratorSettings::default();
        settings.transform_count_min = 4;
        settings.transform_count_max = 4;
        let mut rng = rand_pcg::Pcg64Mcg::seed_from_u64(seed);
        let mut config = FractalConfig::default();
        config.flame = generate_random_flame_with_rng(&settings, &mut rng);
        config.deterministic_rng = true;
        config
    }

    fn render_px(
        renderer: &mut FlameRenderer,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        config: &FractalConfig,
    ) -> Vec<u8> {
        let job = RenderJob::new(config, 64, 64).with_iterations(500_000);
        pollster::block_on(render_with(renderer, device, queue, job, &mut NoProgress))
            .expect("render failed")
            .rgba_data
    }

    fn renderer_for(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        config: &FractalConfig,
    ) -> FlameRenderer {
        FlameRenderer::with_palette_size(
            device,
            queue,
            wgpu::TextureFormat::Rgba8Unorm,
            64,
            64,
            &config.flame,
            config.palette_size,
        )
    }

    /// The provable identity: dead extras never change pixels. The
    /// target's own order already matches canonical (one transform,
    /// canonically-emitted names), so specialized and sticky share live
    /// order and must produce identical bytes — extras are
    /// branch-skipped, never summed.
    #[test]
    fn extras_alone_never_change_pixels() {
        let Some((device, queue)) = device() else {
            eprintln!("skipped: no GPU adapter");
            return;
        };
        let mut target = FractalConfig::default();
        target.flame = flame_of(&[&["linear", "spherical", "swirl"]]);
        target.deterministic_rng = true;

        let mut r1 = renderer_for(&device, &queue, &target);
        r1.set_sticky_enabled(false);
        let reference = render_px(&mut r1, &device, &queue, &target);
        r1.destroy();

        // Sticky renderer, pre-warmed with unrelated variations so the
        // target compiles with real extras.
        let other = seed_config(3);
        let mut r2 = renderer_for(&device, &queue, &other);
        {
            let mut enc = device.create_command_encoder(&Default::default());
            r2.load_config(&device, &mut enc, &queue, &other, &other.palette, 1, 0);
            queue.submit(Some(enc.finish()));
        }
        let sticky_px = render_px(&mut r2, &device, &queue, &target);
        let (_, extras) = r2.sticky_stats();
        r2.destroy();

        assert!(
            extras > 0,
            "the target must have compiled with extras or this proves nothing"
        );
        assert_eq!(
            reference, sticky_px,
            "dead extras changed pixels — the branch-skip guarantee is broken"
        );
    }

    /// The convergence claim, end to end: a batch drawing from one pool
    /// stops recompiling once the sticky set covers the pool.
    #[test]
    fn a_random_batch_stops_recompiling() {
        let Some((device, queue)) = device() else {
            eprintln!("skipped: no GPU adapter");
            return;
        };
        let first = seed_config(1);
        let mut r = renderer_for(&device, &queue, &first);

        let mut rebuilds_at_12 = 0;
        for seed in 1..=20u64 {
            let config = seed_config(seed);
            let _ = render_px(&mut r, &device, &queue, &config);
            if seed == 12 {
                rebuilds_at_12 = r.shader_rebuild_stats().0;
            }
        }
        let (rebuilds, _, _) = r.shader_rebuild_stats();
        r.destroy();

        assert_eq!(
            rebuilds, rebuilds_at_12,
            "seeds 13-20 must not compile anything new once the pool is covered"
        );
        assert!(
            rebuilds <= 10,
            "20 pool seeds should converge in a handful of compiles, got {rebuilds}"
        );
    }
}
