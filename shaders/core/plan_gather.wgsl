// The planner's candidate gather on the GPU -- gpu-cylinder-planning.md
// phase 4, and `Backward::gather_seen` exactly.
//
// A gather job reads one of the walk's indexes (sample indices filed by
// grid cell, sorted by cell) over a sorted list of cells, and takes every
// `step`-th entry of the concatenated per-cell runs, `step` chosen so at
// most `cap` come out. Three passes, one submission:
//
//   gather_ranges  one thread per (job, cell): the cell's run in the index,
//                  by two binary searches
//   gather_scan    one workgroup per job: the runs' starting positions in
//                  the concatenation, the total, `step` and the count
//   gather_fill    one thread per output slot: the entry at position
//                  `slot * step`, found by a binary search over the runs
//
// The plan's check of the candidates is `plan_eval_gathered` in
// `plan_eval.wgsl`, which reads `gcands` as this leaves it.
//
// Integer arithmetic throughout, so the candidates are the CPU's to the
// bit, and a plan does not depend on which side gathered.

struct GatherView {
    // Gather jobs, (job, cell) pairs, and output slots.
    jobs: u32,
    pairs: u32,
    slots: u32,
    // Threads per row of the pair and slot dispatches; workgroups per row
    // of the scan dispatch.
    row_pairs: u32,
    row_slots: u32,
    row_jobs: u32,
    // Where the cell lists start in `gdata`, in u32s.
    cells_base: u32,
    _pad: u32,
}

// Every index, concatenated: the cells sorted within each index, and the
// sample index filed under each.
@group(0) @binding(0) var<storage, read> ix_cells: array<vec2<i32>>;
@group(0) @binding(1) var<storage, read> ix_idx: array<u32>;
// Job g at gdata[8g .. 8g + 8]: the index's entry range [lo, hi), where
// its cells start and how many, where its pairs start, its cap, where its
// output slots start. Then the cell lists, (x, y) as i32 bits.
@group(0) @binding(2) var<storage, read> gdata: array<u32>;
// Per pair: the run's first entry, its length, and its position in the
// job's concatenation.
@group(0) @binding(3) var<storage, read_write> gpairs: array<u32>;
// Per job: [count, step]; then one slot per possible output, holding a
// sample index or NONE.
@group(0) @binding(4) var<storage, read_write> gcands: array<u32>;
@group(0) @binding(5) var<uniform> gview: GatherView;

const JOB: u32 = 8u;
const NONE: u32 = 0xFFFFFFFFu;

fn gather_cell(i: u32) -> vec2<i32> {
    let b = gview.cells_base + 2u * i;
    return vec2<i32>(bitcast<i32>(gdata[b]), bitcast<i32>(gdata[b + 1u]));
}

// a < b, x first -- the order `Index` sorts its `(Cell, u32)` entries in.
fn gather_less(a: vec2<i32>, b: vec2<i32>) -> bool {
    return a.x < b.x || (a.x == b.x && a.y < b.y);
}

// The first entry in [lo, hi) whose cell is not below `c` (`upper` false)
// or is above it (`upper` true): `partition_point` both ways.
fn gather_bound(lo0: u32, hi0: u32, c: vec2<i32>, upper: bool) -> u32 {
    var lo = lo0;
    var hi = hi0;
    loop {
        if (lo >= hi) {
            break;
        }
        let mid = lo + (hi - lo) / 2u;
        let m = ix_cells[mid];
        let right = select(gather_less(m, c), !gather_less(c, m), upper);
        if (right) {
            lo = mid + 1u;
        } else {
            hi = mid;
        }
    }
    return lo;
}

// The last job whose offset at `field` is <= e. Offsets are running sums,
// so a job with nothing shares its offset with the next and is skipped.
fn gather_job_of(e: u32, field: u32) -> u32 {
    var lo = 0u;
    var hi = gview.jobs - 1u;
    loop {
        if (lo >= hi) {
            break;
        }
        let mid = (lo + hi + 1u) / 2u;
        if (gdata[JOB * mid + field] <= e) {
            lo = mid;
        } else {
            hi = mid - 1u;
        }
    }
    return lo;
}

