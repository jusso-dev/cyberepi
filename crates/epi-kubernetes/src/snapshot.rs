//! A cluster snapshot contains names and relationships, never secret bytes.

use epi_core::{Channel, EntityType, RelationKind};
use epi_graph::{GraphBuilder, PopulationGraph};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ClusterSnapshot {
    pub nodes: Vec<String>,
    pub namespaces: Vec<KNamespace>,
    pub pods: Vec<KPod>,
    pub deployments: Vec<KDeployment>,
    pub services: Vec<KService>,
    pub service_accounts: Vec<KServiceAccount>,
    pub roles: Vec<KRole>,
    pub role_bindings: Vec<KRoleBinding>,
    /// Name, namespace, and type only. There is no data field on purpose.
    pub secrets: Vec<KSecretMeta>,
    pub network_policies: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct KNamespace {
    pub name: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct KPod {
    pub name: String,
    pub namespace: String,
    pub node: Option<String>,
    pub service_account: Option<String>,
    pub deployment: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct KDeployment {
    pub name: String,
    pub namespace: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct KService {
    pub name: String,
    pub namespace: String,
    pub pods: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct KServiceAccount {
    pub name: String,
    pub namespace: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct KRole {
    pub name: String,
    pub namespace: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct KRoleBinding {
    pub name: String,
    pub namespace: String,
    pub subjects: Vec<String>,
    pub role: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct KSecretMeta {
    pub name: String,
    pub namespace: String,
    pub secret_type: String,
}

pub fn snapshot_to_population(snapshot: &ClusterSnapshot) -> PopulationGraph {
    let mut builder = GraphBuilder::new();
    let mut namespaces = std::collections::BTreeMap::new();
    for ns in &snapshot.namespaces {
        let id = builder.add_typed(&ns.name, EntityType::KubernetesNamespace);
        namespaces.insert(ns.name.clone(), id);
    }
    let mut accounts = std::collections::BTreeMap::new();
    for account in &snapshot.service_accounts {
        let id = builder.add_typed(
            format!("{}/{}", account.namespace, account.name),
            EntityType::KubernetesServiceAccount,
        );
        accounts.insert((account.namespace.clone(), account.name.clone()), id);
        if let Some(ns) = namespaces.get(&account.namespace) {
            builder.connect(
                id,
                *ns,
                RelationKind::MemberOf,
                Channel::Cloud,
                0.4,
                0.5,
                0.4,
                0.3,
                0.6,
            );
        }
    }
    let mut roles = std::collections::BTreeMap::new();
    for role in &snapshot.roles {
        let id = builder.add_typed(
            format!("role:{}/{}", role.namespace, role.name),
            EntityType::SecurityControl,
        );
        roles.insert((role.namespace.clone(), role.name.clone()), id);
    }
    for binding in &snapshot.role_bindings {
        let role = roles
            .get(&(binding.namespace.clone(), binding.role.clone()))
            .copied();
        for subject in &binding.subjects {
            if let Some(account) = accounts.get(&(binding.namespace.clone(), subject.clone())) {
                if let Some(role_id) = role {
                    builder.connect(
                        *account,
                        role_id,
                        RelationKind::KubernetesRbac,
                        Channel::Cloud,
                        0.8,
                        0.7,
                        0.5,
                        0.2,
                        0.85,
                    );
                }
            }
        }
    }
    let mut pod_ids = std::collections::BTreeMap::new();
    for pod in &snapshot.pods {
        let id = builder.add_typed(
            format!("{}/{}", pod.namespace, pod.name),
            EntityType::KubernetesPod,
        );
        if let Some(ns) = namespaces.get(&pod.namespace) {
            builder.connect(
                id,
                *ns,
                RelationKind::RunsOn,
                Channel::Cloud,
                0.5,
                0.4,
                0.4,
                0.25,
                0.7,
            );
        }
        if let Some(account_name) = &pod.service_account {
            if let Some(account) = accounts.get(&(pod.namespace.clone(), account_name.clone())) {
                builder.connect(
                    id,
                    *account,
                    RelationKind::UsesIdentity,
                    Channel::Identity,
                    0.9,
                    0.6,
                    0.5,
                    0.15,
                    0.8,
                );
            }
        }
        pod_ids.insert((pod.namespace.clone(), pod.name.clone()), id);
    }
    for service in &snapshot.services {
        let id = builder.add_typed(
            format!("svc:{}/{}", service.namespace, service.name),
            EntityType::Api,
        );
        for pod_name in &service.pods {
            if let Some(pod_id) = pod_ids.get(&(service.namespace.clone(), pod_name.clone())) {
                builder.connect(
                    id,
                    *pod_id,
                    RelationKind::ConnectsTo,
                    Channel::Network,
                    0.6,
                    0.4,
                    0.4,
                    0.2,
                    0.7,
                );
            }
        }
    }
    for secret in &snapshot.secrets {
        let id = builder.add_typed(
            format!("secret-meta:{}/{}", secret.namespace, secret.name),
            EntityType::StorageService,
        );
        if let Some(ns) = namespaces.get(&secret.namespace) {
            builder.connect(
                id,
                *ns,
                RelationKind::MemberOf,
                Channel::Cloud,
                0.2,
                0.2,
                0.2,
                0.5,
                0.3,
            );
        }
        let _ = &secret.secret_type;
    }
    builder.finish()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rbac_chain_is_a_contact_path_and_secrets_have_no_values() {
        let snapshot = ClusterSnapshot {
            namespaces: vec![KNamespace { name: "lab".into() }],
            pods: vec![KPod {
                name: "api".into(),
                namespace: "lab".into(),
                node: Some("node-a".into()),
                service_account: Some("api".into()),
                deployment: Some("api".into()),
            }],
            service_accounts: vec![KServiceAccount {
                name: "api".into(),
                namespace: "lab".into(),
            }],
            roles: vec![KRole {
                name: "read".into(),
                namespace: "lab".into(),
            }],
            role_bindings: vec![KRoleBinding {
                name: "bind".into(),
                namespace: "lab".into(),
                subjects: vec!["api".into()],
                role: "read".into(),
            }],
            secrets: vec![KSecretMeta {
                name: "app".into(),
                namespace: "lab".into(),
                secret_type: "Opaque".into(),
            }],
            ..ClusterSnapshot::default()
        };
        let graph = snapshot_to_population(&snapshot);
        assert!(graph.entities.iter().any(|entity| entity.name == "lab/api"));
        assert!(graph
            .edges
            .iter()
            .any(|edge| edge.kind == RelationKind::UsesIdentity));
        assert!(graph
            .edges
            .iter()
            .any(|edge| edge.kind == RelationKind::KubernetesRbac));
        let rendered = serde_json::to_string(&snapshot).unwrap();
        assert!(!rendered.contains("\"data\""));
        assert!(!rendered.contains("stringData"));
    }
}
