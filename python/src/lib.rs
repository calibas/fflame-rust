//! `pyfflame` — Python bindings for building, editing and converting
//! fractal flames.
//!
//! This crate is deliberately a STANDALONE build (its own `[workspace]`)
//! that depends on `fractal_flame_wgpu` by path. Nothing here changes the
//! app: no features are toggled, no modules are gated, so the app's
//! codegen is byte-for-byte what it was and its build is untouched. The
//! price of that choice is that the wheel links the app's whole
//! dependency graph, including the GPU and window crates it never calls —
//! a fatter wheel and a slower first build, traded for zero risk to the
//! editor.
//!
//! No rendering lives here. The flame model, the `.fflame`/`.flame`
//! readers and writers, the variation registry and the Rhai engine are
//! all reused from the main crate, so a script authored in the app runs
//! identically in a Python pipeline and a file written here is the same
//! file the app writes.

use std::collections::HashMap;
use std::path::PathBuf;

use pyo3::exceptions::{PyIOError, PyIndexError, PyValueError};
use pyo3::prelude::*;
use pyo3::types::PyDict;

use fractal_flame_wgpu::config::FractalConfig;
use fractal_flame_wgpu::scene::transforms::{RenderMode, Transform};
use fractal_flame_wgpu::script::{ParamDecl, ParamValue, ScriptHost};
use fractal_flame_wgpu::variations::global_registry;

/// A complete flame: the model plus its camera, colour and render
/// settings — the same structure a `.fflame` file holds.
// `from_py_object`: pyo3 is making this derive opt-in for Clone
// pyclasses. Config needs it — `run_script(base=...)` takes one BY
// VALUE from Python.
#[pyclass(module = "pyfflame", from_py_object)]
#[derive(Clone)]
pub struct Config {
    inner: FractalConfig,
}

impl Config {
    fn wrap(inner: FractalConfig) -> Self {
        Self { inner }
    }

    /// Transforms are addressed by index throughout; a bad one is a
    /// programming error worth naming precisely.
    fn xform(&self, index: usize) -> PyResult<&Transform> {
        let n = self.inner.flame.transforms.len();
        self.inner
            .flame
            .transforms
            .get(index)
            .ok_or_else(|| PyIndexError::new_err(format!("transform {index} out of range (flame has {n})")))
    }

    fn xform_mut(&mut self, index: usize) -> PyResult<&mut Transform> {
        let n = self.inner.flame.transforms.len();
        self.inner
            .flame
            .transforms
            .get_mut(index)
            .ok_or_else(|| PyIndexError::new_err(format!("transform {index} out of range (flame has {n})")))
    }
}

#[pymethods]
impl Config {
    /// A new flame with the app's defaults.
    #[new]
    fn new() -> Self {
        Self::wrap(FractalConfig::default())
    }

    /// Read a `.fflame` (JSON) file.
    #[staticmethod]
    fn load(path: PathBuf) -> PyResult<Self> {
        FractalConfig::load_from_file(&path)
            .map(Self::wrap)
            .map_err(|e| PyIOError::new_err(format!("{}: {e}", path.display())))
    }

    /// Parse a `.fflame` from a JSON string.
    #[staticmethod]
    fn from_json(text: &str) -> PyResult<Self> {
        FractalConfig::from_json(text)
            .map(Self::wrap)
            .map_err(|e| PyValueError::new_err(e.to_string()))
    }

    /// Read an Apophysis/JWildfire `.flame` XML file. One file may hold
    /// several flames, so this always returns a list.
    #[staticmethod]
    fn load_flame_xml(path: PathBuf) -> PyResult<Vec<Self>> {
        let xml = std::fs::read_to_string(&path)
            .map_err(|e| PyIOError::new_err(format!("{}: {e}", path.display())))?;
        Self::parse_flame_xml(&xml)
    }

    /// Parse `.flame` XML from a string.
    #[staticmethod]
    fn parse_flame_xml(xml: &str) -> PyResult<Vec<Self>> {
        fractal_flame_wgpu::flame_xml::parse_flame_xml(xml)
            .map(|v| v.into_iter().map(Self::wrap).collect())
            .map_err(|e| PyValueError::new_err(e.to_string()))
    }

    /// Write a `.fflame` (JSON) file.
    fn save(&self, path: PathBuf) -> PyResult<()> {
        self.inner
            .save_to_file(&path)
            .map_err(|e| PyIOError::new_err(format!("{}: {e}", path.display())))
    }

