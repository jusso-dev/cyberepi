//! Pluggable transmission laws.
//!
//! The conceptual product requested by the model is:
//!
//! ```text
//! pathogen infectiousness
//! × channel weight
//! × source infectiousness
//! × target susceptibility
//! × edge transmission modifier
//! × contact frequency
//! × privilege modifier
//! × environmental modifier
//! × control modifier
//! ```
//!
//! Contact frequency is a rate (opportunities per day) and can exceed 1, so the
//! raw product is not itself a probability. `ClampedProduct` clamps that
//! product into `[0, 1]` and treats the result as a **per-day transmission
//! probability**. The matching continuous hazard is `λ = -ln(1 - p)`, which is
//! what the Gillespie engine integrates. `ExponentialSaturation` instead uses
//! `p = 1 - exp(-raw)` so large products saturate smoothly without a hard cap
//! before the exponential.

use epi_core::{clamp01, nonneg, Channel, MAX_HAZARD_PER_DAY};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TransmissionContext {
    pub pathogen_infectiousness: f64,
    pub channel_weight: f64,
    pub source_infectiousness: f64,
    pub target_susceptibility: f64,
    pub edge_modifier: f64,
    pub contact_frequency: f64,
    pub privilege_modifier: f64,
    pub environmental_modifier: f64,
    pub control_modifier: f64,
}

impl TransmissionContext {
    pub fn channel(mut self, channel: Channel, weight: f64) -> Self {
        let _ = channel;
        self.channel_weight = weight;
        self
    }
}

pub fn raw_product(ctx: &TransmissionContext) -> f64 {
    nonneg(ctx.pathogen_infectiousness)
        * nonneg(ctx.channel_weight)
        * nonneg(ctx.source_infectiousness)
        * nonneg(ctx.target_susceptibility)
        * nonneg(ctx.edge_modifier)
        * nonneg(ctx.contact_frequency)
        * nonneg(ctx.privilege_modifier)
        * nonneg(ctx.environmental_modifier)
        * nonneg(ctx.control_modifier)
}

pub trait TransmissionStrategy: Send + Sync {
    fn probability(&self, ctx: &TransmissionContext) -> f64;
    fn hazard_per_day(&self, ctx: &TransmissionContext) -> f64;
}

#[derive(Clone, Copy, Debug, Default)]
pub struct ClampedProduct;

impl TransmissionStrategy for ClampedProduct {
    fn probability(&self, ctx: &TransmissionContext) -> f64 {
        clamp01(raw_product(ctx))
    }

    fn hazard_per_day(&self, ctx: &TransmissionContext) -> f64 {
        hazard_from_probability(self.probability(ctx))
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct ExponentialSaturation;

impl TransmissionStrategy for ExponentialSaturation {
    fn probability(&self, ctx: &TransmissionContext) -> f64 {
        let raw = raw_product(ctx);
        if raw <= 0.0 {
            0.0
        } else {
            clamp01(1.0 - (-raw).exp())
        }
    }

    fn hazard_per_day(&self, ctx: &TransmissionContext) -> f64 {
        raw_product(ctx).min(MAX_HAZARD_PER_DAY)
    }
}

pub fn transmission_probability(ctx: &TransmissionContext) -> f64 {
    ClampedProduct.probability(ctx)
}

pub fn transmission_hazard_per_day(ctx: &TransmissionContext) -> f64 {
    ClampedProduct.hazard_per_day(ctx)
}

pub fn probability_in_unit_interval(value: f64) -> bool {
    value.is_finite() && (0.0..=1.0).contains(&value)
}

fn hazard_from_probability(probability: f64) -> f64 {
    if probability <= 0.0 {
        0.0
    } else if probability >= 1.0 {
        MAX_HAZARD_PER_DAY
    } else {
        -(1.0 - probability).ln()
    }
}

/// Privilege modifier in `(0, 1]`. Illustrative.
pub fn privilege_modifier(source_privilege: f64, access_strength: f64) -> f64 {
    let privilege = clamp01(source_privilege);
    let access = clamp01(access_strength);
    0.62 + 0.38 * privilege * (0.40 + 0.60 * access)
}

/// Environmental modifier in `(0, 1]`. Segmentation and patch level reduce it.
/// Target exposure increases it. Illustrative.
pub fn environmental_modifier(segmentation: f64, patch_level: f64, exposure: f64) -> f64 {
    let segmentation = clamp01(segmentation);
    let patch_level = clamp01(patch_level);
    let exposure = clamp01(exposure);
    let segment = 1.0 - 0.55 * segmentation;
    let patch = 1.0 - 0.22 * patch_level;
    let exposed = 0.82 + 0.18 * exposure;
    (segment * patch * exposed).clamp(0.0, 1.0)
}

pub fn edge_modifier(transmission_modifier: f64, trust_level: f64) -> f64 {
    let trust = 0.45 + 0.55 * clamp01(trust_level);
    nonneg(transmission_modifier) * trust
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ctx(contact: f64) -> TransmissionContext {
        TransmissionContext {
            pathogen_infectiousness: 0.5,
            channel_weight: 1.0,
            source_infectiousness: 1.0,
            target_susceptibility: 1.0,
            edge_modifier: 1.0,
            contact_frequency: contact,
            privilege_modifier: 1.0,
            environmental_modifier: 1.0,
            control_modifier: 1.0,
        }
    }

    #[test]
    fn probabilities_stay_inside_unit_interval() {
        let strategy = ClampedProduct;
        for contact in [0.0, 0.2, 1.0, 4.0, 100.0] {
            let p = strategy.probability(&ctx(contact));
            assert!(probability_in_unit_interval(p), "{p}");
            let hazard = strategy.hazard_per_day(&ctx(contact));
            assert!(hazard.is_finite() && hazard >= 0.0);
            assert!(hazard <= MAX_HAZARD_PER_DAY + 1e-9);
        }
        let saturated = ExponentialSaturation.probability(&ctx(50.0));
        assert!(saturated <= 1.0 && saturated > 0.99);
    }

    #[test]
    fn negative_and_nan_factors_do_not_leak() {
        let mut sample = ctx(1.0);
        sample.source_infectiousness = f64::NAN;
        sample.contact_frequency = -3.0;
        assert_eq!(transmission_probability(&sample), 0.0);
    }
}
