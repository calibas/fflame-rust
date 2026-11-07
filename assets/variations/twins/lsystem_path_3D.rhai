/// Finite-depth 3D IFS path: up to 12 full 3D affine maps composed by
/// the digits of the curve parameter. Written by the L-System scripts.
///
/// # Authors
/// - Fractals for All
/// - Claude Fable 5

// L-System Path 3D, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    iterations: #{
        value: 5, min: 1, max: 12,
        caption: "Iterations",
        hint: "L-system depth: the path has map_count^iterations segments. A live parameter.",
    },
    map_count: #{
        value: 4, min: 2, max: 12,
        caption: "Map Count",
        hint: "How many of the twelve map slots are in use. Set by the L-System script.",
    },
    connect: #{
        value: true,
        caption: "Connect",
        hint: "Draw the connecting segments between consecutive vertices. Off plots only the vertices.",
    },
    dc: #{
        value: true,
        caption: "Direct Color",
        hint: "Color by the curve parameter t (needs the transform's Direct Color at 1).",
    },
    m0_xx: #{
        value: 1.0, min: -4.0, max: 4.0, free: true,
        caption: "M0 XX",
        hint: "Map 0 affine coefficient xx. Normally written by the L-System script.",
    },
    m0_xy: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "M0 XY",
        hint: "Map 0 affine coefficient xy. Normally written by the L-System script.",
    },
    m0_xz: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "M0 XZ",
        hint: "Map 0 affine coefficient xz. Normally written by the L-System script.",
    },
    m0_yx: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "M0 YX",
        hint: "Map 0 affine coefficient yx. Normally written by the L-System script.",
    },
    m0_yy: #{
        value: 1.0, min: -4.0, max: 4.0, free: true,
        caption: "M0 YY",
        hint: "Map 0 affine coefficient yy. Normally written by the L-System script.",
    },
    m0_yz: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "M0 YZ",
        hint: "Map 0 affine coefficient yz. Normally written by the L-System script.",
    },
    m0_zx: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "M0 ZX",
        hint: "Map 0 affine coefficient zx. Normally written by the L-System script.",
    },
    m0_zy: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "M0 ZY",
        hint: "Map 0 affine coefficient zy. Normally written by the L-System script.",
    },
    m0_zz: #{
        value: 1.0, min: -4.0, max: 4.0, free: true,
        caption: "M0 ZZ",
        hint: "Map 0 affine coefficient zz. Normally written by the L-System script.",
    },
    m0_tx: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "M0 TX",
        hint: "Map 0 affine coefficient tx. Normally written by the L-System script.",
    },
    m0_ty: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "M0 TY",
        hint: "Map 0 affine coefficient ty. Normally written by the L-System script.",
    },
    m0_tz: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "M0 TZ",
        hint: "Map 0 affine coefficient tz. Normally written by the L-System script.",
    },
    m1_xx: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "M1 XX",
        hint: "Map 1 affine coefficient xx. Normally written by the L-System script.",
    },
    m1_xy: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "M1 XY",
        hint: "Map 1 affine coefficient xy. Normally written by the L-System script.",
    },
    m1_xz: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "M1 XZ",
        hint: "Map 1 affine coefficient xz. Normally written by the L-System script.",
    },
    m1_yx: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "M1 YX",
        hint: "Map 1 affine coefficient yx. Normally written by the L-System script.",
    },
    m1_yy: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "M1 YY",
        hint: "Map 1 affine coefficient yy. Normally written by the L-System script.",
    },
    m1_yz: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "M1 YZ",
        hint: "Map 1 affine coefficient yz. Normally written by the L-System script.",
    },
    m1_zx: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "M1 ZX",
        hint: "Map 1 affine coefficient zx. Normally written by the L-System script.",
    },
    m1_zy: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "M1 ZY",
        hint: "Map 1 affine coefficient zy. Normally written by the L-System script.",
    },
    m1_zz: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "M1 ZZ",
        hint: "Map 1 affine coefficient zz. Normally written by the L-System script.",
    },
    m1_tx: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "M1 TX",
        hint: "Map 1 affine coefficient tx. Normally written by the L-System script.",
    },
    m1_ty: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "M1 TY",
        hint: "Map 1 affine coefficient ty. Normally written by the L-System script.",
    },
    m1_tz: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "M1 TZ",
        hint: "Map 1 affine coefficient tz. Normally written by the L-System script.",
    },
    m2_xx: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "M2 XX",
        hint: "Map 2 affine coefficient xx. Normally written by the L-System script.",
    },
    m2_xy: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "M2 XY",
        hint: "Map 2 affine coefficient xy. Normally written by the L-System script.",
    },
    m2_xz: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "M2 XZ",
        hint: "Map 2 affine coefficient xz. Normally written by the L-System script.",
    },
    m2_yx: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "M2 YX",
        hint: "Map 2 affine coefficient yx. Normally written by the L-System script.",
    },
    m2_yy: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "M2 YY",
        hint: "Map 2 affine coefficient yy. Normally written by the L-System script.",
    },
    m2_yz: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "M2 YZ",
        hint: "Map 2 affine coefficient yz. Normally written by the L-System script.",
    },
    m2_zx: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "M2 ZX",
        hint: "Map 2 affine coefficient zx. Normally written by the L-System script.",
    },
    m2_zy: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "M2 ZY",
        hint: "Map 2 affine coefficient zy. Normally written by the L-System script.",
    },
    m2_zz: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "M2 ZZ",
        hint: "Map 2 affine coefficient zz. Normally written by the L-System script.",
    },
    m2_tx: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "M2 TX",
        hint: "Map 2 affine coefficient tx. Normally written by the L-System script.",
    },
    m2_ty: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "M2 TY",
        hint: "Map 2 affine coefficient ty. Normally written by the L-System script.",
    },
    m2_tz: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "M2 TZ",
        hint: "Map 2 affine coefficient tz. Normally written by the L-System script.",
    },
    m3_xx: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "M3 XX",
        hint: "Map 3 affine coefficient xx. Normally written by the L-System script.",
    },
    m3_xy: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "M3 XY",
        hint: "Map 3 affine coefficient xy. Normally written by the L-System script.",
    },
    m3_xz: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "M3 XZ",
        hint: "Map 3 affine coefficient xz. Normally written by the L-System script.",
    },
    m3_yx: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "M3 YX",
        hint: "Map 3 affine coefficient yx. Normally written by the L-System script.",
    },
    m3_yy: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "M3 YY",
        hint: "Map 3 affine coefficient yy. Normally written by the L-System script.",
    },
    m3_yz: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "M3 YZ",
        hint: "Map 3 affine coefficient yz. Normally written by the L-System script.",
    },
    m3_zx: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "M3 ZX",
        hint: "Map 3 affine coefficient zx. Normally written by the L-System script.",
    },
    m3_zy: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "M3 ZY",
        hint: "Map 3 affine coefficient zy. Normally written by the L-System script.",
    },
    m3_zz: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "M3 ZZ",
        hint: "Map 3 affine coefficient zz. Normally written by the L-System script.",
    },
    m3_tx: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "M3 TX",
        hint: "Map 3 affine coefficient tx. Normally written by the L-System script.",
    },
    m3_ty: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "M3 TY",
        hint: "Map 3 affine coefficient ty. Normally written by the L-System script.",
    },
    m3_tz: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "M3 TZ",
        hint: "Map 3 affine coefficient tz. Normally written by the L-System script.",
    },
    m4_xx: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "M4 XX",
        hint: "Map 4 affine coefficient xx. Normally written by the L-System script.",
    },
    m4_xy: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "M4 XY",
        hint: "Map 4 affine coefficient xy. Normally written by the L-System script.",
    },
    m4_xz: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "M4 XZ",
        hint: "Map 4 affine coefficient xz. Normally written by the L-System script.",
    },
    m4_yx: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "M4 YX",
        hint: "Map 4 affine coefficient yx. Normally written by the L-System script.",
    },
    m4_yy: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "M4 YY",
        hint: "Map 4 affine coefficient yy. Normally written by the L-System script.",
    },
    m4_yz: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "M4 YZ",
        hint: "Map 4 affine coefficient yz. Normally written by the L-System script.",
    },
    m4_zx: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "M4 ZX",
        hint: "Map 4 affine coefficient zx. Normally written by the L-System script.",
    },
    m4_zy: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "M4 ZY",
        hint: "Map 4 affine coefficient zy. Normally written by the L-System script.",
    },
    m4_zz: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "M4 ZZ",
        hint: "Map 4 affine coefficient zz. Normally written by the L-System script.",
    },
    m4_tx: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "M4 TX",
        hint: "Map 4 affine coefficient tx. Normally written by the L-System script.",
    },
    m4_ty: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "M4 TY",
        hint: "Map 4 affine coefficient ty. Normally written by the L-System script.",
    },
    m4_tz: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "M4 TZ",
        hint: "Map 4 affine coefficient tz. Normally written by the L-System script.",
    },
    m5_xx: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "M5 XX",
        hint: "Map 5 affine coefficient xx. Normally written by the L-System script.",
    },
    m5_xy: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "M5 XY",
        hint: "Map 5 affine coefficient xy. Normally written by the L-System script.",
    },
    m5_xz: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "M5 XZ",
        hint: "Map 5 affine coefficient xz. Normally written by the L-System script.",
    },
    m5_yx: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "M5 YX",
        hint: "Map 5 affine coefficient yx. Normally written by the L-System script.",
    },
    m5_yy: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "M5 YY",
        hint: "Map 5 affine coefficient yy. Normally written by the L-System script.",
    },
    m5_yz: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "M5 YZ",
        hint: "Map 5 affine coefficient yz. Normally written by the L-System script.",
    },
    m5_zx: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "M5 ZX",
        hint: "Map 5 affine coefficient zx. Normally written by the L-System script.",
    },
    m5_zy: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "M5 ZY",
        hint: "Map 5 affine coefficient zy. Normally written by the L-System script.",
    },
    m5_zz: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "M5 ZZ",
        hint: "Map 5 affine coefficient zz. Normally written by the L-System script.",
    },
    m5_tx: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "M5 TX",
        hint: "Map 5 affine coefficient tx. Normally written by the L-System script.",
    },
    m5_ty: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "M5 TY",
        hint: "Map 5 affine coefficient ty. Normally written by the L-System script.",
    },
    m5_tz: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "M5 TZ",
        hint: "Map 5 affine coefficient tz. Normally written by the L-System script.",
    },
    m6_xx: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "M6 XX",
        hint: "Map 6 affine coefficient xx. Normally written by the L-System script.",
    },
    m6_xy: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "M6 XY",
        hint: "Map 6 affine coefficient xy. Normally written by the L-System script.",
    },
    m6_xz: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "M6 XZ",
        hint: "Map 6 affine coefficient xz. Normally written by the L-System script.",
    },
    m6_yx: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "M6 YX",
        hint: "Map 6 affine coefficient yx. Normally written by the L-System script.",
    },
    m6_yy: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "M6 YY",
        hint: "Map 6 affine coefficient yy. Normally written by the L-System script.",
    },
    m6_yz: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "M6 YZ",
        hint: "Map 6 affine coefficient yz. Normally written by the L-System script.",
    },
    m6_zx: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "M6 ZX",
        hint: "Map 6 affine coefficient zx. Normally written by the L-System script.",
    },
    m6_zy: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "M6 ZY",
        hint: "Map 6 affine coefficient zy. Normally written by the L-System script.",
    },
    m6_zz: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "M6 ZZ",
        hint: "Map 6 affine coefficient zz. Normally written by the L-System script.",
    },
    m6_tx: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "M6 TX",
        hint: "Map 6 affine coefficient tx. Normally written by the L-System script.",
    },
    m6_ty: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "M6 TY",
        hint: "Map 6 affine coefficient ty. Normally written by the L-System script.",
    },
    m6_tz: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "M6 TZ",
        hint: "Map 6 affine coefficient tz. Normally written by the L-System script.",
    },
    m7_xx: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "M7 XX",
        hint: "Map 7 affine coefficient xx. Normally written by the L-System script.",
    },
    m7_xy: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "M7 XY",
        hint: "Map 7 affine coefficient xy. Normally written by the L-System script.",
    },
    m7_xz: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "M7 XZ",
        hint: "Map 7 affine coefficient xz. Normally written by the L-System script.",
    },
    m7_yx: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "M7 YX",
        hint: "Map 7 affine coefficient yx. Normally written by the L-System script.",
    },
    m7_yy: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "M7 YY",
        hint: "Map 7 affine coefficient yy. Normally written by the L-System script.",
    },
    m7_yz: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "M7 YZ",
        hint: "Map 7 affine coefficient yz. Normally written by the L-System script.",
    },
    m7_zx: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "M7 ZX",
        hint: "Map 7 affine coefficient zx. Normally written by the L-System script.",
    },
    m7_zy: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "M7 ZY",
        hint: "Map 7 affine coefficient zy. Normally written by the L-System script.",
    },
    m7_zz: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "M7 ZZ",
        hint: "Map 7 affine coefficient zz. Normally written by the L-System script.",
    },
    m7_tx: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "M7 TX",
        hint: "Map 7 affine coefficient tx. Normally written by the L-System script.",
    },
    m7_ty: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "M7 TY",
        hint: "Map 7 affine coefficient ty. Normally written by the L-System script.",
    },
    m7_tz: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "M7 TZ",
        hint: "Map 7 affine coefficient tz. Normally written by the L-System script.",
    },
    m8_xx: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "M8 XX",
        hint: "Map 8 affine coefficient xx. Normally written by the L-System script.",
    },
    m8_xy: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "M8 XY",
        hint: "Map 8 affine coefficient xy. Normally written by the L-System script.",
    },
    m8_xz: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "M8 XZ",
        hint: "Map 8 affine coefficient xz. Normally written by the L-System script.",
    },
    m8_yx: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "M8 YX",
        hint: "Map 8 affine coefficient yx. Normally written by the L-System script.",
    },
    m8_yy: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "M8 YY",
        hint: "Map 8 affine coefficient yy. Normally written by the L-System script.",
    },
    m8_yz: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "M8 YZ",
        hint: "Map 8 affine coefficient yz. Normally written by the L-System script.",
    },
    m8_zx: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "M8 ZX",
        hint: "Map 8 affine coefficient zx. Normally written by the L-System script.",
    },
    m8_zy: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "M8 ZY",
        hint: "Map 8 affine coefficient zy. Normally written by the L-System script.",
    },
    m8_zz: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "M8 ZZ",
        hint: "Map 8 affine coefficient zz. Normally written by the L-System script.",
    },
    m8_tx: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "M8 TX",
        hint: "Map 8 affine coefficient tx. Normally written by the L-System script.",
    },
    m8_ty: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "M8 TY",
        hint: "Map 8 affine coefficient ty. Normally written by the L-System script.",
    },
    m8_tz: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "M8 TZ",
        hint: "Map 8 affine coefficient tz. Normally written by the L-System script.",
    },
    m9_xx: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "M9 XX",
        hint: "Map 9 affine coefficient xx. Normally written by the L-System script.",
    },
    m9_xy: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "M9 XY",
        hint: "Map 9 affine coefficient xy. Normally written by the L-System script.",
    },
    m9_xz: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "M9 XZ",
        hint: "Map 9 affine coefficient xz. Normally written by the L-System script.",
    },
    m9_yx: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "M9 YX",
        hint: "Map 9 affine coefficient yx. Normally written by the L-System script.",
    },
    m9_yy: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "M9 YY",
        hint: "Map 9 affine coefficient yy. Normally written by the L-System script.",
    },
    m9_yz: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "M9 YZ",
        hint: "Map 9 affine coefficient yz. Normally written by the L-System script.",
    },
    m9_zx: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "M9 ZX",
        hint: "Map 9 affine coefficient zx. Normally written by the L-System script.",
    },
    m9_zy: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "M9 ZY",
        hint: "Map 9 affine coefficient zy. Normally written by the L-System script.",
    },
    m9_zz: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "M9 ZZ",
        hint: "Map 9 affine coefficient zz. Normally written by the L-System script.",
    },
    m9_tx: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "M9 TX",
        hint: "Map 9 affine coefficient tx. Normally written by the L-System script.",
    },
    m9_ty: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "M9 TY",
        hint: "Map 9 affine coefficient ty. Normally written by the L-System script.",
    },
    m9_tz: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "M9 TZ",
        hint: "Map 9 affine coefficient tz. Normally written by the L-System script.",
    },
    m10_xx: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "M10 XX",
        hint: "Map 10 affine coefficient xx. Normally written by the L-System script.",
    },
    m10_xy: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "M10 XY",
        hint: "Map 10 affine coefficient xy. Normally written by the L-System script.",
    },
    m10_xz: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "M10 XZ",
        hint: "Map 10 affine coefficient xz. Normally written by the L-System script.",
    },
    m10_yx: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "M10 YX",
        hint: "Map 10 affine coefficient yx. Normally written by the L-System script.",
    },
    m10_yy: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "M10 YY",
        hint: "Map 10 affine coefficient yy. Normally written by the L-System script.",
    },
    m10_yz: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "M10 YZ",
        hint: "Map 10 affine coefficient yz. Normally written by the L-System script.",
    },
    m10_zx: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "M10 ZX",
        hint: "Map 10 affine coefficient zx. Normally written by the L-System script.",
    },
    m10_zy: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "M10 ZY",
        hint: "Map 10 affine coefficient zy. Normally written by the L-System script.",
    },
    m10_zz: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "M10 ZZ",
        hint: "Map 10 affine coefficient zz. Normally written by the L-System script.",
    },
    m10_tx: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "M10 TX",
        hint: "Map 10 affine coefficient tx. Normally written by the L-System script.",
    },
    m10_ty: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "M10 TY",
        hint: "Map 10 affine coefficient ty. Normally written by the L-System script.",
    },
    m10_tz: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "M10 TZ",
        hint: "Map 10 affine coefficient tz. Normally written by the L-System script.",
    },
    m11_xx: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "M11 XX",
        hint: "Map 11 affine coefficient xx. Normally written by the L-System script.",
    },
    m11_xy: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "M11 XY",
        hint: "Map 11 affine coefficient xy. Normally written by the L-System script.",
    },
    m11_xz: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "M11 XZ",
        hint: "Map 11 affine coefficient xz. Normally written by the L-System script.",
    },
    m11_yx: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "M11 YX",
        hint: "Map 11 affine coefficient yx. Normally written by the L-System script.",
    },
    m11_yy: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "M11 YY",
        hint: "Map 11 affine coefficient yy. Normally written by the L-System script.",
    },
    m11_yz: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "M11 YZ",
        hint: "Map 11 affine coefficient yz. Normally written by the L-System script.",
    },
    m11_zx: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "M11 ZX",
        hint: "Map 11 affine coefficient zx. Normally written by the L-System script.",
    },
    m11_zy: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "M11 ZY",
        hint: "Map 11 affine coefficient zy. Normally written by the L-System script.",
    },
    m11_zz: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "M11 ZZ",
        hint: "Map 11 affine coefficient zz. Normally written by the L-System script.",
    },
    m11_tx: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "M11 TX",
        hint: "Map 11 affine coefficient tx. Normally written by the L-System script.",
    },
    m11_ty: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "M11 TY",
        hint: "Map 11 affine coefficient ty. Normally written by the L-System script.",
    },
    m11_tz: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "M11 TZ",
        hint: "Map 11 affine coefficient tz. Normally written by the L-System script.",
    },
    anchored: #{
        value: false,
        caption: "Anchored",
        hint: "Vertex-chain mode: connect the images of the anchor point in consecutive cells instead of each cell's entry-to-exit span. Space-filling curves need this — their spans lie on the cell-edge lattice and overlap each other (three-way joins, phantom dead ends); the centre chain is the classic self-avoiding drawing.",
    },
    anchor_x: #{
        value: 0.5, min: -2.0, max: 2.0, free: true,
        caption: "Anchor X",
        hint: "Anchor point x (the attractor's centre, set by the script).",
    },
    anchor_y: #{
        value: 0.0, min: -2.0, max: 2.0, free: true,
        caption: "Anchor Y",
        hint: "Anchor point y.",
    },
    anchor_z: #{
        value: 0.0, min: -2.0, max: 2.0, free: true,
        caption: "Anchor Z",
        hint: "Anchor point z.",
    },
    thickness: #{
        value: 0.0, min: 0.0, max: 0.2,
        caption: "Thickness",
        hint: "Tube radius around the drawn line, in the curve's own units (the whole curve spans 1). Samples land on the pipe SURFACE, and corners are joined seamlessly: each cylinder is clipped at the mitre plane it shares with its neighbours and the sphere patch the turn exposes is filled, so the surface is covered exactly once — no bead at a vertex. Note the density trade: the same samples spread over more surface, so a thick tube is dimmer; raise Brightness to match.",
    },
    soft: #{
        value: false,
        caption: "Soft Edges",
        hint: "Gaussian shell instead of a hard surface: samples fade inward and outward from the pipe wall. Hard reads as drawn geometry; soft reads as a glow.",
    },
    offset_x: #{
        value: 0.0, min: -2.0, max: 2.0, free: true,
        caption: "Offset X",
        hint: "Move the whole curve along x, in the curve's own units. The path is built in its own frame — the unit cube for a space-filling curve — so an offset of minus its centre puts the object on the origin, which is what camera rotation and zoom orbit around. The script sets this to centre the curve.",
    },
    offset_y: #{
        value: 0.0, min: -2.0, max: 2.0, free: true,
        caption: "Offset Y",
        hint: "Move the whole curve along y.",
    },
    offset_z: #{
        value: 0.0, min: -2.0, max: 2.0, free: true,
        caption: "Offset Z",
        hint: "Move the whole curve along z.",
    },
};

