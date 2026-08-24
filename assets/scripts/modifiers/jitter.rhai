// Jitter
//
// A "modifier" script: instead of building a flame, it changes the one
// you already have. Good for nudging a flame you almost like.
//
// From the command line, pass the flame to modify with --base:
//   fractal_flame_wgpu generate --script jitter.rhai --base mine.fflame --seed 7

script("Jitter", "modifier");

let amount = param("amount", 0.5, 0.0, 2.0);
let jitter_colors = param_bool("jitter_colors", true);

// flame.transform_count() tells you how many there are; transform(i)
// hands you one. Note the `let t = ...` — you need to name it before
// changing its properties.
for i in 0..flame.transform_count() {
    let t = flame.transform(i);

    t.translate(rand(-amount, amount), rand(-amount, amount));
    t.rotate(rand(-amount, amount) * 30.0);
    t.scale(1.0 + rand(-amount, amount) * 0.5);

    if jitter_colors {
        // Keep the result inside 0.0–1.0 so it stays on the palette.
        let c = t.color + rand(-amount, amount);
        if c < 0.0 { c = 0.0; }
        if c > 1.0 { c = 1.0; }
        t.color = c;
    }
}

print("jittered " + flame.transform_count() + " transforms");
