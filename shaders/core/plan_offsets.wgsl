// **The planner's replays in offsets** (tracker P6, `PlanGpu::offset_replays`).
//
// Appended after the planner's shader and `replay_delta.wgsl`. Past the
// view radius where the planner's absolute f32 resolves a view
// (`Backward::gpu_resolves`), a replay runs as the render's offset
// replay does: a job is a child of a node the walk made references for,
// and a child's word is its symbol, then its node's word -- so the thread
// runs the symbol and the node's steps before `m` absolutely, then the
// node's chains in offsets (`ct_offsets`), and tests the end, relative to
// the view's centre, against the view's radius.
//
//   po_data[0 .. P]            the points' indices into the sample
//   po_data[P + 4j .. +4]      job j: the node's record and block in the
//                              replay table, the child's symbol, unused
//   po_out                     one bit a point, P rounded up to 32 a job

@group(2) @binding(0) var<storage, read> cylinders: array<f32>;
@group(2) @binding(1) var<storage, read> po_data: array<u32>;
@group(2) @binding(2) var<storage, read> po_points: array<vec2<f32>>;
@group(2) @binding(3) var<storage, read_write> po_out: array<atomic<u32>>;

struct PoView {
    radius_bits: u32,
    points: u32,
    jobs: u32,
    row: u32,
}
@group(2) @binding(4) var<uniform> po_view: PoView;

@compute @workgroup_size(64, 1, 1)
fn plan_offsets(@builtin(global_invocation_id) gid: vec3<u32>) {
    let e = gid.x + gid.y * po_view.row;
    let n = po_view.points;
    if (e >= n * po_view.jobs) {
        return;
    }
    let j = e / n;
    let k = e % n;
    let at = n + 4u * j;
    let b = po_data[at];
    let blk = po_data[at + 1u];
    let len = u32(cylinders[b + 3u]);
    let m = min(u32(cylinders[blk]), len);
    var rng = rng_init(e, 0x9E3779B9u);
    var p = ct_apply_symbol(po_points[po_data[k]], po_data[at + 2u], &rng, 0.0);
    for (var s = 0u; s < m; s = s + 1u) {
        p = ct_apply_symbol(p, u32(cylinders[b + 4u + s]), &rng, 0.0);
    }
    ct_forced_arm = -1;
    let rel = ct_offsets(p, b, blk, m, len);
    // A bad value is outside: the negated comparison, as fast-math wants.
    let bad = !(abs(rel.x) <= 1.0e30) || !(abs(rel.y) <= 1.0e30);
    let r = bitcast<f32>(po_view.radius_bits);
    if (!bad && dot(rel, rel) <= r * r) {
        atomicOr(&po_out[j * ((n + 31u) / 32u) + k / 32u], 1u << (k % 32u));
    }
}
