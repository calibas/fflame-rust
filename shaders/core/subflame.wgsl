// Subflame iteration helper — runs ONE step of a nested chaos game
// using the subflame's transform pool. Called from the
// `subflame_wf` variation body.
//
// State storage: the (point, current_xform_idx, color) tuple lives
// in the parent variation's 5 state slots, keyed by the *parent*
// (xform_id, variation_id) pair. The state mechanism gives each
// `subflame_wf` instance its own independent chaos-game state, so
// the same subflame used by two parent transforms gets two
// independent observers.
//
// Lazy per-thread prefuse: the state starts at (0,0) per thread
// (var<private> thread_state zero-init). For low-weight parent
// transforms — where subflame_wf is dispatched only a handful of
// times across the parent's 256 iterations — the subflame's chaos
// game never converges per-thread, and the plotted points are
// biased toward the chaos game's starting neighborhood. That bias
// is visible as a hot edge on the rendered subflame. JWildfire's
// reference (SubFlameWFFunc.prefuseIter) starts from a random point
// in [-0.5, 0.5]^2 with a random colour and walks 42 steps once at
// init time; we do the same on the FIRST subflame_iterate call per
// thread (detected via the position being exactly (0,0)). Cost is
// amortized — only the first call pays it. After prefuse the
// position lives in the attractor and the (0,0) check is
// essentially guaranteed false for subsequent calls. Like JWF
// (subflameIter's isInfinite / isNaN test), a walk that has blown up
// is prefused again, rather than carrying Inf/NaN forever.
//
// The walk and the output are two things, as in JWF's subflameIter:
// the normal transforms step the state p -> p; the subflame's finals
// then map p to the output q, which the parent sees. q is never
// stored, so the finals shape what is plotted without feeding the
// walk -- the parent pipeline's own rule for finals.
//
// Subflame xforms occupy real slots in the variation_params,
// thread_state, and transforms buffers immediately after parent
// xforms — addressed by `xform_id = SUBFLAME_XFORM_ID_BASE + offset`
// (= 128 + offset). get_param, get_state, and needs_transform reads
// all work the same way as for parent xforms, so parametric
// variations (julian, blob, klein_group, etc.) render with their
// configured params rather than defaults. See archived design doc
// `docs/archive/subflame-variations-v2.md` for the unification work.
//
// Remaining limitation: no xaos in subflames — transforms are picked
// by raw weight.

// JWF's prefuse length (SubFlameWFFunc.prefuseIter). It was 20 here
// while this header claimed JWF's 42.
const SUBFLAME_PREFUSE_ITERS: u32 = 42u;