    /// The `.fflame` JSON as a string.
    fn to_json(&self) -> PyResult<String> {
        self.inner
            .to_json()
            .map_err(|e| PyValueError::new_err(e.to_string()))
    }

    /// The Apophysis/JWildfire `.flame` XML as a string.
    fn to_flame_xml(&self) -> String {
        fractal_flame_wgpu::flame_xml::write_flame_xml(&self.inner)
    }

    /// Write a `.flame` XML file.
    fn save_flame_xml(&self, path: PathBuf) -> PyResult<()> {
        std::fs::write(&path, self.to_flame_xml())
            .map_err(|e| PyIOError::new_err(format!("{}: {e}", path.display())))
    }

    // ---- flame-level properties ----

    #[getter]
    fn name(&self) -> String {
        self.inner.flame.name.clone()
    }
    #[setter]
    fn set_name(&mut self, value: String) {
        self.inner.flame.name = value;
    }

    /// `"2d"` or `"3d"`.
    #[getter]
    fn render_mode(&self) -> &'static str {
        match self.inner.render_mode {
            RenderMode::TwoD => "2d",
            RenderMode::ThreeD => "3d",
        }
    }
    #[setter]
    fn set_render_mode(&mut self, value: &str) -> PyResult<()> {
        self.inner.render_mode = match value {
            "2d" | "2D" => RenderMode::TwoD,
            "3d" | "3D" => RenderMode::ThreeD,
            other => {
                return Err(PyValueError::new_err(format!(
                    "render_mode must be \"2d\" or \"3d\", got {other:?}"
                )))
            }
        };
        Ok(())
    }

    #[getter]
    fn zoom(&self) -> f32 {
        self.inner.zoom
    }
    #[setter]
    fn set_zoom(&mut self, v: f32) {
        self.inner.zoom = v;
    }

    #[getter]
    fn pan_x(&self) -> f32 {
        self.inner.pan_x
    }
    #[setter]
    fn set_pan_x(&mut self, v: f32) {
        self.inner.pan_x = v;
    }

    #[getter]
    fn pan_y(&self) -> f32 {
        self.inner.pan_y
    }
    #[setter]
    fn set_pan_y(&mut self, v: f32) {
        self.inner.pan_y = v;
    }

    #[getter]
    fn rotation(&self) -> f32 {
        self.inner.rotation
    }
    #[setter]
    fn set_rotation(&mut self, v: f32) {
        self.inner.rotation = v;
    }

    #[getter]
    fn camera_pitch(&self) -> f32 {
        self.inner.camera_rotation_x
    }
    #[setter]
    fn set_camera_pitch(&mut self, v: f32) {
        self.inner.camera_rotation_x = v;
    }

    #[getter]
    fn camera_yaw(&self) -> f32 {
        self.inner.camera_rotation_y
    }
    #[setter]
    fn set_camera_yaw(&mut self, v: f32) {
        self.inner.camera_rotation_y = v;
    }

    #[getter]
    fn perspective_strength(&self) -> f32 {
        self.inner.perspective_strength
    }
    #[setter]
    fn set_perspective_strength(&mut self, v: f32) {
        self.inner.perspective_strength = v;
    }

    #[getter]
    fn preserve_z(&self) -> bool {
        self.inner.preserve_z
    }
    #[setter]
    fn set_preserve_z(&mut self, v: bool) {
        self.inner.preserve_z = v;
    }

    #[getter]
    fn max_iterations(&self) -> u64 {
        self.inner.max_iterations
    }
    #[setter]
    fn set_max_iterations(&mut self, v: u64) {
        self.inner.max_iterations = v;
    }

    // ---- transforms ----

    #[getter]
    fn transform_count(&self) -> usize {
        self.inner.flame.transforms.len()
    }

    /// Append a transform and return its index.
    fn add_transform(&mut self) -> usize {
        self.inner.flame.transforms.push(Transform::default());
        self.inner.flame.transforms.len() - 1
    }

    fn remove_transform(&mut self, index: usize) -> PyResult<()> {
        let n = self.inner.flame.transforms.len();
        if index >= n {
            return Err(PyIndexError::new_err(format!(
                "transform {index} out of range (flame has {n})"
            )));
        }
        self.inner.flame.transforms.remove(index);
        Ok(())
    }

    fn get_weight(&self, index: usize) -> PyResult<f32> {
        Ok(self.xform(index)?.weight)
    }
    fn set_weight(&mut self, index: usize, value: f32) -> PyResult<()> {
        self.xform_mut(index)?.weight = value;
        Ok(())
    }

    fn get_color(&self, index: usize) -> PyResult<f32> {
        Ok(self.xform(index)?.color)
    }
    fn set_color(&mut self, index: usize, value: f32) -> PyResult<()> {
        self.xform_mut(index)?.color = value;
        Ok(())
    }

    fn get_color_speed(&self, index: usize) -> PyResult<f32> {
        Ok(self.xform(index)?.color_speed)
    }
    fn set_color_speed(&mut self, index: usize, value: f32) -> PyResult<()> {
        self.xform_mut(index)?.color_speed = value;
        Ok(())
    }

    fn get_opacity(&self, index: usize) -> PyResult<f32> {
        Ok(self.xform(index)?.opacity)
    }
    fn set_opacity(&mut self, index: usize, value: f32) -> PyResult<()> {
        self.xform_mut(index)?.opacity = value;
        Ok(())
    }

    /// The affine as `(a, b, c, d, e, f)`, matching
    /// `x' = a*x + b*y + e`, `y' = c*x + d*y + f`.
    fn get_affine(&self, index: usize) -> PyResult<(f32, f32, f32, f32, f32, f32)> {
        let t = self.xform(index)?;
        Ok((t.a, t.b, t.c, t.d, t.e, t.f))
    }

    #[allow(clippy::too_many_arguments)]
    fn set_affine(
        &mut self,
        index: usize,
        a: f32,
        b: f32,
        c: f32,
        d: f32,
        e: f32,
        f: f32,
    ) -> PyResult<()> {
        let t = self.xform_mut(index)?;
        t.a = a;
        t.b = b;
        t.c = c;
        t.d = d;
        t.e = e;
        t.f = f;
        Ok(())
    }

    // ---- variations ----

    /// `{name: weight}` for the transform's active variations.
    fn get_variations(&self, index: usize) -> PyResult<HashMap<String, f32>> {
        Ok(self.xform(index)?.variations.clone())
    }

    /// Add (or re-weight) a variation. The name is checked against the
    /// registry — an unknown one is a typo that would otherwise render as
    /// a silently missing variation.
    fn add_variation(&mut self, index: usize, name: &str, weight: f32) -> PyResult<()> {
        if global_registry().get(name).is_none() {
            return Err(PyValueError::new_err(format!(
                "unknown variation {name:?} (see pyfflame.variations())"
            )));
        }
        self.xform_mut(index)?
            .variations
            .insert(name.to_string(), weight);
        Ok(())
    }

    fn remove_variation(&mut self, index: usize, name: &str) -> PyResult<()> {
        self.xform_mut(index)?.variations.remove(name);
        Ok(())
    }

    /// Set a variation parameter, keyed `"variation.param"` exactly as
    /// `.fflame` files store it.
    fn set_variation_param(&mut self, index: usize, key: &str, value: f32) -> PyResult<()> {
        let (var, param) = key.split_once('.').ok_or_else(|| {
            PyValueError::new_err(format!(
                "parameter key must be \"variation.param\", got {key:?}"
            ))
        })?;
        let registry = global_registry();
        let info = registry
            .get(var)
            .ok_or_else(|| PyValueError::new_err(format!("unknown variation {var:?}")))?;
        if info.get_param(param).is_none() {
            let known: Vec<&str> = info.parameters.iter().map(|p| p.name.as_str()).collect();
            return Err(PyValueError::new_err(format!(
                "variation {var:?} has no parameter {param:?}; it has: {}",
                known.join(", ")
            )));
        }
        self.xform_mut(index)?
            .variation_params
            .insert(key.to_string(), value);
        Ok(())
    }

    fn get_variation_params(&self, index: usize) -> PyResult<HashMap<String, f32>> {
        Ok(self.xform(index)?.variation_params.clone())
    }

    fn __repr__(&self) -> String {
        format!(
            "<pyfflame.Config {:?} {} transform(s) {}>",
            self.inner.flame.name,
            self.inner.flame.transforms.len(),
            self.render_mode()
        )
    }
}

