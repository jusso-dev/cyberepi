//! Cyber reproduction numbers.
//!
//! `R0` is the average number of secondary compromises caused by one
//! compromised entity in an otherwise susceptible population. For the
//! exponential-clock model this expectation is exact by linearity:
//!
//! ```text
//! P(i infects j) = λ / (λ + γ)
//!                + (δ / (λ + γ)) · (λ / (λ + γ_d))
//! R_i = Σ_j P(i infects j)
//! R0  = mean_i R_i
//! ```
//!
//! `λ` is the daily transmission hazard, `γ` is the rate of leaving the
//! infectious compartment, `δ` is the detection rate, and `γ_d` is the rate of
//! leaving the detected compartment. Quarantine and recovery do not transmit.
//!
//! The mean can hide heterogeneity. `invasion_threshold` is the leading
//! eigenvalue of the next-generation operator and is the better threshold for
//! "can a spark grow?". Both are reported. Neither is an empirical constant.

use crate::compile::CompiledModel;
use epi_core::EpiState;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    Infectious,
    Detected,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Reproduction {
    /// Mean individual reproduction number. This is the headline cyber R0.
    pub r0_mean: f64,
    /// Leading eigenvalue of the next-generation operator.
    pub invasion_threshold: f64,
    pub expected_secondary: Vec<f64>,
}

pub fn pair_infection_probability(
    lambda: f64,
    detect: f64,
    recover_i: f64,
    isolate: f64,
    recover_d: f64,
    phase: Phase,
) -> f64 {
    let lambda = if lambda.is_finite() && lambda > 0.0 {
        lambda
    } else {
        return 0.0;
    };
    match phase {
        Phase::Infectious => {
            let gamma = (detect + recover_i).max(0.0);
            if gamma <= 1e-15 {
                return 1.0;
            }
            let during_i = lambda / (lambda + gamma);
            let reach_d = detect.max(0.0) / (lambda + gamma);
            let gamma_d = (isolate + recover_d).max(0.0);
            let during_d = if gamma_d <= 1e-15 {
                1.0
            } else {
                lambda / (lambda + gamma_d)
            };
            (during_i + reach_d * during_d).clamp(0.0, 1.0)
        }
        Phase::Detected => {
            let gamma_d = (isolate + recover_d).max(0.0);
            if gamma_d <= 1e-15 {
                1.0
            } else {
                (lambda / (lambda + gamma_d)).clamp(0.0, 1.0)
            }
        }
    }
}

pub fn expected_secondary(
    model: &CompiledModel,
    phase_of: impl Fn(usize) -> Option<Phase>,
) -> Vec<f64> {
    let n = model.node_count();
    let mut scores = vec![0.0; n];
    for src in 0..n {
        let Some(phase) = phase_of(src) else {
            continue;
        };
        if model.segmented_seed[src] {
            continue;
        }
        let node = &model.nodes[src];
        let mut total = 0.0;
        for &edge_index in &model.outgoing[src] {
            let edge_index = edge_index as usize;
            let dst = model.dst[edge_index];
            if model.initial_state[dst] != EpiState::Susceptible {
                continue;
            }
            let lambda = model.edge_hazard[edge_index];
            total += pair_infection_probability(
                lambda,
                node.detect,
                node.recover_i,
                node.isolate,
                node.recover_d,
                phase,
            );
        }
        scores[src] = total;
    }
    scores
}

pub fn reproduction(model: &CompiledModel) -> Reproduction {
    let expected = expected_secondary(model, |_| Some(Phase::Infectious));
    let mut sum = 0.0;
    let mut count = 0.0;
    for (index, score) in expected.iter().enumerate() {
        if model.initial_state[index] == EpiState::Susceptible {
            sum += score;
            count += 1.0;
        }
    }
    let r0_mean = if count > 0.0 { sum / count } else { 0.0 };
    Reproduction {
        invasion_threshold: invasion_threshold(model, &expected),
        r0_mean,
        expected_secondary: expected,
    }
}

/// Power iteration on `v_j ← Σ_i v_i P_ij` using the precomputed expected
/// secondary probabilities stored per source. We recompute `P_ij` on edges.
pub fn invasion_threshold(model: &CompiledModel, _expected: &[f64]) -> f64 {
    let n = model.node_count();
    if n == 0 {
        return 0.0;
    }
    let mut v = vec![1.0 / n as f64; n];
    let mut next = vec![0.0; n];
    let mut eigenvalue = 0.0;
    for _ in 0..28 {
        next.fill(0.0);
        for src in 0..n {
            if model.initial_state[src] == EpiState::Protected || v[src] == 0.0 {
                continue;
            }
            let node = &model.nodes[src];
            for &edge_index in &model.outgoing[src] {
                let edge_index = edge_index as usize;
                let dst = model.dst[edge_index];
                if model.initial_state[dst] != EpiState::Susceptible {
                    continue;
                }
                let p = pair_infection_probability(
                    model.edge_hazard[edge_index],
                    node.detect,
                    node.recover_i,
                    node.isolate,
                    node.recover_d,
                    Phase::Infectious,
                );
                next[dst] += v[src] * p;
            }
        }
        let norm = next.iter().sum::<f64>();
        if norm <= 1e-15 {
            return 0.0;
        }
        eigenvalue = norm;
        for (slot, value) in v.iter_mut().zip(next.iter()) {
            *slot = value / norm;
        }
    }
    eigenvalue
}

/// Mean remaining offspring of entities that are currently infectious or detected.
pub fn instantaneous_rt(model: &CompiledModel, state: &[EpiState], transmitters: &[u32]) -> f64 {
    if transmitters.is_empty() {
        return 0.0;
    }
    let mut sum = 0.0;
    let mut count = 0.0;
    for &src_u in transmitters {
        let src = src_u as usize;
        let phase = match state[src] {
            EpiState::Infectious => Phase::Infectious,
            EpiState::Detected => Phase::Detected,
            _ => continue,
        };
        let node = &model.nodes[src];
        let mut local = 0.0;
        for &edge_index in &model.outgoing[src] {
            let edge_index = edge_index as usize;
            let dst = model.dst[edge_index];
            if state[dst] != EpiState::Susceptible {
                continue;
            }
            local += pair_infection_probability(
                model.edge_hazard[edge_index],
                node.detect,
                node.recover_i,
                node.isolate,
                node.recover_d,
                phase,
            );
        }
        sum += local;
        count += 1.0;
    }
    if count == 0.0 {
        0.0
    } else {
        sum / count
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pair_probability_matches_hand_calculation() {
        // λ = 1, detection 1, no recovery from I, isolation 1, no recovery from D.
        // P = 1/2 + (1/2)*(1/2) = 0.75
        let p = pair_infection_probability(1.0, 1.0, 0.0, 1.0, 0.0, Phase::Infectious);
        assert!((p - 0.75).abs() < 1e-9);
        let detected = pair_infection_probability(1.0, 1.0, 0.0, 1.0, 0.0, Phase::Detected);
        assert!((detected - 0.5).abs() < 1e-9);
        assert_eq!(
            pair_infection_probability(0.0, 1.0, 1.0, 1.0, 1.0, Phase::Infectious),
            0.0
        );
    }
}