const INFO = #{ title: "L-System Path 3D", name: "rhai_lsystem_path_3d", category: "full_3d" };

fn prepare(p) {
    let out = array(157, 0.0);
    out[0] = to_float(p.iterations);
    out[1] = to_float(p.map_count);
    out[2] = to_float(p.connect);
    out[3] = to_float(p.dc);
    out[4] = p.m0_xx;
    out[5] = p.m0_xy;
    out[6] = p.m0_xz;
    out[7] = p.m0_yx;
    out[8] = p.m0_yy;
    out[9] = p.m0_yz;
    out[10] = p.m0_zx;
    out[11] = p.m0_zy;
    out[12] = p.m0_zz;
    out[13] = p.m0_tx;
    out[14] = p.m0_ty;
    out[15] = p.m0_tz;
    out[16] = p.m1_xx;
    out[17] = p.m1_xy;
    out[18] = p.m1_xz;
    out[19] = p.m1_yx;
    out[20] = p.m1_yy;
    out[21] = p.m1_yz;
    out[22] = p.m1_zx;
    out[23] = p.m1_zy;
    out[24] = p.m1_zz;
    out[25] = p.m1_tx;
    out[26] = p.m1_ty;
    out[27] = p.m1_tz;
    out[28] = p.m2_xx;
    out[29] = p.m2_xy;
    out[30] = p.m2_xz;
    out[31] = p.m2_yx;
    out[32] = p.m2_yy;
    out[33] = p.m2_yz;
    out[34] = p.m2_zx;
    out[35] = p.m2_zy;
    out[36] = p.m2_zz;
    out[37] = p.m2_tx;
    out[38] = p.m2_ty;
    out[39] = p.m2_tz;
    out[40] = p.m3_xx;
    out[41] = p.m3_xy;
    out[42] = p.m3_xz;
    out[43] = p.m3_yx;
    out[44] = p.m3_yy;
    out[45] = p.m3_yz;
    out[46] = p.m3_zx;
    out[47] = p.m3_zy;
    out[48] = p.m3_zz;
    out[49] = p.m3_tx;
    out[50] = p.m3_ty;
    out[51] = p.m3_tz;
    out[52] = p.m4_xx;
    out[53] = p.m4_xy;
    out[54] = p.m4_xz;
    out[55] = p.m4_yx;
    out[56] = p.m4_yy;
    out[57] = p.m4_yz;
    out[58] = p.m4_zx;
    out[59] = p.m4_zy;
    out[60] = p.m4_zz;
    out[61] = p.m4_tx;
    out[62] = p.m4_ty;
    out[63] = p.m4_tz;
    out[64] = p.m5_xx;
    out[65] = p.m5_xy;
    out[66] = p.m5_xz;
    out[67] = p.m5_yx;
    out[68] = p.m5_yy;
    out[69] = p.m5_yz;
    out[70] = p.m5_zx;
    out[71] = p.m5_zy;
    out[72] = p.m5_zz;
    out[73] = p.m5_tx;
    out[74] = p.m5_ty;
    out[75] = p.m5_tz;
    out[76] = p.m6_xx;
    out[77] = p.m6_xy;
    out[78] = p.m6_xz;
    out[79] = p.m6_yx;
    out[80] = p.m6_yy;
    out[81] = p.m6_yz;
    out[82] = p.m6_zx;
    out[83] = p.m6_zy;
    out[84] = p.m6_zz;
    out[85] = p.m6_tx;
    out[86] = p.m6_ty;
    out[87] = p.m6_tz;
    out[88] = p.m7_xx;
    out[89] = p.m7_xy;
    out[90] = p.m7_xz;
    out[91] = p.m7_yx;
    out[92] = p.m7_yy;
    out[93] = p.m7_yz;
    out[94] = p.m7_zx;
    out[95] = p.m7_zy;
    out[96] = p.m7_zz;
    out[97] = p.m7_tx;
    out[98] = p.m7_ty;
    out[99] = p.m7_tz;
    out[100] = p.m8_xx;
    out[101] = p.m8_xy;
    out[102] = p.m8_xz;
    out[103] = p.m8_yx;
    out[104] = p.m8_yy;
    out[105] = p.m8_yz;
    out[106] = p.m8_zx;
    out[107] = p.m8_zy;
    out[108] = p.m8_zz;
    out[109] = p.m8_tx;
    out[110] = p.m8_ty;
    out[111] = p.m8_tz;
    out[112] = p.m9_xx;
    out[113] = p.m9_xy;
    out[114] = p.m9_xz;
    out[115] = p.m9_yx;
    out[116] = p.m9_yy;
    out[117] = p.m9_yz;
    out[118] = p.m9_zx;
    out[119] = p.m9_zy;
    out[120] = p.m9_zz;
    out[121] = p.m9_tx;
    out[122] = p.m9_ty;
    out[123] = p.m9_tz;
    out[124] = p.m10_xx;
    out[125] = p.m10_xy;
    out[126] = p.m10_xz;
    out[127] = p.m10_yx;
    out[128] = p.m10_yy;
    out[129] = p.m10_yz;
    out[130] = p.m10_zx;
    out[131] = p.m10_zy;
    out[132] = p.m10_zz;
    out[133] = p.m10_tx;
    out[134] = p.m10_ty;
    out[135] = p.m10_tz;
    out[136] = p.m11_xx;
    out[137] = p.m11_xy;
    out[138] = p.m11_xz;
    out[139] = p.m11_yx;
    out[140] = p.m11_yy;
    out[141] = p.m11_yz;
    out[142] = p.m11_zx;
    out[143] = p.m11_zy;
    out[144] = p.m11_zz;
    out[145] = p.m11_tx;
    out[146] = p.m11_ty;
    out[147] = p.m11_tz;
    out[148] = to_float(p.anchored);
    out[149] = p.anchor_x;
    out[150] = p.anchor_y;
    out[151] = p.anchor_z;
    out[152] = p.thickness;
    out[153] = to_float(p.soft);
    out[154] = p.offset_x;
    out[155] = p.offset_y;
    out[156] = p.offset_z;
    #{ slots: out }
}

