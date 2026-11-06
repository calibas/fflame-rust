// PRose 3D, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    l: #{
        value: 1.0, min: 0.0, max: 3.0, free: true,
        caption: "Length",
        hint: "Rose length / overall scale (`length`). 0 is clamped to a tiny value.",
    },
    k: #{
        value: 6.0, min: -15.0, max: 15.0, free: true,
        caption: "Petals",
        hint: "Petal count `k` (the polar-rose frequency); may be fractional. Sign flips petal parity.",
    },
    c: #{
        value: 0.0, min: -3.1415927, max: 3.1415927, free: true,
        caption: "Constant",
        hint: "Angular phase constant added inside `cos(k·θ + c)`.",
    },
    z1: #{
        value: 1.0, min: -2.0, max: 2.0, free: true,
        caption: "Scale Z1",
        hint: "Z scale of the main (non-reflected) petals.",
    },
    z2: #{
        value: 1.0, min: -2.0, max: 2.0, free: true,
        caption: "Scale Z2",
        hint: "Z scale of the reflected (ghost) petals.",
    },
    refSc: #{
        value: 1.0, min: 0.0, max: 2.0, free: true,
        caption: "Reflection Scale",
        hint: "XY scale applied to reflected points.",
    },
    opt: #{
        value: 1.0, min: -2.0, max: 2.0, free: true,
        caption: "Opt",
        hint: "Waggle blend selector (`smooth12 = |opt|`, capped at 2). Its sign sets the Z direction; `opt > 0` also disables reflections.",
    },
    optSc: #{
        value: 1.0, min: 0.0, max: 2.0, free: true,
        caption: "Opt Scale",
        hint: "Frequency scale on the curve `sin()` terms (`opScale = |optSc|`).",
    },
    opt3: #{
        value: 0.0, min: -1.0, max: 1.0, free: true,
        caption: "Opt3",
        hint: "Blends in the `wag3` waggle term (`smooth3 = |opt3|`, capped at 1). Negative forces the down direction.",
    },
    transp: #{
        value: 0.5, min: 0.0, max: 1.0, free: true,
        caption: "Transparency",
        hint: "Reflection probability — a point reflects when a random < `transp²` (and `opt ≤ 0`).",
    },
    dist: #{
        value: 1.0, min: 0.0, max: 3.0, free: true,
        caption: "Distance",
        hint: "Constant Z offset added to every point.",
    },
    wagsc: #{
        value: 0.0, min: -2.5, max: 2.5, free: true,
        caption: "Wag Scale",
        hint: "Radial weight of the waggle terms.",
    },
    srvsc: #{
        value: 0.0, min: 0.0, max: 3.0, free: true,
        caption: "Curve Scale",
        hint: "Weight of the curve3/curve2 cosine/sine terms in the waggle (JWF param name `srvsc`).",
    },
    f: #{
        value: 3.0, min: 0.0, max: 6.0, free: true,
        caption: "Frequency",
        hint: "Perimeter ripple frequency (`frequency = f·2π`).",
    },
    wigsc: #{
        value: 0.0, min: -2.5, max: 2.5, free: true,
        caption: "Wig Scale",
        hint: "Amplitude of the radial `sin(wig)` Z ripple.",
    },
    offset: #{
        value: 0.0, min: -2.0, max: 2.0, free: true,
        caption: "Offset",
        hint: "Phase offset added to the ripple argument (`+ offset·cycle`).",
    },
};

const INFO = #{ title: "PRose 3D", name: "rhai_prose3d" };

fn variation(z, p) {
    let l = p.l;
    let k = p.k;
    let c = p.c;
    let refSc = p.refSc;
    let opt = p.opt;
    let opt3 = p.opt3;
    let transp = p.transp;
    let length = l;
    if (length == 0.0) {
        length = 0.000001;
    }
    let petalsSign = (if k < 0.0 { -1.0 } else { 1.0 });
    let numPetals = k;
    if (abs(numPetals) < 0.0001) {
        numPetals = 0.0001 * petalsSign;
    }
    let th = atan2(z.im, z.re);
    let sth = sin(th);
    let cth = cos(th);
    let optDir = (if opt < 0.0 { -1.0 } else { 1.0 });
    if (opt3 < 0.0) {
        optDir = -1.0;
    }
    let ghostPrep = rand();
    let posNeg_neg = rand() < 0.5;
    let ghost = transp * transp;
    if (optDir > 0.0) {
        ghost = 0.0;
    }
    let rose = length * cos(numPetals * th + c);
    if (ghostPrep < ghost && posNeg_neg) {
        return complex(0.5 * refSc * rose * cth, 0.5 * refSc * rose * sth);
    }
    complex(0.5 * rose * cth, 0.5 * rose * sth)
}

