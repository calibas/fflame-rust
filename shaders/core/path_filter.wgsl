// Path filter checking utilities

// Extract the last N iterations from the current path as a packed u32
// For suffix matching - gets the most recent iterations
fn get_suffix_pattern(path: array<u32, 4>, path_iteration: u32, length: u32) -> u32 {
    // The path is stored with LSB = earliest iteration
    // For suffix, we want the last 'length' iterations
    // These are at positions (path_iteration - length) to (path_iteration - 1)

    var result = 0u;
    let start_iter = path_iteration - length;

    for (var j = 0u; j < length; j++) {
        let iter_idx = start_iter + j;
        let slot = iter_idx / 8u;
        let pos = (iter_idx % 8u) * 4u;

        var word: u32;
        if (slot == 0u) {
            word = path[0];
        } else if (slot == 1u) {
            word = path[1];
        } else if (slot == 2u) {
            word = path[2];
        } else {
            word = path[3];
        }

        let xform = (word >> pos) & 0xFu;
        result = result | (xform << (j * 4u));
    }

    return result;
}

// Extract iterations [start, start+length) from the path as a packed u32
// For exact depth matching
fn get_path_pattern(path: array<u32, 4>, start: u32, length: u32) -> u32 {
    var result = 0u;

    for (var j = 0u; j < length; j++) {
        let iter_idx = start + j;
        let slot = iter_idx / 8u;
        let pos = (iter_idx % 8u) * 4u;

        var word: u32;
        if (slot == 0u) {
            word = path[0];
        } else if (slot == 1u) {
            word = path[1];
        } else if (slot == 2u) {
            word = path[2];
        } else {
            word = path[3];
        }

        let xform = (word >> pos) & 0xFu;
        result = result | (xform << (j * 4u));
    }

    return result;
}

// Check if current path matches any filter and should be blocked
// Returns true if the thread should be terminated
fn check_path_filters(path: array<u32, 4>, path_iteration: u32) -> bool {
    // Early exit if no filters
    if (params.num_path_filters == 0u) {
        return false;
    }

    // Check each filter
    for (var f = 0u; f < params.num_path_filters; f++) {
        let pf = path_filters[f];

        if (pf.depth == 0u) {
            // Suffix mode: check if we have enough iterations and suffix matches
            if (path_iteration >= pf.length && path_iteration >= params.min_suffix_filter_length) {
                let suffix = get_suffix_pattern(path, path_iteration, pf.length);
                if (suffix == pf.pattern) {
                    return true;  // Block this path
                }
            }
        } else {
            // Exact depth mode: only check at the specific depth
            if (path_iteration == pf.depth) {
                // Pattern starts at iteration (depth - length)
                let start = pf.depth - pf.length;
                let current_pattern = get_path_pattern(path, start, pf.length);
                if (current_pattern == pf.pattern) {
                    return true;  // Block this path
                }
            }
        }
    }

    return false;
}
