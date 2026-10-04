use std::env;

#[derive(Clone, Debug)]
pub struct Config {
    pub database_url: String,
    pub nats_url: String,
    pub bind: String,
    pub metrics_bind: String,
    pub bootstrap_user: String,
    pub bootstrap_password: Option<String>,
    pub batch_size: u32,
    pub worker_id: String,
    pub public_origin: String,
}

impl Config {
    pub fn from_env() -> Self {
        Self {
            database_url: env::var("DATABASE_URL")
                .unwrap_or_else(|_| "postgres://cyberepi:cyberepi@127.0.0.1:5432/cyberepi".into()),
            nats_url: env::var("NATS_URL").unwrap_or_else(|_| "nats://127.0.0.1:4222".into()),
            bind: env::var("CYBEREPI_BIND").unwrap_or_else(|_| "0.0.0.0:8080".into()),
            metrics_bind: env::var("CYBEREPI_METRICS_BIND")
                .unwrap_or_else(|_| "0.0.0.0:9102".into()),
            bootstrap_user: env::var("CYBEREPI_BOOTSTRAP_USER").unwrap_or_else(|_| "admin".into()),
            bootstrap_password: env::var("CYBEREPI_BOOTSTRAP_PASSWORD").ok(),
            batch_size: env::var("CYBEREPI_BATCH_SIZE")
                .ok()
                .and_then(|value| value.parse().ok())
                .unwrap_or(25)
                .max(1),
            worker_id: env::var("CYBEREPI_WORKER_ID")
                .unwrap_or_else(|_| format!("worker-{}", &uuid::Uuid::new_v4().to_string()[..8])),
            public_origin: env::var("CYBEREPI_PUBLIC_ORIGIN")
                .unwrap_or_else(|_| "http://localhost".into()),
        }
    }
}
