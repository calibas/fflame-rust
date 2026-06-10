"""One-shot migration: rework the (uncommitted) JWildfire diminish-Z port
into our own far-density-fade feature.

The JWF semantic turned out to be a color lerp (fog-like, can even
brighten); what we want is genuine density falloff with distance. This
renames dimish_z / dim_z_distance -> far_density_fade /
far_density_fade_start, drops the dimish color everywhere (fields, GPU
params, ConfigPaths, UI), and removes the cam_zdimish XML round-trip
(semantics no longer match JWF, so keeping their attribute would lie).

Run from the repo root: python scripts/rework_dimish_to_far_density_fade.py
Safe to delete after the containing commit lands.
"""
import io

def patch(path, subs, label):
    s = io.open(path, encoding='utf-8').read()
    for old, new in subs:
        if old not in s:
            print(f"MISS in {label}: {old[:70]!r}")
        s = s.replace(old, new)
    io.open(path, 'w', encoding='utf-8', newline='').write(s)

# --- flame_xml.rs: remove XML import/export + test; rename Flame literal fields ---
p = 'src/flame_xml.rs'
s = io.open(p, encoding='utf-8').read()
old = """    // JWildfire diminish-Z: far samples lerp toward a color. JWF
    // writes the triple only when cam_zdimish != 0; color channels
    // are already 0..1 in the XML (JWF divides its 0..255 internals
    // by 255 when serializing).
    let mut dimish_z: f32 = 0.0;
    let mut dim_z_distance: f32 = 0.0;
    let mut dimish_z_color: [f32; 3] = [0.0, 0.0, 0.0];
"""
assert old in s, "vars block"
s = s.replace(old, "", 1)
old = """            // JWildfire diminish-Z triple.
            "cam_zdimish" => dimish_z = value.parse().unwrap_or(0.0),
            "cam_zdimdist" => dim_z_distance = value.parse().unwrap_or(0.0),
            "cam_zdimcolor" => {
                let parts: Vec<&str> = value.split_whitespace().collect();
                if parts.len() == 3 {
                    // Already 0..1 in the XML — no /255 (unlike `background`).
                    dimish_z_color[0] = parts[0].parse::<f32>().unwrap_or(0.0);
                    dimish_z_color[1] = parts[1].parse::<f32>().unwrap_or(0.0);
                    dimish_z_color[2] = parts[2].parse::<f32>().unwrap_or(0.0);
                }
            }
"""
assert old in s, "import arms"
s = s.replace(old, "", 1)
old = """        // JWF diminish-Z triple (cam_zdimish / cam_zdimdist /
        // cam_zdimcolor), parsed above.
        dimish_z,
        dim_z_distance,
        dimish_z_color,
"""
assert old in s, "flame literal"
s = s.replace(old, "        // Our own extension; no .flame XML attribute (see Flame field docs)\n        far_density_fade: 0.0,\n        far_density_fade_start: 0.0,\n", 1)
old = '''    // JWildfire diminish-Z triple — emitted together and only when
    // active, matching JWF's own conditional serialization
    // (AbstractFlameWriter: `if (pFlame.getDimishZ() != 0.0)`).
    if flame.dimish_z.abs() > 1e-6 {
        out.push_str(&format!(" cam_zdimish=\\"{}\\"", fmt_f32(flame.dimish_z)));
        out.push_str(&format!(" cam_zdimdist=\\"{}\\"", fmt_f32(flame.dim_z_distance)));
        out.push_str(&format!(
            " cam_zdimcolor=\\"{} {} {}\\"",
            fmt_f32(flame.dimish_z_color[0]),
            fmt_f32(flame.dimish_z_color[1]),
            fmt_f32(flame.dimish_z_color[2])
        ));
    }
'''
assert old in s, "export block"
s = s.replace(old, "", 1)
start = s.find("    #[test]\n    fn test_dimish_z_roundtrip() {")
assert start != -1, "test start"
end = s.find("    #[test]\n    fn test_camera_position_roundtrip() {")
assert end != -1 and end > start, "test end"
s = s[:start] + s[end:]
io.open(p, 'w', encoding='utf-8', newline='').write(s)
print("flame_xml ok")

