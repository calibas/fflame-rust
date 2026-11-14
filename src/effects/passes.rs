//! A multi-pass effect's passes and buffers (docs/projects/multipass-effects.md).
//!
//! An effect without `INFO.passes` is one pass, `fn effect`, as every
//! effect was. One with them runs each pass in order -- each its own
//! fragment entry point in the effect's one module -- each writing a
//! buffer or, the last, the chain's output; a pass may repeat. Buffers are
//! `Rgba32Float`: plain ones a ping-pong pair (a pass that reads the buffer
//! it writes reads the previous version), pyramids one texture with mips,
//! persistent ones kept from one frame to the next.
//!
//! The effect chain (`renderer::effect_chain`) runs what this describes;
//! the Rhai compiler (`effects::custom`) and the WGSL `distance_field`
//! describe themselves with it.

use serde_json::Value;

/// Buffers an effect may declare: three bindings (the image, its sampler,
/// the uniform) and these, under the 16 sampled textures a fragment stage
/// allows on every platform.
pub const MAX_BUFFERS: usize = 8;
/// Passes an effect may declare (`lang::wgsl::PASS_PREFIXES`).
pub const MAX_PASSES: usize = 16;
/// A pyramid's levels at most.
pub const MAX_LEVELS: u32 = 12;
/// A fixed repeat at most.
pub const MAX_REPEAT: u32 = 64;
/// Frames a temporal effect may ask to be warmed up over.
pub const MAX_WARMUP: u32 = 600;

/// A buffer's declaration.
#[derive(Clone, Debug, PartialEq)]
pub struct BufferDecl {
    pub name: String,
    /// Its size against the frame's (each side, at least one texel).
    pub scale: f32,
    /// 1 for a plain buffer; more for a pyramid, level `i` half of `i - 1`.
    pub levels: u32,
    /// Kept from one frame to the next.
    pub persist: bool,
}

impl BufferDecl {
    pub fn pyramid(&self) -> bool {
        self.levels > 1
    }

    /// Its size for a frame of `w` x `h`: the frame's by `scale`, at least
    /// a texel each way.
    pub fn size(&self, w: u32, h: u32) -> (u32, u32) {
        let side = |n: u32| ((n as f32 * self.scale).round() as u32).max(1);
        (side(w), side(h))
    }

    /// Its levels for a frame of `w` x `h`: as declared, no more than the
    /// size halves to one texel.
    pub fn levels_at(&self, w: u32, h: u32) -> u32 {
        let (bw, bh) = self.size(w, h);
        let most = 32 - bw.max(bh).leading_zeros();
        self.levels.min(most).max(1)
    }
}

/// How many times a pass runs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Repeat {
    Times(u32),
    /// The covering power of two of the frame's larger side: the jump
    /// flood's steps n/2, n/4, ..., 1.
    Log2,
    /// Once a level of its pyramid target, in `Order`.
    Levels,
}

/// The order a [`Repeat::Levels`] pass writes its pyramid's levels in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Order {
    /// Finest first, each level reading its target one level finer.
    Down,
    /// Coarsest first, each reading its target one level coarser.
    Up,
}

/// A pass's declaration.
#[derive(Clone, Debug, PartialEq)]
pub struct PassDecl {
    /// The function it runs, as the source names it.
    pub function: String,
    /// The module's fragment entry point for it.
    pub entry: String,
    /// The buffer it writes, by index; `None` the chain's output (the last
    /// pass, and only it).
    pub to: Option<usize>,
    pub repeat: Repeat,
    pub order: Order,
}

impl PassDecl {
    /// Its runs for a frame of `w` x `h` whose buffers are `buffers`.
    pub fn runs(&self, buffers: &[BufferDecl], w: u32, h: u32) -> u32 {
        match self.repeat {
            Repeat::Times(n) => n,
            Repeat::Log2 => log2_passes(w, h),
            Repeat::Levels => self.to.map_or(1, |t| buffers[t].levels_at(w, h)),
        }
    }
}

/// The jump flood's pass count for a frame: steps n/2, ..., 1 with n the
/// covering power of two of its larger side.
pub fn log2_passes(w: u32, h: u32) -> u32 {
    let max_dim = w.max(h).max(2);
    32 - (max_dim - 1).leading_zeros()
}

