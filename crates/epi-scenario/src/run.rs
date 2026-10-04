use crate::parse::{Scenario, ScenarioError};
use crate::scenario_hash;
use epi_controls::ControlSet;
use epi_core::{EventMode, PolicyKind, VERSION};
use epi_generator::{generate, GeneratorConfig};
use epi_models::{compile, CompileInputs, TimingOverrides};
use epi_pathogens::builtin;
use epi_simulator::{
    aggregate, render_report, run_monte_carlo, Provenance, RunOptions, SimulationReport,
};
use std::sync::Arc;

#[derive(Default)]
pub struct ExecuteOptions {
    pub runs_override: Option<u32>,
    pub seed_override: Option<u64>,
    pub threads: Option<usize>,
    pub progress: bool,
}

pub fn execute(
    scenario: &Scenario,
    options: &ExecuteOptions,
) -> Result<SimulationReport, ScenarioError> {
    let population_name = if scenario.population.name.is_empty() {
        format!("Synthetic {}", scenario.population.template.label())
    } else {
        scenario.population.name.clone()
    };
    let graph = generate(&GeneratorConfig {
        template: scenario.population.template,
        size: scenario.population.size,
        seed: scenario.population.seed,
        name: population_name,
    })?;
    let pathogen =
        builtin(&scenario.pathogen.kind).map_err(|err| ScenarioError::Message(err.to_string()))?;
    let runs = options.runs_override.unwrap_or(scenario.simulation.runs);
    let seed = options.seed_override.unwrap_or(scenario.simulation.seed);
    let threads = options
        .threads
        .or(scenario.simulation.threads)
        .unwrap_or_else(|| {
            std::thread::available_parallelism()
                .map(|n| n.get())
                .unwrap_or(1)
        });
    let timing = TimingOverrides {
        detection_delay: scenario.simulation.detection_delay,
        isolation_delay: scenario.simulation.isolation_delay,
        recovery_time: scenario.simulation.recovery_time,
    };
    let variants = if scenario.variants.is_empty() {
        vec![VariantRun {
            name: scenario.metadata.name.clone(),
            controls: scenario.controls.clone(),
            policy: scenario.defender.policy,
        }]
    } else {
        scenario
            .variants
            .iter()
            .map(|variant| VariantRun {
                name: variant.name.clone(),
                controls: scenario.controls.merge(&variant.controls),
                policy: variant.policy.unwrap_or(scenario.defender.policy),
            })
            .collect()
    };

    let mut reports = Vec::with_capacity(variants.len());
    for variant in &variants {
        if options.progress {
            eprintln!(
                "variant `{}`: {runs} runs on {} entities",
                variant.name,
                graph.node_count()
            );
        }
        let compiled = compile(&CompileInputs {
            graph: &graph,
            pathogen: &pathogen,
            controls: &variant.controls,
            preset: scenario.model,
            law: scenario.simulation.transmission,
            timing: &timing,
            coverage_seed: scenario.population.seed,
        })?;
        let run_options = RunOptions {
            algorithm: scenario.simulation.algorithm,
            duration: scenario.simulation.duration,
            initial_infected: scenario.initial_conditions.infected_entities,
            strategy: scenario.initial_conditions.strategy,
            event_mode: if runs > 1 && scenario.simulation.event_mode == EventMode::Full {
                EventMode::Sampled
            } else {
                scenario.simulation.event_mode
            },
            sample_every: scenario.simulation.sample_interval,
            dt: scenario.simulation.dt,
            policy: variant.policy,
            extinction_fraction: scenario.simulation.extinction_threshold_fraction,
            extinction_minimum: 10,
        };
        let progress_flag = options.progress;
        let summaries = run_monte_carlo(
            Arc::clone(&compiled),
            run_options,
            runs,
            seed,
            threads,
            &|finished| {
                if progress_flag {
                    eprintln!("  {finished}/{runs}");
                }
            },
        );
        reports.push(aggregate(&variant.name, &compiled, &graph, &summaries));
    }

    Ok(SimulationReport {
        population: graph.node_count() as u32,
        initial_compromises: scenario.initial_conditions.infected_entities,
        duration_seconds: scenario.simulation.duration.as_secs_f64(),
        algorithm: scenario.simulation.algorithm.label().to_string(),
        pathogen: pathogen.name.clone(),
        provenance: Provenance {
            cyberepi_version: VERSION.to_string(),
            scenario_hash: scenario_hash(scenario),
            seed,
            algorithm: scenario.simulation.algorithm.label().to_string(),
            pathogen_id: pathogen.id,
            model: scenario.model.label().to_string(),
            generator_version: epi_core::GENERATOR_VERSION.to_string(),
            model_version: epi_core::MODEL_VERSION.to_string(),
            timestamp: chrono::Utc::now().to_rfc3339(),
            worker_version: VERSION.to_string(),
        },
        variants: reports,
    })
}

