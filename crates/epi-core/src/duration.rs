use serde::{Deserialize, Serialize};
use thiserror::Error;

/// A non-negative duration stored as whole seconds.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SimDuration(u64);

impl SimDuration {
    pub const fn zero() -> Self {
        Self(0)
    }

    pub const fn seconds(seconds: u64) -> Self {
        Self(seconds)
    }

    pub const fn minutes(minutes: u64) -> Self {
        Self(minutes.saturating_mul(60))
    }

    pub const fn hours(hours: u64) -> Self {
        Self(hours.saturating_mul(3_600))
    }

    pub const fn days(days: u64) -> Self {
        Self(days.saturating_mul(86_400))
    }

    pub const fn as_secs(self) -> u64 {
        self.0
    }

    pub fn as_days(self) -> f64 {
        self.0 as f64 / 86_400.0
    }

    pub fn as_secs_f64(self) -> f64 {
        self.0 as f64
    }
}

impl Default for SimDuration {
    fn default() -> Self {
        Self::zero()
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum DurationError {
    #[error("duration is empty")]
    Empty,
    #[error("duration `{0}` is missing a unit (d, h, m, or s)")]
    MissingUnit(String),
    #[error("duration `{0}` has an invalid unit")]
    InvalidUnit(String),
    #[error("duration `{0}` is not a recognised duration")]
    Invalid(String),
}

/// Parse compact durations such as `7d`, `15m`, `1h30m`, or `45s`.
pub fn parse_duration(input: &str) -> Result<SimDuration, DurationError> {
    let text = input.trim();
    if text.is_empty() {
        return Err(DurationError::Empty);
    }
    let bytes = text.as_bytes();
    let mut index = 0;
    let mut total: u64 = 0;
    let mut saw = false;
    while index < bytes.len() {
        if !bytes[index].is_ascii_digit() {
            return Err(DurationError::Invalid(text.to_string()));
        }
        let mut number: u64 = 0;
        while index < bytes.len() && bytes[index].is_ascii_digit() {
            number = number
                .saturating_mul(10)
                .saturating_add(u64::from(bytes[index] - b'0'));
            index += 1;
        }
        if index >= bytes.len() {
            return Err(DurationError::MissingUnit(text.to_string()));
        }
        let unit = bytes[index] as char;
        index += 1;
        let factor = match unit {
            'd' | 'D' => 86_400,
            'h' | 'H' => 3_600,
            'm' | 'M' => 60,
            's' | 'S' => 1,
            _ => return Err(DurationError::InvalidUnit(text.to_string())),
        };
        total = total.saturating_add(number.saturating_mul(factor));
        saw = true;
    }
    if !saw {
        return Err(DurationError::Invalid(text.to_string()));
    }
    Ok(SimDuration(total))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_compound_durations() {
        assert_eq!(parse_duration("7d").unwrap().as_secs(), 7 * 86_400);
        assert_eq!(parse_duration("15m").unwrap().as_secs(), 15 * 60);
        assert_eq!(parse_duration("1h30m").unwrap().as_secs(), 5_400);
        assert!(parse_duration("12").is_err());
    }
}
