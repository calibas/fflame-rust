// The planner's arithmetic on the GPU -- docs/projects/gpu-cylinder-planning.md.
//
// Appended to a flame's definitions by `ShaderBuilder::build_plan_eval`,
// built with CYLINDER_REPLAY on, so a word is applied by
// `ct_apply_symbol` (`replay.wgsl`) -- the function the render's replay
// arm calls. The planner and the picture cannot disagree about where a
// word sends a point.
//
// One thread per (job, point). A job is a word and a run of sample
// indices; the thread applies the word to its point and writes 1 if the
// result lands in the view disc, 0 otherwise.

// One batch's jobs, words and point indices share ONE buffer,
// `plan_data`: four storage buffers of the flame's in group 0 and three
// here stay inside WebGPU's default of eight per stage.
//
//   plan_data[4j .. 4j+4]          job j: word offset (from words_base),
//                                  word length, first entry, entry count
//   plan_data[words_base + ..]     the words' symbols, in the order they
//                                  apply
//   plan_data[idx_base + e]        entry e's index into `plan_points`
struct PlanView {
    centre: vec2<f32>,
    radius: f32,
    // Total entries across all jobs, and the number of jobs.
    entries: u32,
    jobs: u32,
    // Threads per row of the dispatch grid (workgroups x 64).
    row: u32,
    words_base: u32,
    idx_base: u32,
    // 0: one u32 per entry, 1 where the point lands in the disc.
    // 1: two per entry, the end point's bits -- x then y, and x = NaN
    //    bits where the word sent the point to a bad value.
    mode: u32,
    _pad0: u32,
    _pad1: u32,
    _pad2: u32,
}

@group(1) @binding(0) var<storage, read> plan_points: array<vec2<f32>>;
@group(1) @binding(1) var<storage, read> plan_data: array<u32>;
@group(1) @binding(2) var<storage, read_write> plan_out: array<u32>;
@group(1) @binding(3) var<uniform> plan_view: PlanView;

@compute @workgroup_size(64, 1, 1)
fn plan_eval(@builtin(global_invocation_id) gid: vec3<u32>) {
    let e = gid.x + gid.y * plan_view.row;
    if (e >= plan_view.entries) {
        return;
    }
    // The job whose run holds entry `e`: the last whose offset is <= e.
    var lo = 0u;
    var hi = plan_view.jobs - 1u;
    loop {
        if (lo >= hi) {
            break;
        }
        let mid = (lo + hi + 1u) / 2u;
        if (plan_data[4u * mid + 2u] <= e) {
            lo = mid;
        } else {
            hi = mid - 1u;
        }
    }
    let word_offset = plan_view.words_base + plan_data[4u * lo];
    let word_len = plan_data[4u * lo + 1u];

    var p = plan_points[plan_data[plan_view.idx_base + e]];
    var rng = rng_init(e, 0x9E3779B9u);
    for (var k = 0u; k < word_len; k = k + 1u) {
        p = ct_apply_symbol(p, plan_data[word_offset + k], &rng, 0.0);
    }
    ct_forced_arm = -1;

    // A bad value is outside. The negated-comparison idiom, because a
    // plain `x <= r` against a NaN is not reliable under Metal's
    // fast-math (see CLAUDE.md).
    let bad = !(abs(p.x) <= 1.0e30) || !(abs(p.y) <= 1.0e30);
    if (plan_view.mode == 1u) {
        // Integer ops only: a NaN written through a float is not
        // reliable under fast-math.
        plan_out[2u * e] = select(bitcast<u32>(p.x), 0x7fc00000u, bad);
        plan_out[2u * e + 1u] = bitcast<u32>(p.y);
        return;
    }
    let d = p - plan_view.centre;
    let inside = dot(d, d) <= plan_view.radius * plan_view.radius;
    plan_out[e] = select(0u, 1u, inside && !bad);
}
