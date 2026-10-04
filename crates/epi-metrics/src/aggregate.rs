use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct NumericSummary {
    pub mean: f64,
    pub median: f64,
    pub p95: f64,
    pub p99: f64,
    pub minimum: f64,
    pub maximum: f64,
}

impl NumericSummary {
    pub fn from_samples(samples: &mut [f64]) -> Self {
        if samples.is_empty() {
            return Self {
                mean: 0.0,
                median: 0.0,
                p95: 0.0,
                p99: 0.0,
                minimum: 0.0,
                maximum: 0.0,
            };
        }
        samples.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let mean = samples.iter().sum::<f64>() / samples.len() as f64;
        Self {
            mean,
            median: percentile(samples, 0.50),
            p95: percentile(samples, 0.95),
            p99: percentile(samples, 0.99),
            minimum: samples[0],
            maximum: *samples.last().unwrap_or(&0.0),
        }
    }
}

/// Nearest-rank percentile on a slice that is already sorted ascending.
pub fn percentile(sorted: &[f64], p: f64) -> f64 {
    if sorted.is_empty() {
        return 0.0;
    }
    let p = p.clamp(0.0, 1.0);
    let index = ((sorted.len() - 1) as f64 * p).round() as usize;
    sorted[index.min(sorted.len() - 1)]
}

pub fn summarize_runs(samples: &[f64]) -> NumericSummary {
    let mut owned = samples.to_vec();
    NumericSummary::from_samples(&mut owned)
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Histogram {
    pub bins: Vec<HistogramBin>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct HistogramBin {
    pub start: f64,
    pub end: f64,
    pub count: u32,
}

impl Histogram {
    pub fn from_samples(samples: &[f64], bins: usize) -> Self {
        let bins = bins.max(1);
        if samples.is_empty() {
            return Self { bins: Vec::new() };
        }
        let mut min = f64::MAX;
        let mut max = f64::MIN;
        for value in samples {
            if value.is_finite() {
                min = min.min(*value);
                max = max.max(*value);
            }
        }
        if !min.is_finite() || max < min {
            return Self { bins: Vec::new() };
        }
        if (max - min).abs() < 1e-9 {
            return Self {
                bins: vec![HistogramBin {
                    start: min,
                    end: max + 1.0,
                    count: samples.len() as u32,
                }],
            };
        }
        let width = (max - min) / bins as f64;
        let mut counts = vec![0u32; bins];
        for value in samples {
            if !value.is_finite() {
                continue;
            }
            let mut index = ((value - min) / width).floor() as usize;
            if index >= bins {
                index = bins - 1;
            }
            counts[index] += 1;
        }
        Self {
            bins: counts
                .into_iter()
                .enumerate()
                .map(|(index, count)| HistogramBin {
                    start: min + index as f64 * width,
                    end: min + (index as f64 + 1.0) * width,
                    count,
                })
                .collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn percentiles_of_known_sample() {
        let summary = summarize_runs(&[1.0, 2.0, 3.0, 4.0, 100.0]);
        assert!((summary.median - 3.0).abs() < 1e-9);
        assert!((summary.mean - 22.0).abs() < 1e-9);
        assert!((summary.maximum - 100.0).abs() < 1e-9);
        assert_eq!(percentile(&[1.0, 2.0, 3.0, 4.0], 0.0), 1.0);
    }

    #[test]
    fn histogram_counts_match_sample_size() {
        let hist = Histogram::from_samples(&[1.0, 2.0, 3.0, 4.0], 4);
        let total: u32 = hist.bins.iter().map(|bin| bin.count).sum();
        assert_eq!(total, 4);
    }
}
