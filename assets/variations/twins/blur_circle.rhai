// Blur Circle, the built-in variation's twin, translated from its WGSL.

const INFO = #{ title: "Blur Circle", name: "rhai_blur_circle" };

fn variation(z, p) {
    let PI = 3.14159265359;
    let PI_4 = 0.78539816339;
    let x = 2.0 * rand() - 1.0;
    let y = 2.0 * rand() - 1.0;
    let absx = abs(x);
    let absy = abs(y);
    let perimeter = 0.0;
    let side = 0.0;
    if (absx >= absy) {
        if (x >= absy) {
            perimeter = absx + y;
        } else {
            perimeter = 5.0 * absx - y;
        }
        side = absx;
    } else {
        if (y >= absx) {
            perimeter = 3.0 * absy - x;
        } else {
            perimeter = 7.0 * absy + x;
        }
        side = absy;
    }
    let r = side;
    let angle = PI_4 * perimeter / side - PI_4;
    complex(r * cos(angle), r * sin(angle))
}
