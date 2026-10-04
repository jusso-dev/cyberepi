//! Read-only translation of Kubernetes metadata into a synthetic contact graph.
//!
//! This crate never requests secret values. Live discovery, when enabled,
//! asks the API for `PartialObjectMetadata` of Secret objects so `.data` and
//! `.stringData` are not part of the response schema used here.

mod snapshot;

pub use snapshot::{
    snapshot_to_population, ClusterSnapshot, KDeployment, KNamespace, KPod, KRole, KRoleBinding,
    KSecretMeta, KService, KServiceAccount,
};

#[cfg(feature = "live")]
mod live;

#[cfg(feature = "live")]
pub use live::import_current_cluster;
