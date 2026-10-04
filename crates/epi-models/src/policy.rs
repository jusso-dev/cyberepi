//! Deterministic defender policies.
//!
//! Future learning or language-model policies can implement [`DefenderPolicy`]
//! without changing the simulator. This crate ships only deterministic policies.
//! Decisions change simulation parameters. They never touch a real environment.

use epi_core::{EntityType, EpiState, PolicyKind, StateCounts};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Decision {
    Isolate,
    IncreaseMonitoring,
    RevokeIdentity,
    SegmentGraph,
    PrioritisePatching,
    InvestigateCluster,
    DoNothing,
}

pub struct Observation<'a> {
    pub time_days: f64,
    pub entity: u32,
    pub entity_type: EntityType,
    pub state: EpiState,
    pub criticality: f64,
    pub privilege: f64,
    pub counts: &'a StateCounts,
}

pub trait DefenderPolicy: Send + Sync {
    fn observe(&self, observation: &Observation<'_>) -> Decision;
}

#[derive(Clone, Copy, Debug)]
pub struct PolicyKindImpl(pub PolicyKind);

impl DefenderPolicy for PolicyKindImpl {
    fn observe(&self, observation: &Observation<'_>) -> Decision {
        match self.0 {
            PolicyKind::NoResponse => Decision::DoNothing,
            PolicyKind::ImmediateIsolation => {
                if observation.state == EpiState::Detected {
                    Decision::Isolate
                } else {
                    Decision::DoNothing
                }
            }
            PolicyKind::RiskBasedIsolation => {
                if observation.state == EpiState::Detected && observation.criticality >= 0.7 {
                    Decision::Isolate
                } else if observation.state == EpiState::Detected && observation.privilege >= 0.8 {
                    Decision::RevokeIdentity
                } else {
                    Decision::DoNothing
                }
            }
            PolicyKind::CriticalAssetProtection => {
                if observation.criticality >= 0.8
                    && matches!(
                        observation.state,
                        EpiState::Susceptible | EpiState::Recovered
                    )
                {
                    Decision::PrioritisePatching
                } else if observation.state == EpiState::Detected && observation.criticality >= 0.6
                {
                    Decision::Isolate
                } else {
                    Decision::DoNothing
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn obs(state: EpiState, criticality: f64) -> (StateCounts, Observation<'static>) {
        let counts = StateCounts::default();
        // Leak via Box to satisfy the lifetime in the test by reconstructing below.
        let _ = counts;
        (StateCounts::default(), dummy(state, criticality))
    }

    fn dummy(state: EpiState, criticality: f64) -> Observation<'static> {
        Observation {
            time_days: 0.0,
            entity: 0,
            entity_type: EntityType::Server,
            state,
            criticality,
            privilege: 0.2,
            counts: &EMPTY_COUNTS,
        }
    }

    static EMPTY_COUNTS: StateCounts = StateCounts {
        susceptible: 0,
        exposed: 0,
        infectious: 0,
        detected: 0,
        quarantined: 0,
        recovered: 0,
        protected: 0,
    };

    #[test]
    fn policies_are_deterministic() {
        let _ = obs(EpiState::Detected, 0.9);
        let immediate = PolicyKindImpl(PolicyKind::ImmediateIsolation);
        assert_eq!(
            immediate.observe(&dummy(EpiState::Detected, 0.1)),
            Decision::Isolate
        );
        let none = PolicyKindImpl(PolicyKind::NoResponse);
        assert_eq!(
            none.observe(&dummy(EpiState::Detected, 1.0)),
            Decision::DoNothing
        );
        let risk = PolicyKindImpl(PolicyKind::RiskBasedIsolation);
        assert_eq!(
            risk.observe(&dummy(EpiState::Detected, 0.9)),
            Decision::Isolate
        );
        assert_eq!(
            risk.observe(&dummy(EpiState::Detected, 0.1)),
            Decision::DoNothing
        );
    }
}