# --- buffers.rs ---
patch('src/gpu/buffers.rs', [
    ("""    // JWildfire diminish-Z: far samples lerp toward dimish_color by
    // exp(-(dim_z_distance - camera_z)^2 * dimish_z). 0 = off.
    pub dimish_z: f32,
    pub dim_z_distance: f32,
    pub dimish_color_r: f32,
    pub dimish_color_g: f32,
    pub dimish_color_b: f32,
""",
     """    // Far density fade: sample density weighted by
    // exp(-(far_density_fade_start - camera_z)^2 * far_density_fade)
    // beyond the start depth. 0 = off.
    pub far_density_fade: f32,
    pub far_density_fade_start: f32,
"""),
    ("""            dimish_z: 0.0,
            dim_z_distance: 0.0,
            dimish_color_r: 0.0,
            dimish_color_g: 0.0,
            dimish_color_b: 0.0,
""",
     """            far_density_fade: 0.0,
            far_density_fade_start: 0.0,
"""),
], 'buffers')

# pad: 37 scalars = 148 bytes -> 12 bytes pad to reach 160
p = 'src/gpu/buffers.rs'
s = io.open(p, encoding='utf-8').read()
old = "    pub post_symmetry: GpuPostSymmetry,"
assert old in s
s = s.replace(old, "    // std140 alignment pad: 37 scalars x 4 = 148 bytes; post_symmetry\n    // (a struct) must start on a 16-byte boundary -> 12 bytes pad to 160.\n    // Mirror in shaders/core/header.wgsl.\n    pub _pad_before_post_symmetry: [u32; 3],\n" + old, 1)
old2 = "            post_symmetry: GpuPostSymmetry::default(),"
if old2 in s:
    s = s.replace(old2, "            _pad_before_post_symmetry: [0; 3],\n" + old2, 1)
else:
    print("NOTE: default ctor post_symmetry line not found")
io.open(p, 'w', encoding='utf-8', newline='').write(s)

# --- compute_kernel.rs ---
patch('src/renderer/compute_kernel.rs', [
    ("    dimish_z: f32,\n    dim_z_distance: f32,\n    dimish_z_color: [f32; 3],\n",
     "    far_density_fade: f32,\n    far_density_fade_start: f32,\n"),
    ("            dimish_z: flame.dimish_z,\n            dim_z_distance: flame.dim_z_distance,\n            dimish_z_color: flame.dimish_z_color,\n",
     "            far_density_fade: flame.far_density_fade,\n            far_density_fade_start: flame.far_density_fade_start,\n"),
    ("        self.dimish_z = config.flame.dimish_z;\n        self.dim_z_distance = config.flame.dim_z_distance;\n        self.dimish_z_color = config.flame.dimish_z_color;\n",
     "        self.far_density_fade = config.flame.far_density_fade;\n        self.far_density_fade_start = config.flame.far_density_fade_start;\n"),
    ("        self.dimish_z = flame.dimish_z;\n        self.dim_z_distance = flame.dim_z_distance;\n        self.dimish_z_color = flame.dimish_z_color;\n",
     "        self.far_density_fade = flame.far_density_fade;\n        self.far_density_fade_start = flame.far_density_fade_start;\n"),
    ("            dimish_z: self.dimish_z,\n            dim_z_distance: self.dim_z_distance,\n            dimish_color_r: self.dimish_z_color[0],\n            dimish_color_g: self.dimish_z_color[1],\n            dimish_color_b: self.dimish_z_color[2],\n",
     "            far_density_fade: self.far_density_fade,\n            far_density_fade_start: self.far_density_fade_start,\n            _pad_before_post_symmetry: [0; 3],\n"),
], 'kernel')

# --- high_res.rs ---
patch('src/export/high_res.rs', [
    ("                dimish_z: config.flame.dimish_z,\n                dim_z_distance: config.flame.dim_z_distance,\n                dimish_color_r: config.flame.dimish_z_color[0],\n                dimish_color_g: config.flame.dimish_z_color[1],\n                dimish_color_b: config.flame.dimish_z_color[2],\n",
     "                far_density_fade: config.flame.far_density_fade,\n                far_density_fade_start: config.flame.far_density_fade_start,\n                _pad_before_post_symmetry: [0; 3],\n"),
], 'high_res')

