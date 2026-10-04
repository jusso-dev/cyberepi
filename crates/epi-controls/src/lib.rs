//! Defensive controls as parameter modifiers.
//!
//! Built-in multipliers are illustrative simulation assumptions. They are not
//! claimed as measured efficacy. Every value can be overridden in a scenario.

use epi_core::{covered, parse_duration, Channel, SimDuration};
use serde::de::{self, Deserializer, Visitor};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fmt;
use thiserror::Error;

pub const ASSUMPTION: &str = "illustrative simulation assumption";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ControlKind {
    Mfa,
    Edr,
    Segmentation,
    Patching,
    RapidIsolation,
    CredentialRevocation,
    LeastPrivilege,
    ApplicationAllowlisting,
    EmailSecurity,
    DnsFiltering,
    PrivilegedAccessManagement,
    ConditionalAccess,
    ZeroTrustPolicy,
    BackupIsolation,
    UserAwareness,
    AssetRemoval,
}

impl ControlKind {
    pub const ALL: [ControlKind; 16] = [
        Self::Mfa,
        Self::Edr,
        Self::Segmentation,
        Self::Patching,
        Self::RapidIsolation,
        Self::CredentialRevocation,
        Self::LeastPrivilege,
        Self::ApplicationAllowlisting,
        Self::EmailSecurity,
        Self::DnsFiltering,
        Self::PrivilegedAccessManagement,
        Self::ConditionalAccess,
        Self::ZeroTrustPolicy,
        Self::BackupIsolation,
        Self::UserAwareness,
        Self::AssetRemoval,
    ];

    pub fn parse(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().replace('-', "_").as_str() {
            "mfa" => Some(Self::Mfa),
            "edr" => Some(Self::Edr),
            "segmentation" | "network_segmentation" => Some(Self::Segmentation),
            "patching" => Some(Self::Patching),
            "rapid_isolation" => Some(Self::RapidIsolation),
            "credential_revocation" => Some(Self::CredentialRevocation),
            "least_privilege" => Some(Self::LeastPrivilege),
            "application_allowlisting" | "allowlisting" => Some(Self::ApplicationAllowlisting),
            "email_security" => Some(Self::EmailSecurity),
            "dns_filtering" => Some(Self::DnsFiltering),
            "privileged_access_management" | "pam" => Some(Self::PrivilegedAccessManagement),
            "conditional_access" => Some(Self::ConditionalAccess),
            "zero_trust_policy" | "zero_trust" => Some(Self::ZeroTrustPolicy),
            "backup_isolation" => Some(Self::BackupIsolation),
            "user_awareness" => Some(Self::UserAwareness),
            "asset_removal" => Some(Self::AssetRemoval),
            _ => None,
        }
    }

    pub const fn id(self) -> &'static str {
        match self {
            Self::Mfa => "mfa",
            Self::Edr => "edr",
            Self::Segmentation => "segmentation",
            Self::Patching => "patching",
            Self::RapidIsolation => "rapid_isolation",
            Self::CredentialRevocation => "credential_revocation",
            Self::LeastPrivilege => "least_privilege",
            Self::ApplicationAllowlisting => "application_allowlisting",
            Self::EmailSecurity => "email_security",
            Self::DnsFiltering => "dns_filtering",
            Self::PrivilegedAccessManagement => "privileged_access_management",
            Self::ConditionalAccess => "conditional_access",
            Self::ZeroTrustPolicy => "zero_trust_policy",
            Self::BackupIsolation => "backup_isolation",
            Self::UserAwareness => "user_awareness",
            Self::AssetRemoval => "asset_removal",
        }
    }

    pub const fn salt(self) -> u64 {
        self as u64 + 1
    }

    pub const fn summary(self) -> &'static str {
        match self {
            Self::Mfa => "Reduces identity-channel transmission where coverage applies.",
            Self::Edr => "Raises detection and trims endpoint-channel transmission.",
            Self::Segmentation => {
                "Reduces network-channel transmission by the configured strength."
            }
            Self::Patching => "Moves covered entities into the protected compartment at t = 0.",
            Self::RapidIsolation => "Shortens the detected-to-quarantine delay.",
            Self::CredentialRevocation => {
                "Reduces identity transmission and speeds isolation of identity-like nodes."
            }
            Self::LeastPrivilege => "Shrinks the privilege modifier on covered sources.",
            Self::ApplicationAllowlisting => "Reduces endpoint transmission.",
            Self::EmailSecurity => "Reduces email-channel transmission.",
            Self::DnsFiltering => "Reduces network-channel transmission modestly.",
            Self::PrivilegedAccessManagement => "Reduces administrative identity transmission.",
            Self::ConditionalAccess => "Reduces identity and SaaS transmission.",
            Self::ZeroTrustPolicy => "Applies a broad but partial reduction across channels.",
            Self::BackupIsolation => "Reduces storage and backup contact transmission.",
            Self::UserAwareness => "Modestly reduces email and identity transmission.",
            Self::AssetRemoval => {
                "Moves covered entities to protected, as if removed from the population."
            }
        }
    }
}

