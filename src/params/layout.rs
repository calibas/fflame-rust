//! How a kind's parameters are laid out in its panel, beyond a list
//! (rhai-plugins P7): a parameter, a grid or a grid's column shown only
//! while other parameters hold certain values ([`ShowIf`]), and grids of
//! cells -- rows by columns of scalar parameters, named by a template,
//! drawn as a table ([`ParamGrid`]). A built-in declares its layout in
//! Rust (McCabe's per-scale table, `sim::models::mccabe_layout`); a script
//! in its `PARAMS` (`lang::rhai::param_layout`), whose `type: "grid"`
//! entry the lowering expands into the cells.
//!
//! The cells stay scalar parameters of their own names, so the config,
//! undo, animation and presets see nothing new: a grid is how they are
//! drawn, filled and read.

use crate::scripted::Label;
use std::collections::BTreeMap;
use std::sync::Arc;

/// When something is shown: every named parameter at one of its listed
/// values (rounded, as a choice or a switch is). Empty: always.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ShowIf(pub Vec<(String, Vec<f32>)>);

impl ShowIf {
    /// Whether it holds at these values.
    pub fn holds(&self, value: &dyn Fn(&str) -> f32) -> bool {
        self.0.iter().all(|(name, values)| {
            let v = value(name);
            values.iter().any(|w| (v - w).abs() < 0.5)
        })
    }

    /// The parameters it reads.
    pub fn reads(&self) -> impl Iterator<Item = &str> {
        self.0.iter().map(|(n, _)| n.as_str())
    }
}

/// A text a panel shows: a locale key a built-in resolves when drawn, or
/// a script's label.
#[derive(Clone, Debug)]
pub enum Text {
    Key(&'static str),
    Label(Label),
}

impl Text {
    pub fn resolve(&self) -> String {
        match self {
            Text::Key(k) => t!(*k).to_string(),
            Text::Label(l) => l.resolve(),
        }
    }
}

/// A grid's column: the key its cells' names carry, its header, whether
/// its values are whole numbers (drawn and set so, though stored as the
/// floats they are), and when it shows.
#[derive(Clone, Debug)]
pub struct GridColumn {
    pub key: String,
    pub header: Text,
    pub whole: bool,
    pub show_if: ShowIf,
}

/// A row's values by column key, as a grid's fill gives them.
pub type Row = BTreeMap<String, f32>;

/// A grid's fill: the values of row `row` its other parameters make (a
/// generator's layout), at the values `value` gives.
pub type FillFn = Arc<dyn Fn(&dyn Fn(&str) -> f32, usize) -> Row + Send + Sync>;

/// A grid's warning for a row: its index, its values by column key, the
/// parameters' values, the simulation grid's size, and whether the row is
/// the fill's while the grid is hidden (the panel then says which row);
/// a message, or none.
pub type WarnFn = Arc<dyn Fn(usize, &Row, &dyn Fn(&str) -> f32, (u32, u32), bool) -> Option<String> + Send + Sync>;

/// When a grid fills: an edit that makes `when` hold, where it did not,
/// writes the fill into the cells in the same undo step -- so switching
/// to the grid changes nothing on screen. Where the grid is hidden, the
/// fill is what its warnings read.
#[derive(Clone)]
pub struct GridFill {
    pub when: ShowIf,
    pub cells: FillFn,
}

/// A grid of cells: `rows` rows, the first `live_rows` of them live (a
/// parameter's value, clamped; all when none names it), by its columns.
#[derive(Clone)]
pub struct ParamGrid {
    pub name: String,
    /// The cells' names: `{row}` and `{column}` replaced (`s{row}_{column}`).
    pub cell: String,
    pub rows: usize,
    pub live_rows: Option<String>,
    pub columns: Vec<GridColumn>,
    pub show_if: ShowIf,
    pub fill: Option<GridFill>,
    pub warn: Option<WarnFn>,
    /// Drawn as a grid; else another section draws its cells (McCabe's
    /// per-scale warps, the Warp section's), and it is how they are read.
    pub drawn: bool,
}

impl ParamGrid {
    /// The parameter of row `row`, column `column`.
    pub fn cell(&self, row: usize, column: &str) -> String {
        self.cell.replace("{row}", &row.to_string()).replace("{column}", column)
    }

    /// Whether `name` is one of its cells.
    pub fn is_cell(&self, name: &str) -> bool {
        (0..self.rows).any(|r| self.columns.iter().any(|c| self.cell(r, &c.key) == name))
    }

