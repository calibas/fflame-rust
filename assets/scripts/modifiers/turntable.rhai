// Turntable
//
// Adds a looping rotation to the flame you have open, without touching
// the flame itself — this is a script that only describes MOVEMENT.
//
// A full turn ends exactly where it started, so the video loops
// seamlessly. In 3D that means orbiting the camera; in 2D there is no
// camera to orbit, so the image itself spins.
//
// Run it, then open the Animation panel and press play.

script("Turntable", "modifier", ["norng"]);

let seconds = param("seconds", 8.0, 1.0, 120.0);
let turns = param_int("turns", 1, 1, 8);
let easing = param_choice("motion", ["Constant speed", "Ease in and out"], 0);

// A turntable spins whatever axis actually turns the picture. In 3D
// that's the camera's yaw; in 2D there is no camera, so it's the image
// rotation. Reading the mode rather than assuming keeps one script
// useful for both.
let is_3d = false;
try {
    is_3d = config.get("render_mode") == "3d";
} catch {
    // render_mode is skipped when it's at its default (2d).
}

let target = if is_3d { "camera_rotation_y" } else { "rotation" };

// Start from wherever the flame already sits, so the first frame is the
// image you were just looking at rather than a jump to zero.
let start = 0.0;
try {
    start = config.get(target);
} catch {
    // Absent means it's at the default of 0.
}

anim.name = "Turntable";
anim.duration = seconds;

// Ease across the WHOLE move rather than per-segment: easing each key
// separately would stall at every quarter turn.
let ease = if easing == "Ease in and out" { "ease_in_out" } else { "linear" };
anim.key(target, 0.0, start, ease);
// Angles are stored in RADIANS (the View panel converts for display),
// so a turn is 2*PI, not 360. Exact, so the loop closes on itself.
anim.key(target, seconds, start + 2.0 * PI() * turns, ease);

if is_3d {
    print("3D flame: orbiting the camera (yaw) " + turns + " turn(s) over " + seconds + "s.");
} else {
    print("2D flame: rotating the image " + turns + " turn(s) over " + seconds + "s.");
}
print("The end angle is a whole number of turns, so the loop is seamless.");
print("Open the Animation panel to play it, or export a video.");
