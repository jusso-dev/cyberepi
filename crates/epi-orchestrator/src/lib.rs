//! Shared controller and worker runtime.
//!
//! Jobs are idempotent: a batch id is stored before results are published, and
//! a redelivered batch is acknowledged without being simulated twice.

mod auth;
mod config;
mod jobs;
mod store;

pub use auth::{hash_password, new_session_token, verify_password};
pub use config::Config;
pub use jobs::{
    connect_nats, ensure_stream, publish_batches, worker_consumer, BatchJob, ProgressEvent,
    JOB_SUBJECT, PROGRESS_SUBJECT,
};
pub use store::{
    finish_if_ready, migrate, record_batch, save_organisation, AppDb, ExperimentRow, ProgressNote,
    StoredScenario,
};

pub const STREAM: &str = "CYBEREPI";
pub const CONSUMER: &str = "cyberepi-workers";
