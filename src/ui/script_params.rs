//! Rendering a script's declared parameters, by the one parameter
//! renderer (`ui::params`; docs/archive/projects/one-parameter-system.md) with
//! the script's run inputs as its store ([`super::params::LocalStore`]).
//!
//! Shared, because the Scripts panel is no longer the only surface that
//! runs scripts: the Palette Editor offers the palette-flagged ones, and
//! anything else that grows a "run a script" section will want the same
//! controls rather than its own near-copy.

use std::collections::HashMap;

use egui;

use crate::script::{ParamValue, ScriptMeta};

/// Draw a control per declared parameter, writing edits into `values`.
///
/// Values are keyed by parameter name and left untouched where the user
/// has not set one, so a script keeps its own defaults.
pub fn render(ui: &mut egui::Ui, meta: &ScriptMeta, values: &mut HashMap<String, ParamValue>) {
    if meta.params.is_empty() {
        ui.label(egui::RichText::new("This script has no settings.").weak());
        return;
    }
    super::params::list(ui, &mut super::params::LocalStore(values), &super::params::List::new(&meta.params, "script"));
}