/// An animation a script defined: a duration and a set of parameter
/// tracks. Opaque here — save it and open it in the app.
// `skip_from_py_object`: Animation is only ever RETURNED to Python,
// never taken as an argument, so the derive would add nothing but an
// implicit clone-on-extract.
#[pyclass(module = "pyfflame", skip_from_py_object)]
#[derive(Clone)]
pub struct Animation {
    inner: fractal_flame_wgpu::animation::Animation,
}

#[pymethods]
impl Animation {
    /// Total length in seconds.
    #[getter]
    fn duration(&self) -> f64 {
        self.inner.duration
    }

    #[getter]
    fn name(&self) -> String {
        self.inner.name.clone()
    }

    /// The `ConfigPath` key each track animates, e.g. `"Zoom"` or
    /// `"Transform.0.VariationParam.julian.power"`.
    #[getter]
    fn targets(&self) -> Vec<String> {
        self.inner.tracks.iter().map(|t| t.target.clone()).collect()
    }

    /// The flame the animation was built alongside, carried inside it so
    /// the `.anim` stands alone.
    #[getter]
    fn config(&self) -> Option<Config> {
        self.inner.base_config.clone().map(Config::wrap)
    }

    fn to_json(&self) -> PyResult<String> {
        self.inner
            .to_json()
            .map_err(|e| PyValueError::new_err(e.to_string()))
    }