fn variation3(v, p) {
    let PI = 3.14159265359;
    let M_2PI = 6.28318530718;
    let l = p.l;
    let k = p.k;
    let c = p.c;
    let z1 = p.z1;
    let z2 = p.z2;
    let refSc = p.refSc;
    let opt = p.opt;
    let optSc = p.optSc;
    let opt3 = p.opt3;
    let transp = p.transp;
    let dist = p.dist;
    let wagsc = p.wagsc;
    let srvsc = p.srvsc;
    let f = p.f;
    let wigsc = p.wigsc;
    let offset = p.offset;
    let length = l;
    if (length == 0.0) {
        length = 0.000001;
    }
    let petalsSign = (if k < 0.0 { -1.0 } else { 1.0 });
    let numPetals = k;
    if (abs(numPetals) < 0.0001) {
        numPetals = 0.0001 * petalsSign;
    }
    let rad = sqrt(v.x * v.x + v.y * v.y);
    let curve1 = rad / length;
    let curve2 = (rad / length) * (rad / length);
    let curve3 = (rad - length * 0.5) / length;
    let curve4 = curve2 * curve2;
    let smooth12 = abs(opt);
    if (smooth12 > 2.0) {
        smooth12 = 2.0;
    }
    let smooth3 = abs(opt3);
    if (smooth3 > 1.0) {
        smooth3 = 1.0;
    }
    let opScale = abs(optSc);
    let antiOpt1 = 2.0 - smooth12;
    let wagScale = wagsc;
    let curveScale = srvsc;
    let frequency = f * M_2PI;
    let wigScale = wigsc * 0.5;
    let th = atan2(v.y, v.x);
    let sth = sin(th);
    let cth = cos(th);
    let optDir = (if opt < 0.0 { -1.0 } else { 1.0 });
    if (opt3 < 0.0) {
        optDir = -1.0;
    }
    let ghostPrep = rand();
    let posNeg_neg = rand() < 0.5;
    let kc = (if k == 0.0 { k + 0.000001 } else { k });
    let cycle = M_2PI / kc;
    let pang = th / cycle;
    let wig = pang * frequency * 0.5 + offset * cycle;
    let wag = 0.0;
    let wag3 = 0.0;
    if (optDir < 0.0) {
        wag = sin(curve1 * PI() * opScale) + wagScale * 0.4 * rad + curveScale * 0.5 * sin(curve2 * PI());
        wag3 = sin(curve4 * PI() * opScale) + wagScale * rad * rad * 0.4 + curveScale * 0.5 * cos(curve3 * PI());
    } else {
        wag = sin(curve1 * PI() * opScale) + wagScale * 0.4 * rad + curveScale * 0.5 * cos(curve3 * PI());
        wag3 = sin(curve4 * PI() * opScale) + wagScale * rad * rad * 0.4 + curveScale * 0.5 * sin(curve2 * PI());
    }
    let wag2 = sin(curve2 * PI() * opScale) + wagScale * 0.4 * rad + curveScale * 0.5 * cos(curve3 * PI());
    let wag12 = wag;
    if (smooth12 > 1.0) {
        wag12 = wag2 * (1.0 - antiOpt1) + wag * antiOpt1;
    }
    let waggle = wag12;
    if (smooth3 > 0.0) {
        waggle = wag12 * (1.0 - smooth3) + wag3 * smooth3;
    }
    let ghost = transp * transp;
    if (optDir > 0.0) {
        ghost = 0.0;
    }
    let rose = length * cos(numPetals * th + c);
    let zwig = (rad * 0.5) * (rad * 0.5) * sin(wig) * wigScale;
    if (ghostPrep < ghost && posNeg_neg) {
        return vec3(0.5 * refSc * rose * cth, 0.5 * refSc * rose * sth, -0.5 * ((z2 * waggle + zwig) + dist));
    }
    vec3(0.5 * rose * cth, 0.5 * rose * sth, 0.5 * ((z1 * waggle + zwig) + dist))
}
