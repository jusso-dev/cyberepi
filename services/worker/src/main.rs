//! Simulation worker. Pulls JetStream batches, runs them, and records results.
//!
//! Re-delivery is safe: a completed batch is acknowledged without a second run.

use async_nats::jetstream::consumer::PullConsumer;
use epi_core::VERSION;
use epi_orchestrator::{
    connect_nats, finish_if_ready, migrate, record_batch, worker_consumer, BatchJob, Config,
    ProgressEvent, CONSUMER, PROGRESS_SUBJECT,
};
use epi_scenario::run_slice;
use futures_util::StreamExt;
use prometheus::{Encoder, Histogram, HistogramOpts, IntCounter, Opts, Registry, TextEncoder};
use std::sync::Arc;
use std::time::Instant;
use tracing::info;

struct Metrics {
    registry: Registry,
    simulations: IntCounter,
    failed: IntCounter,
    duration: Histogram,
}

fn metrics() -> Metrics {
    let registry = Registry::new();
    let simulations = IntCounter::with_opts(Opts::new(
        "cyberepi_simulations_total",
        "Completed simulation runs",
    ))
    .unwrap();
    let failed = IntCounter::with_opts(Opts::new(
        "cyberepi_simulations_failed_total",
        "Failed simulation batches",
    ))
    .unwrap();
    let duration = Histogram::with_opts(HistogramOpts::new(
        "cyberepi_simulation_duration_seconds",
        "Batch wall time",
    ))
    .unwrap();
    registry.register(Box::new(simulations.clone())).unwrap();
    registry.register(Box::new(failed.clone())).unwrap();
    registry.register(Box::new(duration.clone())).unwrap();
    Metrics {
        registry,
        simulations,
        failed,
        duration,
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .init();
    let config = Config::from_env();
    info!(worker = %config.worker_id, version = VERSION, "cyberepi worker starting");
    let db = migrate(&config.database_url).await?;
    let client = connect_nats(&config.nats_url).await?;
    let stream = epi_orchestrator::ensure_stream(&client).await?;
    let consumer: PullConsumer = worker_consumer(&stream).await?;
    let metrics = Arc::new(metrics());
    let metrics_bind = config.metrics_bind.clone();
    let registry = metrics.registry.clone();
    tokio::spawn(async move {
        let app = axum::Router::new().route(
            "/metrics",
            axum::routing::get(move || {
                let registry = registry.clone();
                async move {
                    let encoder = TextEncoder::new();
                    let metric_families = registry.gather();
                    let mut buffer = Vec::new();
                    encoder.encode(&metric_families, &mut buffer).ok();
                    String::from_utf8(buffer).unwrap_or_default()
                }
            }),
        );
        let listener = tokio::net::TcpListener::bind(&metrics_bind).await.ok();
        if let Some(listener) = listener {
            let _ = axum::serve(listener, app).await;
        }
    });
    let mut messages = consumer.messages().await?;
    let _ = CONSUMER;
    while let Some(next) = messages.next().await {
        let message = next?;
        let job: BatchJob = match serde_json::from_slice(&message.payload) {
            Ok(job) => job,
            Err(err) => {
                tracing::error!(%err, "discarding malformed batch");
                message
                    .ack()
                    .await
                    .map_err(|err| anyhow::anyhow!(err.to_string()))?;
                continue;
            }
        };
        if db.batch_status(job.batch_id).await?.as_deref() == Some("completed") {
            message
                .ack()
                .await
                .map_err(|err| anyhow::anyhow!(err.to_string()))?;
            continue;
        }
        if db.experiment_status(job.experiment_id).await?.as_deref() == Some("cancelled") {
            message
                .ack()
                .await
                .map_err(|err| anyhow::anyhow!(err.to_string()))?;
            continue;
        }
        let started = Instant::now();
        let scenario = match epi_scenario::load_scenario_str(&job.scenario_yaml) {
            Ok(scenario) => scenario,
            Err(err) => {
                metrics.failed.inc();
                tracing::error!(%err, "scenario rejected");
                message
                    .ack()
                    .await
                    .map_err(|err| anyhow::anyhow!(err.to_string()))?;
                continue;
            }
        };
        let threads = std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(1);
        match run_slice(
            &scenario,
            &job.variant_name,
            job.run_offset,
            job.run_count,
            job.seed,
            threads,
        ) {
            Ok(output) => {
                metrics.simulations.inc_by(u64::from(job.run_count));
                metrics.duration.observe(started.elapsed().as_secs_f64());
                let note = record_batch(&db, &job, &output, &config.worker_id).await?;
                let _ = client
                    .publish(
                        PROGRESS_SUBJECT,
                        serde_json::to_vec(&ProgressEvent {
                            experiment_id: note.experiment_id,
                            completed: note.completed as u32,
                            total: note.total as u32,
                            status: note.status.clone(),
                            worker_id: config.worker_id.clone(),
                        })?
                        .into(),
                    )
                    .await;
                if let Some(report) = finish_if_ready(&db, job.experiment_id).await? {
                    let _ = client
                        .publish(
                            PROGRESS_SUBJECT,
                            serde_json::to_vec(&ProgressEvent {
                                experiment_id: job.experiment_id,
                                completed: report.variants.iter().map(|v| v.runs).sum(),
                                total: report.variants.iter().map(|v| v.runs).sum(),
                                status: "completed".into(),
                                worker_id: config.worker_id.clone(),
                            })?
                            .into(),
                        )
                        .await;
                    info!(experiment = %job.experiment_id, "experiment completed");
                }
                message
                    .ack()
                    .await
                    .map_err(|err| anyhow::anyhow!(err.to_string()))?;
            }
            Err(err) => {
                metrics.failed.inc();
                tracing::error!(%err, batch = %job.batch_id, "batch failed");
                message.ack().await.ok();
            }
        }
    }
    Ok(())
}
