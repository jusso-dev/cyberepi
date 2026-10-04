//! Text, CSV, and aggregate reports for Monte Carlo experiments.

use crate::engine::{CurveSample, RunSummary};
use epi_core::{format_count, format_sim_duration, StateCounts};
use epi_graph::PopulationGraph;
use epi_metrics::{centrality, rank_superspreaders, summarize_runs, Histogram, SuperspreaderScore};
use epi_models::CompiledModel;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Provenance {
    pub cyberepi_version: String,
    pub scenario_hash: String,
    pub seed: u64,
    pub algorithm: String,
    pub pathogen_id: String,
    pub model: String,
    pub generator_version: String,
    pub model_version: String,
    pub timestamp: String,
    pub worker_version: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct VariantReport {
    pub name: String,
    pub runs: u32,
    pub r0: f64,
    pub invasion_threshold: f64,
    pub median_peak: f64,
    pub median_attack_rate: f64,
    pub median_outbreak: f64,
    pub mean_outbreak: f64,
    pub p95_outbreak: f64,
    pub p99_outbreak: f64,
    pub extinction_probability: f64,
    pub median_containment_seconds: f64,
    pub median_rt_below_seconds: Option<f64>,
    pub mean_time_to_detection_seconds: Option<f64>,
    pub mean_time_to_isolation_seconds: Option<f64>,
    pub mean_time_to_recovery_seconds: Option<f64>,
    pub final_counts: StateCounts,
    pub superspreaders: Vec<SuperspreaderScore>,
    pub curve: Vec<CurveSample>,
    pub outbreak_sizes: Vec<u32>,
    pub histogram: Histogram,
    pub run_seeds: Vec<u64>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SimulationReport {
    pub population: u32,
    pub initial_compromises: u32,
    pub duration_seconds: f64,
    pub algorithm: String,
    pub pathogen: String,
    pub provenance: Provenance,
    pub variants: Vec<VariantReport>,
}

pub fn aggregate(
    name: &str,
    model: &CompiledModel,
    graph: &PopulationGraph,
    runs: &[RunSummary],
) -> VariantReport {
    let mut peaks = Vec::with_capacity(runs.len());
    let mut attacks = Vec::with_capacity(runs.len());
    let mut outbreaks = Vec::with_capacity(runs.len());
    let mut containment = Vec::with_capacity(runs.len());
    let mut crossings = Vec::new();
    let mut extinct = 0u32;
    let mut detect = Vec::new();
    let mut isolate = Vec::new();
    let mut recover = Vec::new();
    let mut secondary = vec![0.0; model.node_count()];
    for run in runs {
        peaks.push(f64::from(run.peak_prevalence));
        attacks.push(run.attack_rate);
        outbreaks.push(f64::from(run.outbreak_size));
        if let Some(seconds) = run.containment_seconds {
            containment.push(seconds);
        }
        if let Some(seconds) = run.rt_below_one_seconds {
            crossings.push(seconds);
        }
        if run.extinct {
            extinct += 1;
        }
        if let Some(value) = run.mean_time_to_detection_seconds {
            detect.push(value);
        }
        if let Some(value) = run.mean_time_to_isolation_seconds {
            isolate.push(value);
        }
        if let Some(value) = run.mean_time_to_recovery_seconds {
            recover.push(value);
        }
        for (index, count) in run.secondary.iter().enumerate() {
            if index < secondary.len() {
                secondary[index] += f64::from(*count);
            }
        }
    }
    let denom = (runs.len().max(1)) as f64;
    for value in &mut secondary {
        *value /= denom;
    }
    let peak_summary = summarize_runs(&peaks);
    let attack_summary = summarize_runs(&attacks);
    let outbreak_summary = summarize_runs(&outbreaks);
    let containment_summary = summarize_runs(&containment);
    let crossing_summary = summarize_runs(&crossings);
    let scores = centrality(graph);
    let superspreaders = rank_superspreaders(
        &model.names,
        &model.types,
        &model.reproduction.expected_secondary,
        &secondary,
        &scores.degree,
        &scores.pagerank,
        &model.privilege,
        &model.criticality,
        &scores.betweenness,
        20,
    );
    VariantReport {
        name: name.to_string(),
        runs: runs.len() as u32,
        r0: model.reproduction.r0_mean,
        invasion_threshold: model.reproduction.invasion_threshold,
        median_peak: peak_summary.median,
        median_attack_rate: attack_summary.median,
        median_outbreak: outbreak_summary.median,
        mean_outbreak: outbreak_summary.mean,
        p95_outbreak: outbreak_summary.p95,
        p99_outbreak: outbreak_summary.p99,
        extinction_probability: f64::from(extinct) / denom,
        median_containment_seconds: containment_summary.median,
        median_rt_below_seconds: if crossings.is_empty() {
            None
        } else {
            Some(crossing_summary.median)
        },
        mean_time_to_detection_seconds: average(&detect),
        mean_time_to_isolation_seconds: average(&isolate),
        mean_time_to_recovery_seconds: average(&recover),
        final_counts: runs.last().map(|run| run.final_counts).unwrap_or_default(),
        superspreaders,
        curve: mean_curve(runs),
        outbreak_sizes: runs.iter().map(|run| run.outbreak_size).collect(),
        histogram: Histogram::from_samples(&outbreaks, 16),
        run_seeds: runs.iter().map(|run| run.seed).collect(),
    }
}

fn average(values: &[f64]) -> Option<f64> {
    if values.is_empty() {
        None
    } else {
        Some(values.iter().sum::<f64>() / values.len() as f64)
    }
}

fn mean_curve(runs: &[RunSummary]) -> Vec<CurveSample> {
    let width = runs.iter().map(|run| run.samples.len()).max().unwrap_or(0);
    if width == 0 || runs.is_empty() {
        return Vec::new();
    }
    let mut curve = Vec::with_capacity(width);
    for index in 0..width {
        let mut count = 0.0;
        let mut acc = CurveSample {
            t_seconds: 0.0,
            susceptible: 0.0,
            exposed: 0.0,
            infectious: 0.0,
            detected: 0.0,
            quarantined: 0.0,
            recovered: 0.0,
            protected: 0.0,
            rt: 0.0,
            cumulative: 0.0,
        };
        for run in runs {
            if let Some(sample) = run.samples.get(index) {
                count += 1.0;
                acc.t_seconds += sample.t_seconds;
                acc.susceptible += sample.susceptible;
                acc.exposed += sample.exposed;
                acc.infectious += sample.infectious;
                acc.detected += sample.detected;
                acc.quarantined += sample.quarantined;
                acc.recovered += sample.recovered;
                acc.protected += sample.protected;
                acc.rt += sample.rt;
                acc.cumulative += sample.cumulative;
            }
        }
        if count > 0.0 {
            curve.push(CurveSample {
                t_seconds: acc.t_seconds / count,
                susceptible: acc.susceptible / count,
                exposed: acc.exposed / count,
                infectious: acc.infectious / count,
                detected: acc.detected / count,
                quarantined: acc.quarantined / count,
                recovered: acc.recovered / count,
                protected: acc.protected / count,
                rt: acc.rt / count,
                cumulative: acc.cumulative / count,
            });
        }
    }
    curve
}

pub fn render_report(report: &SimulationReport) -> String {
    let mut out = String::new();
    for variant in &report.variants {
        out.push_str(&render_variant(report, variant));
        out.push('\n');
    }
    if report.variants.len() > 1 {
        out.push_str(&render_comparison_table(report));
    }
    out.push_str(&format!(
        "\nProvenance\n  version:     {}\n  scenario:    {}\n  seed:        {}\n  algorithm:   {}\n  pathogen:    {}\n  generator:   {}\n  model:       {}\n  recorded:    {}\n",
        report.provenance.cyberepi_version,
        report.provenance.scenario_hash,
        report.provenance.seed,
        report.provenance.algorithm,
        report.provenance.pathogen_id,
        report.provenance.generator_version,
        report.provenance.model_version,
        report.provenance.timestamp,
    ));
    out
}

fn render_variant(report: &SimulationReport, variant: &VariantReport) -> String {
    let title = if report.variants.len() == 1 {
        "CyberEpi Digital Outbreak Simulation".to_string()
    } else {
        format!("CyberEpi Digital Outbreak Simulation — {}", variant.name)
    };
    let mut lines = vec![
        title,
        row("Population:", &format_count(u64::from(report.population))),
        row(
            "Initial compromises:",
            &format_count(u64::from(report.initial_compromises)),
        ),
        row(
            "Simulation duration:",
            &format_sim_duration(report.duration_seconds),
        ),
        row("Runs:", &format_count(u64::from(variant.runs))),
        row("R0:", &format!("{:.2}", variant.r0)),
        row(
            "Invasion threshold:",
            &format!("{:.2}", variant.invasion_threshold),
        ),
        row(
            "Median peak prevalence:",
            &format_count(variant.median_peak.round().max(0.0) as u64),
        ),
        row(
            "Median attack rate:",
            &format!("{:.1}%", variant.median_attack_rate * 100.0),
        ),
        row(
            "Median outbreak size:",
            &format_count(variant.median_outbreak.round().max(0.0) as u64),
        ),
        row(
            "95th percentile size:",
            &format_count(variant.p95_outbreak.round().max(0.0) as u64),
        ),
        row(
            "99th percentile size:",
            &format_count(variant.p99_outbreak.round().max(0.0) as u64),
        ),
        row(
            "Median containment:",
            &format_sim_duration(variant.median_containment_seconds),
        ),
        row(
            "Extinction probability:",
            &format!("{:.1}%", variant.extinction_probability * 100.0),
        ),
    ];
    lines.push("Top superspreaders:".to_string());
    if variant.superspreaders.is_empty() {
        lines.push("  (none)".to_string());
    }
    for (index, node) in variant.superspreaders.iter().take(5).enumerate() {
        lines.push(format!("{}. {}  ({:.2})", index + 1, node.name, node.score));
    }
    let crossing = variant
        .median_rt_below_seconds
        .map(format_sim_duration)
        .unwrap_or_else(|| "n/a".to_string());
    lines.push("Rt dropped below 1:".to_string());
    lines.push(crossing);
    lines.join("\n")
}

fn render_comparison_table(report: &SimulationReport) -> String {
    let mut lines = vec![
        "Intervention comparison".to_string(),
        format!(
            "{:<28} {:>12} {:>10} {:>12} {:>14}",
            "Scenario", "Median size", "R0", "Peak", "Containment"
        ),
    ];
    for variant in &report.variants {
        lines.push(format!(
            "{:<28} {:>12} {:>10.2} {:>12} {:>14}",
            truncate(&variant.name, 28),
            format_count(variant.median_outbreak.round().max(0.0) as u64),
            variant.r0,
            format_count(variant.median_peak.round().max(0.0) as u64),
            format_sim_duration(variant.median_containment_seconds),
        ));
    }
    lines.join("\n")
}

pub fn render_comparison_csv(report: &SimulationReport) -> String {
    let mut csv = String::from(
        "scenario,runs,r0,invasion_threshold,median_outbreak,mean_outbreak,p95_outbreak,p99_outbreak,median_peak,median_attack_rate,extinction_probability,median_containment_seconds,median_rt_below_seconds\n",
    );
    for variant in &report.variants {
        csv.push_str(&format!(
            "{},{},{:.6},{:.6},{:.4},{:.4},{:.4},{:.4},{:.4},{:.6},{:.6},{:.4},{}\n",
            csv_escape(&variant.name),
            variant.runs,
            variant.r0,
            variant.invasion_threshold,
            variant.median_outbreak,
            variant.mean_outbreak,
            variant.p95_outbreak,
            variant.p99_outbreak,
            variant.median_peak,
            variant.median_attack_rate,
            variant.extinction_probability,
            variant.median_containment_seconds,
            variant
                .median_rt_below_seconds
                .map(|value| format!("{value:.4}"))
                .unwrap_or_default(),
        ));
    }
    csv
}

fn row(label: &str, value: &str) -> String {
    format!("{label:<24}{value}")
}

fn truncate(value: &str, width: usize) -> String {
    if value.chars().count() <= width {
        value.to_string()
    } else {
        value
            .chars()
            .take(width.saturating_sub(1))
            .collect::<String>()
            + "…"
    }
}

fn csv_escape(value: &str) -> String {
    if value.contains(',') || value.contains('"') {
        format!("\"{}\"", value.replace('"', "\"\""))
    } else {
        value.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn comparison_header_is_stable() {
        let csv = render_comparison_csv(&SimulationReport {
            population: 10,
            initial_compromises: 1,
            duration_seconds: 86_400.0,
            algorithm: "gillespie".into(),
            pathogen: "IdentityStealer".into(),
            provenance: Provenance {
                cyberepi_version: "0.1.0".into(),
                scenario_hash: "abc".into(),
                seed: 1,
                algorithm: "gillespie".into(),
                pathogen_id: "identity-stealer".into(),
                model: "seidqrp".into(),
                generator_version: "1".into(),
                model_version: "1".into(),
                timestamp: "2026-01-01T00:00:00Z".into(),
                worker_version: "0.1.0".into(),
            },
            variants: vec![],
        });
        assert!(csv.starts_with("scenario,runs,r0"));
    }
}
