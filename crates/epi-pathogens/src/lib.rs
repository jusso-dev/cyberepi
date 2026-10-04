//! Synthetic pathogen profiles.
//!
//! A profile is a bundle of rates and channel weights. It does not describe
//! an exploit, a payload, or a procedure. Every built-in number is an
//! illustrative simulation assumption.

use epi_core::{Channel, SimDuration};
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ChannelWeights {
    pub identity: f64,
    pub endpoint: f64,
    pub network: f64,
    pub saas: f64,
    pub cloud: f64,
    pub email: f64,
    pub supply_chain: f64,
    pub insider: f64,
}

impl ChannelWeights {
    pub const fn zero() -> Self {
        Self {
            identity: 0.0,
            endpoint: 0.0,
            network: 0.0,
            saas: 0.0,
            cloud: 0.0,
            email: 0.0,
            supply_chain: 0.0,
            insider: 0.0,
        }
    }

    pub fn weight(&self, channel: Channel) -> f64 {
        let value = match channel {
            Channel::Identity => self.identity,
            Channel::Endpoint => self.endpoint,
            Channel::Network => self.network,
            Channel::Saas => self.saas,
            Channel::Cloud => self.cloud,
            Channel::Email => self.email,
            Channel::SupplyChain => self.supply_chain,
            Channel::Insider => self.insider,
        };
        if value.is_finite() && value > 0.0 {
            value
        } else {
            0.0
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Pathogen {
    pub id: String,
    pub name: String,
    pub description: String,
    /// Base factor in the transmission product. Illustrative, not empirical.
    pub infectiousness: f64,
    pub latent_period: SimDuration,
    /// Weight of the detection exit from the infectious compartment.
    pub detection_probability: f64,
    /// Weight of the undetected-recovery exit from the infectious compartment.
    pub recovery_probability: f64,
    pub reinfection_probability: f64,
    pub immunity_period: SimDuration,
    pub channels: ChannelWeights,
    pub illustrative: bool,
}

impl Pathogen {
    pub fn latent_days(&self) -> f64 {
        (self.latent_period.as_days()).max(1.0 / 1_440.0)
    }

    pub fn immunity_days(&self) -> f64 {
        (self.immunity_period.as_days()).max(1.0 / 24.0)
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
#[error("unknown synthetic pathogen `{0}`")]
pub struct UnknownPathogen(pub String);

pub fn builtin(id: &str) -> Result<Pathogen, UnknownPathogen> {
    let key = id.trim().to_ascii_lowercase().replace('_', "-");
    let found = builtins().into_iter().find(|item| {
        item.id.eq_ignore_ascii_case(&key) || item.name.eq_ignore_ascii_case(id.trim())
    });
    found.ok_or_else(|| UnknownPathogen(id.to_string()))
}

pub fn builtins() -> Vec<Pathogen> {
    vec![
        pathogen(
            "identity-stealer",
            "IdentityStealer",
            "Synthetic identity-focused pathogen. Propagation is a rate on identity edges, not a credential-theft procedure.",
            0.74,
            SimDuration::minutes(30),
            0.22,
            0.55,
            0.15,
            weights(1.0, 0.25, 0.12, 0.8, 0.35, 0.2, 0.45, 0.3),
        ),
        pathogen(
            "rapid-worm",
            "RapidWorm",
            "Synthetic fast-moving pathogen biased toward endpoint and network contact.",
            0.9,
            SimDuration::minutes(5),
            0.45,
            0.4,
            0.05,
            weights(0.35, 1.0, 0.95, 0.2, 0.4, 0.15, 0.2, 0.05),
        ),
        pathogen(
            "email-borne",
            "EmailBorne",
            "Synthetic pathogen that concentrates on email contact and light identity follow-on.",
            0.7,
            SimDuration::hours(2),
            0.35,
            0.6,
            0.1,
            weights(0.4, 0.3, 0.1, 0.25, 0.1, 1.0, 0.05, 0.15),
        ),
        pathogen(
            "cloud-control-plane",
            "CloudControlPlane",
            "Synthetic pathogen biased toward cloud control-plane and identity relationships.",
            0.68,
            SimDuration::hours(1),
            0.3,
            0.45,
            0.12,
            weights(0.75, 0.2, 0.25, 0.55, 1.0, 0.05, 0.35, 0.1),
        ),
        pathogen(
            "supply-chain",
            "SupplyChain",
            "Synthetic pathogen biased toward repository and CI/CD relationships.",
            0.58,
            SimDuration::hours(6),
            0.18,
            0.4,
            0.2,
            weights(0.45, 0.3, 0.15, 0.4, 0.5, 0.05, 1.0, 0.1),
        ),
        pathogen(
            "slow-persistent",
            "SlowPersistent",
            "Synthetic low-intensity pathogen with a long latent period and weak detection.",
            0.28,
            SimDuration::days(2),
            0.08,
            0.25,
            0.3,
            weights(0.7, 0.4, 0.2, 0.35, 0.3, 0.15, 0.4, 0.2),
        ),
        pathogen(
            "ransomware-like",
            "RansomwareLike",
            "Synthetic high-visibility pathogen. The profile only changes rates; it does not encrypt or destroy anything.",
            0.8,
            SimDuration::minutes(20),
            0.62,
            0.35,
            0.02,
            weights(0.4, 0.95, 0.7, 0.2, 0.35, 0.25, 0.15, 0.05),
        ),
        pathogen(
            "insider-like",
            "InsiderLike",
            "Synthetic slow pathogen concentrated on trusted identity and insider-weighted edges.",
            0.45,
            SimDuration::hours(12),
            0.12,
            0.3,
            0.25,
            weights(0.85, 0.2, 0.1, 0.4, 0.3, 0.2, 0.35, 1.0),
        ),
    ]
}

#[allow(clippy::too_many_arguments)]
fn weights(
    identity: f64,
    endpoint: f64,
    network: f64,
    saas: f64,
    cloud: f64,
    email: f64,
    supply_chain: f64,
    insider: f64,
) -> ChannelWeights {
    ChannelWeights {
        identity,
        endpoint,
        network,
        saas,
        cloud,
        email,
        supply_chain,
        insider,
    }
}

#[allow(clippy::too_many_arguments)]
fn pathogen(
    id: &str,
    name: &str,
    description: &str,
    infectiousness: f64,
    latent: SimDuration,
    detection: f64,
    recovery: f64,
    reinfection: f64,
    channels: ChannelWeights,
) -> Pathogen {
    Pathogen {
        id: id.to_string(),
        name: name.to_string(),
        description: description.to_string(),
        infectiousness,
        latent_period: latent,
        detection_probability: detection,
        recovery_probability: recovery,
        reinfection_probability: reinfection,
        immunity_period: SimDuration::days(30),
        channels,
        illustrative: true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtins_resolve() {
        assert_eq!(builtins().len(), 8);
        assert_eq!(builtin("IdentityStealer").unwrap().id, "identity-stealer");
        assert!(builtin("not-a-pathogen").is_err());
        assert!(builtin("RapidWorm").unwrap().illustrative);
    }
}
