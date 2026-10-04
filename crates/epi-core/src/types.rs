use serde::{Deserialize, Serialize};

use crate::SimDuration;

/// Stable index of an entity inside one population graph.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct EntityId(pub u32);

impl EntityId {
    pub const fn new(index: u32) -> Self {
        Self(index)
    }

    pub const fn index(self) -> usize {
        self.0 as usize
    }
}

impl From<u32> for EntityId {
    fn from(value: u32) -> Self {
        Self(value)
    }
}

/// Epidemiological compartment for the SEIDQRP model and its presets.
///
/// An entity occupies exactly one compartment. The simulator never treats
/// these labels as permissions to run code on a real system.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[repr(u8)]
pub enum EpiState {
    Susceptible = 0,
    Exposed = 1,
    Infectious = 2,
    Detected = 3,
    Quarantined = 4,
    Recovered = 5,
    Protected = 6,
}

impl EpiState {
    pub const ALL: [EpiState; 7] = [
        Self::Susceptible,
        Self::Exposed,
        Self::Infectious,
        Self::Detected,
        Self::Quarantined,
        Self::Recovered,
        Self::Protected,
    ];

    pub const fn as_index(self) -> usize {
        self as u8 as usize
    }

    pub const fn from_index(index: u8) -> Option<Self> {
        match index {
            0 => Some(Self::Susceptible),
            1 => Some(Self::Exposed),
            2 => Some(Self::Infectious),
            3 => Some(Self::Detected),
            4 => Some(Self::Quarantined),
            5 => Some(Self::Recovered),
            6 => Some(Self::Protected),
            _ => None,
        }
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::Susceptible => "susceptible",
            Self::Exposed => "exposed",
            Self::Infectious => "compromised",
            Self::Detected => "detected",
            Self::Quarantined => "quarantined",
            Self::Recovered => "recovered",
            Self::Protected => "protected",
        }
    }

    pub const fn cyber_equivalent(self) -> &'static str {
        match self {
            Self::Susceptible => "reachable and not yet exposed",
            Self::Exposed => "exposed, not yet able to propagate",
            Self::Infectious => "compromised and able to propagate",
            Self::Detected => "compromise noticed, still able to propagate",
            Self::Quarantined => "isolated from the contact graph",
            Self::Recovered => "restored, temporary immunity",
            Self::Protected => "patched or otherwise immune",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EntityType {
    User,
    Identity,
    Endpoint,
    Server,
    NetworkDevice,
    CloudWorkload,
    Container,
    KubernetesPod,
    KubernetesServiceAccount,
    KubernetesNamespace,
    SaaSApplication,
    Database,
    Repository,
    CiCdPipeline,
    ServiceAccount,
    Api,
    IdentityProvider,
    StorageService,
    SecurityControl,
}

impl EntityType {
    pub const ALL: [EntityType; 19] = [
        Self::User,
        Self::Identity,
        Self::Endpoint,
        Self::Server,
        Self::NetworkDevice,
        Self::CloudWorkload,
        Self::Container,
        Self::KubernetesPod,
        Self::KubernetesServiceAccount,
        Self::KubernetesNamespace,
        Self::SaaSApplication,
        Self::Database,
        Self::Repository,
        Self::CiCdPipeline,
        Self::ServiceAccount,
        Self::Api,
        Self::IdentityProvider,
        Self::StorageService,
        Self::SecurityControl,
    ];

    pub const fn label(self) -> &'static str {
        match self {
            Self::User => "user",
            Self::Identity => "identity",
            Self::Endpoint => "endpoint",
            Self::Server => "server",
            Self::NetworkDevice => "network device",
            Self::CloudWorkload => "cloud workload",
            Self::Container => "container",
            Self::KubernetesPod => "kubernetes pod",
            Self::KubernetesServiceAccount => "kubernetes service account",
            Self::KubernetesNamespace => "kubernetes namespace",
            Self::SaaSApplication => "saas application",
            Self::Database => "database",
            Self::Repository => "repository",
            Self::CiCdPipeline => "ci/cd pipeline",
            Self::ServiceAccount => "service account",
            Self::Api => "api",
            Self::IdentityProvider => "identity provider",
            Self::StorageService => "storage service",
            Self::SecurityControl => "security control",
        }
    }

    pub const fn is_identity_like(self) -> bool {
        matches!(
            self,
            Self::User
                | Self::Identity
                | Self::ServiceAccount
                | Self::KubernetesServiceAccount
                | Self::IdentityProvider
        )
    }
}