struct VariantRun {
    name: String,
    controls: ControlSet,
    policy: PolicyKind,
}

/// Run one Monte Carlo slice for a single variant. Used by distributed workers.
pub fn run_slice(
    scenario: &Scenario,
    variant_name: &str,
    offset: u32,
    count: u32,
    base_seed: u64,
    threads: usize,
) -> Result<SliceOutput, ScenarioError> {
    run_slice_inner(
        scenario,
        variant_name,
        offset,
        count,
        base_seed,
        threads,
        false,
    )
}

pub fn run_trace(
    scenario: &Scenario,
    variant_name: &str,
    run_index: u32,
    base_seed: u64,
) -> Result<epi_simulator::RunSummary, ScenarioError> {
    let mut output = run_slice_inner(scenario, variant_name, run_index, 1, base_seed, 1, true)?;
    output
        .runs
        .pop()
        .ok_or_else(|| ScenarioError::Message("trace produced no run".into()))
}

fn run_slice_inner(
    scenario: &Scenario,
    variant_name: &str,
    offset: u32,
    count: u32,
    base_seed: u64,
    threads: usize,
    record_events: bool,
) -> Result<SliceOutput, ScenarioError> {
    let population_name = if scenario.population.name.is_empty() {
        format!("Synthetic {}", scenario.population.template.label())
    } else {
        scenario.population.name.clone()
    };
    let graph = generate(&GeneratorConfig {
        template: scenario.population.template,
        size: scenario.population.size,
        seed: scenario.population.seed,
        name: population_name,
    })?;
    let pathogen =
        builtin(&scenario.pathogen.kind).map_err(|err| ScenarioError::Message(err.to_string()))?;
    let controls = if scenario.variants.is_empty() {
        scenario.controls.clone()
    } else {
        scenario
            .variants
            .iter()
            .find(|variant| variant.name == variant_name)
            .map(|variant| scenario.controls.merge(&variant.controls))
            .ok_or_else(|| ScenarioError::Message(format!("unknown variant `{variant_name}`")))?
    };
    let policy = scenario
        .variants
        .iter()
        .find(|variant| variant.name == variant_name)
        .and_then(|variant| variant.policy)
        .unwrap_or(scenario.defender.policy);
    let timing = TimingOverrides {
        detection_delay: scenario.simulation.detection_delay,
        isolation_delay: scenario.simulation.isolation_delay,
        recovery_time: scenario.simulation.recovery_time,
    };
    let compiled = compile(&CompileInputs {
        graph: &graph,
        pathogen: &pathogen,
        controls: &controls,
        preset: scenario.model,
        law: scenario.simulation.transmission,
        timing: &timing,
        coverage_seed: scenario.population.seed,
    })?;
    let summaries = run_monte_carlo(
        Arc::clone(&compiled),
        RunOptions {
            algorithm: scenario.simulation.algorithm,
            duration: scenario.simulation.duration,
            initial_infected: scenario.initial_conditions.infected_entities,
            strategy: scenario.initial_conditions.strategy,
            event_mode: if record_events {
                EventMode::Full
            } else {
                EventMode::Summary
            },
            sample_every: scenario.simulation.sample_interval,
            dt: scenario.simulation.dt,
            policy,
            extinction_fraction: scenario.simulation.extinction_threshold_fraction,
            extinction_minimum: 10,
        },
        count,
        base_seed.wrapping_add(u64::from(offset)),
        threads.max(1),
        &|_| {},
    );
    let mut secondary_sum = vec![0u64; graph.node_count()];
    for summary in &summaries {
        for (index, count) in summary.secondary.iter().enumerate() {
            if index < secondary_sum.len() {
                secondary_sum[index] = secondary_sum[index].saturating_add(u64::from(*count));
            }
        }
    }
    let runs = summaries
        .into_iter()
        .map(|mut summary| {
            summary.secondary.clear();
            if !record_events {
                summary.parent.clear();
                summary.events.clear();
            }
            summary
        })
        .collect();
    Ok(SliceOutput {
        runs,
        secondary_sum,
    })
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct SliceOutput {
    pub runs: Vec<epi_simulator::RunSummary>,
    pub secondary_sum: Vec<u64>,
}

pub fn render(scenario: &Scenario, options: &ExecuteOptions) -> Result<String, ScenarioError> {
    let report = execute(scenario, options)?;
    Ok(render_report(&report))
}
