//! Scenario documents: YAML in, a reproducible experiment out.
//!
//! A scenario describes a synthetic population and a mathematical outbreak.
//! Loading one never contacts a real network.

mod parse;
mod run;

pub use parse::{
    load_scenario, load_scenario_str, scenario_hash, validate_scenario, InitialConditions,
    Metadata, PathogenRef, PopulationSpec, Scenario, ScenarioError, SimulationSpec, VariantSpec,
};
pub use run::{execute, render, run_slice, run_trace, ExecuteOptions, SliceOutput};