/// Transmission channel used to scale a synthetic pathogen.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Channel {
    Identity,
    Endpoint,
    Network,
    Saas,
    Cloud,
    Email,
    SupplyChain,
    Insider,
}

impl Channel {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Identity => "identity",
            Self::Endpoint => "endpoint",
            Self::Network => "network",
            Self::Saas => "saas",
            Self::Cloud => "cloud",
            Self::Email => "email",
            Self::SupplyChain => "supply_chain",
            Self::Insider => "insider",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RelationKind {
    AuthenticatesTo,
    ConnectsTo,
    Administers,
    DependsOn,
    Trusts,
    HasAccessTo,
    MemberOf,
    RunsOn,
    UsesIdentity,
    CallsApi,
    MountsStorage,
    Manages,
    SharesNetwork,
    RepositoryAccess,
    CiCdAccess,
    KubernetesRbac,
}

impl RelationKind {
    pub const fn label(self) -> &'static str {
        match self {
            Self::AuthenticatesTo => "authenticates_to",
            Self::ConnectsTo => "connects_to",
            Self::Administers => "administers",
            Self::DependsOn => "depends_on",
            Self::Trusts => "trusts",
            Self::HasAccessTo => "has_access_to",
            Self::MemberOf => "member_of",
            Self::RunsOn => "runs_on",
            Self::UsesIdentity => "uses_identity",
            Self::CallsApi => "calls_api",
            Self::MountsStorage => "mounts_storage",
            Self::Manages => "manages",
            Self::SharesNetwork => "shares_network",
            Self::RepositoryAccess => "repository_access",
            Self::CiCdAccess => "ci_cd_access",
            Self::KubernetesRbac => "kubernetes_rbac",
        }
    }

    pub const fn default_channel(self) -> Channel {
        match self {
            Self::AuthenticatesTo
            | Self::UsesIdentity
            | Self::HasAccessTo
            | Self::Administers
            | Self::Trusts
            | Self::Manages
            | Self::MemberOf
            | Self::KubernetesRbac => Channel::Identity,
            Self::ConnectsTo | Self::SharesNetwork => Channel::Network,
            Self::DependsOn | Self::RunsOn | Self::MountsStorage => Channel::Endpoint,
            Self::CallsApi => Channel::Saas,
            Self::RepositoryAccess | Self::CiCdAccess => Channel::SupplyChain,
        }
    }
}

/// Common attributes shared by every simulated entity.
///
/// Rates derived from these fields are illustrative simulation assumptions.
/// They are not empirical measurements of any real product or organisation.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Entity {
    pub id: EntityId,
    pub name: String,
    pub entity_type: EntityType,
    pub susceptibility: f64,
    pub exposure: f64,
    pub infectiousness: f64,
    pub privilege: f64,
    pub criticality: f64,
    pub patch_level: f64,
    pub control_coverage: f64,
    pub detection_probability: f64,
    pub isolation_probability: f64,
    pub detection_delay: SimDuration,
    pub isolation_delay: SimDuration,
    pub recovery_time: SimDuration,
    pub state: EpiState,
}