@compute @workgroup_size(64, 1, 1)
fn gather_ranges(@builtin(global_invocation_id) gid: vec3<u32>) {
    let p = gid.x + gid.y * gview.row_pairs;
    if (p >= gview.pairs) {
        return;
    }
    let g = gather_job_of(p, 4u);
    let b = JOB * g;
    let c = gather_cell(gdata[b + 2u] + (p - gdata[b + 4u]));
    let lo = gather_bound(gdata[b], gdata[b + 1u], c, false);
    let hi = gather_bound(lo, gdata[b + 1u], c, true);
    gpairs[3u * p] = lo;
    gpairs[3u * p + 1u] = hi - lo;
}

var<workgroup> wg_sum: array<u32, 256>;
var<workgroup> wg_n: u32;

@compute @workgroup_size(256, 1, 1)
fn gather_scan(@builtin(workgroup_id) wid: vec3<u32>, @builtin(local_invocation_index) li: u32) {
    // One workgroup per job, so `g` is uniform and so is the early return.
    let g = wid.x + wid.y * gview.row_jobs;
    if (g >= gview.jobs) {
        return;
    }
    let b = JOB * g;
    if (li == 0u) {
        wg_n = gdata[b + 3u];
    }
    // The loop bound must be uniform for the barriers inside it.
    let n = workgroupUniformLoad(&wg_n);
    let p0 = gdata[b + 4u];
    var running = 0u;
    var t = 0u;
    loop {
        if (t >= n) {
            break;
        }
        let i = t + li;
        var v = 0u;
        if (i < n) {
            v = gpairs[3u * (p0 + i) + 1u];
        }
        wg_sum[li] = v;
        workgroupBarrier();
        // Inclusive scan, Hillis-Steele.
        for (var d = 1u; d < 256u; d = d * 2u) {
            var add = 0u;
            if (li >= d) {
                add = wg_sum[li - d];
            }
            workgroupBarrier();
            wg_sum[li] = wg_sum[li] + add;
            workgroupBarrier();
        }
        if (i < n) {
            gpairs[3u * (p0 + i) + 2u] = running + wg_sum[li] - v;
        }
        let tile = wg_sum[255];
        workgroupBarrier();
        running = running + tile;
        t = t + 256u;
    }
    if (li == 0u) {
        // `Backward::gather_seen`: step = max(ceil(total / cap), 1), and
        // the positions taken are the multiples of step below total.
        let cap = max(gdata[b + 5u], 1u);
        let step = max((running + cap - 1u) / cap, 1u);
        gcands[2u * g] = (running + step - 1u) / step;
        gcands[2u * g + 1u] = step;
    }
}

@compute @workgroup_size(64, 1, 1)
fn gather_fill(@builtin(global_invocation_id) gid: vec3<u32>) {
    let s = gid.x + gid.y * gview.row_slots;
    if (s >= gview.slots) {
        return;
    }
    let g = gather_job_of(s, 6u);
    let b = JOB * g;
    let j = s - gdata[b + 6u];
    let slot = 2u * gview.jobs + s;
    if (j >= gcands[2u * g]) {
        gcands[slot] = NONE;
        return;
    }
    let at = j * gcands[2u * g + 1u];
    // The run holding position `at`: the last pair starting at or before
    // it. A pair with an empty run starts where the next does, so the
    // last is never an empty one while `at` is below the total.
    let p0 = gdata[b + 4u];
    var lo = 0u;
    var hi = gdata[b + 3u] - 1u;
    loop {
        if (lo >= hi) {
            break;
        }
        let mid = (lo + hi + 1u) / 2u;
        if (gpairs[3u * (p0 + mid) + 2u] <= at) {
            lo = mid;
        } else {
            hi = mid - 1u;
        }
    }
    let p = p0 + lo;
    gcands[slot] = ix_idx[gpairs[3u * p] + (at - gpairs[3u * p + 2u])];
}
