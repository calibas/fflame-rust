//! A variation's parameters, wherever they live -- a transform in the
//! flame, an escape transformation, the camera lens -- drawn by the one
//! parameter renderer (`ui::params`; docs/projects/one-parameter-system.md).
//! What is a variation's own here is where its values are read and
//! written ([`ParamTarget`]).

use crate::{
    config::{ConfigManager, ConfigPath, TransformRef, UpdateType},
    params::ParamDef,
};

/// Where a variation's parameters are read from and written to: the
/// current value, and the `ConfigPath` an edit writes. `TransformRef` is
/// one implementation; the escape engine's camera lens is another,
/// reading a name-keyed map on the escape config instead of a transform
/// in a flame.
pub trait ParamTarget {
    /// The value to show, or `None` if the target has gone (a pool
    /// member deleted under the panel), in which case rendering stops
    /// rather than inventing one.
    fn stored(&self, config_manager: &ConfigManager, var_name: &str, param: &ParamDef) -> Option<f32>;

    /// The path an edit writes to.
    fn path(&self, var_name: &str, param_name: &str) -> ConfigPath;
}

impl ParamTarget for TransformRef {
    fn stored(&self, config_manager: &ConfigManager, var_name: &str, param: &ParamDef) -> Option<f32> {
        // Routes through `active_flame()` -- when editing a subflame,
        // that is the subflame's slot; otherwise the main flame.
        let transform = self.get(config_manager.active_flame())?;
        Some(transform.get_variation_param_or_default(var_name, &param.name, &crate::variations::global_registry()))
    }

    fn path(&self, var_name: &str, param_name: &str) -> ConfigPath {
        self.variation_param_path(var_name.to_string(), param_name.to_string())
    }
}

/// Draw a variation's parameters on `target` (a pool member's, an escape
/// transformation's, the lens's): the largest update the edits asked for.
pub fn render_variation_params(
    ui: &mut egui::Ui,
    config_manager: &mut ConfigManager,
    target: &dyn ParamTarget,
    var_name: &str,
    parameters: &[ParamDef],
) -> UpdateType {
    // A Rhai variation's conditions and grids.
    #[cfg(feature = "lang")]
    let layout = crate::variations::custom::param_layout(var_name);
    #[cfg(not(feature = "lang"))]
    let layout: Option<std::sync::Arc<crate::params::layout::ParamLayout>> = None;
    let mut store = super::params::ConfigStore::new(config_manager, |m, d| target.stored(m, var_name, d), |n| target.path(var_name, n));
    super::params::list(ui, &mut store, &super::params::List::new(parameters, var_name).layout(layout.as_deref()));
    store.update
}

/// The escape engine's **camera lens**.
///
/// Its parameters live in a name-keyed map on the escape config
/// rather than in a transform, so an absent one reads the registry's
/// default here. `ConfigManager::get_value` would report 0.0 instead
/// -- deliberately, since a def's default is a registry concern -- and
/// resolving it at the panel is the same dance the formula and
/// coloring params already do.
pub struct LensTarget;

impl ParamTarget for LensTarget {
    fn stored(
        &self,
        config_manager: &ConfigManager,
        _var_name: &str,
        param: &ParamDef,
    ) -> Option<f32> {
        Some(
            config_manager
                .active_config()
                .escape
                .lens_params
                .get(&*param.name)
                .copied()
                .unwrap_or(param.slot_default()),
        )
    }

