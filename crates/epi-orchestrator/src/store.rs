use crate::auth::{hash_password, token_hash};
use crate::jobs::BatchJob;
use anyhow::Context;
use chrono::{Duration, Utc};
use epi_controls::catalog;
use epi_core::{GENERATOR_VERSION, VERSION};
use epi_generator::{generate, summarise, GeneratorConfig};
use epi_pathogens::builtins;
use epi_scenario::SliceOutput;
use epi_scenario::{load_scenario_str, scenario_hash, Scenario};
use epi_simulator::SimulationReport;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;
use uuid::Uuid;

#[derive(Clone)]
pub struct AppDb {
    pub pool: PgPool,
}

#[derive(Clone, Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct StoredScenario {
    pub id: Uuid,
    pub name: String,
    pub yaml: String,
    pub content_hash: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct ExperimentRow {
    pub id: Uuid,
    pub name: String,
    pub status: String,
    pub runs_requested: i32,
    pub runs_completed: i32,
    pub seed: i64,
    pub algorithm: String,
    pub scenario_hash: String,
    pub scenario_yaml: String,
    pub summary: Option<Value>,
    pub error: Option<String>,
    pub created_at: chrono::DateTime<Utc>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ProgressNote {
    pub experiment_id: Uuid,
    pub completed: i32,
    pub total: i32,
    pub status: String,
}

pub async fn migrate(database_url: &str) -> anyhow::Result<AppDb> {
    let pool = PgPoolOptions::new()
        .max_connections(8)
        .connect(database_url)
        .await
        .context("connect postgres")?;
    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .context("migrate")?;
    let db = AppDb { pool };
    db.seed_catalog().await?;
    Ok(db)
}

impl AppDb {
    pub async fn seed_catalog(&self) -> anyhow::Result<()> {
        for pathogen in builtins() {
            sqlx::query(
                "INSERT INTO pathogens (id, definition, builtin) VALUES ($1, $2, true)
                 ON CONFLICT (id) DO UPDATE SET definition = EXCLUDED.definition",
            )
            .bind(&pathogen.id)
            .bind(serde_json::to_value(&pathogen)?)
            .execute(&self.pool)
            .await?;
        }
        for control in catalog() {
            sqlx::query(
                "INSERT INTO controls (id, definition, assumption) VALUES ($1, $2, $3)
                 ON CONFLICT (id) DO UPDATE SET definition = EXCLUDED.definition",
            )
            .bind(control.kind.id())
            .bind(serde_json::to_value(&control)?)
            .bind(control.assumption)
            .execute(&self.pool)
            .await?;
        }
        Ok(())
    }

    pub async fn bootstrap_admin(&self, username: &str, password: &str) -> anyhow::Result<bool> {
        let count: (i64,) = sqlx::query_as("SELECT count(*) FROM users")
            .fetch_one(&self.pool)
            .await?;
        if count.0 > 0 {
            return Ok(false);
        }
        let hash = hash_password(password).map_err(|err| anyhow::anyhow!(err.to_string()))?;
        sqlx::query("INSERT INTO users (id, username, password_hash) VALUES ($1, $2, $3)")
            .bind(Uuid::new_v4())
            .bind(username)
            .bind(hash)
            .execute(&self.pool)
            .await?;
        Ok(true)
    }

    pub async fn login(&self, username: &str, password: &str) -> anyhow::Result<Option<String>> {
        let row: Option<(Uuid, String)> =
            sqlx::query_as("SELECT id, password_hash FROM users WHERE username = $1")
                .bind(username)
                .fetch_optional(&self.pool)
                .await?;
        let Some((user_id, encoded)) = row else {
            return Ok(None);
        };
        if !crate::auth::verify_password(password, &encoded) {
            return Ok(None);
        }
        let (raw, hash) = crate::auth::new_session_token();
        sqlx::query("INSERT INTO sessions (token_hash, user_id, expires_at) VALUES ($1, $2, $3)")
            .bind(&hash)
            .bind(user_id)
            .bind(Utc::now() + Duration::days(7))
            .execute(&self.pool)
            .await?;
        Ok(Some(raw))
    }

    pub async fn user_for_token(&self, token: &str) -> anyhow::Result<Option<(Uuid, String)>> {
        let hash = token_hash(token);
        let row = sqlx::query_as(
            "SELECT users.id, users.username FROM sessions
             JOIN users ON users.id = sessions.user_id
             WHERE sessions.token_hash = $1 AND sessions.expires_at > now()",
        )
        .bind(hash)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row)
    }

    pub async fn logout(&self, token: &str) -> anyhow::Result<()> {
        sqlx::query("DELETE FROM sessions WHERE token_hash = $1")
            .bind(token_hash(token))
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn insert_scenario(&self, yaml: &str) -> anyhow::Result<StoredScenario> {
        let scenario = load_scenario_str(yaml)?;
        let id = Uuid::new_v4();
        let hash = scenario_hash(&scenario);
        let spec = serde_json::to_value(&scenario)?;
        sqlx::query(
            "INSERT INTO scenarios (id, name, yaml, spec, content_hash) VALUES ($1, $2, $3, $4, $5)",
        )
        .bind(id)
        .bind(&scenario.metadata.name)
        .bind(yaml)
        .bind(spec)
        .bind(&hash)
        .execute(&self.pool)
        .await?;
        Ok(StoredScenario {
            id,
            name: scenario.metadata.name,
            yaml: yaml.to_string(),
            content_hash: hash,
        })
    }

    pub async fn list_scenarios(&self) -> anyhow::Result<Vec<StoredScenario>> {
        let rows = sqlx::query_as(
            "SELECT id, name, yaml, content_hash FROM scenarios ORDER BY created_at DESC LIMIT 200",
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    pub async fn scenario(&self, id: Uuid) -> anyhow::Result<Option<StoredScenario>> {
        Ok(
            sqlx::query_as("SELECT id, name, yaml, content_hash FROM scenarios WHERE id = $1")
                .bind(id)
                .fetch_optional(&self.pool)
                .await?,
        )
    }

    pub async fn create_experiment(
        &self,
        yaml: &str,
        batch_size: u32,
    ) -> anyhow::Result<(Uuid, Vec<BatchJob>)> {
        let scenario = load_scenario_str(yaml)?;
        let stored = self.insert_scenario(yaml).await?;
        let id = Uuid::new_v4();
        let planned = crate::jobs::plan_batches(&scenario, batch_size);
        let total_runs = planned.iter().map(|(_, _, count)| *count).sum::<u32>();
        sqlx::query(
            "INSERT INTO experiments (
                id, scenario_id, name, status, runs_requested, seed, algorithm,
                scenario_hash, scenario_yaml, cyberepi_version, generator_version,
                pathogen, controls, started_at
             ) VALUES ($1,$2,$3,'running',$4,$5,$6,$7,$8,$9,$10,$11,$12, now())",
        )
        .bind(id)
        .bind(stored.id)
        .bind(&scenario.metadata.name)
        .bind(total_runs as i32)
        .bind(scenario.simulation.seed as i64)
        .bind(scenario.simulation.algorithm.label())
        .bind(scenario_hash(&scenario))
        .bind(yaml)
        .bind(VERSION)
        .bind(GENERATOR_VERSION)
        .bind(serde_json::to_value(&scenario.pathogen)?)
        .bind(serde_json::to_value(&scenario.controls)?)
        .execute(&self.pool)
        .await?;
        let mut names = Vec::new();
        for (name, _, _) in &planned {
            if !names.contains(name) {
                names.push(name.clone());
            }
        }
        for name in names {
            let controls = variant_controls(&scenario, &name);
            sqlx::query(
                "INSERT INTO experiment_variants (id, experiment_id, name, controls) VALUES ($1, $2, $3, $4)",
            )
            .bind(Uuid::new_v4())
            .bind(id)
            .bind(&name)
            .bind(serde_json::to_value(&controls)?)
            .execute(&self.pool)
            .await?;
        }
        let jobs = planned
            .into_iter()
            .map(|(name, offset, count)| BatchJob {
                batch_id: Uuid::new_v4(),
                experiment_id: id,
                variant_name: name,
                scenario_yaml: yaml.to_string(),
                run_offset: offset,
                run_count: count,
                seed: scenario.simulation.seed,
            })
            .collect::<Vec<_>>();
        for job in &jobs {
            sqlx::query(
                "INSERT INTO simulation_batches (id, experiment_id, variant_name, run_offset, run_count, status)
                 VALUES ($1, $2, $3, $4, $5, 'queued')",
            )
            .bind(job.batch_id)
            .bind(job.experiment_id)
            .bind(&job.variant_name)
            .bind(job.run_offset as i32)
            .bind(job.run_count as i32)
            .execute(&self.pool)
            .await?;
        }
        Ok((id, jobs))
    }

    pub async fn list_experiments(&self) -> anyhow::Result<Vec<ExperimentRow>> {
        Ok(sqlx::query_as(
            "SELECT id, name, status, runs_requested, runs_completed, seed, algorithm,
                    scenario_hash, scenario_yaml, summary, error, created_at
             FROM experiments ORDER BY created_at DESC LIMIT 100",
        )
        .fetch_all(&self.pool)
        .await?)
    }

    pub async fn experiment(&self, id: Uuid) -> anyhow::Result<Option<ExperimentRow>> {
        Ok(sqlx::query_as(
            "SELECT id, name, status, runs_requested, runs_completed, seed, algorithm,
                    scenario_hash, scenario_yaml, summary, error, created_at
             FROM experiments WHERE id = $1",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?)
    }

    pub async fn cancel(&self, id: Uuid) -> anyhow::Result<()> {
        sqlx::query(
            "UPDATE experiments SET status = 'cancelled', finished_at = now()
             WHERE id = $1 AND status IN ('queued', 'running')",
        )
        .bind(id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn batch_status(&self, batch_id: Uuid) -> anyhow::Result<Option<String>> {
        let row: Option<(String,)> =
            sqlx::query_as("SELECT status FROM simulation_batches WHERE id = $1")
                .bind(batch_id)
                .fetch_optional(&self.pool)
                .await?;
        Ok(row.map(|value| value.0))
    }

    pub async fn experiment_status(&self, id: Uuid) -> anyhow::Result<Option<String>> {
        let row: Option<(String,)> = sqlx::query_as("SELECT status FROM experiments WHERE id = $1")
            .bind(id)
            .fetch_optional(&self.pool)
            .await?;
        Ok(row.map(|value| value.0))
    }
}

fn variant_controls(scenario: &Scenario, name: &str) -> epi_controls::ControlSet {
    if scenario.variants.is_empty() {
        return scenario.controls.clone();
    }
    scenario
        .variants
        .iter()
        .find(|variant| variant.name == name)
        .map(|variant| scenario.controls.merge(&variant.controls))
        .unwrap_or_else(|| scenario.controls.clone())
}

pub async fn record_batch(
    db: &AppDb,
    job: &BatchJob,
    output: &SliceOutput,
    worker_id: &str,
) -> anyhow::Result<ProgressNote> {
    let mut tx = db.pool.begin().await?;
    sqlx::query(
        "UPDATE simulation_batches SET status = 'completed', result = $2, worker_id = $3 WHERE id = $1",
    )
    .bind(job.batch_id)
    .bind(serde_json::to_value(output)?)
    .bind(worker_id)
    .execute(&mut *tx)
    .await?;
    for run in output.runs.iter().enumerate() {
        let (index, summary) = run;
        let run_index = job.run_offset + index as u32;
        let mut compact = summary.clone();
        compact.secondary.clear();
        compact.parent.clear();
        compact.events.clear();
        sqlx::query(
            "INSERT INTO simulation_runs (id, experiment_id, variant_name, run_index, seed, status, summary)
             VALUES ($1, $2, $3, $4, $5, 'completed', $6)
             ON CONFLICT (experiment_id, variant_name, run_index)
             DO UPDATE SET summary = EXCLUDED.summary, status = 'completed'",
        )
        .bind(Uuid::new_v4())
        .bind(job.experiment_id)
        .bind(&job.variant_name)
        .bind(run_index as i32)
        .bind(summary.seed as i64)
        .bind(serde_json::to_value(&compact)?)
        .execute(&mut *tx)
        .await?;
    }
    let updated: (i32, i32, String) = sqlx::query_as(
        "UPDATE experiments SET runs_completed = runs_completed + $2
         WHERE id = $1
         RETURNING runs_completed, runs_requested, status",
    )
    .bind(job.experiment_id)
    .bind(output.runs.len() as i32)
    .fetch_one(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(ProgressNote {
        experiment_id: job.experiment_id,
        completed: updated.0,
        total: updated.1,
        status: updated.2,
    })
}

pub async fn finish_if_ready(
    db: &AppDb,
    experiment_id: Uuid,
) -> anyhow::Result<Option<SimulationReport>> {
    let claimed: Option<(Uuid,)> = sqlx::query_as(
        "UPDATE experiments SET status = 'aggregating'
         WHERE id = $1 AND status = 'running' AND runs_completed >= runs_requested
         RETURNING id",
    )
    .bind(experiment_id)
    .fetch_optional(&db.pool)
    .await?;
    if claimed.is_none() {
        return Ok(None);
    }
    let row = db
        .experiment(experiment_id)
        .await?
        .context("experiment disappeared")?;
    let scenario = load_scenario_str(&row.scenario_yaml)?;
    let report = reconstruct(&db.pool, &scenario, &row).await?;
    sqlx::query(
        "UPDATE experiments SET status = 'completed', summary = $2, finished_at = now() WHERE id = $1",
    )
    .bind(experiment_id)
    .bind(serde_json::to_value(&report)?)
    .execute(&db.pool)
    .await?;
    Ok(Some(report))
}

async fn reconstruct(
    pool: &PgPool,
    scenario: &Scenario,
    row: &ExperimentRow,
) -> anyhow::Result<SimulationReport> {
    use epi_generator::generate;
    use epi_models::{compile, CompileInputs, TimingOverrides};
    use epi_pathogens::builtin;
    use epi_simulator::{aggregate, Provenance};
    use std::collections::BTreeMap;

    let graph = generate(&GeneratorConfig {
        template: scenario.population.template,
        size: scenario.population.size,
        seed: scenario.population.seed,
        name: scenario.population.name.clone(),
    })?;
    let pathogen = builtin(&scenario.pathogen.kind)?;
    let batches: Vec<(String, Value)> = sqlx::query_as(
        "SELECT variant_name, result FROM simulation_batches
         WHERE experiment_id = $1 AND status = 'completed' AND result IS NOT NULL
         ORDER BY variant_name, run_offset",
    )
    .bind(row.id)
    .fetch_all(pool)
    .await?;
    let mut grouped: BTreeMap<String, Vec<SliceOutput>> = BTreeMap::new();
    for (name, value) in batches {
        let output: SliceOutput = serde_json::from_value(value)?;
        grouped.entry(name).or_default().push(output);
    }
    let timing = TimingOverrides {
        detection_delay: scenario.simulation.detection_delay,
        isolation_delay: scenario.simulation.isolation_delay,
        recovery_time: scenario.simulation.recovery_time,
    };
    let mut variants = Vec::new();
    for (name, outputs) in grouped {
        let controls = variant_controls(scenario, &name);
        let compiled = compile(&CompileInputs {
            graph: &graph,
            pathogen: &pathogen,
            controls: &controls,
            preset: scenario.model,
            law: scenario.simulation.transmission,
            timing: &timing,
            coverage_seed: scenario.population.seed,
        })?;
        let mut runs = Vec::new();
        let mut secondary = vec![0u64; graph.node_count()];
        for output in outputs {
            for (index, count) in output.secondary_sum.iter().enumerate() {
                if index < secondary.len() {
                    secondary[index] = secondary[index].saturating_add(*count);
                }
            }
            runs.extend(output.runs);
        }
        if let Some(first) = runs.first_mut() {
            first.secondary = secondary.iter().map(|value| *value as u32).collect();
        }
        variants.push(aggregate(&name, &compiled, &graph, &runs));
    }
    Ok(SimulationReport {
        population: graph.node_count() as u32,
        initial_compromises: scenario.initial_conditions.infected_entities,
        duration_seconds: scenario.simulation.duration.as_secs_f64(),
        algorithm: scenario.simulation.algorithm.label().to_string(),
        pathogen: pathogen.name,
        provenance: Provenance {
            cyberepi_version: VERSION.to_string(),
            scenario_hash: row.scenario_hash.clone(),
            seed: row.seed as u64,
            algorithm: row.algorithm.clone(),
            pathogen_id: pathogen_id(&scenario.pathogen.kind),
            model: scenario.model.label().to_string(),
            generator_version: GENERATOR_VERSION.to_string(),
            model_version: epi_core::MODEL_VERSION.to_string(),
            timestamp: Utc::now().to_rfc3339(),
            worker_version: VERSION.to_string(),
        },
        variants,
    })
}

fn pathogen_id(name: &str) -> String {
    builtin_id(name)
}

fn builtin_id(name: &str) -> String {
    builtins()
        .into_iter()
        .find(|item| item.id == name || item.name == name)
        .map(|item| item.id)
        .unwrap_or_else(|| name.to_string())
}

pub async fn save_organisation(
    db: &AppDb,
    template: &str,
    size: u32,
    seed: u64,
    name: &str,
) -> anyhow::Result<Value> {
    let kind =
        epi_generator::OrgTemplate::parse(template).context("unknown organisation template")?;
    let config = GeneratorConfig {
        template: kind,
        size,
        seed,
        name: name.to_string(),
    };
    let graph = generate(&config)?;
    let summary = summarise(&config, &graph);
    let value = serde_json::to_value(&summary)?;
    sqlx::query(
        "INSERT INTO organisations (id, name, template, size, seed, summary) VALUES ($1,$2,$3,$4,$5,$6)",
    )
    .bind(Uuid::new_v4())
    .bind(name)
    .bind(template)
    .bind(size as i32)
    .bind(seed as i64)
    .bind(&value)
    .execute(&db.pool)
    .await?;
    Ok(value)
}
