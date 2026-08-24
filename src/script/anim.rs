//! Script-defined animation.
//!
//! A script may optionally declare what *varies*, not just what a flame
//! is — a generator that sets a `spiral` parameter can also say how that
//! parameter should move. The result is an [`Animation`] carrying the
//! flame it was built alongside as its `base_config`, so the emitted
//! `.anim` is self-contained and reproducible on its own.
//!
//! Animation is entirely opt-in: a script that never touches `anim`
//! produces `None`, and nothing about the existing scripts changes.
//!
//! **Targets are resolved through [`ConfigPath`], never hand-formatted.**
//! Track targets are `ConfigPath::to_string_key()` strings, and the
//! loader parses them back with `from_string_key`. Building the enum
//! value and asking it for its key means a target this module emits is
//! one the loader accepts by construction — a hand-built
//! `"Transform.0.Weight"` would be a second spelling of the same thing,
//! free to drift.

use serde_json::Value;

use crate::animation::{Animation, EasingFunction, Interpolation, Keyframe, Track, TrackSource};
use crate::config::{AffineParam, ConfigPath, TransformRef};
use crate::config::FractalConfig;

/// One track under construction.
#[derive(Debug, Clone)]
pub(crate) struct TrackDraft {
    /// A `ConfigPath::to_string_key()` string.
    pub target: String,
    pub keys: Vec<Keyframe>,
    pub interpolation: Interpolation,
}

/// Accumulates whatever the script asked to animate.
///
/// Tracks are kept in a `Vec` in first-touched order rather than a map:
/// a `HashMap` would reorder the emitted `.anim` between runs, and this
/// engine's whole determinism story is that a given (script, seed)
/// produces the same bytes every time.
#[derive(Debug, Default, Clone)]
pub(crate) struct AnimBuilder {
    pub name: Option<String>,
    pub duration: Option<f64>,
    pub tracks: Vec<TrackDraft>,
    /// True once the script has touched `anim` in any way, so that
    /// setting only a name or duration still produces an animation
    /// (an empty one is a clearer bug report than silence).
    pub touched: bool,
}

impl AnimBuilder {
    pub fn add_key(&mut self, target: String, time: f64, value: Value, easing: EasingFunction) {
        self.touched = true;
        let key = Keyframe { time, value, easing };
        match self.tracks.iter_mut().find(|t| t.target == target) {
            Some(track) => track.keys.push(key),
            None => self.tracks.push(TrackDraft {
                target,
                keys: vec![key],
                interpolation: Interpolation::default(),
            }),
        }
    }

    /// Set a track's interpolation. The track need not exist yet — a
    /// script that reads top-down will often name the curve before
    /// laying down its keys.
    pub fn set_interpolation(&mut self, target: String, interpolation: Interpolation) {
        self.touched = true;
        match self.tracks.iter_mut().find(|t| t.target == target) {
            Some(track) => track.interpolation = interpolation,
            None => self.tracks.push(TrackDraft {
                target,
                keys: Vec::new(),
                interpolation,
            }),
        }
    }

    /// Assemble the animation, or `None` if the script never asked for
    /// one. `warnings` collects anything worth telling the author.
    pub fn build(
        &self,
        config: &FractalConfig,
        script_name: &str,
        warnings: &mut Vec<String>,
    ) -> Option<Animation> {
        if !self.touched {
            return None;
        }

        let latest = self
            .tracks
            .iter()
            .flat_map(|t| t.keys.iter().map(|k| k.time))
            .fold(0.0_f64, f64::max);

        // An explicit duration wins. Otherwise run to the last keyframe,
        // which is what "animate this over 10 seconds" means when the
        // author only wrote the keys.
        let duration = match self.duration {
            Some(d) if d > 0.0 => d,
            Some(_) => {
                warnings.push("anim.duration must be greater than 0 — using the last keyframe time".into());
                latest
            }
            None => latest,
        };
        let duration = if duration > 0.0 {
            duration
        } else {
            warnings.push(
                "animation has no duration and no keyframes past t=0 — defaulting to 1 second".into(),
            );
            1.0
        };

        let name = self
            .name
            .clone()
            .unwrap_or_else(|| script_name.to_string());

        // Self-contained: the .anim carries the flame it belongs to, so
        // it reproduces without hunting for a matching .fflame.
        let mut animation = Animation::with_config(name, duration, config.clone());

        for draft in &self.tracks {
            if draft.keys.is_empty() {
                warnings.push(format!(
                    "no keyframes for `{}` — the track was dropped",
                    draft.target
                ));
                continue;
            }
            let mut keys = draft.keys.clone();
            // The player walks keyframes in order; a script is free to
            // add them in any. Stable so equal times keep script order.
            keys.sort_by(|a, b| a.time.partial_cmp(&b.time).unwrap_or(std::cmp::Ordering::Equal));

            let mut track = Track::new(draft.target.clone(), TrackSource::Keyframes { keyframes: keys });
            track.interpolation = draft.interpolation;
            animation.add_track(track);
        }

        if animation.tracks.is_empty() {
            warnings.push("the script defined an animation with no usable tracks".into());
        }
        Some(animation)
    }
}