    /// Write a `.anim` file.
    fn save(&self, path: PathBuf) -> PyResult<()> {
        let json = self.to_json()?;
        std::fs::write(&path, json)
            .map_err(|e| PyIOError::new_err(format!("{}: {e}", path.display())))
    }

    #[staticmethod]
    fn load(path: PathBuf) -> PyResult<Self> {
        let text = std::fs::read_to_string(&path)
            .map_err(|e| PyIOError::new_err(format!("{}: {e}", path.display())))?;
        fractal_flame_wgpu::animation::Animation::from_json(&text)
            .map(|inner| Self { inner })
            .map_err(|e| PyValueError::new_err(e.to_string()))
    }

    fn __repr__(&self) -> String {
        format!(
            "<pyfflame.Animation {:?} {:.3}s {} track(s)>",
            self.inner.name,
            self.inner.duration,
            self.inner.tracks.len()
        )
    }
}

/// What a script produced: the flame, plus whatever it printed.
#[pyclass(module = "pyfflame")]
pub struct ScriptResult {
    #[pyo3(get)]
    config: Config,
    /// The animation the script defined, or `None` if it defined none.
    #[pyo3(get)]
    animation: Option<Animation>,
    /// `print()` output, in order.
    #[pyo3(get)]
    messages: Vec<String>,
    /// Non-fatal problems the script or host flagged.
    #[pyo3(get)]
    warnings: Vec<String>,
}

#[pymethods]
impl ScriptResult {
    fn __repr__(&self) -> String {
        format!(
            "<pyfflame.ScriptResult {} message(s) {} warning(s)>",
            self.messages.len(),
            self.warnings.len()
        )
    }
}

/// Script errors lead with the line number: for this audience that's the
/// difference between a fixable typo and a dead end.
fn script_err(e: fractal_flame_wgpu::script::ScriptError) -> PyErr {
    PyValueError::new_err(match e.line {
        Some(line) => format!("line {line}: {}", e.message),
        None => e.message,
    })
}

