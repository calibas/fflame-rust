// Trails: what moves leaves an afterimage that fades -- the picture's
// light remembered from frame to frame, as a long exposure or a slow
// phosphor would.
//
// A built-in effect written in Rhai, with no WGSL behind it
// (docs/main/EFFECTS.md, Rhai effects): the app runs this file -- on
// desktop the copy beside the app, so an edit shows on the next start.
// A temporal effect (docs/projects/multipass-effects.md): its trail is a
// persistent buffer, carried on while the animation plays (in a video,
// frame by frame), held while it is paused, and empty in a still -- a
// still shows the picture as it is.

const PARAMS = #{
    persistence: #{
        value: 0.85,
        min: 0.0,
        max: 0.95,
        caption: "Persistence",
        hint: "How much of the trail a thirtieth of a second keeps: 0.85 fades it to a tenth in half a second, 0.95 in a second and a half.",
    },
    intensity: #{ value: 1.0, min: 0.0, max: 1.0, caption: "Intensity" },
};
// At most 0.95 a thirtieth of a second, a trail is below an 8-bit step
// within 240 frames at 60 frames a second: a frame rendered alone warms
// up over that many.
const INFO = #{
    title: "Trails",
    description: "What moves leaves an afterimage that fades -- the picture's light remembered from frame to frame, as a long exposure or a slow phosphor would.",
    id: "trails",
    buffers: #{ trail: #{ persist: true } },
    passes: [#{ run: "keep", to: "trail" }, #{ run: "effect" }],
    warmup: 240,
};

// The trail: the last frame's, faded by the time since it, or the
// picture where the picture is brighter.
fn keep(c, p, ctx) {
    let fade = pow(p.persistence, ctx.dt * 30.0);
    let before = buffer("trail", ctx.uv);
    rgba(max(c.r, before.r * fade), max(c.g, before.g * fade), max(c.b, before.b * fade), 1.0)
}

fn effect(c, p, ctx) {
    let trail = buffer("trail", ctx.uv);
    rgba(c.rgb + (trail.rgb - c.rgb) * p.intensity, c.a)
}