# --- randomize.rs + api/sync.rs ---
patch('src/scene/randomize.rs', [
    ("        dimish_z: 0.0,\n        dim_z_distance: 0.0,\n        dimish_z_color: [0.0, 0.0, 0.0],\n",
     "        far_density_fade: 0.0,\n        far_density_fade_start: 0.0,\n"),
], 'randomize')
patch('src/api/sync.rs', [
    ("        dimish_z: 0.0,\n        dim_z_distance: 0.0,\n        dimish_z_color: [0.0, 0.0, 0.0],\n",
     "        far_density_fade: 0.0,\n        far_density_fade_start: 0.0,\n"),
], 'sync')

# --- delta.rs ---
p = 'src/config/delta.rs'
s = io.open(p, encoding='utf-8').read()
subs = [
    ("    DimishZ,\n    DimZDistance,\n    DimishZColorR,\n    DimishZColorG,\n    DimishZColorB,\n",
     "    FarDensityFade,\n    FarDensityFadeStart,\n"),
    ('            ConfigPath::DimishZ => write!(f, "Diminish Z"),\n            ConfigPath::DimZDistance => write!(f, "Diminish Z Distance"),\n            ConfigPath::DimishZColorR => write!(f, "Diminish Z Color R"),\n            ConfigPath::DimishZColorG => write!(f, "Diminish Z Color G"),\n            ConfigPath::DimishZColorB => write!(f, "Diminish Z Color B"),\n',
     '            ConfigPath::FarDensityFade => write!(f, "Far Density Fade"),\n            ConfigPath::FarDensityFadeStart => write!(f, "Far Density Fade Start"),\n'),
    ('            ConfigPath::DimishZ => I18nKey::simple("history.param.dimish_z"),\n            ConfigPath::DimZDistance => I18nKey::simple("history.param.dim_z_distance"),\n            ConfigPath::DimishZColorR => I18nKey::simple("history.param.dimish_z_color_r"),\n            ConfigPath::DimishZColorG => I18nKey::simple("history.param.dimish_z_color_g"),\n            ConfigPath::DimishZColorB => I18nKey::simple("history.param.dimish_z_color_b"),\n',
     '            ConfigPath::FarDensityFade => I18nKey::simple("history.param.far_density_fade"),\n            ConfigPath::FarDensityFadeStart => I18nKey::simple("history.param.far_density_fade_start"),\n'),
    ("            | ConfigPath::DimishZ\n            | ConfigPath::DimZDistance\n            | ConfigPath::DimishZColorR\n            | ConfigPath::DimishZColorG\n            | ConfigPath::DimishZColorB\n",
     "            | ConfigPath::FarDensityFade\n            | ConfigPath::FarDensityFadeStart\n"),
    ('            ConfigPath::DimishZ => "DimishZ".to_string(),\n            ConfigPath::DimZDistance => "DimZDistance".to_string(),\n            ConfigPath::DimishZColorR => "DimishZColorR".to_string(),\n            ConfigPath::DimishZColorG => "DimishZColorG".to_string(),\n            ConfigPath::DimishZColorB => "DimishZColorB".to_string(),\n',
     '            ConfigPath::FarDensityFade => "FarDensityFade".to_string(),\n            ConfigPath::FarDensityFadeStart => "FarDensityFadeStart".to_string(),\n'),
    ('            "DimishZ" => return Some(ConfigPath::DimishZ),\n            "DimZDistance" => return Some(ConfigPath::DimZDistance),\n            "DimishZColorR" => return Some(ConfigPath::DimishZColorR),\n            "DimishZColorG" => return Some(ConfigPath::DimishZColorG),\n            "DimishZColorB" => return Some(ConfigPath::DimishZColorB),\n',
     '            "FarDensityFade" => return Some(ConfigPath::FarDensityFade),\n            "FarDensityFadeStart" => return Some(ConfigPath::FarDensityFadeStart),\n'),
    ("        | ConfigPath::DimishZ\n        | ConfigPath::DimZDistance\n        | ConfigPath::DimishZColorR\n        | ConfigPath::DimishZColorG\n        | ConfigPath::DimishZColorB\n",
     "        | ConfigPath::FarDensityFade\n        | ConfigPath::FarDensityFadeStart\n"),
    ("            ConfigPath::DimishZ,\n            ConfigPath::DimZDistance,\n            ConfigPath::DimishZColorR,\n            ConfigPath::DimishZColorG,\n            ConfigPath::DimishZColorB,\n",
     "            ConfigPath::FarDensityFade,\n            ConfigPath::FarDensityFadeStart,\n"),
]
for old, new in subs:
    if old not in s:
        print("MISS delta:", old[:70].replace("\n", "/n"))
    s = s.replace(old, new)