/// A multi-pass effect's whole plan.
#[derive(Clone, Debug, PartialEq)]
pub struct Passes {
    pub buffers: Vec<BufferDecl>,
    pub passes: Vec<PassDecl>,
    /// The frames a temporal effect forgets past (`INFO.warmup`): a frame
    /// rendered alone in a sequence warms up from `n - warmup`; `None`
    /// from frame 0.
    pub warmup: Option<u32>,
}

impl Passes {
    /// Every pass's runs for a frame of `w` x `h`: the parameter slots
    /// the effect takes in it.
    pub fn runs(&self, w: u32, h: u32) -> usize {
        self.passes.iter().map(|p| p.runs(&self.buffers, w, h) as usize).sum()
    }

    /// Whether any buffer outlives the frame.
    pub fn temporal(&self) -> bool {
        self.buffers.iter().any(|b| b.persist)
    }

    /// A buffer's index by its name.
    pub fn buffer(&self, name: &str) -> Option<usize> {
        self.buffers.iter().position(|b| b.name == name)
    }
}

/// The plan an effect's `INFO` declares (`passes`, `buffers`, `warmup`,
/// read by `lang::rhai::info_json`), or `None` for a one-pass effect.
/// Buffers take their indices in their names' sorted order
/// (`lang::rhai::effect_buffer_names`, which the compiler's reads use).
pub fn from_info(passes: Option<&Value>, buffers: Option<&Value>, warmup: Option<&Value>) -> Result<Option<Passes>, String> {
    let Some(passes) = passes else {
        if buffers.is_some() {
            return Err("`INFO.buffers` without `INFO.passes`: a buffer is written by a pass".into());
        }
        if warmup.is_some() {
            return Err("`INFO.warmup` is a multi-pass effect's, one with a persistent buffer".into());
        }
        return Ok(None);
    };
    let mut decls = Vec::new();
    match buffers {
        None => {}
        Some(Value::Object(m)) => {
            let mut names: Vec<&String> = m.keys().collect();
            names.sort();
            for name in names {
                decls.push(buffer_decl(name, &m[name])?);
            }
        }
        Some(_) => return Err("`INFO.buffers` is a map of name to `#{ scale, levels, persist }`".into()),
    }
    if decls.len() > MAX_BUFFERS {
        return Err(format!("{} buffers: an effect has at most {MAX_BUFFERS}", decls.len()));
    }
    let Value::Array(list) = passes else {
        return Err("`INFO.passes` is a list of `#{ run, to, repeat }`".into());
    };
    if list.is_empty() || list.len() > MAX_PASSES {
        return Err(format!("an effect has 1 to {MAX_PASSES} passes, not {}", list.len()));
    }
    let mut out = Vec::new();
    for (k, p) in list.iter().enumerate() {
        out.push(pass_decl(k, p, &decls)?);
    }
    let last = out.last().expect("not empty");
    if last.function != "effect" || last.to.is_some() {
        return Err("the last pass is `effect`, writing the effect's output (no `to`)".into());
    }
    if let Some(p) = out[..out.len() - 1].iter().find(|p| p.to.is_none()) {
        return Err(format!("the pass `{}` writes nothing: each pass but the last has `to`, a buffer", p.function));
    }
    if out.last().is_some_and(|p| p.repeat != Repeat::Times(1)) {
        return Err("the last pass, `effect`, runs once".into());
    }
    // A persistent buffer is written once a frame: what the next frame
    // reads is that one write.
    for (b, d) in decls.iter().enumerate() {
        let writers: Vec<&PassDecl> = out.iter().filter(|p| p.to == Some(b)).collect();
        if d.persist && (writers.len() > 1 || writers.iter().any(|p| p.repeat != Repeat::Times(1))) {
            return Err(format!("the persistent buffer `{}` is written by one pass, once a frame", d.name));
        }
        if writers.is_empty() {
            return Err(format!("no pass writes the buffer `{}`", d.name));
        }
    }
    let warmup = match warmup {
        None => None,
        Some(Value::Number(n)) => match n.as_u64() {
            Some(k) if k <= MAX_WARMUP as u64 => Some(k as u32),
            _ => return Err(format!("`warmup` is a whole number of frames, 0 to {MAX_WARMUP}")),
        },
        Some(_) => return Err(format!("`warmup` is a whole number of frames, 0 to {MAX_WARMUP}")),
    };
    if warmup.is_some() && !decls.iter().any(|d| d.persist) {
        return Err("`warmup` is for an effect with a persistent buffer: one without remembers no frames".into());
    }
    Ok(Some(Passes { buffers: decls, passes: out, warmup }))
}