fn lsp3_point(p, idx, iters, n, anchor) {
    let v = anchor;
    let rem = idx;
    for j in 0..to_int(iters) {
        let d = rem % n;
        rem = rem / n;
        let base = u32(4) + d * u32(12);
        let m00 = p.slots[to_int(base)];
        let m01 = p.slots[to_int(base + u32(1))];
        let m02 = p.slots[to_int(base + u32(2))];
        let m10 = p.slots[to_int(base + u32(3))];
        let m11 = p.slots[to_int(base + u32(4))];
        let m12 = p.slots[to_int(base + u32(5))];
        let m20 = p.slots[to_int(base + u32(6))];
        let m21 = p.slots[to_int(base + u32(7))];
        let m22 = p.slots[to_int(base + u32(8))];
        let tx = p.slots[to_int(base + u32(9))];
        let ty = p.slots[to_int(base + u32(10))];
        let tz = p.slots[to_int(base + u32(11))];
        v = vec3(m00 * v.x + m01 * v.y + m02 * v.z + tx, m10 * v.x + m11 * v.y + m12 * v.z + ty, m20 * v.x + m21 * v.y + m22 * v.z + tz);
    }
    v
}

fn variation(q, p, ctx) {
    let color = ctx.color;
    let iters = clamp(u32(to_float(p.iterations)), u32(1), u32(12));
    let n = clamp(u32(to_float(p.map_count)), u32(2), u32(12));
    let connect = to_float(p.connect) > 0.5;
    let dc = to_float(p.dc) > 0.5;
    let anchored = to_float(p.anchored) > 0.5;
    let total = u32(1);
    for j in 0..to_int(iters) {
        total = min(total * n, u32(16000000));
    }
    let t = rand();
    let thickness = p.thickness;
    let thick = thickness > 0.0;
    let seg_a = vec3(0.0, 0.0, 0.0);
    let seg_b = vec3(0.0, 0.0, 0.0);
    let frac = 0.0;
    let dir_in = vec3(0.0, 0.0, 0.0);
    let dir_out = vec3(0.0, 0.0, 0.0);
    if (anchored) {
        let anchor = vec3(p.anchor_x, p.anchor_y, p.anchor_z);
        let segs = max(total - u32(1), u32(1));
        let ts = t * to_float(segs);
        let idx = min(u32(ts), segs - u32(1));
        seg_a = lsp3_point(p, idx, iters, n, anchor);
        seg_b = lsp3_point(p, idx + u32(1), iters, n, anchor);
        frac = clamp(ts - to_float(idx), 0.0, 1.0);
        if (thick && idx > u32(0)) {
            dir_in = seg_a - lsp3_point(p, idx - u32(1), iters, n, anchor);
        }
        if (thick && idx + u32(2) <= segs) {
            dir_out = lsp3_point(p, idx + u32(2), iters, n, anchor) - seg_b;
        }
    } else {
        let idx = min(u32(t * to_float(total)), total - u32(1));
        seg_a = lsp3_point(p, idx, iters, n, vec3(0.0, 0.0, 0.0));
        seg_b = lsp3_point(p, idx, iters, n, vec3(1.0, 0.0, 0.0));
        frac = clamp(t * to_float(total) - to_float(idx), 0.0, 1.0);
        if (thick && idx > u32(0)) {
            let pa = lsp3_point(p, idx - u32(1), iters, n, vec3(0.0, 0.0, 0.0));
            let pb = lsp3_point(p, idx - u32(1), iters, n, vec3(1.0, 0.0, 0.0));
            if (length(pb - seg_a) < thickness) {
                dir_in = pb - pa;
            }
        }
        if (thick && idx + u32(1) < total) {
            let na = lsp3_point(p, idx + u32(1), iters, n, vec3(0.0, 0.0, 0.0));
            let nb = lsp3_point(p, idx + u32(1), iters, n, vec3(1.0, 0.0, 0.0));
            if (length(na - seg_b) < thickness) {
                dir_out = nb - na;
            }
        }
    }
    let out = seg_a;
    if (connect) {
        out = mix(seg_a, seg_b, frac);
    }
    if (thick) {
        let soft = to_float(p.soft) > 0.5;
        let dseg = seg_b - seg_a;
        let dlen = length(dseg);
        let has_seg = connect && dlen > 1.0e-9;
        let r = thickness;
        let rad = 1.0;
        if (soft) {
            rad = 1.0 + (rand() + rand() + rand() + rand() - 2.0) * 0.35;
        }
        let ang = rand() * 6.28318530718;
        if (soft || !has_seg) {
            let zc = rand() * 2.0 - 1.0;
            let rc = sqrt(max(0.0, 1.0 - zc * zc));
            out = out + vec3(rc * cos(ang), rc * sin(ang), zc) * (r * rad);
        } else {
            let v = dseg / dlen;
            let ref_axis = vec3(0.0, 0.0, 1.0);
            if (abs(v.z) > 0.9) {
                ref_axis = vec3(1.0, 0.0, 0.0);
            }
            let ub = normalize(cross(v, ref_axis));
            let vb = cross(v, ub);
            let n0 = vec3(0.0, 0.0, 0.0);
            let phi0 = 0.0;
            let li = length(dir_in);
            if (li > 1.0e-9) {
                let u = dir_in / li;
                phi0 = acos(clamp(dot(u, v), -1.0, 1.0));
                if (phi0 > 1.0e-4 && phi0 < 3.1) {
                    n0 = normalize(u + v);
                }
            }
            let n1 = vec3(0.0, 0.0, 0.0);
            let phi1 = 0.0;
            let lo = length(dir_out);
            if (lo > 1.0e-9) {
                let z = dir_out / lo;
                phi1 = acos(clamp(dot(v, z), -1.0, 1.0));
                if (phi1 > 1.0e-4 && phi1 < 3.1) {
                    n1 = normalize(v + z);
                }
            }
            let cut0 = r * min(8.0, tan(0.5 * phi0));
            let cut1 = r * min(8.0, tan(0.5 * phi1));
            let shaft = max(0.0, 3.14159265359 * dlen - cut0 - cut1);
            let lune = phi0 * r;
            if (rand() * (shaft + lune) < lune) {
                let u = dir_in / li;
                let w = normalize(cross(u, v));
                let e2 = cross(w, u);
                let zc = rand() * 2.0 - 1.0;
                let rc = sqrt(max(0.0, 1.0 - zc * zc));
                let beta = -1.5707963268 + phi0 * rand();
                let dir = w * zc + (u * cos(beta) + e2 * sin(beta)) * rc;
                out = seg_a + dir * (r * rad);
            } else {
                let s = 0.0;
                let off = vec3(0.0, 0.0, 0.0);
                for k in 0..8 {
                    s = rand() * dlen;
                    let a2 = rand() * 6.28318530718;
                    off = (ub * cos(a2) + vb * sin(a2)) * (r * rad);
                    let rel = v * s + off;
                    let in0 = dot(n0, n0) < 0.5 || dot(rel, n0) >= 0.0;
                    let in1 = dot(n1, n1) < 0.5 || dot(rel - dseg, n1) <= 0.0;
                    if (in0 && in1) {
                        break;
                    }
                }
                out = seg_a + v * s + off;
            }
        }
    }
    out = out + vec3(p.offset_x, p.offset_y, p.offset_z);
    if (dc) {
        color = t;
    }
    #{ z: complex(out.x, out.y), color: color }
}