impl Entity {
    pub fn baseline(id: EntityId, name: impl Into<String>, entity_type: EntityType) -> Self {
        Self {
            id,
            name: name.into(),
            entity_type,
            susceptibility: 0.92,
            exposure: 0.55,
            infectiousness: 0.85,
            privilege: 0.25,
            criticality: 0.35,
            patch_level: 0.4,
            control_coverage: 0.0,
            detection_probability: 0.45,
            isolation_probability: 0.75,
            detection_delay: SimDuration::hours(12),
            isolation_delay: SimDuration::hours(6),
            recovery_time: SimDuration::hours(24),
            state: EpiState::Susceptible,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModelPreset {
    Sis,
    Sir,
    Seir,
    Seirs,
    Seidqrp,
}

impl ModelPreset {
    pub fn parse(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "sis" => Some(Self::Sis),
            "sir" => Some(Self::Sir),
            "seir" => Some(Self::Seir),
            "seirs" => Some(Self::Seirs),
            "seidqrp" => Some(Self::Seidqrp),
            _ => None,
        }
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::Sis => "sis",
            Self::Sir => "sir",
            Self::Seir => "seir",
            Self::Seirs => "seirs",
            Self::Seidqrp => "seidqrp",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Algorithm {
    Gillespie,
    Discrete,
}

impl Algorithm {
    pub fn parse(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "gillespie" | "ssa" => Some(Self::Gillespie),
            "discrete" | "discrete-time" | "tau" => Some(Self::Discrete),
            _ => None,
        }
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::Gillespie => "gillespie",
            Self::Discrete => "discrete",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EventMode {
    Summary,
    Full,
    Sampled,
}

impl EventMode {
    pub fn parse(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "summary" | "summary-only" => Some(Self::Summary),
            "full" | "full-event" => Some(Self::Full),
            "sampled" | "sampled-event" => Some(Self::Sampled),
            _ => None,
        }
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::Summary => "summary",
            Self::Full => "full",
            Self::Sampled => "sampled",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum TransmissionLaw {
    /// Conceptual product from the methodology notes, clamped to `[0, 1]`.
    ClampedProduct,
    /// `1 - exp(-raw)`, which saturates without a hard ceiling on the product.
    ExponentialSaturation,
}

impl TransmissionLaw {
    pub fn parse(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "clamped-product" | "clamped" | "product" => Some(Self::ClampedProduct),
            "exponential" | "exponential-saturation" => Some(Self::ExponentialSaturation),
            _ => None,
        }
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::ClampedProduct => "clamped-product",
            Self::ExponentialSaturation => "exponential-saturation",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PolicyKind {
    NoResponse,
    ImmediateIsolation,
    RiskBasedIsolation,
    CriticalAssetProtection,
}

impl PolicyKind {
    pub fn parse(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "no-response" | "none" => Some(Self::NoResponse),
            "immediate-isolation" => Some(Self::ImmediateIsolation),
            "risk-based-isolation" | "risk-based" => Some(Self::RiskBasedIsolation),
            "critical-asset-protection" | "critical-asset" => Some(Self::CriticalAssetProtection),
            _ => None,
        }
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::NoResponse => "no-response",
            Self::ImmediateIsolation => "immediate-isolation",
            Self::RiskBasedIsolation => "risk-based-isolation",
            Self::CriticalAssetProtection => "critical-asset-protection",
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SeedStrategy {
    #[default]
    Random,
    Targeted,
}

impl SeedStrategy {
    pub fn parse(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "random" => Some(Self::Random),
            "targeted" | "highest-risk" => Some(Self::Targeted),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct StateCounts {
    pub susceptible: u32,
    pub exposed: u32,
    pub infectious: u32,
    pub detected: u32,
    pub quarantined: u32,
    pub recovered: u32,
    pub protected: u32,
}

impl StateCounts {
    pub fn from_array(values: [u32; 7]) -> Self {
        Self {
            susceptible: values[0],
            exposed: values[1],
            infectious: values[2],
            detected: values[3],
            quarantined: values[4],
            recovered: values[5],
            protected: values[6],
        }
    }

    pub fn total(&self) -> u32 {
        self.susceptible
            + self.exposed
            + self.infectious
            + self.detected
            + self.quarantined
            + self.recovered
            + self.protected
    }

    /// Entities that can still propagate: compromised or detected.
    pub fn active_cases(&self) -> u32 {
        self.infectious + self.detected
    }

    pub fn transmitting_or_latent(&self) -> u32 {
        self.exposed + self.infectious + self.detected
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn states_round_trip_index() {
        for state in EpiState::ALL {
            assert_eq!(EpiState::from_index(state as u8), Some(state));
        }
        assert_eq!(EpiState::from_index(9), None);
    }

    #[test]
    fn presets_parse() {
        assert_eq!(ModelPreset::parse("SEIDQRP"), Some(ModelPreset::Seidqrp));
        assert_eq!(Algorithm::parse("gillespie"), Some(Algorithm::Gillespie));
    }
}