fn buffer_decl(name: &str, v: &Value) -> Result<BufferDecl, String> {
    let Value::Object(m) = v else {
        return Err(format!("the buffer `{name}` is `#{{ scale, levels, persist }}` (each optional: `#{{}}`)"));
    };
    let mut d = BufferDecl { name: name.to_string(), scale: 1.0, levels: 1, persist: false };
    for (k, x) in m {
        match (k.as_str(), x) {
            ("scale", Value::Number(n)) => match n.as_f64() {
                Some(s) if s > 0.0 && s <= 4.0 => d.scale = s as f32,
                _ => return Err(format!("the buffer `{name}`'s `scale` is more than 0, at most 4")),
            },
            ("levels", Value::Number(n)) => match n.as_u64() {
                Some(l) if (1..=MAX_LEVELS as u64).contains(&l) => d.levels = l as u32,
                _ => return Err(format!("the buffer `{name}`'s `levels` is 1 to {MAX_LEVELS}")),
            },
            ("persist", Value::Bool(b)) => d.persist = *b,
            (k, _) => return Err(format!("the buffer `{name}` has `scale` (a number), `levels` (a whole number) and `persist` (a bool), not `{k}` as written")),
        }
    }
    if d.persist && d.pyramid() {
        return Err(format!("the buffer `{name}` persists or is a pyramid, not both"));
    }
    Ok(d)
}

