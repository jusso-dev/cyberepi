//! Shared types for CyberEpi.
//!
//! CyberEpi models compromise as an abstract epidemiological process on a
//! contact graph. These types carry no exploit logic, payloads, or commands.

mod duration;
mod types;

pub use duration::{parse_duration, DurationError, SimDuration};
pub use types::*;

pub const VERSION: &str = env!("CARGO_PKG_VERSION");
pub const GENERATOR_VERSION: &str = "2";
pub const MODEL_VERSION: &str = "1";

/// Maximum continuous-time hazard, in events per day, used when a clamped
/// daily probability saturates at 1. This is a numerical ceiling, not a
/// measured contact rate.
pub const MAX_HAZARD_PER_DAY: f64 = 24.0;

#[inline]
pub fn clamp01(value: f64) -> f64 {
    if !value.is_finite() {
        return 0.0;
    }
    value.clamp(0.0, 1.0)
}

#[inline]
pub fn nonneg(value: f64) -> f64 {
    if value.is_finite() && value > 0.0 {
        value
    } else {
        0.0
    }
}

/// Deterministic coverage draw in `[0, 1)`.
///
/// The same `(seed, entity, salt)` triple always returns the same unit value,
/// so control coverage is reproducible and independent of hash-map iteration.
pub fn coverage_unit(seed: u64, entity: u32, salt: u64) -> f64 {
    let mut z = seed
        .wrapping_mul(0x9E37_79B9_7F4A_7C15)
        .wrapping_add(u64::from(entity))
        .wrapping_mul(0xBF58_476D_1CE4_E5B9)
        ^ salt.wrapping_mul(0x94D0_49BB_1331_11EB);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^= z >> 31;
    (z >> 11) as f64 / (1u64 << 53) as f64
}

pub fn covered(seed: u64, entity: u32, salt: u64, coverage: f64) -> bool {
    if coverage >= 1.0 {
        return true;
    }
    if coverage <= 0.0 {
        return false;
    }
    coverage_unit(seed, entity, salt) < coverage
}

pub fn format_count(value: u64) -> String {
    let raw = value.to_string();
    let mut out = String::with_capacity(raw.len() + raw.len() / 3);
    for (i, ch) in raw.chars().rev().enumerate() {
        if i > 0 && i % 3 == 0 {
            out.push(',');
        }
        out.push(ch);
    }
    out.chars().rev().collect()
}

pub fn format_sim_duration(seconds: f64) -> String {
    if !seconds.is_finite() || seconds < 0.0 {
        return "n/a".to_string();
    }
    let total = seconds.round() as u64;
    if total > 0 && total.is_multiple_of(86_400) {
        let days = total / 86_400;
        return if days == 1 {
            "1 day".to_string()
        } else {
            format!("{days} days")
        };
    }
    let hours = total / 3_600;
    let minutes = (total % 3_600) / 60;
    if hours > 0 {
        format!("{hours}h {minutes}m")
    } else {
        format!("{minutes}m")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clamp_rejects_non_finite() {
        assert_eq!(clamp01(f64::NAN), 0.0);
        assert_eq!(clamp01(-0.2), 0.0);
        assert_eq!(clamp01(1.4), 1.0);
        assert_eq!(clamp01(0.25), 0.25);
    }

    #[test]
    fn coverage_is_stable() {
        let a = coverage_unit(9, 4, 3);
        let b = coverage_unit(9, 4, 3);
        assert_eq!(a, b);
        assert!((0.0..1.0).contains(&a));
        assert!(covered(1, 1, 1, 1.0));
        assert!(!covered(1, 1, 1, 0.0));
    }

    #[test]
    fn commas_and_duration() {
        assert_eq!(format_count(10_000), "10,000");
        assert_eq!(format_sim_duration(29.0 * 3600.0 + 14.0 * 60.0), "29h 14m");
        assert_eq!(format_sim_duration(31.0 * 3600.0 + 42.0 * 60.0), "31h 42m");
    }
}
