// Itinerary, the built-in colouring's twin (from techmatt's engine): the
// orbit's angular address. Each iterate lands in one of `sectors` equal
// sectors, counted counter-clockwise from the negative real axis, and
// the sectors are the digits of one base-k fraction. Head is the first
// `depth` symbols; Tail rolls, keeping the last `depth` before the orbit
// stopped. In f32 the address holds 24 bits: 12 base-4 symbols.

const PARAMS = #{
    scale: #{
        value: 1.0, min: 0.001, max: 100.0,
        caption: "Scale",
        hint: "Palette turns across the whole address range, 0 to 1.",
    },
    sectors: #{
        value: 4.0, min: 2.0, max: 16.0,
        caption: "Sectors",
        hint: "How many equal angular sectors the plane is cut into: each step's sector is the next digit of the address, in base sectors.",
    },
    depth: #{
        value: 12.0, min: 1.0, max: 24.0,
        caption: "Symbols",
        hint: "How many symbols the address holds. f32 holds 24 bits: 12 symbols at 4 sectors, 8 at 8. Past that the deepest symbols round.",
    },
    window: #{
        value: 0, choices: ["Head", "Tail"],
        caption: "Window",
        hint: "Head: the first symbols the orbit spells. Tail: the last ones before it stopped, which puts the structure where the orbits are long.",
    },
};

const INFO = #{ title: "Itinerary" };

// The address, the symbols spelled so far, the weight the next head
// symbol carries, and the tail's bottom place k^-depth. The two place
// values are built by division on the first step, not by a power, which
// GPUs approximate: at a power-of-two k they are then exact.
fn start(z, p) {
    #{ address: 0.0, spelled: 0.0, weight: 0.0, bottom: 0.0 }
}

fn step(s, z, p) {
    let k = max(floor(p.sectors), 2.0);
    let depth = max(floor(p.depth), 1.0);
    if s.spelled == 0.0 {
        s.weight = 1.0 / k;
        let bottom = 1.0;
        for i in 0..to_int(depth) {
            bottom = bottom / k;
        }
        s.bottom = bottom;
    }
    // The sector, counter-clockwise from the negative real axis. The
    // origin reads as angle 0 rather than reaching the angle with a
    // zero pair.
    let turns = if max(abs(z.re), abs(z.im)) > 0.0 { (z.arg() + 3.14159265) / 6.2831855 } else { 0.5 };
    let sector = min(max(floor(turns * k), 0.0), k - 1.0);
    if p.window == 1 {
        // Tail: shift the window up a place, drop the oldest symbol,
        // write this one at the bottom.
        s.address = fract(s.address * k) + sector * s.bottom;
    } else if s.spelled < depth {
        s.address = s.address + sector * s.weight;
        s.weight = s.weight / k;
    }
    s.spelled = s.spelled + 1.0;
    s
}

fn finish(s, z, p) {
    s.address * p.scale
}
