// PCG random number generator
// Provides deterministic random number generation for shader execution

// RNG state: a 32-bit PCG state and its stream (the LCG's increment,
// always odd).
//
// The stream is what keeps a long render drawing new points. With one
// fixed increment every thread of every dispatch walked the SAME cycle
// of 2^32 states, only starting at different places in it; and since
// the chaos game forgets where it started within a few dozen
// iterations, a plotted point is a function of the recent draws, i.e.
// of the place in that cycle. So a render could hold at most ~2^32
// distinct samples: past a few billion iterations it re-plotted the
// same points and stopped adding detail (a deep zoom showed no change
// between 50B and 150B iterations). A per-thread, per-dispatch odd
// increment puts each walk on its own cycle: 2^63 starting places.
struct RngState {
    state: u32,
    inc: u32,
}

// PCG hash function
fn pcg_hash(input: u32) -> u32 {
    var state = input * 747796405u + 2891336453u;
    var word = ((state >> ((state >> 28u) + 4u)) ^ state) * 277803737u;
    return (word >> 22u) ^ word;
}

// Initialize RNG with thread ID and global seed
fn rng_init(thread_id: u32, seed: u32) -> RngState {
    var rng: RngState;
    rng.state = pcg_hash(thread_id ^ seed);
    // Not a function of `thread_id ^ seed` alone, or the pairs that share
    // the xor would share the stream too.
    rng.inc = (pcg_hash(pcg_hash(thread_id) + seed) << 1u) | 1u;
    return rng;
}

// Generate random u32
fn rng_next(rng: ptr<function, RngState>) -> u32 {
    let old_state = (*rng).state;
    (*rng).state = old_state * 747796405u + (*rng).inc;
    let xor_shifted = ((old_state >> ((old_state >> 28u) + 4u)) ^ old_state) * 277803737u;
    return (xor_shifted >> 22u) ^ xor_shifted;
}

// Generate random f32 in [0, 1)
//
// The top 24 bits, which an f32 holds exactly: a multiple of 2^-24, at
// most 1 - 2^-24. `f32(u32) / 2^32` rounded the top 128 values up to 2^32
// and returned exactly 1.0, one draw in 2^25 -- so `rng_nextf() < opacity`
// dropped a plot at opacity 1, and `u32(rng_nextf() * n)` could be `n`.
fn rng_nextf(rng: ptr<function, RngState>) -> f32 {
    return f32(rng_next(rng) >> 8u) * (1.0 / 16777216.0);
}
