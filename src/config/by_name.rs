//! A setting addressed by name, as a script's `config.set` and Python's
//! `Config.set` address it: by the field's name in the saved file
//! (`"sim.terrain.height"`) or by its track name (`"Sim.Terrain.Height"`).
//!
//! A name that is a [`ConfigPath`] is written by the one setter
//! ([`super::apply`]), with its clamps and side effects, so a value a
//! script sets means what the same value set in a panel means: a time
//! step of 0 is refused, not a crash (the 2026-11-04 `max_dt` crash went
//! through a script's own write). A field no path carries -- a group, a
//! list, a pick the panels make whole -- is written into the file's JSON,
//! as every name was before.

use super::delta::{ConfigPath, ConfigValue};
use super::fractal_config::FractalConfig;
use super::manager::{ConfigError, ConfigManager, EditingTarget};

/// What a write by name did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Applied {
    Changed,
    /// Nothing changed and the name is not a setting the config knows: a
    /// misspelling, or a field at its default set to that default (the
    /// file leaves it out, so the two cannot be told apart).
    NoVisibleChange,
}

/// snake_case → PascalCase, so the names a script uses for `config.set`
/// also name tracks: the file's `camera_rotation_x` is the track's
/// `CameraRotationX`, and the two differ only by case and separators.
pub fn pascal_case(s: &str) -> String {
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

/// The path a setting's name names: its track name (`"CameraRotationX"`),
/// or the field's name in the file (`"camera_rotation_x"`), a dotted one
/// segment by segment (`"sim.terrain.height"` is `"Sim.Terrain.Height"`).
/// The lighting rig's fields, a group in the file, are flat tracks:
/// `"solid_shading.ambient"` is `"SolidAmbient"`, `"solid_shading.ssao_radius"`
/// `"SsaoRadius"`.
pub fn path_named(name: &str) -> Option<ConfigPath> {
    let dotted = name.split('.').map(pascal_case).collect::<Vec<_>>().join(".");
    let mut candidates = vec![name.to_string(), pascal_case(name), dotted];
    if let Some(field) = name.strip_prefix("solid_shading.").filter(|f| !f.contains('.')) {
        candidates.extend([format!("Solid{}", pascal_case(field)), pascal_case(field)]);
    }
    candidates.iter().find_map(|c| ConfigPath::from_string_key(c))
}

/// `json` as the value `path` takes, read off the type its value has now;
/// `None` for a kind of value the setter takes whole (an enum, a curve).
/// An error for an element the config does not have (`"Sim.Layer.3.Param.feed"`
/// with two layers).
fn value_for(config: &FractalConfig, path: &ConfigPath, json: &serde_json::Value) -> Result<Option<ConfigValue>, ConfigError> {
    let current = match ConfigManager::get_value_from_config(config, path, EditingTarget::Main) {
        Ok(v) => v,
        Err(ConfigError::InvalidIndex) => return Err(ConfigError::InvalidIndex),
        Err(_) => return Ok(None),
    };
    Ok(value_like(&current, json))
}

/// `json` as a value of `current`'s kind.
fn value_like(current: &ConfigValue, json: &serde_json::Value) -> Option<ConfigValue> {
    // A count from a script or Python may arrive as a whole float.
    let whole = |j: &serde_json::Value| {
        j.as_i64().or_else(|| j.as_f64().filter(|f| f.fract() == 0.0 && f.abs() < 9.0e15).map(|f| f as i64))
    };
    Some(match current {
        ConfigValue::Float(_) => ConfigValue::Float(json.as_f64()? as f32),
        ConfigValue::Double(_) => ConfigValue::Double(json.as_f64()?),
        ConfigValue::Int(_) => ConfigValue::Int(i32::try_from(whole(json)?).ok()?),
        ConfigValue::UInt(_) => ConfigValue::UInt(u32::try_from(whole(json)?).ok()?),
        ConfigValue::UInt64(_) => ConfigValue::UInt64(u64::try_from(whole(json)?).ok()?),
        ConfigValue::Bool(_) => ConfigValue::Bool(json.as_bool()?),
        ConfigValue::String(_) => ConfigValue::String(json.as_str()?.to_string()),
        _ => return None,
    })
}

/// Set one setting of `cfg` by name (see the module docs). The flame is
/// refused: it has richer structure, and session-local ids a JSON round
/// trip would reset, so a script edits it through its `flame` object and
/// Python through `Config`'s transform calls.
pub fn set(cfg: &mut FractalConfig, key: &str, value: serde_json::Value) -> Result<Applied, String> {
    const FLAME: &str = "use the `flame` object to edit the flame, not config.set";
    if key == "flame" || key.starts_with("flame.") {
        return Err(FLAME.to_string());
    }
    if let Some(path) = path_named(key) {
        if let Some(v) = value_for(cfg, &path, &value).map_err(|e| format!("`{key}`: {e}"))? {
            let mut next = cfg.clone();
            // A number goes over as an f64 first: a setter that reads one
            // (the escape zoom, the pan) keeps every digit, one that reads
            // an f32 refuses it and takes the f32.
            let wide = match (&v, value.as_f64()) {
                (ConfigValue::Float(_), Some(x)) => Some(ConfigValue::Double(x)),
                _ => None,
            };
            let applied = match wide.map(|w| super::apply(&mut next, EditingTarget::Main, &path, w)) {
                Some(Err(ConfigError::TypeMismatch)) | None => {
                    next = cfg.clone();
                    super::apply(&mut next, EditingTarget::Main, &path, v)
                }
                Some(r) => r,
            };
            applied.map_err(|e| format!("`{key}`: {e}"))?;
            if serde_json::to_value(&next.flame).ok() != serde_json::to_value(&cfg.flame).ok() {
                return Err(FLAME.to_string());
            }
            *cfg = next;
            return Ok(Applied::Changed);
        }
    }
    patch(cfg, key, value)
}

/// Set a field by its `.fflame` JSON path, through serde, so every field
/// the file holds is reachable by the name the file gives it.
fn patch(cfg: &mut FractalConfig, key: &str, value: serde_json::Value) -> Result<Applied, String> {
    let before = serde_json::to_value(&*cfg).map_err(|e| e.to_string())?;
    // A key already present in the JSON is a known setting, even if the
    // assignment turns out to be a no-op (setting brightness to the value
    // it already has). Only a key that is BOTH absent and produced no
    // change is ambiguous enough to warn about.
    let key_existed = json_at(&before, key).is_some();
    let mut patched = before.clone();

    // Walk to the parent, creating a missing group as an empty object: a
    // group at its defaults is omitted from the JSON entirely (`sim`,
    // `escape`, `solid_shading`, `sim.terrain` all skip-if-default), and
    // each deserializes from a partial object, as a compact save loads.
    // A misspelt group lands as an unknown key serde ignores, which the
    // "changed nothing" warning then reports.
    let parts: Vec<&str> = key.split('.').collect();
    let (last, parents) = parts.split_last().ok_or("empty setting name")?;
    let mut cur = &mut patched;
    for p in parents {
        cur = cur
            .as_object_mut()
            .ok_or_else(|| format!("`{key}` is not a settable field"))?
            .entry((*p).to_string())
            .or_insert_with(|| serde_json::Value::Object(Default::default()));
    }
    let obj = cur.as_object_mut().ok_or_else(|| format!("`{key}` is not a settable field"))?;
    // Insert rather than require presence: fields sitting at their
    // default are omitted from the JSON entirely (skip-if-default).
    obj.insert((*last).to_string(), value);

    let flame = cfg.flame.clone();
    let mut next: FractalConfig =
        serde_json::from_value(patched).map_err(|e| format!("invalid value for `{key}`: {e}"))?;
    // Session-local IDs are #[serde(skip)]; restore the flame wholesale
    // and re-issue IDs for anything the round trip blanked.
    next.flame = flame;
    next.fixup_ids();

    let after = serde_json::to_value(&next).map_err(|e| e.to_string())?;
    let changed = after != before;
    *cfg = next;
    Ok(if changed || key_existed { Applied::Changed } else { Applied::NoVisibleChange })
}

/// The value at a dotted path in a config's JSON.
pub fn json_at<'a>(root: &'a serde_json::Value, key: &str) -> Option<&'a serde_json::Value> {
    let mut cur = root;
    for p in key.split('.') {
        cur = cur.get(p)?;
    }
    Some(cur)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn leaves(v: &serde_json::Value, at: &str, out: &mut Vec<(String, serde_json::Value)>) {
        match v {
            serde_json::Value::Object(m) => {
                for (k, x) in m {
                    let key = if at.is_empty() { k.clone() } else { format!("{at}.{k}") };
                    leaves(x, &key, out);
                }
            }
            _ => out.push((at.to_string(), v.clone())),
        }
    }

    /// The fields whose values differ between two configs' JSON.
    fn moved(a: &FractalConfig, b: &FractalConfig) -> std::collections::BTreeSet<String> {
        let (mut x, mut y) = (Vec::new(), Vec::new());
        leaves(&serde_json::to_value(a).expect("a"), "", &mut x);
        leaves(&serde_json::to_value(b).expect("b"), "", &mut y);
        let (x, y): (std::collections::BTreeMap<_, _>, std::collections::BTreeMap<_, _>) =
            (x.into_iter().collect(), y.into_iter().collect());
        x.keys().chain(y.keys()).filter(|k| x.get(*k) != y.get(*k)).cloned().collect()
    }

    /// The presets and every visual-suite config: between them, most of
    /// the fields a config can hold set to something other than their
    /// defaults, so the file writes them out.
    fn configs() -> Vec<FractalConfig> {
        let mut out = crate::resources::presets::load_embedded_presets().expect("presets");
        let mut stack = vec![std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/visual/configs")];
        while let Some(dir) = stack.pop() {
            for entry in std::fs::read_dir(&dir).expect("configs").flatten() {
                let path = entry.path();
                if path.is_dir() {
                    stack.push(path);
                } else if path.extension().is_some_and(|e| e == "fflame") {
                    out.extend(std::fs::read_to_string(&path).ok().and_then(|t| FractalConfig::from_json(&t).ok()));
                }
            }
        }
        out
    }

    /// **A name writes its own field.** Every field the presets and the
    /// visual suite hold, outside the flame, set by its name in the file:
    /// where the name is a track's, the setter's write moves that field
    /// and no other -- a name aliased to the wrong path would move another
    /// -- and a number or a switch moves at all.
    #[test]
    fn every_name_in_the_file_writes_its_own_field() {
        let mut seen = std::collections::BTreeSet::new();
        let mut through_the_setter = 0;
        let mut wrong = Vec::new();
        for config in configs() {
            let mut fields = Vec::new();
            leaves(&serde_json::to_value(&config).expect("json"), "", &mut fields);
            for (key, now) in fields {
                if key.starts_with("flame.") || !seen.insert(key.clone()) {
                    continue;
                }
                let Some(path) = path_named(&key) else { continue };
                if value_for(&config, &path, &now).ok().flatten().is_none() {
                    continue;
                }
                through_the_setter += 1;
                let probes = match &now {
                    serde_json::Value::Number(n) if n.is_f64() => {
                        let f = n.as_f64().expect("f64");
                        vec![serde_json::json!(f * 0.9 + 0.013), serde_json::json!(f * 1.1 - 0.017)]
                    }
                    serde_json::Value::Number(n) => {
                        let i = n.as_i64().expect("int");
                        vec![serde_json::json!(i + 1), serde_json::json!((i - 1).max(0))]
                    }
                    serde_json::Value::Bool(b) => vec![serde_json::json!(!b)],
                    other => vec![other.clone()],
                };
                let mut moved_itself = now.is_string();
                for probe in probes {
                    let mut c = config.clone();
                    match set(&mut c, &key, probe.clone()) {
                        Ok(_) => {}
                        Err(e) => wrong.push(format!("{key} = {probe}: {e}")),
                    }
                    let fields = moved(&config, &c);
                    moved_itself |= fields.contains(&key);
                    let others: Vec<_> = fields.into_iter().filter(|k| *k != key).collect();
                    if !others.is_empty() {
                        wrong.push(format!("{key} = {probe} ({}) also moved {others:?}", path.to_string_key()));
                    }
                }
                if !moved_itself {
                    wrong.push(format!("{key} ({}) never moved", path.to_string_key()));
                }
            }
        }
        assert!(through_the_setter > 100, "only {through_the_setter} names reached the setter");
        assert!(wrong.is_empty(), "{} names write the wrong field:\n  {}", wrong.len(), wrong.join("\n  "));
    }

    /// The setter's rules hold for a script: a time step of 0 is refused
    /// to the solver's floor (the 2026-11-04 `max_dt` crash), a palette
    /// size is held to the panel's range, the lighting rig's names reach
    /// their tracks, and the escape zoom, an f64, keeps every digit.
    #[test]
    fn a_name_is_set_by_the_setters_rules() {
        let mut c = FractalConfig::default();
        set(&mut c, "sim.dt", serde_json::json!(0.0)).expect("dt");
        assert!(c.sim.dt > 0.0, "a time step of {} reached the solver", c.sim.dt);
        set(&mut c, "palette_size", serde_json::json!(3)).expect("size");
        let mut panel = FractalConfig::default();
        let three = value_for(&panel, &ConfigPath::PaletteSize, &serde_json::json!(3)).ok().flatten().expect("a count");
        super::super::apply(&mut panel, EditingTarget::Main, &ConfigPath::PaletteSize, three).expect("panel");
        assert_ne!(panel.palette_size, 3, "the panel's range holds");
        assert_eq!(c.palette_size, panel.palette_size);
        assert_eq!(path_named("solid_shading.ambient"), Some(ConfigPath::SolidAmbient));
        assert_eq!(path_named("solid_shading.shading_strength"), Some(ConfigPath::ShadingStrength));
        set(&mut c, "escape.zoom_log2", serde_json::json!(150.123_456_789_012_3)).expect("zoom");
        assert_eq!(c.escape.zoom_log2, 150.123_456_789_012_3);
        assert!(set(&mut c, "Transform.0.Weight", serde_json::json!(0.5)).is_err(), "no transform 0 to set");
        let mut preset = crate::resources::presets::create_default_preset();
        let err = set(&mut preset, "Transform.0.Weight", serde_json::json!(0.5)).expect_err("the flame");
        assert!(err.contains("`flame` object"), "{err}");
    }
}
