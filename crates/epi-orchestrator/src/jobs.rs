use crate::{CONSUMER, STREAM};
use epi_scenario::Scenario;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub const JOB_SUBJECT: &str = "cyberepi.jobs";
pub const PROGRESS_SUBJECT: &str = "cyberepi.progress";

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BatchJob {
    pub batch_id: Uuid,
    pub experiment_id: Uuid,
    pub variant_name: String,
    pub scenario_yaml: String,
    pub run_offset: u32,
    pub run_count: u32,
    pub seed: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ProgressEvent {
    pub experiment_id: Uuid,
    pub completed: u32,
    pub total: u32,
    pub status: String,
    pub worker_id: String,
}

pub fn plan_batches(scenario: &Scenario, batch_size: u32) -> Vec<(String, u32, u32)> {
    let variants = if scenario.variants.is_empty() {
        vec![scenario.metadata.name.clone()]
    } else {
        scenario
            .variants
            .iter()
            .map(|variant| variant.name.clone())
            .collect()
    };
    let mut planned = Vec::new();
    for name in variants {
        let mut offset = 0u32;
        while offset < scenario.simulation.runs {
            let count = batch_size.min(scenario.simulation.runs - offset);
            planned.push((name.clone(), offset, count));
            offset += count;
        }
    }
    planned
}

pub async fn connect_nats(url: &str) -> anyhow::Result<async_nats::Client> {
    let mut last = String::from("not attempted");
    for attempt in 1..=30 {
        match async_nats::connect(url).await {
            Ok(client) => return Ok(client),
            Err(err) => {
                last = err.to_string();
                tracing::warn!(attempt, %last, "waiting for NATS");
                tokio::time::sleep(std::time::Duration::from_secs(2)).await;
            }
        }
    }
    anyhow::bail!("could not reach NATS at {url}: {last}")
}

pub async fn ensure_stream(
    client: &async_nats::Client,
) -> anyhow::Result<async_nats::jetstream::stream::Stream> {
    let jetstream = async_nats::jetstream::new(client.clone());
    let stream = jetstream
        .get_or_create_stream(async_nats::jetstream::stream::Config {
            name: STREAM.to_string(),
            subjects: vec![JOB_SUBJECT.to_string()],
            retention: async_nats::jetstream::stream::RetentionPolicy::WorkQueue,
            max_age: std::time::Duration::from_secs(7 * 24 * 3600),
            ..Default::default()
        })
        .await?;
    let _ = CONSUMER;
    Ok(stream)
}

pub async fn publish_batches(client: &async_nats::Client, jobs: &[BatchJob]) -> anyhow::Result<()> {
    let jetstream = async_nats::jetstream::new(client.clone());
    for job in jobs {
        let payload = serde_json::to_vec(job)?;
        jetstream
            .publish(JOB_SUBJECT, payload.into())
            .await?
            .await?;
    }
    Ok(())
}

pub async fn worker_consumer(
    stream: &async_nats::jetstream::stream::Stream,
) -> anyhow::Result<async_nats::jetstream::consumer::PullConsumer> {
    let consumer = stream
        .get_or_create_consumer(
            CONSUMER,
            async_nats::jetstream::consumer::pull::Config {
                durable_name: Some(CONSUMER.to_string()),
                filter_subject: JOB_SUBJECT.to_string(),
                ack_policy: async_nats::jetstream::consumer::AckPolicy::Explicit,
                max_deliver: 5,
                ack_wait: std::time::Duration::from_secs(60 * 30),
                ..Default::default()
            },
        )
        .await?;
    Ok(consumer)
}
