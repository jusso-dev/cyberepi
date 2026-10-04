#![allow(clippy::needless_range_loop)]
//! Transmission laws, compartment presets, and the cyber reproduction number.
//!
//! The mathematics are documented in `docs/modelling`. Nothing in this crate
//! performs an action on a real network.

mod compile;
mod policy;
mod reproduction;
mod transmission;

pub use compile::{
    compile, CompileError, CompileInputs, CompiledModel, NodeRates, TimingOverrides,
};
pub use policy::{Decision, DefenderPolicy, Observation, PolicyKindImpl};
pub use reproduction::{
    expected_secondary, instantaneous_rt, invasion_threshold, pair_infection_probability, Phase,
    Reproduction,
};
pub use transmission::{
    environmental_modifier, privilege_modifier, probability_in_unit_interval, raw_product,
    transmission_hazard_per_day, transmission_probability, ClampedProduct, ExponentialSaturation,
    TransmissionContext, TransmissionStrategy,
};