fn variation3(q, p, ctx) {
    let color = ctx.color;
    let iters = clamp(u32(to_float(p.iterations)), u32(1), u32(12));
    let n = clamp(u32(to_float(p.map_count)), u32(2), u32(12));
    let connect = to_float(p.connect) > 0.5;
    let dc = to_float(p.dc) > 0.5;
    let anchored = to_float(p.anchored) > 0.5;
    let total = u32(1);
    for j in 0..to_int(iters) {
        total = min(total * n, u32(16000000));
    }
    let t = rand();
    let thickness = p.thickness;
    let thick = thickness > 0.0;
    let seg_a = vec3(0.0, 0.0, 0.0);
    let seg_b = vec3(0.0, 0.0, 0.0);
    let frac = 0.0;
    let dir_in = vec3(0.0, 0.0, 0.0);
    let dir_out = vec3(0.0, 0.0, 0.0);
    if (anchored) {
        let anchor = vec3(p.anchor_x, p.anchor_y, p.anchor_z);
        let segs = max(total - u32(1), u32(1));
        let ts = t * to_float(segs);
        let idx = min(u32(ts), segs - u32(1));
        seg_a = lsp3_point(p, idx, iters, n, anchor);
        seg_b = lsp3_point(p, idx + u32(1), iters, n, anchor);
        frac = clamp(ts - to_float(idx), 0.0, 1.0);
        if (thick && idx > u32(0)) {
            dir_in = seg_a - lsp3_point(p, idx - u32(1), iters, n, anchor);
        }
        if (thick && idx + u32(2) <= segs) {
            dir_out = lsp3_point(p, idx + u32(2), iters, n, anchor) - seg_b;
        }
    } else {
        let idx = min(u32(t * to_float(total)), total - u32(1));
        seg_a = lsp3_point(p, idx, iters, n, vec3(0.0, 0.0, 0.0));
        seg_b = lsp3_point(p, idx, iters, n, vec3(1.0, 0.0, 0.0));
        frac = clamp(t * to_float(total) - to_float(idx), 0.0, 1.0);
        if (thick && idx > u32(0)) {
            let pa = lsp3_point(p, idx - u32(1), iters, n, vec3(0.0, 0.0, 0.0));
            let pb = lsp3_point(p, idx - u32(1), iters, n, vec3(1.0, 0.0, 0.0));
            if (length(pb - seg_a) < thickness) {
                dir_in = pb - pa;
            }
        }
        if (thick && idx + u32(1) < total) {
            let na = lsp3_point(p, idx + u32(1), iters, n, vec3(0.0, 0.0, 0.0));
            let nb = lsp3_point(p, idx + u32(1), iters, n, vec3(1.0, 0.0, 0.0));
            if (length(na - seg_b) < thickness) {
                dir_out = nb - na;
            }
        }
    }
    let out = seg_a;
    if (connect) {
        out = mix(seg_a, seg_b, frac);
    }
    if (thick) {
        let soft = to_float(p.soft) > 0.5;
        let dseg = seg_b - seg_a;
        let dlen = length(dseg);
        let has_seg = connect && dlen > 1.0e-9;
        let r = thickness;
        let rad = 1.0;
        if (soft) {
            rad = 1.0 + (rand() + rand() + rand() + rand() - 2.0) * 0.35;
        }
        let ang = rand() * 6.28318530718;
        if (soft || !has_seg) {
            let zc = rand() * 2.0 - 1.0;
            let rc = sqrt(max(0.0, 1.0 - zc * zc));
            out = out + vec3(rc * cos(ang), rc * sin(ang), zc) * (r * rad);
        } else {
            let v = dseg / dlen;
            let ref_axis = vec3(0.0, 0.0, 1.0);
            if (abs(v.z) > 0.9) {
                ref_axis = vec3(1.0, 0.0, 0.0);
            }
            let ub = normalize(cross(v, ref_axis));
            let vb = cross(v, ub);
            let n0 = vec3(0.0, 0.0, 0.0);
            let phi0 = 0.0;
            let li = length(dir_in);
            if (li > 1.0e-9) {
                let u = dir_in / li;
                phi0 = acos(clamp(dot(u, v), -1.0, 1.0));
                if (phi0 > 1.0e-4 && phi0 < 3.1) {
                    n0 = normalize(u + v);
                }
            }
            let n1 = vec3(0.0, 0.0, 0.0);
            let phi1 = 0.0;
            let lo = length(dir_out);
            if (lo > 1.0e-9) {
                let z = dir_out / lo;
                phi1 = acos(clamp(dot(v, z), -1.0, 1.0));
                if (phi1 > 1.0e-4 && phi1 < 3.1) {
                    n1 = normalize(v + z);
                }
            }
            let cut0 = r * min(8.0, tan(0.5 * phi0));
            let cut1 = r * min(8.0, tan(0.5 * phi1));
            let shaft = max(0.0, 3.14159265359 * dlen - cut0 - cut1);
            let lune = phi0 * r;
            if (rand() * (shaft + lune) < lune) {
                let u = dir_in / li;
                let w = normalize(cross(u, v));
                let e2 = cross(w, u);
                let zc = rand() * 2.0 - 1.0;
                let rc = sqrt(max(0.0, 1.0 - zc * zc));
                let beta = -1.5707963268 + phi0 * rand();
                let dir = w * zc + (u * cos(beta) + e2 * sin(beta)) * rc;
                out = seg_a + dir * (r * rad);
            } else {
                let s = 0.0;
                let off = vec3(0.0, 0.0, 0.0);
                for k in 0..8 {
                    s = rand() * dlen;
                    let a2 = rand() * 6.28318530718;
                    off = (ub * cos(a2) + vb * sin(a2)) * (r * rad);
                    let rel = v * s + off;
                    let in0 = dot(n0, n0) < 0.5 || dot(rel, n0) >= 0.0;
                    let in1 = dot(n1, n1) < 0.5 || dot(rel - dseg, n1) <= 0.0;
                    if (in0 && in1) {
                        break;
                    }
                }
                out = seg_a + v * s + off;
            }
        }
    }
    out = out + vec3(p.offset_x, p.offset_y, p.offset_z);
    if (dc) {
        color = t;
    }
    #{ z: out, color: color }
}
