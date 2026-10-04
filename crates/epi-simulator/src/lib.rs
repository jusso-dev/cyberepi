#![allow(clippy::needless_range_loop)]
//! Stochastic cyber-epidemiology simulator.
//!
//! Propagation is a continuous-time or discrete-time Markov chain on a contact
//! graph. The engine does not execute payloads, scan networks, or change any
//! system outside the process.

mod engine;
mod rate_tree;
mod report;

pub use engine::{run_monte_carlo, run_once, CurveSample, RunOptions, RunSummary, SimEvent};
pub use report::{
    aggregate, render_comparison_csv, render_report, Provenance, SimulationReport, VariantReport,
};