/// Scenario override. Missing fields inherit the catalog default.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ControlConfig {
    pub coverage: Option<f64>,
    pub strength: Option<f64>,
    pub identity_transmission_multiplier: Option<f64>,
    pub endpoint_transmission_multiplier: Option<f64>,
    pub network_transmission_multiplier: Option<f64>,
    pub saas_transmission_multiplier: Option<f64>,
    pub cloud_transmission_multiplier: Option<f64>,
    pub email_transmission_multiplier: Option<f64>,
    pub supply_chain_transmission_multiplier: Option<f64>,
    pub detection_multiplier: Option<f64>,
    pub isolation_delay_multiplier: Option<f64>,
    pub susceptibility_multiplier: Option<f64>,
    pub privilege_multiplier: Option<f64>,
    #[serde(default, deserialize_with = "de_opt_duration")]
    pub detection_delay: Option<SimDuration>,
    #[serde(default, deserialize_with = "de_opt_duration")]
    pub isolation_delay: Option<SimDuration>,
    #[serde(default, deserialize_with = "de_opt_duration")]
    pub recovery_time: Option<SimDuration>,
    #[serde(default)]
    pub protects: Option<bool>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ControlCatalogEntry {
    pub kind: ControlKind,
    pub description: String,
    pub assumption: &'static str,
    pub defaults: ControlConfig,
}

pub fn catalog() -> Vec<ControlCatalogEntry> {
    ControlKind::ALL
        .into_iter()
        .map(|kind| ControlCatalogEntry {
            kind,
            description: kind.summary().to_string(),
            assumption: ASSUMPTION,
            defaults: defaults(kind),
        })
        .collect()
}

