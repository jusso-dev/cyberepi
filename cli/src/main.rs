//! CyberEpi command line.
//!
//! `cyberepi simulate` runs entirely in-process. It does not require Kubernetes,
//! NATS, or PostgreSQL, and it does not touch a real environment.

use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand};
use epi_controls::catalog;
use epi_core::VERSION;
use epi_generator::{generate, summarise, GeneratorConfig, OrgTemplate, TEMPLATE_IDS};
use epi_pathogens::builtins;
use epi_scenario::{execute, load_scenario, load_scenario_str, validate_scenario, ExecuteOptions};
use epi_simulator::{render_comparison_csv, render_report};
use std::path::PathBuf;

#[derive(Parser)]
#[command(
    name = "cyberepi",
    version = VERSION,
    about = "CyberEpi digital outbreak laboratory"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Run a scenario locally and print epidemiological results.
    Simulate {
        scenario: PathBuf,
        #[arg(long)]
        runs: Option<u32>,
        #[arg(long)]
        seed: Option<u64>,
        #[arg(long)]
        threads: Option<usize>,
        #[arg(long, default_value = "text")]
        format: String,
    },
    /// Validate or inspect a scenario document.
    Scenario {
        #[command(subcommand)]
        action: ScenarioCommand,
    },
    /// Run an experiment locally and optionally write a JSON bundle.
    Experiment {
        #[command(subcommand)]
        action: ExperimentCommand,
    },
    /// Generate a synthetic organisation summary.
    Generate {
        #[command(subcommand)]
        action: GenerateCommand,
    },
    /// Report local cluster tooling. Live import is read-only.
    Cluster {
        #[command(subcommand)]
        action: ClusterCommand,
    },
}

#[derive(Subcommand)]
enum ScenarioCommand {
    /// Validate a scenario against the schema and model rules.
    Validate { scenario: PathBuf },
}

#[derive(Subcommand)]
enum ExperimentCommand {
    /// Same engine as `simulate`, with a JSON artefact for later inspection.
    Run {
        scenario: PathBuf,
        #[arg(long)]
        out: Option<PathBuf>,
        #[arg(long)]
        runs: Option<u32>,
        #[arg(long)]
        threads: Option<usize>,
    },
    /// Print a previously saved experiment bundle.
    Results { bundle: PathBuf },
    /// Show the status stored in a local bundle.
    Status { bundle: PathBuf },
}

#[derive(Subcommand)]
enum GenerateCommand {
    /// Build a synthetic organisation and print its summary.
    Organisation {
        #[arg(long)]
        template: String,
        #[arg(long, default_value_t = 500)]
        users: u32,
        #[arg(long, default_value_t = 1)]
        seed: u64,
        #[arg(long)]
        name: Option<String>,
    },
}

#[derive(Subcommand)]
enum ClusterCommand {
    /// Describe whether a kubeconfig is available. Does not mutate the cluster.
    Status,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Command::Simulate {
            scenario,
            runs,
            seed,
            threads,
            format,
        } => {
            let loaded = load_scenario(&scenario)?;
            let report = execute(
                &loaded,
                &ExecuteOptions {
                    runs_override: runs,
                    seed_override: seed,
                    threads,
                    progress: true,
                },
            )?;
            match format.as_str() {
                "text" => print!("{}", render_report(&report)),
                "json" => println!("{}", serde_json::to_string_pretty(&report)?),
                "csv" => print!("{}", render_comparison_csv(&report)),
                "yaml" => println!("{}", serde_yaml::to_string(&loaded)?),
                other => bail!("unknown format `{other}` (text, json, csv, yaml)"),
            }
        }
        Command::Scenario {
            action: ScenarioCommand::Validate { scenario },
        } => {
            let text = std::fs::read_to_string(&scenario)
                .with_context(|| format!("read {}", scenario.display()))?;
            let loaded = load_scenario_str(&text)?;
            validate_scenario(&loaded)?;
            println!("valid: {}", loaded.metadata.name);
        }
        Command::Experiment {
            action:
                ExperimentCommand::Run {
                    scenario,
                    out,
                    runs,
                    threads,
                },
        } => {
            let loaded = load_scenario(&scenario)?;
            let report = execute(
                &loaded,
                &ExecuteOptions {
                    runs_override: runs,
                    seed_override: None,
                    threads,
                    progress: true,
                },
            )?;
            print!("{}", render_report(&report));
            if let Some(path) = out {
                std::fs::write(&path, serde_json::to_vec_pretty(&report)?)
                    .with_context(|| format!("write {}", path.display()))?;
                eprintln!("wrote {}", path.display());
            }
        }
        Command::Experiment {
            action: ExperimentCommand::Results { bundle } | ExperimentCommand::Status { bundle },
        } => {
            let text = std::fs::read_to_string(&bundle)
                .with_context(|| format!("read {}", bundle.display()))?;
            let report: epi_simulator::SimulationReport = serde_json::from_str(&text)?;
            println!(
                "{}  status=completed  runs={}  scenario={}",
                report.provenance.scenario_hash,
                report.variants.first().map(|v| v.runs).unwrap_or(0),
                report
                    .variants
                    .first()
                    .map(|v| v.name.as_str())
                    .unwrap_or("none"),
            );
            print!("{}", render_report(&report));
        }
        Command::Generate {
            action:
                GenerateCommand::Organisation {
                    template,
                    users,
                    seed,
                    name,
                },
        } => {
            let kind = OrgTemplate::parse(&template).with_context(|| {
                format!(
                    "unknown template `{template}`. Known: {}",
                    TEMPLATE_IDS.join(", ")
                )
            })?;
            // `--users` is the population-size hint from the documented command.
            let size = users.max(32);
            let config = GeneratorConfig {
                template: kind,
                size,
                seed,
                name: name.unwrap_or_else(|| format!("Synthetic {}", kind.label())),
            };
            let graph = generate(&config)?;
            let summary = summarise(&config, &graph);
            println!("{}", serde_json::to_string_pretty(&summary)?);
        }
        Command::Cluster {
            action: ClusterCommand::Status,
        } => {
            println!("CyberEpi {VERSION}");
            println!("Infrastructure cluster and simulated population are separate.");
            match std::env::var("KUBECONFIG") {
                Ok(path) => println!("KUBECONFIG={path}"),
                Err(_) => println!(
                    "KUBECONFIG is unset; kubectl will use the default kubeconfig if present."
                ),
            }
            println!("Live topology import is read-only and is performed by `cyberepi-controller`, not by simulating attacks.");
            println!("Pathogens: {}", builtins().len());
            println!("Controls: {}", catalog().len());
        }
    }
    Ok(())
}
