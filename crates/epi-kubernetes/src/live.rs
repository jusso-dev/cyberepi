//! Live discovery. Secret objects are requested as partial metadata so the
//! client type has no `.data` field. Do not replace this with `Api<Secret>`.

use crate::snapshot::{
    snapshot_to_population, ClusterSnapshot, KDeployment, KNamespace, KPod, KRole, KRoleBinding,
    KSecretMeta, KService, KServiceAccount,
};
use epi_graph::PopulationGraph;
use k8s_openapi::api::apps::v1::Deployment;
use k8s_openapi::api::core::v1::{Namespace, Pod, Secret, Service, ServiceAccount};
use k8s_openapi::api::rbac::v1::{Role, RoleBinding};
use kube::api::{Api, ListParams, PartialObjectMeta};
use kube::Client;

pub async fn import_current_cluster() -> Result<PopulationGraph, kube::Error> {
    let client = Client::try_default().await?;
    let namespaces: Api<Namespace> = Api::all(client.clone());
    let pods: Api<Pod> = Api::all(client.clone());
    let services: Api<Service> = Api::all(client.clone());
    let accounts: Api<ServiceAccount> = Api::all(client.clone());
    let roles: Api<Role> = Api::all(client.clone());
    let bindings: Api<RoleBinding> = Api::all(client.clone());
    let deployments: Api<Deployment> = Api::all(client.clone());
    let secrets: Api<PartialObjectMeta<Secret>> = Api::all(client.clone());
    let params = ListParams::default();

    let mut snapshot = ClusterSnapshot::default();
    for item in namespaces.list(&params).await?.items {
        if let Some(name) = item.metadata.name {
            snapshot.namespaces.push(KNamespace { name });
        }
    }
    for item in pods.list(&params).await?.items {
        let Some(name) = item.metadata.name else {
            continue;
        };
        let Some(namespace) = item.metadata.namespace else {
            continue;
        };
        snapshot.pods.push(KPod {
            name,
            namespace,
            node: item.spec.as_ref().and_then(|spec| spec.node_name.clone()),
            service_account: item
                .spec
                .as_ref()
                .and_then(|spec| spec.service_account_name.clone()),
            deployment: None,
        });
    }
    for item in services.list(&params).await?.items {
        let Some(name) = item.metadata.name else {
            continue;
        };
        let Some(namespace) = item.metadata.namespace else {
            continue;
        };
        snapshot.services.push(KService {
            name,
            namespace,
            pods: Vec::new(),
        });
    }
    for item in accounts.list(&params).await?.items {
        if let (Some(name), Some(namespace)) = (item.metadata.name, item.metadata.namespace) {
            snapshot
                .service_accounts
                .push(KServiceAccount { name, namespace });
        }
    }
    for item in roles.list(&params).await?.items {
        if let (Some(name), Some(namespace)) = (item.metadata.name, item.metadata.namespace) {
            snapshot.roles.push(KRole { name, namespace });
        }
    }
    for item in bindings.list(&params).await?.items {
        let Some(name) = item.metadata.name else {
            continue;
        };
        let Some(namespace) = item.metadata.namespace else {
            continue;
        };
        let role = item.role_ref.name;
        let subjects = item
            .subjects
            .unwrap_or_default()
            .into_iter()
            .map(|subject| subject.name)
            .collect();
        snapshot.role_bindings.push(KRoleBinding {
            name,
            namespace,
            subjects,
            role,
        });
    }
    for item in deployments.list(&params).await?.items {
        if let (Some(name), Some(namespace)) = (item.metadata.name, item.metadata.namespace) {
            snapshot.deployments.push(KDeployment { name, namespace });
        }
    }
    for item in secrets.list(&params).await?.items {
        if let (Some(name), Some(namespace)) = (item.metadata.name, item.metadata.namespace) {
            snapshot.secrets.push(KSecretMeta {
                name,
                namespace,
                secret_type: "PartialObjectMetadata".into(),
            });
        }
    }
    Ok(snapshot_to_population(&snapshot))
}
