// PCG random number generator
// Provides deterministic random number generation for shader execution

// RNG state
struct RngState {
    state: u32,
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
    return rng;
}

// Generate random u32
fn rng_next(rng: ptr<function, RngState>) -> u32 {
    let old_state = (*rng).state;
    (*rng).state = old_state * 747796405u + 2891336453u;
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
