// A flame variation written in Rhai: a whorl that turns each point by an
// angle growing with its distance from the centre, inner and outer
// rings turning by different amounts.
//
// Use it from a flame mode: "Use as variation" puts it in every
// transform's variation list under its name below, and the flame keeps
// its source, so a saved flame renders anywhere.

const PARAMS = #{
    inside: #{ value: 1.2, min: -6.0, max: 6.0, caption: "Inside", hint: "How far points inside the unit circle turn, per unit of radius." },
    outside: #{ value: -0.6, min: -6.0, max: 6.0, caption: "Outside", hint: "How far points outside it turn." },
};

const INFO = #{ title: "Whorl (Rhai)", name: "rhai_whorl" };

fn variation(z, p) {
    let r = z.abs();
    let turn = if r < 1.0 { p.inside } else { p.outside };
    let a = z.arg() + turn * r;
    complex(r * cos(a), r * sin(a))
}