/// Turn one Python value into the `ParamValue` the script's declaration
/// calls for. Mirrors the app's `generate --set` coercion, including
/// naming a choice by its option text or its index.
fn coerce_param(key: &str, value: &Bound<'_, PyAny>, decl: Option<&ParamDecl>) -> PyResult<ParamValue> {
    let wrong = |want: &str| {
        PyValueError::new_err(format!("parameter {key:?} expects {want}"))
    };
    match decl {
        Some(ParamDecl::Float { .. }) => Ok(ParamValue::Float(
            value.extract::<f64>().map_err(|_| wrong("a number"))?,
        )),
        Some(ParamDecl::Int { .. }) => Ok(ParamValue::Int(
            value.extract::<i64>().map_err(|_| wrong("a whole number"))?,
        )),
        Some(ParamDecl::Bool { .. }) => Ok(ParamValue::Bool(
            value.extract::<bool>().map_err(|_| wrong("True or False"))?,
        )),
        Some(ParamDecl::Text { .. }) => Ok(ParamValue::Text(
            value.extract::<String>().map_err(|_| wrong("a string"))?,
        )),
        // Colours arrive as "#ff8800" or an (r, g, b) triple in 0..1.
        Some(ParamDecl::Color { .. }) => {
            if let Ok(text) = value.extract::<String>() {
                return fractal_flame_wgpu::script::color::ScriptColor::from_hex(&text)
                    .map(|c| ParamValue::Color(c.to_rgb()))
                    .map_err(PyValueError::new_err);
            }
            let rgb: [f32; 3] = value
                .extract()
                .map_err(|_| wrong("a hex string like \"#ff8800\" or an (r, g, b) triple"))?;
            Ok(ParamValue::Color(rgb))
        }
        Some(ParamDecl::Choice { options, .. }) => {
            // By option text (case-insensitively) or by index.
            let idx = if let Ok(s) = value.extract::<String>() {
                options.iter().position(|o| o.eq_ignore_ascii_case(&s))
            } else if let Ok(i) = value.extract::<usize>() {
                (i < options.len()).then_some(i)
            } else {
                None
            };
            idx.map(ParamValue::Choice).ok_or_else(|| {
                PyValueError::new_err(format!(
                    "parameter {key:?} expects one of [{}] (by name or index), got {}",
                    options.join(", "),
                    value.repr().map(|r| r.to_string()).unwrap_or_default()
                ))
            })
        }
        // Not declared by the script. Pass a best-guess value through so
        // the host raises its own "unknown parameter" warning, which
        // names the keys the script does declare.
        None => {
            // bool before int: Python's bool IS an int.
            if let Ok(b) = value.extract::<bool>() {
                Ok(ParamValue::Bool(b))
            } else if let Ok(i) = value.extract::<i64>() {
                Ok(ParamValue::Int(i))
            } else if let Ok(f) = value.extract::<f64>() {
                Ok(ParamValue::Float(f))
            } else if let Ok(s) = value.extract::<String>() {
                Ok(ParamValue::Text(s))
            } else {
                Err(wrong("a bool, int, float or str"))
            }
        }
    }
}

/// Run a Rhai flame script — the same sandboxed engine, and the same
/// seeded RNG, the app uses, so a given (script, seed, params) produces
/// the same flame here as in the editor.
///
/// `params` supplies the script's declared parameters by name; anything
/// unsupplied takes the script's own default. `base` is the flame a
/// modifier script starts from (generators ignore it).
#[pyfunction]
#[pyo3(signature = (source, seed=1, params=None, base=None))]
fn run_script(
    source: &str,
    seed: u64,
    params: Option<&Bound<'_, PyDict>>,
    base: Option<Config>,
) -> PyResult<ScriptResult> {
    let base = base.map(|c| c.inner).unwrap_or_default();
    let host = ScriptHost::new();

    // Coerce against what the script DECLARES, exactly as the app's
    // `generate --set` does. Guessing from the Python type instead would
    // hand a choice through as an integer, which the script silently
    // ignores in favour of its default — the failure looks like the
    // script misbehaving rather than the argument being dropped.
    let declared = match params {
        Some(_) => host.collect(source, &base).map_err(script_err)?.params,
        None => Vec::new(),
    };

    let mut supplied: HashMap<String, ParamValue> = HashMap::new();
    if let Some(dict) = params {
        for (key, value) in dict.iter() {
            let key: String = key.extract()?;
            let decl = declared.iter().find(|d| d.key() == key);
            let parsed = coerce_param(&key, &value, decl)?;
            supplied.insert(key, parsed);
        }
    }
    match host.run(source, &base, seed, supplied) {
        Ok(outcome) => Ok(ScriptResult {
            config: Config::wrap(outcome.config),
            animation: outcome.animation.map(|inner| Animation { inner }),
            messages: outcome.messages,
            warnings: outcome.warnings,
        }),
        Err(e) => Err(script_err(e)),
    }
}

/// Every variation name the registry knows, in registration order.
#[pyfunction]
fn variations() -> Vec<String> {
    global_registry().names().to_vec()
}

/// The parameter names a variation accepts (without the `"name."`
/// prefix). Raises for an unknown variation.
#[pyfunction]
fn variation_params(name: &str) -> PyResult<Vec<String>> {
    let registry = global_registry();
    let info = registry
        .get(name)
        .ok_or_else(|| PyValueError::new_err(format!("unknown variation {name:?}")))?;
    Ok(info.parameters.iter().map(|p| p.name.to_string()).collect())
}

#[pymodule]
fn pyfflame(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<Config>()?;
    m.add_class::<ScriptResult>()?;
    m.add_class::<Animation>()?;
    m.add_function(wrap_pyfunction!(run_script, m)?)?;
    m.add_function(wrap_pyfunction!(variations, m)?)?;
    m.add_function(wrap_pyfunction!(variation_params, m)?)?;
    m.add("__version__", env!("CARGO_PKG_VERSION"))?;
    Ok(())
}