    /// The rows live at these values.
    pub fn live(&self, value: &dyn Fn(&str) -> f32) -> usize {
        match &self.live_rows {
            Some(p) => (value(p).round().max(1.0) as usize).min(self.rows),
            None => self.rows,
        }
    }

    /// Row `row` as the cells hold it.
    pub fn row(&self, row: usize, value: &dyn Fn(&str) -> f32) -> Row {
        self.columns.iter().map(|c| (c.key.clone(), value(&self.cell(row, &c.key)))).collect()
    }

    /// The cell values an edit from `before` to `after` writes beside it:
    /// the fill, every row, when the edit makes its `when` hold.
    pub fn fill_on(&self, before: &dyn Fn(&str) -> f32, after: &dyn Fn(&str) -> f32) -> Vec<(String, f32)> {
        let Some(f) = &self.fill else { return Vec::new() };
        if f.when.holds(before) || !f.when.holds(after) {
            return Vec::new();
        }
        (0..self.rows).flat_map(|r| (f.cells)(after, r).into_iter().map(move |(c, v)| (r, c, v))).map(|(r, c, v)| (self.cell(r, &c), v)).collect()
    }
}

/// A kind's layout: the parameters shown only while others hold certain
/// values, and its grids, drawn after its other parameters.
#[derive(Clone, Default)]
pub struct ParamLayout {
    pub show_if: Vec<(String, ShowIf)>,
    pub grids: Vec<ParamGrid>,
}

impl ParamLayout {
    /// Whether parameter `name` is drawn in the list: not a grid's cell,
    /// and its condition holds.
    pub fn lists(&self, name: &str, value: &dyn Fn(&str) -> f32) -> bool {
        !self.grids.iter().any(|g| g.is_cell(name)) && self.show_if.iter().filter(|(n, _)| n == name).all(|(_, s)| s.holds(value))
    }

    /// Every cell an edit of one parameter from `before` to `after` fills.
    pub fn fills(&self, before: &dyn Fn(&str) -> f32, after: &dyn Fn(&str) -> f32) -> Vec<(String, f32)> {
        self.grids.iter().flat_map(|g| g.fill_on(before, after)).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn grid() -> ParamGrid {
        let col = |k: &str, show_if: ShowIf| GridColumn { key: k.into(), header: Text::Label(Label::text(k)), whole: false, show_if };
        ParamGrid {
            name: "table".into(),
            cell: "s{row}_{column}".into(),
            rows: 3,
            live_rows: Some("n".into()),
            columns: vec![col("a", ShowIf::default()), col("b", ShowIf(vec![("mode".into(), vec![2.0])]))],
            show_if: ShowIf(vec![("layout".into(), vec![1.0])]),
            fill: Some(GridFill { when: ShowIf(vec![("layout".into(), vec![1.0])]), cells: Arc::new(|v, r| [("a".to_string(), v("base") * (r + 1) as f32)].into()) }),
            warn: None,
            drawn: true,
        }
    }

    /// A grid's cells by name, its live rows, a condition, and its fill
    /// on the edit that turns it on (only that one).
    #[test]
    fn a_grid_names_its_cells_and_fills_when_switched_on() {
        let g = grid();
        assert_eq!(g.cell(2, "b"), "s2_b");
        assert!(g.is_cell("s0_a") && !g.is_cell("s3_a") && !g.is_cell("base"));
        let at = |pairs: &'static [(&'static str, f32)]| move |n: &str| pairs.iter().find(|(k, _)| *k == n).map_or(0.0, |(_, v)| *v);
        assert_eq!(g.live(&at(&[("n", 2.4)])), 2);
        assert_eq!(g.live(&at(&[("n", 9.0)])), 3);
        assert!(!g.columns[1].show_if.holds(&at(&[("mode", 1.0)])) && g.columns[1].show_if.holds(&at(&[("mode", 2.0)])));
        let off = at(&[("layout", 0.0), ("base", 2.0)]);
        let on = at(&[("layout", 1.0), ("base", 2.0)]);
        assert_eq!(g.fill_on(&off, &on), vec![("s0_a".into(), 2.0), ("s1_a".into(), 4.0), ("s2_a".into(), 6.0)]);
        assert!(g.fill_on(&on, &on).is_empty(), "only the edit that turns it on");
        let layout = ParamLayout { show_if: vec![("base".into(), ShowIf(vec![("layout".into(), vec![0.0])]))], grids: vec![g] };
        assert!(layout.lists("base", &off) && !layout.lists("base", &on) && !layout.lists("s1_a", &off) && layout.lists("layout", &on));
    }
}