    fn path(&self, _var_name: &str, param_name: &str) -> ConfigPath {
        ConfigPath::EscapeLensParam {
            param: param_name.to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{ConfigManager, FractalConfig, TransformRef};

    /// Store `thickness` on a flame, draw the parameter section once, and
    /// report what's left in the config afterwards.
    fn stored_after_drawing(thickness: f32) -> f32 {
        let mut config = FractalConfig::default();
        if config.flame.transforms.is_empty() {
            config.flame.transforms.push(Default::default());
        }
        let xform = &mut config.flame.transforms[0];
        xform.variations.insert("lsystem_path_3D".to_string(), 1.0);
        xform
            .variation_params
            .insert("lsystem_path_3D.thickness".to_string(), thickness);

        let mut manager = ConfigManager::new(config);
        let registry = crate::variations::global_registry();
        let params = registry.get("lsystem_path_3D").unwrap().parameters.clone();

        egui::__run_test_ui(|ui| {
            render_variation_params(
                ui,
                &mut manager,
                &TransformRef::Normal(0),
                "lsystem_path_3D",
                &params,
            );
        });

        manager.active_config().flame.transforms[0].variation_params
            ["lsystem_path_3D.thickness"]
    }

    /// Drawing a parameter section must never edit the flame.
    ///
    /// Regression: the Float slider carried `step_by(0.01)`, which rounds
    /// the value it is handed, so merely expanding "L-System Path 3D"
    /// rewrote Thickness 0.006 → 0.01 and doubled the rendered line width.
    /// The export path, which never draws the panel, kept 0.006 — which is
    /// what made the two disagree.
    #[test]
    fn drawing_params_keeps_values_finer_than_a_slider_step() {
        assert_eq!(stored_after_drawing(0.006), 0.006);
    }

    /// The UI min/max are a suggested editing range, not a constraint on
    /// what a flame may hold — scripts and `.flame` imports both write
    /// values outside it. egui's default `SliderClamping::Always` clamps
    /// "even existing ones", so displaying such a param used to silently
    /// rewrite it; `Edits` restricts what the user can enter instead.
    #[test]
    fn drawing_params_keeps_values_outside_the_slider_range() {
        // Thickness is declared 0.0..=0.2.
        assert_eq!(stored_after_drawing(0.5), 0.5);
    }
}

#[cfg(test)]
mod target_tests {
    use super::*;
    use crate::config::{ConfigManager, ConfigValue};

    /// The lens target reads the REGISTRY default for a parameter the
    /// config has never stored -- and so, now, does the manager's own
    /// path: it answered 0.0, and undoing a first edit wrote that 0
    /// back, slamming the parameter to zero.
    #[test]
    fn an_unset_lens_parameter_reads_its_registry_default() {
        let registry = crate::variations::global_registry();
        let mut config = crate::config::FractalConfig::default();
        config.escape.lens = "curl".to_string();
        let cm = ConfigManager::new(config);

        let info = registry.get("curl").expect("curl");
        let c1 = info
            .parameters
            .iter()
            .find(|p| p.name == "c1")
            .expect("c1");
        assert_eq!(c1.slot_default(), 1.0, "curl's c1 default moved");

        let got = LensTarget
            .stored(&cm, "curl", c1)
            .expect("a value");
        assert_eq!(got, 1.0, "an unset parameter did not read its default");

        // And the config path agrees.
        let raw = cm.get_value(&ConfigPath::EscapeLensParam { param: "c1".into() });
        assert_eq!(raw.ok(), Some(ConfigValue::Float(1.0)));
    }

    /// An edit round-trips: the path the target hands back is the one
    /// the manager writes, and the target reads that write back.
    #[test]
    fn a_lens_parameter_edit_round_trips() {
        let registry = crate::variations::global_registry();
        let mut config = crate::config::FractalConfig::default();
        config.escape.lens = "curl".to_string();
        // Seeded, as `lens_choice` seeds it when a lens is picked --
        // without that an undo restores the absent-reads-zero value
        // rather than the default, which is the wart the seeding
        // exists to remove.
        config.escape.lens_params.insert("c1".to_string(), 1.0);
        let mut cm = ConfigManager::new(config);
        let info = registry.get("curl").expect("curl");
        let c1 = info.parameters.iter().find(|p| p.name == "c1").unwrap();

        let path = LensTarget.path("curl", "c1");
        assert_eq!(path, ConfigPath::EscapeLensParam { param: "c1".into() });
        cm.update_param(path, 0.25f32.into()).expect("write");

        assert_eq!(LensTarget.stored(&cm, "curl", c1), Some(0.25));
        assert_eq!(cm.active_config().escape.lens_params.get("c1"), Some(&0.25));

        // Undo must put it back, which is what routing through the
        // manager rather than the config buys.
        cm.undo().expect("undo");
        assert_eq!(LensTarget.stored(&cm, "curl", c1), Some(1.0));
    }

    /// A transform target still behaves exactly as it did before the
    /// trait existed.
    #[test]
    fn a_transform_target_is_unchanged() {
        use crate::config::TransformRef;
        let registry = crate::variations::global_registry();
        let mut config = crate::config::FractalConfig::default();
        if config.flame.transforms.is_empty() {
            config.flame.transforms.push(Default::default());
        }
        config.flame.transforms[0].set_variation("curl", 1.0);
        let cm = ConfigManager::new(config);
        let xref = TransformRef::Normal(0);
        let info = registry.get("curl").expect("curl");
        let c1 = info.parameters.iter().find(|p| p.name == "c1").unwrap();

        assert_eq!(
            TransformRef::Normal(0).path("curl", "c1"),
            ConfigPath::TransformVariationParam {
                index: 0,
                variation: "curl".into(),
                param: "c1".into()
            }
        );
        // Unset reads the registry default here too, via the
        // transform's own resolver.
        assert_eq!(xref.stored(&cm, "curl", c1), Some(1.0));
    }
}