pub fn defaults(kind: ControlKind) -> ControlConfig {
    let mut config = ControlConfig {
        coverage: Some(1.0),
        strength: Some(1.0),
        identity_transmission_multiplier: Some(1.0),
        endpoint_transmission_multiplier: Some(1.0),
        network_transmission_multiplier: Some(1.0),
        saas_transmission_multiplier: Some(1.0),
        cloud_transmission_multiplier: Some(1.0),
        email_transmission_multiplier: Some(1.0),
        supply_chain_transmission_multiplier: Some(1.0),
        detection_multiplier: Some(1.0),
        isolation_delay_multiplier: Some(1.0),
        susceptibility_multiplier: Some(1.0),
        privilege_multiplier: Some(1.0),
        detection_delay: None,
        isolation_delay: None,
        recovery_time: None,
        protects: Some(false),
    };
    match kind {
        ControlKind::Mfa => config.identity_transmission_multiplier = Some(0.35),
        ControlKind::Edr => {
            config.detection_multiplier = Some(2.4);
            config.endpoint_transmission_multiplier = Some(0.7);
            config.isolation_delay_multiplier = Some(0.8);
        }
        ControlKind::Segmentation => {
            config.network_transmission_multiplier = Some(0.4);
            config.strength = Some(0.6);
        }
        ControlKind::Patching => {
            config.protects = Some(true);
            config.susceptibility_multiplier = Some(0.55);
            config.coverage = Some(0.7);
        }
        ControlKind::RapidIsolation => config.isolation_delay_multiplier = Some(0.25),
        ControlKind::CredentialRevocation => {
            config.identity_transmission_multiplier = Some(0.3);
            config.isolation_delay_multiplier = Some(0.4);
        }
        ControlKind::LeastPrivilege => config.privilege_multiplier = Some(0.45),
        ControlKind::ApplicationAllowlisting => config.endpoint_transmission_multiplier = Some(0.5),
        ControlKind::EmailSecurity => config.email_transmission_multiplier = Some(0.35),
        ControlKind::DnsFiltering => config.network_transmission_multiplier = Some(0.75),
        ControlKind::PrivilegedAccessManagement => {
            config.identity_transmission_multiplier = Some(0.4);
            config.privilege_multiplier = Some(0.55);
        }
        ControlKind::ConditionalAccess => {
            config.identity_transmission_multiplier = Some(0.5);
            config.saas_transmission_multiplier = Some(0.55);
        }
        ControlKind::ZeroTrustPolicy => {
            config.identity_transmission_multiplier = Some(0.6);
            config.network_transmission_multiplier = Some(0.65);
            config.saas_transmission_multiplier = Some(0.7);
            config.endpoint_transmission_multiplier = Some(0.75);
        }
        ControlKind::BackupIsolation => {
            config.cloud_transmission_multiplier = Some(0.45);
            config.endpoint_transmission_multiplier = Some(0.7);
        }
        ControlKind::UserAwareness => {
            config.email_transmission_multiplier = Some(0.7);
            config.identity_transmission_multiplier = Some(0.85);
        }
        ControlKind::AssetRemoval => {
            config.protects = Some(true);
            config.coverage = Some(0.0);
        }
    }
    config
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ControlSet {
    pub controls: BTreeMap<String, ControlConfig>,
}

impl ControlSet {
    pub fn is_empty(&self) -> bool {
        self.controls.is_empty()
    }

    pub fn merge(&self, overlay: &ControlSet) -> ControlSet {
        let mut controls = self.controls.clone();
        for (key, value) in &overlay.controls {
            controls.insert(key.clone(), value.clone());
        }
        ControlSet { controls }
    }

    pub fn resolved(&self) -> Result<Vec<ResolvedControl>, ControlError> {
        let mut resolved = Vec::new();
        for (name, override_cfg) in &self.controls {
            let kind =
                ControlKind::parse(name).ok_or_else(|| ControlError::Unknown(name.clone()))?;
            resolved.push(resolve(kind, override_cfg)?);
        }
        Ok(resolved)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ResolvedControl {
    pub kind: ControlKind,
    pub coverage: f64,
    pub strength: f64,
    pub identity_transmission_multiplier: f64,
    pub endpoint_transmission_multiplier: f64,
    pub network_transmission_multiplier: f64,
    pub saas_transmission_multiplier: f64,
    pub cloud_transmission_multiplier: f64,
    pub email_transmission_multiplier: f64,
    pub supply_chain_transmission_multiplier: f64,
    pub detection_multiplier: f64,
    pub isolation_delay_multiplier: f64,
    pub susceptibility_multiplier: f64,
    pub privilege_multiplier: f64,
    pub detection_delay: Option<SimDuration>,
    pub isolation_delay: Option<SimDuration>,
    pub recovery_time: Option<SimDuration>,
    pub protects: bool,
}

impl ResolvedControl {
    pub fn channel_multiplier(&self, channel: Channel) -> f64 {
        let base = match channel {
            Channel::Identity | Channel::Insider => self.identity_transmission_multiplier,
            Channel::Endpoint => self.endpoint_transmission_multiplier,
            Channel::Network => self.network_transmission_multiplier,
            Channel::Saas => self.saas_transmission_multiplier,
            Channel::Cloud => self.cloud_transmission_multiplier,
            Channel::Email => self.email_transmission_multiplier,
            Channel::SupplyChain => self.supply_chain_transmission_multiplier,
        };
        if matches!(self.kind, ControlKind::Segmentation) {
            let strength = epi_core::clamp01(self.strength);
            return (1.0 - 0.85 * strength).clamp(0.05, 1.0) * base.clamp(0.0, 1.0);
        }
        base.clamp(0.0, 4.0)
    }

    pub fn applies_to(&self, seed: u64, entity: u32) -> bool {
        covered(seed, entity, self.kind.salt(), self.coverage)
    }
}

pub fn resolve(
    kind: ControlKind,
    override_cfg: &ControlConfig,
) -> Result<ResolvedControl, ControlError> {
    let defaults = defaults(kind);
    let coverage = pick(override_cfg.coverage, defaults.coverage);
    let strength = pick(override_cfg.strength, defaults.strength);
    if !(0.0..=1.0).contains(&coverage) {
        return Err(ControlError::Range(format!("{kind:?} coverage")));
    }
    if !(0.0..=1.0).contains(&strength) {
        return Err(ControlError::Range(format!("{kind:?} strength")));
    }
    Ok(ResolvedControl {
        kind,
        coverage,
        strength,
        identity_transmission_multiplier: pick(
            override_cfg.identity_transmission_multiplier,
            defaults.identity_transmission_multiplier,
        ),
        endpoint_transmission_multiplier: pick(
            override_cfg.endpoint_transmission_multiplier,
            defaults.endpoint_transmission_multiplier,
        ),
        network_transmission_multiplier: pick(
            override_cfg.network_transmission_multiplier,
            defaults.network_transmission_multiplier,
        ),
        saas_transmission_multiplier: pick(
            override_cfg.saas_transmission_multiplier,
            defaults.saas_transmission_multiplier,
        ),
        cloud_transmission_multiplier: pick(
            override_cfg.cloud_transmission_multiplier,
            defaults.cloud_transmission_multiplier,
        ),
        email_transmission_multiplier: pick(
            override_cfg.email_transmission_multiplier,
            defaults.email_transmission_multiplier,
        ),
        supply_chain_transmission_multiplier: pick(
            override_cfg.supply_chain_transmission_multiplier,
            defaults.supply_chain_transmission_multiplier,
        ),
        detection_multiplier: pick(
            override_cfg.detection_multiplier,
            defaults.detection_multiplier,
        ),
        isolation_delay_multiplier: pick(
            override_cfg.isolation_delay_multiplier,
            defaults.isolation_delay_multiplier,
        ),
        susceptibility_multiplier: pick(
            override_cfg.susceptibility_multiplier,
            defaults.susceptibility_multiplier,
        ),
        privilege_multiplier: pick(
            override_cfg.privilege_multiplier,
            defaults.privilege_multiplier,
        ),
        detection_delay: override_cfg.detection_delay.or(defaults.detection_delay),
        isolation_delay: override_cfg.isolation_delay.or(defaults.isolation_delay),
        recovery_time: override_cfg.recovery_time.or(defaults.recovery_time),
        protects: override_cfg.protects.or(defaults.protects).unwrap_or(false),
    })
}

pub fn de_opt_duration<'de, D>(deserializer: D) -> Result<Option<SimDuration>, D::Error>
where
    D: Deserializer<'de>,
{
    struct DurationVisitor;
    impl<'de> Visitor<'de> for DurationVisitor {
        type Value = Option<SimDuration>;

        fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
            formatter.write_str("a duration such as 15m, 1h, or a number of seconds")
        }

        fn visit_none<E: de::Error>(self) -> Result<Self::Value, E> {
            Ok(None)
        }

        fn visit_unit<E: de::Error>(self) -> Result<Self::Value, E> {
            Ok(None)
        }

        fn visit_str<E: de::Error>(self, value: &str) -> Result<Self::Value, E> {
            if value.trim().is_empty() {
                return Ok(None);
            }
            parse_duration(value).map(Some).map_err(E::custom)
        }

        fn visit_u64<E: de::Error>(self, value: u64) -> Result<Self::Value, E> {
            Ok(Some(SimDuration::seconds(value)))
        }

        fn visit_i64<E: de::Error>(self, value: i64) -> Result<Self::Value, E> {
            if value < 0 {
                return Err(E::custom("duration cannot be negative"));
            }
            Ok(Some(SimDuration::seconds(value as u64)))
        }
    }
    deserializer.deserialize_any(DurationVisitor)
}

fn pick(override_value: Option<f64>, default_value: Option<f64>) -> f64 {
    override_value.or(default_value).unwrap_or(1.0).max(0.0)
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ControlError {
    #[error("unknown control `{0}`")]
    Unknown(String),
    #[error("{0} must be between 0 and 1")]
    Range(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mfa_default_is_marked_illustrative_and_overrideable() {
        let set = ControlSet {
            controls: BTreeMap::from([(
                "mfa".to_string(),
                ControlConfig {
                    coverage: Some(1.0),
                    identity_transmission_multiplier: Some(0.1),
                    ..ControlConfig::default()
                },
            )]),
        };
        let resolved = set.resolved().unwrap();
        assert_eq!(resolved[0].identity_transmission_multiplier, 0.1);
        assert!(catalog()
            .iter()
            .all(|entry| entry.assumption.contains("illustrative")));
    }
}
