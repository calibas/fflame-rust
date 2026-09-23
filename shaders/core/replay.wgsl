// A forced symbol, applied -- the ONE definition of how a word of a
// cylinder plan is replayed. Appended to a flame's definitions (through
// the template processor) when CYLINDER_REPLAY is on, so the render's
// replay arm (`main_template.wgsl`) and the planner's GPU kernel
// (`plan_eval.wgsl`) call the same function: a plan decides a word
// lands in the view by the very arithmetic the render then plots with.
// See docs/projects/gpu-cylinder-planning.md.

// The arm a replayed symbol forces on a many-valued variation, or -1
// between symbols so the free orbit draws its own. A word's symbol
// carries the transform in its low byte and the arm above it (see
// `cylinder::sym_of`); `ct_apply_symbol` sets this before applying the
// transform, and an armed variation's draw is wrapped by the shader
// builder to read it. Private, because the forced prefix and the free
// orbit run on the same thread and the same variation function. The
// caller clears it after the word.
var<private> ct_forced_arm: i32 = -1;

fn ff_forced_arm_f(drawn: f32) -> f32 {
    return select(drawn, f32(ct_forced_arm), ct_forced_arm >= 0);
}

fn ff_forced_arm_i(drawn: i32) -> i32 {
    return select(drawn, ct_forced_arm, ct_forced_arm >= 0);
}

// Apply symbol `w` to `p` exactly as the chaos game applies its
// transform: affine, variations, post affine. `colour` seeds the
// variations' colour register, which is discarded -- a forced prefix
// takes its colour from the fold the CPU composed, not from here. So is
// the hide flag: the plot is gated by the free iteration's own.
{{#if RENDER_3D}}
fn ct_apply_symbol(p: vec3<f32>, w: u32, rng: ptr<function, RngState>, colour: f32) -> vec3<f32> {
{{else}}
fn ct_apply_symbol(p: vec2<f32>, w: u32, rng: ptr<function, RngState>, colour: f32) -> vec2<f32> {
{{/if}}
    let sym = w & 255u;
    ct_forced_arm = i32(w >> 8u);
    let xf = transforms[sym];
    let aff = apply_affine(xf, p);
    var hide = false;
{{#if HAS_ANALYTIC_BLUR}}
{{#if RENDER_3D}}
    var blur = vec3<f32>(0.0, 0.0, 0.0);
{{else}}
    var blur = vec2<f32>(0.0, 0.0);
{{/if}}
{{/if}}
{{#if HAS_DC}}
    // Fresh registers: writing the iteration's own `vc`/`vrc` here
    // would leak the forced prefix's colour into the plot.
    var vc: f32 = colour;
{{/if}}
{{#if HAS_RGB}}
    var vrc: vec3<f32> = vec3<f32>(-1.0e30);
{{/if}}
{{#if HAS_DC}}
{{#if HAS_RGB}}
    var q = apply_variations(xf, sym, aff, rng, &vc, &vrc, &hide{{#if HAS_ANALYTIC_BLUR}}, &blur{{/if}});
{{else}}
    var q = apply_variations(xf, sym, aff, rng, &vc, &hide{{#if HAS_ANALYTIC_BLUR}}, &blur{{/if}});
{{/if}}
{{else}}
{{#if HAS_RGB}}
    var q = apply_variations(xf, sym, aff, rng, &vrc, &hide{{#if HAS_ANALYTIC_BLUR}}, &blur{{/if}});
{{else}}
    var q = apply_variations(xf, sym, aff, rng, &hide{{#if HAS_ANALYTIC_BLUR}}, &blur{{/if}});
{{/if}}
{{/if}}
    if (HAS_POST_AFFINE) {
        if (xf.post_enabled > 0.5) {
            q = apply_post_affine(xf, q);
        }
    }
    return q;
}
