//! One parameter system for every kind that has parameters -- flame
//! variations, effects, escape formulas and colourings, simulations,
//! CPU scripts (docs/projects/one-parameter-system.md).

#[cfg(all(test, feature = "engine-flame", feature = "engine-escape", feature = "engine-sim"))]
mod snapshot;