io.open(p, 'w', encoding='utf-8', newline='').write(s)

# --- manager.rs ---
p = 'src/config/manager.rs'
s = io.open(p, encoding='utf-8').read()
subs = [
    ("            ConfigPath::DimishZ => Ok(flame.dimish_z.into()),\n            ConfigPath::DimZDistance => Ok(flame.dim_z_distance.into()),\n            ConfigPath::DimishZColorR => Ok(flame.dimish_z_color[0].into()),\n            ConfigPath::DimishZColorG => Ok(flame.dimish_z_color[1].into()),\n            ConfigPath::DimishZColorB => Ok(flame.dimish_z_color[2].into()),\n",
     "            ConfigPath::FarDensityFade => Ok(flame.far_density_fade.into()),\n            ConfigPath::FarDensityFadeStart => Ok(flame.far_density_fade_start.into()),\n"),
    ("""            ConfigPath::DimishZ => {
                let v = value.try_into()?;
                self.active_flame_mut()?.dimish_z = v;
            }
            ConfigPath::DimZDistance => {
                let v = value.try_into()?;
                self.active_flame_mut()?.dim_z_distance = v;
            }
            ConfigPath::DimishZColorR => {
                let v = value.try_into()?;
                self.active_flame_mut()?.dimish_z_color[0] = v;
            }
            ConfigPath::DimishZColorG => {
                let v = value.try_into()?;
                self.active_flame_mut()?.dimish_z_color[1] = v;
            }
            ConfigPath::DimishZColorB => {
                let v = value.try_into()?;
                self.active_flame_mut()?.dimish_z_color[2] = v;
            }
""",
     """            ConfigPath::FarDensityFade => {
                let v = value.try_into()?;
                self.active_flame_mut()?.far_density_fade = v;
            }
            ConfigPath::FarDensityFadeStart => {
                let v = value.try_into()?;
                self.active_flame_mut()?.far_density_fade_start = v;
            }
"""),
]
for old, new in subs:
    if old not in s:
        print("MISS manager:", old[:70].replace("\n", "/n"))
    s = s.replace(old, new)
io.open(p, 'w', encoding='utf-8', newline='').write(s)

# --- target_selector + track_editor ---
patch('src/ui/target_selector.rs', [
    ('        TargetItem::new(ConfigPath::DimishZ, "Diminish Z"),\n        TargetItem::new(ConfigPath::DimZDistance, "Diminish Z Distance"),\n',
     '        TargetItem::new(ConfigPath::FarDensityFade, "Far Density Fade"),\n        TargetItem::new(ConfigPath::FarDensityFadeStart, "Far Density Fade Start"),\n'),
], 'target_selector')
patch('src/ui/track_editor.rs', [
    ("        ConfigPath::DimishZ => Some(config.flame.dimish_z as f64),\n        ConfigPath::DimZDistance => Some(config.flame.dim_z_distance as f64),\n        ConfigPath::DimishZColorR => Some(config.flame.dimish_z_color[0] as f64),\n        ConfigPath::DimishZColorG => Some(config.flame.dimish_z_color[1] as f64),\n        ConfigPath::DimishZColorB => Some(config.flame.dimish_z_color[2] as f64),\n",
     "        ConfigPath::FarDensityFade => Some(config.flame.far_density_fade as f64),\n        ConfigPath::FarDensityFadeStart => Some(config.flame.far_density_fade_start as f64),\n"),
], 'track_editor')
print("done")
