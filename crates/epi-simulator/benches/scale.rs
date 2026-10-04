//! Scale timings for the Gillespie engine.
//!
//! Run with `cargo bench -p epi-simulator`.
//! Set `CYBEREPI_BENCH_SCALE=10000` to move past the default 1,000 entities.
//! 100,000 and 1,000,000 are supported; time them on the homelab, not in CI.

use epi_controls::ControlSet;
use epi_core::{ModelPreset, SeedStrategy, SimDuration, TransmissionLaw};
use epi_generator::{generate, GeneratorConfig, OrgTemplate};
use epi_models::{compile, CompileInputs, TimingOverrides};
use epi_pathogens::builtin;
use epi_simulator::{run_once, RunOptions};
use std::time::Instant;

fn main() {
    let scale: u32 = std::env::var("CYBEREPI_BENCH_SCALE")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(1_000);
    let config = GeneratorConfig {
        template: OrgTemplate::MediumEnterprise,
        size: scale.max(32),
        seed: 1,
        name: "bench".into(),
    };
    let started = Instant::now();
    let graph = generate(&config).expect("population");
    let generated = started.elapsed();
    let pathogen = builtin("IdentityStealer").expect("pathogen");
    let timing = TimingOverrides::default();
    let model = compile(&CompileInputs {
        graph: &graph,
        pathogen: &pathogen,
        controls: &ControlSet::default(),
        preset: ModelPreset::Seidqrp,
        law: TransmissionLaw::ClampedProduct,
        timing: &timing,
        coverage_seed: 1,
    })
    .expect("compile");
    let compiled = started.elapsed();
    let summary = run_once(
        model,
        RunOptions {
            duration: SimDuration::days(7),
            initial_infected: 1,
            strategy: SeedStrategy::Random,
            ..RunOptions::default()
        },
        1_234,
    );
    let finished = started.elapsed();
    println!(
        "entities={} edges={} generate={generated:?} compile={compiled:?} simulate={finished:?} outbreak={} r0={:.2}",
        graph.node_count(),
        graph.edge_count(),
        summary.outbreak_size,
        summary.r0
    );
}