{{#if RENDER_3D}}
fn subflame_iterate(
    subflame_id: u32,
    parent_xform_id: u32,
    parent_variation_id: u32,
    rng: ptr<function, RngState>,
) -> vec4<f32> {
    let sf_meta = subflame_metadata[subflame_id];
    if (sf_meta.normals_count == 0u) {
        return vec4<f32>(0.0, 0.0, 0.0, 0.0);
    }

    // Restore (point, color) from state slots. Slot 3 (xform idx) is
    // written below but doesn't need to be read — each iteration picks
    // a fresh transform.
    var current = vec3<f32>(
        get_state(parent_xform_id, parent_variation_id, 0u),
        get_state(parent_xform_id, parent_variation_id, 1u),
        get_state(parent_xform_id, parent_variation_id, 2u),
    );
    var color = get_state(parent_xform_id, parent_variation_id, 4u);

    // First-call prefuse detection (see header comment). After at
    // least one chaos-game step, exact (0,0,0) is overwhelmingly
    // unlikely; treating that state as "fresh" is the cheapest
    // sentinel. A non-finite state is prefused again, as JWF does --
    // tested as `!(abs(x) <= 1e32)`, never a self-compare, which
    // Metal's fast-math folds away.
    let fresh = current.x == 0.0 && current.y == 0.0 && current.z == 0.0;
    let blown = !(abs(current.x) <= 1e32) || !(abs(current.y) <= 1e32)
        || !(abs(current.z) <= 1e32) || !(abs(color) <= 1e32);
    let needs_prefuse = fresh || blown;
    if (needs_prefuse) {
        // JWF's start: a random point in [-0.5, 0.5]^2, z = 0, and a
        // random colour.
        current = vec3<f32>(rng_nextf(rng) - 0.5, rng_nextf(rng) - 0.5, 0.0);
        color = rng_nextf(rng);
    }
    let total_steps = select(1u, SUBFLAME_PREFUSE_ITERS + 1u, needs_prefuse);

    // Total weight is constant across iterations (transforms don't
    // change during this dispatch). Hoist it out.
    var total_weight: f32 = 0.0;
    for (var i: u32 = 0u; i < sf_meta.normals_count; i = i + 1u) {
        // Read from the unified `transforms[]` buffer at this
        // subflame xform's xform_id. Pre-Phase-5 this read went to a
        // separate `subflame_transforms[]` buffer; that path is gone.
        total_weight = total_weight + transforms[sf_meta.xform_id_base + sf_meta.normals_offset + i].weight;
    }
    let total_weight_safe = max(total_weight, 1e-6);

    var picked: u32 = 0u;
    var vc: f32 = color;
{{#if HAS_RGB}}
    // Sentinel vrc for subflames. WritesRgb variations in the
    // subflame's xform set may write here; subflame_iterate
    // discards it (subflame contributes a position + palette
    // index back to the parent, not direct RGB). Same shape as
    // the existing `vc` — needed when the parent flame has any
    // active WritesRgb variation, otherwise apply_subflame_variations'
    // signature has a vrc parameter we'd be omitting.
    var vrc: vec3<f32> = vec3<f32>(0.0);
{{/if}}

    for (var k: u32 = 0u; k < total_steps; k = k + 1u) {
        // Pick next transform by weight (v1: no xaos).
        let r = rng_nextf(rng) * total_weight_safe;
        var cumulative: f32 = 0.0;
        picked = sf_meta.normals_count - 1u;
        for (var i: u32 = 0u; i < sf_meta.normals_count; i = i + 1u) {
            cumulative = cumulative + transforms[sf_meta.xform_id_base + sf_meta.normals_offset + i].weight;
            if (r < cumulative) {
                picked = i;
                break;
            }
        }

        // xform_id_base = MAX_TRANSFORMS, so this lands in the
        // subflame portion of the unified transforms / variation_params
        // buffers.
        let sub_xform_id = sf_meta.xform_id_base + sf_meta.normals_offset + picked;
        let xform = transforms[sub_xform_id];

        // Color blend — Apophysis-standard color_speed lerp. Capture
        // the post-color_speed color as `c_base` so the DC blend below
        // can lerp from it toward whatever DC variations wrote to `vc`.
        let symmetry = xform.color_speed;
        let c_base = color * (1.0 + symmetry) * 0.5 + xform.color * (1.0 - symmetry) * 0.5;
        vc = c_base;

        // Apply pre-affine, variations, post-affine.
        let affine_p = apply_affine(xform, current);
{{#if HAS_RGB}}
        current = apply_subflame_variations(xform, sub_xform_id, affine_p, rng, &vc, &vrc);
{{else}}
        current = apply_subflame_variations(xform, sub_xform_id, affine_p, rng, &vc);
{{/if}}
        if (xform.post_enabled > 0.5) {
            current = apply_post_affine(xform, current);
        }

        // DC blend — matches parent main_template.wgsl: any vc writes
        // (from DC variations in the normal transform) lerp into the chaos-game
        // color state by the subflame xform's direct_color slider. Used
        // to be missing entirely, which silently dropped DC writes and
        // (because `color` then converged to a fixed point of the
        // color_speed recurrence) collapsed colorscale_z * q.w to a
        // single Z plane — flat-looking subflames with no DC.
        color = c_base + xform.direct_color * (vc - c_base);
    }

    // The subflame's finals map the walk's point to the OUTPUT, q (see
    // the header). They used to run inside the loop on `current`, which
    // is stored as the state, so they fed the walk: p -> 0.5p + (1,0)
    // with a final +(10,0) settled at 22 instead of JWF's 12. As the
    // parent's finals do, they leave the colour alone: a DC write goes
    // to a register that is then discarded.
    var q = current;
    for (var i: u32 = 0u; i < sf_meta.finals_count; i = i + 1u) {
        let f_xform_id = sf_meta.xform_id_base + sf_meta.finals_offset + i;
        let f_xform = transforms[f_xform_id];
        let f_affine_p = apply_affine(f_xform, q);
        var final_vc: f32 = color;  // discarded after the call
{{#if HAS_RGB}}
        var final_vrc: vec3<f32> = vec3<f32>(0.0);  // discarded after the call
        q = apply_subflame_variations(f_xform, f_xform_id, f_affine_p, rng, &final_vc, &final_vrc);
{{else}}
        q = apply_subflame_variations(f_xform, f_xform_id, f_affine_p, rng, &final_vc);
{{/if}}
        if (f_xform.post_enabled > 0.5) {
            q = apply_post_affine(f_xform, q);
        }
    }

    // Write the WALK's state back for the next iteration (never q).
    set_state(parent_xform_id, parent_variation_id, 0u, current.x);
    set_state(parent_xform_id, parent_variation_id, 1u, current.y);
    set_state(parent_xform_id, parent_variation_id, 2u, current.z);
    set_state(parent_xform_id, parent_variation_id, 3u, f32(picked));
    set_state(parent_xform_id, parent_variation_id, 4u, color);

    return vec4<f32>(q.x, q.y, q.z, color);
}
{{else}}
fn subflame_iterate(
    subflame_id: u32,
    parent_xform_id: u32,
    parent_variation_id: u32,
    rng: ptr<function, RngState>,
) -> vec4<f32> {
    let sf_meta = subflame_metadata[subflame_id];
    if (sf_meta.normals_count == 0u) {
        return vec4<f32>(0.0, 0.0, 0.0, 0.0);
    }

    // 2D: state slot 2 (Z) is still stored but unused; keep the same
    // layout as 3D for code symmetry.
    var current = vec2<f32>(
        get_state(parent_xform_id, parent_variation_id, 0u),
        get_state(parent_xform_id, parent_variation_id, 1u),
    );
    var color = get_state(parent_xform_id, parent_variation_id, 4u);

    // First-call prefuse detection, and a re-prefuse of a blown-up
    // walk — see the 3D branch.
    let fresh = current.x == 0.0 && current.y == 0.0;
    let blown = !(abs(current.x) <= 1e32) || !(abs(current.y) <= 1e32) || !(abs(color) <= 1e32);
    let needs_prefuse = fresh || blown;
    if (needs_prefuse) {
        current = vec2<f32>(rng_nextf(rng) - 0.5, rng_nextf(rng) - 0.5);
        color = rng_nextf(rng);
    }
    let total_steps = select(1u, SUBFLAME_PREFUSE_ITERS + 1u, needs_prefuse);

    var total_weight: f32 = 0.0;
    for (var i: u32 = 0u; i < sf_meta.normals_count; i = i + 1u) {
        // Read from the unified `transforms[]` buffer at this
        // subflame xform's xform_id. Pre-Phase-5 this read went to a
        // separate `subflame_transforms[]` buffer; that path is gone.
        total_weight = total_weight + transforms[sf_meta.xform_id_base + sf_meta.normals_offset + i].weight;
    }
    let total_weight_safe = max(total_weight, 1e-6);

    var picked: u32 = 0u;
    var vc: f32 = color;
{{#if HAS_RGB}}
    // Sentinel vrc for subflames. WritesRgb variations in the
    // subflame's xform set may write here; subflame_iterate
    // discards it (subflame contributes a position + palette
    // index back to the parent, not direct RGB). Same shape as
    // the existing `vc` — needed when the parent flame has any
    // active WritesRgb variation, otherwise apply_subflame_variations'
    // signature has a vrc parameter we'd be omitting.
    var vrc: vec3<f32> = vec3<f32>(0.0);
{{/if}}

    for (var k: u32 = 0u; k < total_steps; k = k + 1u) {
        let r = rng_nextf(rng) * total_weight_safe;
        var cumulative: f32 = 0.0;
        picked = sf_meta.normals_count - 1u;
        for (var i: u32 = 0u; i < sf_meta.normals_count; i = i + 1u) {
            cumulative = cumulative + transforms[sf_meta.xform_id_base + sf_meta.normals_offset + i].weight;
            if (r < cumulative) {
                picked = i;
                break;
            }
        }

        // See 3D branch for xform_id_base rationale.
        let sub_xform_id = sf_meta.xform_id_base + sf_meta.normals_offset + picked;
        let xform = transforms[sub_xform_id];

        // See 3D branch for the c_base / DC-blend rationale.
        let symmetry = xform.color_speed;
        let c_base = color * (1.0 + symmetry) * 0.5 + xform.color * (1.0 - symmetry) * 0.5;
        vc = c_base;

        let affine_p = apply_affine(xform, current);
{{#if HAS_RGB}}
        current = apply_subflame_variations(xform, sub_xform_id, affine_p, rng, &vc, &vrc);
{{else}}
        current = apply_subflame_variations(xform, sub_xform_id, affine_p, rng, &vc);
{{/if}}
        if (xform.post_enabled > 0.5) {
            current = apply_post_affine(xform, current);
        }

        color = c_base + xform.direct_color * (vc - c_base);
    }

    // The finals map the walk's point to the output -- see the 3D branch.
    var q = current;
    for (var i: u32 = 0u; i < sf_meta.finals_count; i = i + 1u) {
        let f_xform_id = sf_meta.xform_id_base + sf_meta.finals_offset + i;
        let f_xform = transforms[f_xform_id];
        let f_affine_p = apply_affine(f_xform, q);
        var final_vc: f32 = color;  // discarded after the call
{{#if HAS_RGB}}
        var final_vrc: vec3<f32> = vec3<f32>(0.0);  // discarded after the call
        q = apply_subflame_variations(f_xform, f_xform_id, f_affine_p, rng, &final_vc, &final_vrc);
{{else}}
        q = apply_subflame_variations(f_xform, f_xform_id, f_affine_p, rng, &final_vc);
{{/if}}
        if (f_xform.post_enabled > 0.5) {
            q = apply_post_affine(f_xform, q);
        }
    }

    set_state(parent_xform_id, parent_variation_id, 0u, current.x);
    set_state(parent_xform_id, parent_variation_id, 1u, current.y);
    set_state(parent_xform_id, parent_variation_id, 2u, 0.0);
    set_state(parent_xform_id, parent_variation_id, 3u, f32(picked));
    set_state(parent_xform_id, parent_variation_id, 4u, color);

    // Return (x, y, 0, color) for 2D — the variation body keeps the
    // shape consistent across 2D/3D.
    return vec4<f32>(q.x, q.y, 0.0, color);
}
{{/if}}
