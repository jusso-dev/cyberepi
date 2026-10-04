//! CyberEpi HTTP control plane.
//!
//! The controller stores scenarios and publishes simulation batches. It does
//! not run exploits, and the optional cluster endpoint only describes whether
//! a kubeconfig is present.

use std::sync::Arc;

use axum::extract::ws::{Message, WebSocketUpgrade};
use axum::extract::{Path, Query, State};
use axum::http::{header, HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use epi_controls::{catalog, ASSUMPTION};
use epi_core::VERSION;
use epi_generator::TEMPLATE_IDS;
use epi_orchestrator::{
    connect_nats, ensure_stream, migrate, publish_batches, save_organisation, Config,
    PROGRESS_SUBJECT,
};
use epi_pathogens::builtins;
use epi_scenario::{load_scenario_str, validate_scenario};
use futures_util::StreamExt;
use prometheus::{Encoder, Gauge, IntGauge, Opts, Registry, TextEncoder};
use serde::Deserialize;
use serde_json::{json, Value};
use tokio::sync::broadcast;
use tower_http::cors::CorsLayer;
use tracing::info;
use uuid::Uuid;

#[derive(Clone)]
struct AppState {
    db: epi_orchestrator::AppDb,
    config: Config,
    nats: async_nats::Client,
    progress: broadcast::Sender<String>,
    metrics: Arc<Metrics>,
}

struct Metrics {
    registry: Registry,
    queue: IntGauge,
    active: IntGauge,
    workers: Gauge,
    infected: Gauge,
    rt: Gauge,
    attack: Gauge,
    peak: Gauge,
}

fn build_metrics() -> Metrics {
    let registry = Registry::new();
    let make_gauge = |name, help| {
        let gauge = Gauge::with_opts(Opts::new(name, help)).unwrap();
        registry.register(Box::new(gauge.clone())).unwrap();
        gauge
    };
    let queue = IntGauge::with_opts(Opts::new(
        "cyberepi_queue_depth",
        "Queued simulation batches",
    ))
    .unwrap();
    let active = IntGauge::with_opts(Opts::new(
        "cyberepi_active_jobs",
        "Experiments currently running",
    ))
    .unwrap();
    registry.register(Box::new(queue.clone())).unwrap();
    registry.register(Box::new(active.clone())).unwrap();
    Metrics {
        workers: make_gauge("cyberepi_worker_count", "Configured worker hint"),
        infected: make_gauge("cyberepi_current_infected", "Latest median outbreak size"),
        rt: make_gauge("cyberepi_rt", "Latest reproduction number"),
        attack: make_gauge("cyberepi_attack_rate", "Latest median attack rate"),
        peak: make_gauge("cyberepi_peak_prevalence", "Latest median peak prevalence"),
        registry,
        queue,
        active,
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
    let db = migrate(&config.database_url).await?;
    let password = config
        .bootstrap_password
        .clone()
        .unwrap_or_else(generate_password);
    if db
        .bootstrap_admin(&config.bootstrap_user, &password)
        .await?
    {
        if config.bootstrap_password.is_none() {
            info!(user = %config.bootstrap_user, password = %password, "generated bootstrap administrator");
        } else {
            info!(user = %config.bootstrap_user, "bootstrap administrator created");
        }
    }
    let nats = connect_nats(&config.nats_url).await?;
    ensure_stream(&nats).await?;
    let (progress, _) = broadcast::channel(256);
    let metrics = Arc::new(build_metrics());
    metrics.workers.set(
        std::env::var("CYBEREPI_WORKER_REPLICAS")
            .ok()
            .and_then(|value| value.parse().ok())
            .unwrap_or(1.0),
    );
    let state = AppState {
        db,
        config: config.clone(),
        nats: nats.clone(),
        progress: progress.clone(),
        metrics,
    };
    let mut subscriber = nats.subscribe(PROGRESS_SUBJECT).await?;
    let forward = progress.clone();
    tokio::spawn(async move {
        while let Some(message) = subscriber.next().await {
            if let Ok(text) = std::str::from_utf8(&message.payload) {
                let _ = forward.send(text.to_string());
            }
        }
    });
    let cors = CorsLayer::very_permissive();
    let app = Router::new()
        .route("/healthz", get(|| async { "ok" }))
        .route("/readyz", get(|| async { "ok" }))
        .route("/metrics", get(metrics_handler))
        .route("/api/v1/openapi.json", get(openapi))
        .route("/api/docs", get(api_docs))
        .route("/api/v1/auth/login", post(login))
        .route("/api/v1/auth/logout", post(logout))
        .route("/api/v1/auth/me", get(me))
        .route("/api/v1/pathogens", get(pathogens))
        .route("/api/v1/controls", get(controls))
        .route("/api/v1/templates", get(templates))
        .route(
            "/api/v1/scenarios",
            get(list_scenarios).post(create_scenario),
        )
        .route("/api/v1/scenarios/validate", post(validate))
        .route("/api/v1/scenarios/{id}", get(get_scenario))
        .route(
            "/api/v1/experiments",
            get(list_experiments).post(create_experiment),
        )
        .route("/api/v1/experiments/{id}", get(get_experiment))
        .route("/api/v1/experiments/{id}/start", post(start_experiment))
        .route("/api/v1/experiments/{id}/cancel", post(cancel_experiment))
        .route("/api/v1/experiments/{id}/results", get(results))
        .route("/api/v1/experiments/{id}/export", get(export_zip))
        .route(
            "/api/v1/experiments/{id}/charts/epidemic.png",
            get(chart_png),
        )
        .route("/api/v1/experiments/{id}/graph", get(experiment_graph))
        .route("/api/v1/experiments/{id}/trace", get(experiment_trace))
        .route("/api/v1/organisations", post(create_org))
        .route("/api/v1/demo", post(demo))
        .route("/api/v1/cluster", get(cluster_status))
        .route("/api/v1/settings", get(settings))
        .route("/api/v1/ws", get(ws))
        .layer(cors)
        .with_state(state);
    let listener = tokio::net::TcpListener::bind(&config.bind).await?;
    info!(bind = %config.bind, version = VERSION, "cyberepi controller listening");
    axum::serve(listener, app).await?;
    Ok(())
}

fn generate_password() -> String {
    format!("generated-{}", &Uuid::new_v4().to_string()[..8])
}

async fn require_user(state: &AppState, headers: &HeaderMap) -> Result<(Uuid, String), StatusCode> {
    let header = headers
        .get(header::COOKIE)
        .and_then(|value| value.to_str().ok())
        .ok_or(StatusCode::UNAUTHORIZED)?;
    let token = header
        .split(';')
        .filter_map(|part| part.trim().strip_prefix("cyberepi_session="))
        .next()
        .ok_or(StatusCode::UNAUTHORIZED)?;
    state
        .db
        .user_for_token(token)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::UNAUTHORIZED)
}

#[derive(Deserialize)]
struct LoginBody {
    username: String,
    password: String,
}

async fn login(State(state): State<AppState>, Json(body): Json<LoginBody>) -> Response {
    match state.db.login(&body.username, &body.password).await {
        Ok(Some(token)) => {
            let cookie =
                format!("cyberepi_session={token}; HttpOnly; SameSite=Lax; Path=/; Max-Age=604800");
            (
                [(header::SET_COOKIE, cookie)],
                Json(json!({"username": body.username})),
            )
                .into_response()
        }
        Ok(None) => StatusCode::UNAUTHORIZED.into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

async fn logout(State(state): State<AppState>, headers: HeaderMap) -> Response {
    if let Some(cookie) = headers.get(header::COOKIE).and_then(|v| v.to_str().ok()) {
        if let Some(token) = cookie
            .split(';')
            .find_map(|part| part.trim().strip_prefix("cyberepi_session="))
        {
            let _ = state.db.logout(token).await;
        }
    }
    StatusCode::NO_CONTENT.into_response()
}

async fn me(State(state): State<AppState>, headers: HeaderMap) -> Response {
    match require_user(&state, &headers).await {
        Ok((id, username)) => Json(json!({"id": id, "username": username})).into_response(),
        Err(code) => code.into_response(),
    }
}

async fn pathogens() -> Json<Value> {
    Json(json!({"assumption": ASSUMPTION, "pathogens": builtins()}))
}

async fn controls() -> Json<Value> {
    Json(json!({"assumption": ASSUMPTION, "controls": catalog()}))
}

async fn templates() -> Json<Value> {
    Json(json!({"templates": TEMPLATE_IDS}))
}

#[derive(Deserialize)]
struct YamlBody {
    yaml: String,
}

async fn create_scenario(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<YamlBody>,
) -> Response {
    if require_user(&state, &headers).await.is_err() {
        return StatusCode::UNAUTHORIZED.into_response();
    }
    match state.db.insert_scenario(&body.yaml).await {
        Ok(stored) => Json(stored).into_response(),
        Err(err) => (StatusCode::BAD_REQUEST, err.to_string()).into_response(),
    }
}

async fn list_scenarios(State(state): State<AppState>, headers: HeaderMap) -> Response {
    if require_user(&state, &headers).await.is_err() {
        return StatusCode::UNAUTHORIZED.into_response();
    }
    match state.db.list_scenarios().await {
        Ok(rows) => Json(rows).into_response(),
        Err(err) => (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()).into_response(),
    }
}

async fn get_scenario(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Response {
    if require_user(&state, &headers).await.is_err() {
        return StatusCode::UNAUTHORIZED.into_response();
    }
    match state.db.scenario(id).await {
        Ok(Some(row)) => Json(row).into_response(),
        Ok(None) => StatusCode::NOT_FOUND.into_response(),
        Err(err) => (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()).into_response(),
    }
}

async fn validate(Json(body): Json<YamlBody>) -> Response {
    match load_scenario_str(&body.yaml).and_then(|scenario| {
        validate_scenario(&scenario)?;
        Ok(scenario.metadata.name)
    }) {
        Ok(name) => Json(json!({"valid": true, "name": name})).into_response(),
        Err(err) => (
            StatusCode::BAD_REQUEST,
            Json(json!({"valid": false, "error": err.to_string()})),
        )
            .into_response(),
    }
}

async fn create_experiment(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<YamlBody>,
) -> Response {
    if require_user(&state, &headers).await.is_err() {
        return StatusCode::UNAUTHORIZED.into_response();
    }
    match state
        .db
        .create_experiment(&body.yaml, state.config.batch_size)
        .await
    {
        Ok((id, jobs)) => {
            state.metrics.queue.set(jobs.len() as i64);
            state.metrics.active.inc();
            if let Err(err) = publish_batches(&state.nats, &jobs).await {
                return (StatusCode::BAD_GATEWAY, err.to_string()).into_response();
            }
            (
                StatusCode::CREATED,
                Json(json!({"id": id, "batches": jobs.len()})),
            )
                .into_response()
        }
        Err(err) => (StatusCode::BAD_REQUEST, err.to_string()).into_response(),
    }
}

async fn list_experiments(State(state): State<AppState>, headers: HeaderMap) -> Response {
    if require_user(&state, &headers).await.is_err() {
        return StatusCode::UNAUTHORIZED.into_response();
    }
    match state.db.list_experiments().await {
        Ok(rows) => Json(rows).into_response(),
        Err(err) => (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()).into_response(),
    }
}

async fn get_experiment(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Response {
    if require_user(&state, &headers).await.is_err() {
        return StatusCode::UNAUTHORIZED.into_response();
    }
    match state.db.experiment(id).await {
        Ok(Some(row)) => {
            if let Some(summary) = &row.summary {
                if let Some(r0) = summary.pointer("/variants/0/r0").and_then(|v| v.as_f64()) {
                    state.metrics.rt.set(r0);
                }
                if let Some(attack) = summary
                    .pointer("/variants/0/median_attack_rate")
                    .and_then(|v| v.as_f64())
                {
                    state.metrics.attack.set(attack);
                }
                if let Some(peak) = summary
                    .pointer("/variants/0/median_peak")
                    .and_then(|v| v.as_f64())
                {
                    state.metrics.peak.set(peak);
                    state.metrics.infected.set(peak);
                }
            }
            Json(row).into_response()
        }
        Ok(None) => StatusCode::NOT_FOUND.into_response(),
        Err(err) => (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()).into_response(),
    }
}

async fn start_experiment(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Response {
    if require_user(&state, &headers).await.is_err() {
        return StatusCode::UNAUTHORIZED.into_response();
    }
    match state.db.experiment(id).await {
        Ok(Some(row)) if row.status == "completed" || row.status == "running" => {
            Json(json!({"id": id, "status": row.status})).into_response()
        }
        Ok(Some(_)) => (StatusCode::CONFLICT, "experiment already left the queue").into_response(),
        Ok(None) => StatusCode::NOT_FOUND.into_response(),
        Err(err) => (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()).into_response(),
    }
}

async fn cancel_experiment(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Response {
    if require_user(&state, &headers).await.is_err() {
        return StatusCode::UNAUTHORIZED.into_response();
    }
    if state.db.cancel(id).await.is_err() {
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    }
    Json(json!({"id": id, "status": "cancelled"})).into_response()
}

async fn results(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Response {
    if require_user(&state, &headers).await.is_err() {
        return StatusCode::UNAUTHORIZED.into_response();
    }
    match state.db.experiment(id).await {
        Ok(Some(row)) => Json(json!({
            "id": row.id,
            "status": row.status,
            "runs_completed": row.runs_completed,
            "runs_requested": row.runs_requested,
            "summary": row.summary,
            "error": row.error,
        }))
        .into_response(),
        Ok(None) => StatusCode::NOT_FOUND.into_response(),
        Err(err) => (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()).into_response(),
    }
}

async fn export_zip(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Response {
    if require_user(&state, &headers).await.is_err() {
        return StatusCode::UNAUTHORIZED.into_response();
    }
    let Ok(Some(row)) = state.db.experiment(id).await else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let mut cursor = std::io::Cursor::new(Vec::new());
    {
        let mut zip = zip::ZipWriter::new(&mut cursor);
        let opts = zip::write::SimpleFileOptions::default();
        if zip.start_file("scenario.yaml", opts).is_err() {
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
        use std::io::Write;
        let _ = zip.write_all(row.scenario_yaml.as_bytes());
        let _ = zip.start_file("results.json", opts);
        let body = serde_json::to_vec_pretty(&row.summary).unwrap_or_default();
        let _ = zip.write_all(&body);
        let _ = zip.finish();
    }
    (
        [
            (header::CONTENT_TYPE, "application/zip"),
            (
                header::CONTENT_DISPOSITION,
                "attachment; filename=\"cyberepi-experiment.zip\"",
            ),
        ],
        cursor.into_inner(),
    )
        .into_response()
}

async fn chart_png(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Response {
    if require_user(&state, &headers).await.is_err() {
        return StatusCode::UNAUTHORIZED.into_response();
    }
    let Ok(Some(row)) = state.db.experiment(id).await else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let series = row
        .summary
        .as_ref()
        .and_then(|summary| summary.pointer("/variants/0/curve"))
        .and_then(|curve| curve.as_array())
        .map(|points| {
            points
                .iter()
                .filter_map(|point| point.get("infectious").and_then(|v| v.as_f64()))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let png = render_png(&series);
    ([(header::CONTENT_TYPE, "image/png")], png).into_response()
}

fn render_png(series: &[f64]) -> Vec<u8> {
    let width = 640u32;
    let height = 280u32;
    let mut image = image::RgbImage::from_pixel(width, height, image::Rgb([244, 240, 230]));
    let max = series.iter().copied().fold(1.0_f64, f64::max);
    for (index, value) in series.iter().enumerate() {
        let x = ((index as f64 / series.len().max(1) as f64) * (width - 40) as f64) as u32 + 20;
        let y = height - 20 - ((*value / max) * (height - 40) as f64) as u32;
        if x < width && y < height {
            image.put_pixel(x, y, image::Rgb([194, 65, 45]));
        }
    }
    let mut bytes = Vec::new();
    let mut cursor = std::io::Cursor::new(&mut bytes);
    let _ = image::DynamicImage::ImageRgb8(image).write_to(&mut cursor, image::ImageFormat::Png);
    bytes
}

#[derive(Deserialize)]
struct OrgBody {
    template: String,
    size: u32,
    #[serde(default = "one")]
    seed: u64,
    #[serde(default)]
    name: String,
}

fn one() -> u64 {
    1
}

async fn create_org(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<OrgBody>,
) -> Response {
    if require_user(&state, &headers).await.is_err() {
        return StatusCode::UNAUTHORIZED.into_response();
    }
    let name = if body.name.is_empty() {
        format!("Synthetic {}", body.template)
    } else {
        body.name
    };
    match save_organisation(&state.db, &body.template, body.size, body.seed, &name).await {
        Ok(summary) => Json(summary).into_response(),
        Err(err) => (StatusCode::BAD_REQUEST, err.to_string()).into_response(),
    }
}

#[derive(Deserialize)]
struct TraceQuery {
    #[serde(default)]
    run: u32,
    #[serde(default)]
    variant: String,
}

async fn experiment_graph(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Response {
    if require_user(&state, &headers).await.is_err() {
        return StatusCode::UNAUTHORIZED.into_response();
    }
    let Ok(Some(row)) = state.db.experiment(id).await else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let Ok(scenario) = load_scenario_str(&row.scenario_yaml) else {
        return StatusCode::BAD_REQUEST.into_response();
    };
    let Ok(graph) = epi_generator::generate(&epi_generator::GeneratorConfig {
        template: scenario.population.template,
        size: scenario.population.size,
        seed: scenario.population.seed,
        name: scenario.population.name.clone(),
    }) else {
        return StatusCode::BAD_REQUEST.into_response();
    };
    let mut ranked: Vec<(usize, usize)> = graph
        .outgoing
        .iter()
        .enumerate()
        .map(|(index, edges)| (edges.len(), index))
        .collect();
    ranked.sort_by_key(|item| std::cmp::Reverse(item.0));
    let keep: std::collections::BTreeSet<usize> = ranked
        .into_iter()
        .take(280)
        .map(|(_, index)| index)
        .collect();
    let nodes: Vec<Value> = keep
        .iter()
        .map(|index| {
            let entity = &graph.entities[*index];
            json!({
                "id": entity.id.0,
                "name": entity.name,
                "type": entity.entity_type.label(),
                "state": entity.state.label(),
                "criticality": entity.criticality,
                "privilege": entity.privilege,
                "susceptibility": entity.susceptibility,
                "degree": graph.outgoing[*index].len(),
            })
        })
        .collect();
    let mut edges = Vec::new();
    for edge in &graph.edges {
        if keep.contains(&edge.src.index())
            && keep.contains(&edge.dst.index())
            && edges.len() < 1600
        {
            edges.push(json!({
                "source": edge.src.0,
                "target": edge.dst.0,
                "kind": edge.kind.label(),
            }));
        }
    }
    Json(json!({
        "population": graph.node_count(),
        "shown": nodes.len(),
        "note": "Display graph is a high-degree sample of the synthetic population. It is not the infrastructure cluster.",
        "nodes": nodes,
        "edges": edges,
    })).into_response()
}

async fn experiment_trace(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Query(query): Query<TraceQuery>,
) -> Response {
    if require_user(&state, &headers).await.is_err() {
        return StatusCode::UNAUTHORIZED.into_response();
    }
    let Ok(Some(row)) = state.db.experiment(id).await else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let Ok(scenario) = load_scenario_str(&row.scenario_yaml) else {
        return StatusCode::BAD_REQUEST.into_response();
    };
    let variant = if query.variant.is_empty() {
        scenario
            .variants
            .first()
            .map(|variant| variant.name.clone())
            .unwrap_or_else(|| scenario.metadata.name.clone())
    } else {
        query.variant
    };
    match epi_scenario::run_trace(&scenario, &variant, query.run, scenario.simulation.seed) {
        Ok(summary) => Json(json!({
            "seed": summary.seed,
            "outbreak_size": summary.outbreak_size,
            "attack_rate": summary.attack_rate,
            "events": summary.events,
            "samples": summary.samples,
        }))
        .into_response(),
        Err(err) => (StatusCode::BAD_REQUEST, err.to_string()).into_response(),
    }
}

async fn demo(State(state): State<AppState>, headers: HeaderMap) -> Response {
    if require_user(&state, &headers).await.is_err() {
        return StatusCode::UNAUTHORIZED.into_response();
    }
    let yaml = include_str!("../../../scenarios/demos/ui-demo.yaml");
    match state
        .db
        .create_experiment(yaml, state.config.batch_size)
        .await
    {
        Ok((id, jobs)) => {
            if let Err(err) = publish_batches(&state.nats, &jobs).await {
                return (StatusCode::BAD_GATEWAY, err.to_string()).into_response();
            }
            (
                StatusCode::CREATED,
                Json(json!({"id": id, "batches": jobs.len()})),
            )
                .into_response()
        }
        Err(err) => (StatusCode::BAD_REQUEST, err.to_string()).into_response(),
    }
}

async fn cluster_status() -> Json<Value> {
    Json(json!({
        "infrastructure": "separate from the simulated population",
        "import": "read-only metadata only; secret values are never requested",
        "kubeconfig": std::env::var("KUBECONFIG").ok(),
        "note": "One Kubernetes pod is not one simulated host."
    }))
}

async fn settings(State(state): State<AppState>) -> Json<Value> {
    Json(json!({
        "version": VERSION,
        "generator_version": epi_core::GENERATOR_VERSION,
        "model_version": epi_core::MODEL_VERSION,
        "assumption": ASSUMPTION,
        "public_origin": state.config.public_origin,
        "auth": "local",
        "oidc": "reserved; not required for the homelab release"
    }))
}

async fn metrics_handler(State(state): State<AppState>) -> String {
    let encoder = TextEncoder::new();
    let mut buffer = Vec::new();
    let _ = encoder.encode(&state.metrics.registry.gather(), &mut buffer);
    String::from_utf8(buffer).unwrap_or_default()
}

async fn openapi() -> impl IntoResponse {
    (
        [(header::CONTENT_TYPE, "application/json")],
        include_str!("../../../docs/openapi.json"),
    )
}

async fn api_docs() -> Html {
    Html(include_str!("api-docs.html"))
}

struct Html(&'static str);
impl IntoResponse for Html {
    fn into_response(self) -> Response {
        ([(header::CONTENT_TYPE, "text/html; charset=utf-8")], self.0).into_response()
    }
}

#[derive(Deserialize)]
struct WsQuery {
    #[serde(default)]
    token: String,
}

async fn ws(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<WsQuery>,
    upgrade: WebSocketUpgrade,
) -> Response {
    let header_ok = require_user(&state, &headers).await.is_ok();
    let query_ok = if query.token.is_empty() {
        false
    } else {
        state
            .db
            .user_for_token(&query.token)
            .await
            .ok()
            .flatten()
            .is_some()
    };
    if !header_ok && !query_ok {
        return StatusCode::UNAUTHORIZED.into_response();
    }
    let mut rx = state.progress.subscribe();
    upgrade.on_upgrade(move |mut socket| async move {
        loop {
            match rx.recv().await {
                Ok(text) => {
                    if socket.send(Message::Text(text.into())).await.is_err() {
                        break;
                    }
                }
                Err(broadcast::error::RecvError::Lagged(_)) => continue,
                Err(broadcast::error::RecvError::Closed) => break,
            }
        }
    })
}
