/// Constrains points to a rectangle before the rest of the variations run.
/// Points outside the rectangle either collapse to zero or get scattered
/// along the nearest edge.
/// 
/// # Authors
/// - Xyrus02

// Pre Crop, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    left: #{
        value: -1.0, min: -5.0, max: 5.0, free: true,
        caption: "Left",
        hint: "Left edge of the rectangle the points are constrained to.",
    },
    top: #{
        value: -1.0, min: -5.0, max: 5.0, free: true,
        caption: "Top",
        hint: "Top edge of the rectangle.",
    },
    right: #{
        value: 1.0, min: -5.0, max: 5.0, free: true,
        caption: "Right",
        hint: "Right edge of the rectangle.",
    },
    bottom: #{
        value: 1.0, min: -5.0, max: 5.0, free: true,
        caption: "Bottom",
        hint: "Bottom edge of the rectangle.",
    },
    scatter_area: #{
        value: 0.0, min: -1.0, max: 1.0,
        caption: "Scatter Area",
        hint: "Width of the random scatter band along the rectangle's edges. 0 = points snap exactly to the edge.",
    },
    zero: #{
        value: false,
        caption: "Zero",
        hint: "When on, points outside the rectangle collapse to the origin. When off, they scatter back to the nearest edge.",
    },
};

const INFO = #{ title: "Pre Crop", name: "rhai_pre_crop", category: "advanced_2d", phase: "pre" };

fn variation(z, p) {
    let x0 = p.left;
    let y0 = p.top;
    let x1 = p.right;
    let y1 = p.bottom;
    let scatter = p.scatter_area;
    let zero = to_float(p.zero);
    let _x0 = (if x0 < x1 { x0 } else { x1 });
    let _x1 = (if x0 < x1 { x1 } else { x0 });
    let _y0 = (if y0 < y1 { y0 } else { y1 });
    let _y1 = (if y0 < y1 { y1 } else { y0 });
    let w = (_x1 - _x0) * 0.5 * scatter;
    let h = (_y1 - _y0) * 0.5 * scatter;
    let x = z.re;
    let y = z.im;
    if ((x < _x0) || (x > _x1) || (y < _y0) || (y > _y1)) && (zero > 0.5) {
        return complex(0.0, 0.0);
    }
    if x < _x0 {
        x = _x0 + rand() * w;
    } else if x > _x1 {
        x = _x1 - rand() * w;
    }
    if y < _y0 {
        y = _y0 + rand() * h;
    } else if y > _y1 {
        y = _y1 - rand() * h;
    }
    complex(x, y)
}