/// snake_case → PascalCase, so the names a script already uses for
/// `config.set` also name animation targets.
///
/// `config.set` addresses fields by their serde name (`camera_rotation_x`)
/// while tracks use `ConfigPath` keys (`CameraRotationX`). Asking authors
/// to know both spellings would be a trap, and the two happen to differ
/// only by case and separators.
fn pascal_case(s: &str) -> String {
    s.split('_')
        .filter(|part| !part.is_empty())
        .map(|part| {
            let mut chars = part.chars();
            match chars.next() {
                Some(first) => first.to_uppercase().collect::<String>() + chars.as_str().to_lowercase().as_str(),
                None => String::new(),
            }
        })
        .collect()
}

/// Resolve a flame-level parameter name to a track target.
///
/// Accepts the script's own spelling (`"camera_rotation_x"`) and the
/// `ConfigPath` key (`"CameraRotationX"`), so neither is wrong.
pub(crate) fn resolve_flame_target(name: &str) -> Result<String, String> {
    let candidates = [name.to_string(), pascal_case(name)];
    for candidate in &candidates {
        if let Some(path) = ConfigPath::from_string_key(candidate) {
            return Ok(path.to_string_key());
        }
    }
    Err(format!(
        "`{name}` is not an animatable setting. Use the same name `config.set` takes \
         (for example \"zoom\", \"rotation\", \"camera_rotation_x\", \"exposure\"), \
         or a transform's own `key()` for per-transform values."
    ))
}

/// Resolve a per-transform parameter name to a track target.
///
/// A dotted name is a variation parameter (`"julian.power"`), matching
/// how `set_variation_param` already spells them. A single letter a–f is
/// an affine coefficient. Everything else is a transform field.
pub(crate) fn resolve_transform_target(xref: TransformRef, name: &str) -> Result<String, String> {
    if let Some((variation, param)) = name.split_once('.') {
        if variation.is_empty() || param.is_empty() {
            return Err(format!("`{name}` is not a valid variation parameter name"));
        }
        return Ok(xref
            .variation_param_path(variation.to_string(), param.to_string())
            .to_string_key());
    }

    let affine = match name {
        "a" => Some(AffineParam::A),
        "b" => Some(AffineParam::B),
        "c" => Some(AffineParam::C),
        "d" => Some(AffineParam::D),
        "e" => Some(AffineParam::E),
        "f" => Some(AffineParam::F),
        _ => None,
    };
    if let Some(param) = affine {
        return Ok(xref.affine_path(param).to_string_key());
    }

    // Weight, colour and opacity exist only on normal transforms: linked
    // and final ones always run, so there is nothing to weight. Say so
    // rather than emitting a target that resolves to nothing.
    let index = xref.index();
    let path = match (xref, name) {
        (TransformRef::Normal(_), "weight") => ConfigPath::TransformWeight { index },
        (TransformRef::Normal(_), "color") => ConfigPath::TransformColor { index },
        (TransformRef::Normal(_), "color_speed") => ConfigPath::TransformColorSpeed { index },
        (TransformRef::Normal(_), "opacity") => ConfigPath::TransformOpacity { index },
        (_, "weight" | "color" | "color_speed" | "opacity") => {
            return Err(format!(
                "`{name}` only exists on normal transforms — {} transforms always run, \
                 so they carry no weight or colour of their own",
                xref.pool_kind()
            ))
        }
        _ => {
            return Err(format!(
                "`{name}` is not an animatable transform value. Try \"weight\", \"color\", \
                 \"color_speed\", \"opacity\", an affine coefficient \"a\"–\"f\", or a \
                 variation parameter like \"julian.power\"."
            ))
        }
    };
    Ok(path.to_string_key())
}

/// Parse an easing name. Unknown names are an error rather than a silent
/// fallback to linear — a typo'd curve that quietly does nothing is
/// exactly the kind of thing nobody spots in a rendered animation.
pub(crate) fn parse_easing(name: &str) -> Result<EasingFunction, String> {
    let key = name.trim().to_lowercase().replace(['-', ' '], "_");
    Ok(match key.as_str() {
        "linear" => EasingFunction::Linear,
        "ease_in" | "in" => EasingFunction::EaseIn,
        "ease_out" | "out" => EasingFunction::EaseOut,
        "ease_in_out" | "in_out" | "smooth" => EasingFunction::EaseInOut,
        "ease_in_quad" => EasingFunction::EaseInQuad,
        "ease_out_quad" => EasingFunction::EaseOutQuad,
        "ease_in_out_quad" => EasingFunction::EaseInOutQuad,
        "ease_in_cubic" => EasingFunction::EaseInCubic,
        "ease_out_cubic" => EasingFunction::EaseOutCubic,
        "ease_in_out_cubic" => EasingFunction::EaseInOutCubic,
        other => {
            return Err(format!(
                "`{other}` is not an easing. Try \"linear\", \"ease_in\", \"ease_out\", \
                 \"ease_in_out\", or the quad/cubic variants."
            ))
        }
    })
}

/// Parse a track interpolation name.
pub(crate) fn parse_interpolation(name: &str) -> Result<Interpolation, String> {
    let key = name.trim().to_lowercase();
    Ok(match key.as_str() {
        "step" | "hold" => Interpolation::Step,
        "linear" => Interpolation::Linear,
        "smooth" => Interpolation::Smooth,
        "sinusoidal" | "sine" => Interpolation::Sinusoidal,
        "exponential" | "geometric" | "log" => Interpolation::Exponential,
        other => {
            return Err(format!(
                "`{other}` is not an interpolation. Try \"step\", \"linear\", \"smooth\", \
                 \"sinusoidal\" or \"exponential\"."
            ))
        }
    })
}