fn pass_decl(k: usize, v: &Value, buffers: &[BufferDecl]) -> Result<PassDecl, String> {
    let Value::Object(m) = v else {
        return Err(format!("pass {} is `#{{ run: \"name\", to: \"buffer\" }}`", k + 1));
    };
    let mut function = None;
    let mut to = None;
    let mut repeat = Repeat::Times(1);
    let mut order = None;
    for (key, x) in m {
        match (key.as_str(), x) {
            ("run", Value::String(s)) => function = Some(s.clone()),
            ("to", Value::String(s)) => match buffers.iter().position(|b| &b.name == s) {
                Some(b) => to = Some(b),
                None => return Err(format!("pass {} writes `{s}`, which `INFO.buffers` does not declare", k + 1)),
            },
            ("repeat", Value::Number(n)) => match n.as_u64() {
                Some(r) if (1..=MAX_REPEAT as u64).contains(&r) => repeat = Repeat::Times(r as u32),
                _ => return Err(format!("pass {}'s `repeat` is 1 to {MAX_REPEAT}, \"log2\" or \"levels\"", k + 1)),
            },
            ("repeat", Value::String(s)) if s == "log2" => repeat = Repeat::Log2,
            ("repeat", Value::String(s)) if s == "levels" => repeat = Repeat::Levels,
            ("order", Value::String(s)) if s == "down" => order = Some(Order::Down),
            ("order", Value::String(s)) if s == "up" => order = Some(Order::Up),
            (key, _) => {
                return Err(format!(
                    "pass {} has `run` (a function's name), `to` (a buffer's), `repeat` (1 to {MAX_REPEAT}, \"log2\" or \"levels\") and `order` (\"down\" or \"up\"), not `{key}` as written",
                    k + 1
                ))
            }
        }
    }
    let Some(function) = function else { return Err(format!("pass {} names its function: `run: \"name\"`", k + 1)) };
    if repeat == Repeat::Levels && !to.is_some_and(|t| buffers[t].pyramid()) {
        return Err(format!("the pass `{function}` repeats over levels: it writes a pyramid (a buffer with `levels`)"));
    }
    if order.is_some() && repeat != Repeat::Levels {
        return Err(format!("the pass `{function}` has an `order`, which a pass repeating over levels has"));
    }
    Ok(PassDecl { entry: format!("fs_pass{k}"), function, to, repeat, order: order.unwrap_or(Order::Down) })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn plan(passes: Value, buffers: Value) -> Result<Option<Passes>, String> {
        from_info(Some(&passes), Some(&buffers), None)
    }

    #[test]
    fn a_jump_flood_plan() {
        let p = plan(
            json!([{ "run": "seed", "to": "coords" }, { "run": "flood", "to": "coords", "repeat": "log2" }, { "run": "effect" }]),
            json!({ "coords": {} }),
        )
        .unwrap()
        .unwrap();
        assert_eq!(p.buffers.len(), 1);
        assert_eq!(p.passes[1].repeat, Repeat::Log2);
        assert_eq!(p.passes[1].entry, "fs_pass1");
        // 1080p: seed, 11 floods, composite -- the runner's old count.
        assert_eq!(p.runs(1920, 1080), 13);
    }

    #[test]
    fn a_pyramid_plan() {
        let p = plan(
            json!([
                { "run": "down", "to": "down", "repeat": "levels" },
                { "run": "up", "to": "up", "repeat": "levels", "order": "up" },
                { "run": "effect" },
            ]),
            json!({ "up": { "scale": 0.5, "levels": 6 }, "down": { "scale": 0.5, "levels": 6 } }),
        )
        .unwrap()
        .unwrap();
        // Sorted by name: down 0, up 1.
        assert_eq!(p.buffers[0].name, "down");
        assert_eq!(p.passes[1].to, Some(1));
        assert_eq!(p.passes[1].order, Order::Up);
        assert_eq!(p.buffers[0].size(1920, 1080), (960, 540));
        assert_eq!(p.runs(1920, 1080), 6 + 6 + 1);
        // A small frame holds fewer levels.
        assert_eq!(p.buffers[0].levels_at(8, 8), 3);
    }

    #[test]
    fn what_a_plan_cannot_be() {
        let refused = |passes: Value, buffers: Value, why: &str| {
            let e = plan(passes.clone(), buffers.clone()).expect_err(&format!("{passes} {buffers}"));
            assert!(e.contains(why), "expected `{why}` in: {e}");
        };
        let b = json!({ "a": {} });
        refused(json!([]), b.clone(), "1 to 16 passes");
        refused(json!([{ "run": "x", "to": "a" }]), b.clone(), "last pass is `effect`");
        refused(json!([{ "run": "x" }, { "run": "effect" }]), json!({}), "writes nothing");
        refused(json!([{ "run": "x", "to": "b" }, { "run": "effect" }]), b.clone(), "does not declare");
        refused(json!([{ "run": "effect" }]), b.clone(), "no pass writes the buffer `a`");
        refused(json!([{ "run": "x", "to": "a", "repeat": "levels" }, { "run": "effect" }]), b.clone(), "writes a pyramid");
        refused(json!([{ "run": "x", "to": "a", "order": "up" }, { "run": "effect" }]), b.clone(), "has an `order`");
        refused(json!([{ "run": "x", "to": "a" }, { "run": "effect", "repeat": 2 }]), b.clone(), "runs once");
        refused(json!([{ "run": "x", "to": "a", "repeat": 2 }, { "run": "effect" }]), json!({ "a": { "persist": true } }), "once a frame");
        refused(json!([{ "run": "x", "to": "a" }, { "run": "effect" }]), json!({ "a": { "persist": true, "levels": 2 } }), "not both");
        refused(json!([{ "run": "x", "to": "a" }, { "run": "effect" }]), json!({ "a": { "scale": 9 } }), "at most 4");
        assert!(from_info(None, Some(&b), None).unwrap_err().contains("without `INFO.passes`"));
        assert!(from_info(Some(&json!([{ "run": "x", "to": "a" }, { "run": "effect" }])), Some(&b), Some(&json!(4)))
            .unwrap_err()
            .contains("persistent buffer"));
        assert_eq!(from_info(None, None, None), Ok(None));
    }
}
