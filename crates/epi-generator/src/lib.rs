#![allow(clippy::needless_range_loop)]
//! Synthetic organisation generator.
//!
//! Populations are fictional. Names such as `svc-ci-prod` identify graph hubs
//! inside a scenario. They are not accounts on any real system.

mod templates;

use epi_core::{Channel, EntityType, RelationKind, SimDuration};
use epi_graph::{GraphBuilder, PopulationGraph};
use rand::RngExt;
use rand::SeedableRng;
use rand_chacha::ChaCha8Rng;
use serde::{Deserialize, Serialize};
use thiserror::Error;

pub use templates::{template, OrgTemplate, TemplateProfile, TEMPLATE_IDS};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GeneratorConfig {
    pub template: OrgTemplate,
    pub size: u32,
    pub seed: u64,
    pub name: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct OrganisationSummary {
    pub name: String,
    pub template: String,
    pub generator_version: String,
    pub seed: u64,
    pub entities: usize,
    pub edges: usize,
    pub counts: Vec<(String, usize)>,
    pub largest_component: f64,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum GenerateError {
    #[error("population size must be at least {0}")]
    TooSmall(u32),
    #[error("population size must be at most {0}")]
    TooLarge(u32),
}

pub fn summarise(config: &GeneratorConfig, graph: &PopulationGraph) -> OrganisationSummary {
    let mut counts = Vec::new();
    for kind in EntityType::ALL {
        let count = graph
            .entities
            .iter()
            .filter(|entity| entity.entity_type == kind)
            .count();
        if count > 0 {
            counts.push((kind.label().to_string(), count));
        }
    }
    OrganisationSummary {
        name: config.name.clone(),
        template: config.template.id().to_string(),
        generator_version: epi_core::GENERATOR_VERSION.to_string(),
        seed: config.seed,
        entities: graph.node_count(),
        edges: graph.edge_count(),
        counts,
        largest_component: graph.largest_component_fraction(),
    }
}

pub fn generate(config: &GeneratorConfig) -> Result<PopulationGraph, GenerateError> {
    if config.size < 32 {
        return Err(GenerateError::TooSmall(32));
    }
    if config.size > 1_000_000 {
        return Err(GenerateError::TooLarge(1_000_000));
    }
    let profile = template(config.template);
    let mut rng = ChaCha8Rng::seed_from_u64(config.seed);
    let mut builder = GraphBuilder::new();
    let maturity = profile.maturity;

    let idp = push(
        &mut builder,
        "idp-primary",
        EntityType::IdentityProvider,
        maturity,
        0.95,
        0.98,
    );
    let idp_admin = push(
        &mut builder,
        "idp-admin-04",
        EntityType::Identity,
        maturity,
        0.97,
        0.9,
    );
    let backup = push(
        &mut builder,
        "backup-controller",
        EntityType::StorageService,
        maturity,
        0.72,
        0.9,
    );
    let ci = push(
        &mut builder,
        "svc-ci-prod",
        EntityType::ServiceAccount,
        maturity,
        0.93,
        0.86,
    );
    let automation = push(
        &mut builder,
        "shared-automation-01",
        EntityType::ServiceAccount,
        maturity,
        0.9,
        0.8,
    );
    let k8s_admin = push(
        &mut builder,
        "k8s-cluster-admin",
        EntityType::KubernetesServiceAccount,
        maturity,
        0.96,
        0.92,
    );
    let email = push(
        &mut builder,
        "email-gateway",
        EntityType::NetworkDevice,
        maturity,
        0.4,
        0.7,
    );
    let rmm = push(
        &mut builder,
        "rmm-server",
        EntityType::Server,
        maturity,
        0.88,
        0.84,
    );
    let git_owner = push(
        &mut builder,
        "github-org-owner",
        EntityType::Identity,
        maturity,
        0.9,
        0.75,
    );
    let hypervisor = push(
        &mut builder,
        "hypervisor-01",
        EntityType::Server,
        maturity,
        0.8,
        0.88,
    );
    let edr = push(
        &mut builder,
        "edr-console",
        EntityType::SecurityControl,
        maturity,
        0.7,
        0.8,
    );
    let _pam = push(
        &mut builder,
        "pam-vault",
        EntityType::SecurityControl,
        maturity,
        0.85,
        0.86,
    );

    let segments = profile.segments.max(1) as usize;
    let budget = config
        .size
        .saturating_sub(builder.len() as u32 + segments as u32) as usize;
    let quotas = profile.quotas(budget);
    let mut users = Vec::new();
    let mut endpoints = Vec::new();
    let mut servers = Vec::new();
    let mut saas = Vec::new();
    let mut repos = Vec::new();
    let mut pipelines = Vec::new();
    let mut namespaces = Vec::new();
    let mut pods = Vec::new();
    let mut service_accounts = Vec::new();
    let mut admins = Vec::new();

    for index in 0..quotas.users {
        users.push(push(
            &mut builder,
            format!("user-{index:04}"),
            EntityType::User,
            maturity,
            0.18,
            0.25,
        ));
    }
    for index in 0..quotas.endpoints {
        endpoints.push(push(
            &mut builder,
            format!("endpoint-{index:04}"),
            EntityType::Endpoint,
            maturity,
            0.15,
            0.4,
        ));
    }
    for index in 0..quotas.servers {
        servers.push(push(
            &mut builder,
            format!("srv-{index:04}"),
            EntityType::Server,
            maturity,
            0.45,
            0.62,
        ));
    }
    for index in 0..quotas.admins {
        let id = push(
            &mut builder,
            format!("admin-{index:03}"),
            EntityType::Identity,
            maturity,
            0.82,
            0.7,
        );
        admins.push(id);
    }
    for index in 0..quotas.service_accounts {
        service_accounts.push(push(
            &mut builder,
            format!("svc-{index:03}"),
            EntityType::ServiceAccount,
            maturity,
            0.7,
            0.55,
        ));
    }
    for index in 0..quotas.saas {
        saas.push(push(
            &mut builder,
            format!("saas-{index:03}"),
            EntityType::SaaSApplication,
            maturity,
            0.3,
            0.5,
        ));
    }
    for index in 0..quotas.repos {
        repos.push(push(
            &mut builder,
            format!("repo-{index:03}"),
            EntityType::Repository,
            maturity,
            0.35,
            0.48,
        ));
    }
    for index in 0..quotas.pipelines {
        pipelines.push(push(
            &mut builder,
            format!("pipeline-{index:03}"),
            EntityType::CiCdPipeline,
            maturity,
            0.55,
            0.6,
        ));
    }
    for index in 0..quotas.namespaces {
        namespaces.push(push(
            &mut builder,
            format!("ns-{index:02}"),
            EntityType::KubernetesNamespace,
            maturity,
            0.4,
            0.55,
        ));
    }
    for index in 0..quotas.workloads {
        pods.push(push(
            &mut builder,
            format!("pod-{index:03}"),
            EntityType::KubernetesPod,
            maturity,
            0.25,
            0.45,
        ));
    }
    let mut routers = Vec::new();
    for index in 0..segments {
        if builder.len() >= config.size as usize {
            break;
        }
        routers.push(push(
            &mut builder,
            format!("segment-router-{index:02}"),
            EntityType::NetworkDevice,
            maturity,
            0.35,
            0.5,
        ));
    }
    while builder.len() < config.size as usize {
        let index = builder.len();
        let kind = match index % 5 {
            0 => EntityType::CloudWorkload,
            1 => EntityType::Container,
            2 => EntityType::Database,
            3 => EntityType::Api,
            _ => EntityType::User,
        };
        let id = push(
            &mut builder,
            format!("extra-{index}"),
            kind,
            maturity,
            0.3,
            0.4,
        );
        if kind == EntityType::User {
            users.push(id);
        }
    }

    let seg_strength = (0.15 + 0.7 * maturity).clamp(0.05, 0.9);
    for (index, user) in users.iter().enumerate() {
        if let Some(endpoint) = endpoints.get(index % endpoints.len().max(1)) {
            if !endpoints.is_empty() {
                link(
                    &mut builder,
                    *user,
                    *endpoint,
                    RelationKind::AuthenticatesTo,
                    Channel::Identity,
                    2.4,
                    0.55,
                    0.65,
                    0.08,
                    1.0,
                );
                link(
                    &mut builder,
                    *endpoint,
                    *user,
                    RelationKind::UsesIdentity,
                    Channel::Identity,
                    0.45,
                    0.3,
                    0.4,
                    0.08,
                    0.7,
                );
            }
        }
        link(
            &mut builder,
            *user,
            email,
            RelationKind::ConnectsTo,
            Channel::Email,
            2.2,
            0.4,
            0.5,
            0.05,
            0.9,
        );
        link(
            &mut builder,
            email,
            *user,
            RelationKind::ConnectsTo,
            Channel::Email,
            0.8,
            0.25,
            0.35,
            0.05,
            0.6,
        );
        link(
            &mut builder,
            *user,
            idp,
            RelationKind::AuthenticatesTo,
            Channel::Identity,
            2.8,
            0.7,
            0.8,
            0.04,
            1.0,
        );
        link(
            &mut builder,
            idp,
            *user,
            RelationKind::UsesIdentity,
            Channel::Identity,
            1.15,
            0.55,
            0.6,
            0.04,
            0.85,
        );
        if !saas.is_empty() {
            let apps = 2 + (index % 2);
            for hop in 0..apps {
                let app = saas[(index + hop * 3) % saas.len()];
                link(
                    &mut builder,
                    *user,
                    app,
                    RelationKind::CallsApi,
                    Channel::Saas,
                    0.7,
                    0.35,
                    0.45,
                    0.1,
                    0.75,
                );
            }
        }
        if !routers.is_empty() {
            let router = routers[index % routers.len()];
            if let Some(endpoint) = endpoints.get(index % endpoints.len().max(1)) {
                if !endpoints.is_empty() {
                    link(
                        &mut builder,
                        *endpoint,
                        router,
                        RelationKind::SharesNetwork,
                        Channel::Network,
                        0.8,
                        0.25,
                        0.3,
                        seg_strength,
                        0.7,
                    );
                }
            }
        }
    }

    for (index, server) in servers.iter().enumerate() {
        link(
            &mut builder,
            *server,
            hypervisor,
            RelationKind::RunsOn,
            Channel::Endpoint,
            0.35,
            0.7,
            0.5,
            0.15,
            0.8,
        );
        if servers.len() > 1 {
            let other = servers[(index + 1) % servers.len()];
            link(
                &mut builder,
                *server,
                other,
                RelationKind::DependsOn,
                Channel::Endpoint,
                0.4,
                0.4,
                0.35,
                seg_strength * 0.5,
                0.65,
            );
        }
        if !routers.is_empty() {
            let router = routers[index % routers.len()];
            link(
                &mut builder,
                *server,
                router,
                RelationKind::SharesNetwork,
                Channel::Network,
                0.55,
                0.4,
                0.3,
                seg_strength,
                0.6,
            );
        }
    }

    for router_index in 0..routers.len().saturating_sub(1) {
        link(
            &mut builder,
            routers[router_index],
            routers[router_index + 1],
            RelationKind::ConnectsTo,
            Channel::Network,
            0.5,
            0.4,
            0.3,
            seg_strength,
            0.55,
        );
    }

    let admin_fanout = 36.min(servers.len().max(1));
    for (index, admin) in admins.iter().enumerate() {
        link(
            &mut builder,
            *admin,
            idp,
            RelationKind::Administers,
            Channel::Identity,
            1.3,
            0.9,
            0.8,
            0.05,
            1.0,
        );
        for hop in 0..admin_fanout {
            if servers.is_empty() {
                break;
            }
            let server = servers[(index * 7 + hop) % servers.len()];
            link(
                &mut builder,
                *admin,
                server,
                RelationKind::Administers,
                Channel::Identity,
                1.8,
                0.9,
                0.7,
                0.1,
                1.0,
            );
            link(
                &mut builder,
                server,
                *admin,
                RelationKind::Trusts,
                Channel::Identity,
                0.55,
                0.5,
                0.45,
                0.12,
                0.65,
            );
        }
    }

    let user_sample = 400.min(users.len());
    for index in 0..user_sample {
        link(
            &mut builder,
            idp_admin,
            users[index],
            RelationKind::HasAccessTo,
            Channel::Identity,
            0.55,
            0.8,
            0.7,
            0.08,
            0.9,
        );
    }
    link(
        &mut builder,
        idp_admin,
        idp,
        RelationKind::Administers,
        Channel::Identity,
        2.0,
        0.95,
        0.9,
        0.02,
        1.0,
    );

    let backup_fanout = 220.min(servers.len());
    for index in 0..backup_fanout {
        link(
            &mut builder,
            backup,
            servers[index],
            RelationKind::MountsStorage,
            Channel::Cloud,
            0.7,
            0.75,
            0.55,
            0.2,
            0.85,
        );
    }
    let endpoint_sample = 180.min(endpoints.len());
    for index in 0..endpoint_sample {
        link(
            &mut builder,
            rmm,
            endpoints[index],
            RelationKind::Manages,
            Channel::Endpoint,
            0.65,
            0.8,
            0.5,
            0.18,
            0.8,
        );
    }

    for pipeline in &pipelines {
        link(
            &mut builder,
            ci,
            *pipeline,
            RelationKind::CiCdAccess,
            Channel::SupplyChain,
            1.6,
            0.9,
            0.7,
            0.08,
            1.0,
        );
    }
    for repo in &repos {
        link(
            &mut builder,
            ci,
            *repo,
            RelationKind::RepositoryAccess,
            Channel::SupplyChain,
            1.1,
            0.75,
            0.6,
            0.08,
            0.9,
        );
        link(
            &mut builder,
            git_owner,
            *repo,
            RelationKind::RepositoryAccess,
            Channel::SupplyChain,
            0.8,
            0.85,
            0.7,
            0.05,
            0.95,
        );
    }

    let auto_targets = 160.min(servers.len() + saas.len());
    for index in 0..auto_targets {
        if !servers.is_empty() && index % 2 == 0 {
            link(
                &mut builder,
                automation,
                servers[index % servers.len()],
                RelationKind::HasAccessTo,
                Channel::Identity,
                0.9,
                0.7,
                0.55,
                0.1,
                0.85,
            );
        } else if !saas.is_empty() {
            link(
                &mut builder,
                automation,
                saas[index % saas.len()],
                RelationKind::CallsApi,
                Channel::Saas,
                0.8,
                0.6,
                0.5,
                0.1,
                0.8,
            );
        }
    }

    for ns in &namespaces {
        link(
            &mut builder,
            k8s_admin,
            *ns,
            RelationKind::KubernetesRbac,
            Channel::Cloud,
            1.2,
            0.9,
            0.75,
            0.12,
            1.0,
        );
    }
    for (index, pod) in pods.iter().enumerate() {
        link(
            &mut builder,
            k8s_admin,
            *pod,
            RelationKind::KubernetesRbac,
            Channel::Cloud,
            0.45,
            0.7,
            0.5,
            0.15,
            0.75,
        );
        if !namespaces.is_empty() {
            link(
                &mut builder,
                *pod,
                namespaces[index % namespaces.len()],
                RelationKind::RunsOn,
                Channel::Cloud,
                0.3,
                0.4,
                0.4,
                0.2,
                0.6,
            );
        }
    }
    for account in &service_accounts {
        link(
            &mut builder,
            *account,
            idp,
            RelationKind::AuthenticatesTo,
            Channel::Identity,
            0.6,
            0.65,
            0.55,
            0.08,
            0.8,
        );
    }

    link(
        &mut builder,
        edr,
        email,
        RelationKind::ConnectsTo,
        Channel::Network,
        0.2,
        0.3,
        0.4,
        0.3,
        0.4,
    );

    // A few insider-weighted trust edges from the RNG, deterministic.
    let trust_edges = (users.len() / 6).clamp(8, 2_500);
    if users.len() > 2 {
        for _ in 0..trust_edges {
            let left = users[rng.random_range(0..users.len())];
            let right = users[rng.random_range(0..users.len())];
            if left != right {
                link(
                    &mut builder,
                    left,
                    right,
                    RelationKind::Trusts,
                    Channel::Insider,
                    0.9,
                    0.35,
                    0.8,
                    0.05,
                    0.85,
                );
            }
        }
    }

    let mut graph = builder.finish();
    // The segment routers may have pushed the population over `size`.
    // Quotas reserve that room; if a bug overshoots, keep the graph but the
    // size check below surfaces it in tests.
    let _ = &mut graph;
    Ok(graph)
}

fn push(
    builder: &mut GraphBuilder,
    name: impl Into<String>,
    kind: EntityType,
    maturity: f64,
    privilege: f64,
    criticality: f64,
) -> epi_core::EntityId {
    let id = builder.add_typed(name, kind);
    let entity = builder.entity_mut(id);
    entity.privilege = privilege;
    entity.criticality = criticality;
    entity.detection_probability = (0.18 + 0.62 * maturity).clamp(0.05, 0.95);
    entity.patch_level = (0.12 + 0.7 * maturity).clamp(0.0, 0.95);
    entity.isolation_probability = (0.45 + 0.45 * maturity).clamp(0.1, 0.98);
    let detect_hours = (64.0 - 36.0 * maturity).clamp(16.0, 72.0) as u64;
    let isolate_hours = (28.0 - 18.0 * maturity).clamp(4.0, 36.0) as u64;
    entity.detection_delay = SimDuration::hours(detect_hours.max(1));
    entity.isolation_delay = SimDuration::hours(isolate_hours.max(1));
    entity.recovery_time = SimDuration::hours((48.0 - 20.0 * maturity).clamp(12.0, 72.0) as u64);
    if kind.is_identity_like() {
        entity.infectiousness = 0.9;
        entity.susceptibility = 0.88;
    }
    id
}

#[allow(clippy::too_many_arguments)]
fn link(
    builder: &mut GraphBuilder,
    src: epi_core::EntityId,
    dst: epi_core::EntityId,
    kind: RelationKind,
    channel: Channel,
    contact: f64,
    access: f64,
    trust: f64,
    segmentation: f64,
    modifier: f64,
) {
    builder.connect(
        src,
        dst,
        kind,
        channel,
        contact,
        access,
        trust,
        segmentation,
        modifier,
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn size_is_exact_and_reproducible() {
        let config = GeneratorConfig {
            template: OrgTemplate::MediumEnterprise,
            size: 400,
            seed: 11,
            name: "Synthetic Medium".to_string(),
        };
        let first = generate(&config).unwrap();
        let second = generate(&config).unwrap();
        assert_eq!(first.node_count(), 400);
        assert_eq!(first.edge_count(), second.edge_count());
        assert_eq!(first.entities[0].name, second.entities[0].name);
        assert!(first
            .entities
            .iter()
            .any(|entity| entity.name == "svc-ci-prod"));
        assert!(first.edge_count() > first.node_count());
        assert!(first.largest_component_fraction() > 0.5);
        let summary = summarise(&config, &first);
        assert_eq!(summary.generator_version, "2");
    }

    #[test]
    fn rejects_tiny_populations() {
        let config = GeneratorConfig {
            template: OrgTemplate::MicroBusiness,
            size: 8,
            seed: 1,
            name: "tiny".to_string(),
        };
        assert!(generate(&config).is_err());
    }
}
