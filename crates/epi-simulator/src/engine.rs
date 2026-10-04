//! Gillespie direct method and a synchronous discrete-time approximation.
//!
//! Both algorithms consume the same daily hazards from `epi_models::compile`.
//! A run is a pure function of `(compiled model, options, seed)`.

use crate::rate_tree::RateTree;
use epi_core::{
    Algorithm, EpiState, EventMode, PolicyKind, SeedStrategy, SimDuration, StateCounts,
};
use epi_models::CompiledModel;
use epi_models::{instantaneous_rt, Decision, DefenderPolicy, Observation, PolicyKindImpl};
use rand::RngExt;
use rand::SeedableRng;
use rand_chacha::ChaCha8Rng;
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;

const NONE: u32 = u32::MAX;
const MAX_EVENTS: usize = 80_000;

#[derive(Clone, Debug)]
pub struct RunOptions {
    pub algorithm: Algorithm,
    pub duration: SimDuration,
    pub initial_infected: u32,
    pub strategy: SeedStrategy,
    pub event_mode: EventMode,
    pub sample_every: SimDuration,
    pub dt: SimDuration,
    pub policy: PolicyKind,
    pub extinction_fraction: f64,
    pub extinction_minimum: u32,
}

impl Default for RunOptions {
    fn default() -> Self {
        Self {
            algorithm: Algorithm::Gillespie,
            duration: SimDuration::days(7),
            initial_infected: 1,
            strategy: SeedStrategy::Random,
            event_mode: EventMode::Summary,
            sample_every: SimDuration::hours(1),
            dt: SimDuration::minutes(15),
            policy: PolicyKind::NoResponse,
            extinction_fraction: 0.005,
            extinction_minimum: 10,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CurveSample {
    pub t_seconds: f64,
    pub susceptible: f64,
    pub exposed: f64,
    pub infectious: f64,
    pub detected: f64,
    pub quarantined: f64,
    pub recovered: f64,
    pub protected: f64,
    pub rt: f64,
    pub cumulative: f64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EventKind {
    Seeded,
    Exposed,
    Compromised,
    Detected,
    Quarantined,
    Recovered,
    Protected,
    Susceptible,
    Segmented,
    Monitoring,
    Policy,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SimEvent {
    pub t_seconds: f64,
    pub kind: EventKind,
    pub entity: u32,
    pub source: Option<u32>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RunSummary {
    pub seed: u64,
    pub r0: f64,
    pub invasion_threshold: f64,
    pub attack_rate: f64,
    pub outbreak_size: u32,
    pub peak_prevalence: u32,
    pub time_to_peak_seconds: f64,
    pub outbreak_duration_seconds: f64,
    pub containment_seconds: Option<f64>,
    pub rt_below_one_seconds: Option<f64>,
    pub extinct: bool,
    pub final_counts: StateCounts,
    pub mean_time_to_detection_seconds: Option<f64>,
    pub mean_time_to_isolation_seconds: Option<f64>,
    pub mean_time_to_recovery_seconds: Option<f64>,
    pub secondary: Vec<u32>,
    pub parent: Vec<u32>,
    pub samples: Vec<CurveSample>,
    pub events: Vec<SimEvent>,
}

#[derive(Clone, Copy)]
enum Action {
    State { index: usize, state: EpiState },
    Transmit { src: usize, dst: usize },
}

pub struct Engine {
    model: Arc<CompiledModel>,
    options: RunOptions,
    rng: ChaCha8Rng,
    state: Vec<EpiState>,
    entered_at: Vec<f64>,
    compromise_at: Vec<f64>,
    secondary: Vec<u32>,
    parent: Vec<u32>,
    segmented: Vec<bool>,
    monitoring: Vec<bool>,
    cached: Vec<f64>,
    tree: RateTree,
    transmitters: Vec<u32>,
    tx_pos: Vec<u32>,
    counts: [u32; 7],
    time: f64,
    duration: f64,
    sample_every: f64,
    next_sample: f64,
    cumulative: u32,
    peak: u32,
    peak_time: f64,
    contained_at: Option<f64>,
    outbreak_end: Option<f64>,
    rt_above: bool,
    rt_below: Option<f64>,
    samples: Vec<CurveSample>,
    events: Vec<SimEvent>,
    record: bool,
    sum_ttd: f64,
    n_ttd: u32,
    sum_tti: f64,
    n_tti: u32,
    sum_ttr: f64,
    n_ttr: u32,
    seeds: u32,
    policy_depth: u8,
    last_cumulative_sample: u32,
}

impl Engine {
    pub fn new(model: Arc<CompiledModel>, options: RunOptions) -> Self {
        let n = model.node_count();
        Self {
            tree: RateTree::new(n),
            model,
            options,
            rng: ChaCha8Rng::seed_from_u64(1),
            state: vec![EpiState::Susceptible; n],
            entered_at: vec![0.0; n],
            compromise_at: vec![-1.0; n],
            secondary: vec![0; n],
            parent: vec![NONE; n],
            segmented: vec![false; n],
            monitoring: vec![false; n],
            cached: vec![0.0; n],
            transmitters: Vec::new(),
            tx_pos: vec![NONE; n],
            counts: [0; 7],
            time: 0.0,
            duration: 1.0,
            sample_every: 1.0 / 24.0,
            next_sample: 0.0,
            cumulative: 0,
            peak: 0,
            peak_time: 0.0,
            contained_at: None,
            outbreak_end: None,
            rt_above: false,
            rt_below: None,
            samples: Vec::new(),
            events: Vec::new(),
            record: false,
            sum_ttd: 0.0,
            n_ttd: 0,
            sum_tti: 0.0,
            n_tti: 0,
            sum_ttr: 0.0,
            n_ttr: 0,
            seeds: 0,
            policy_depth: 0,
            last_cumulative_sample: 0,
        }
    }

    pub fn run(&mut self, seed: u64) -> RunSummary {
        self.prepare(seed);
        match self.options.algorithm {
            Algorithm::Gillespie => self.run_gillespie(),
            Algorithm::Discrete => self.run_discrete(),
        }
        self.finish(seed)
    }

    fn prepare(&mut self, seed: u64) {
        self.rng = ChaCha8Rng::seed_from_u64(seed);
        let n = self.model.node_count();
        if self.state.len() != n {
            self.state.resize(n, EpiState::Susceptible);
        }
        self.state.copy_from_slice(&self.model.initial_state);
        if self.secondary.len() != n {
            self.secondary.resize(n, 0);
        }
        if self.parent.len() != n {
            self.parent.resize(n, NONE);
        }
        if self.cached.len() != n {
            self.cached.resize(n, 0.0);
        }
        self.entered_at.fill(0.0);
        self.compromise_at.fill(-1.0);
        self.secondary.fill(0);
        self.parent.fill(NONE);
        self.segmented.fill(false);
        self.monitoring.fill(false);
        self.transmitters.clear();
        self.tx_pos.fill(NONE);
        self.time = 0.0;
        self.duration = self.options.duration.as_days().max(1.0 / 1_440.0);
        self.sample_every = self.options.sample_every.as_days().max(1.0 / 1_440.0);
        self.next_sample = 0.0;
        self.cumulative = 0;
        self.peak = 0;
        self.peak_time = 0.0;
        self.contained_at = None;
        self.outbreak_end = None;
        self.rt_above = false;
        self.rt_below = None;
        self.samples.clear();
        self.events.clear();
        self.record = self.options.event_mode == EventMode::Full;
        self.sum_ttd = 0.0;
        self.n_ttd = 0;
        self.sum_tti = 0.0;
        self.n_tti = 0;
        self.sum_ttr = 0.0;
        self.n_ttr = 0;
        self.seeds = 0;
        self.policy_depth = 0;
        self.last_cumulative_sample = 0;
        self.apply_opening_policy();
        self.recount();
        self.rebuild_rates();
        self.rebuild_transmitters();
        self.seed_infections();
        self.capture_sample(0.0);
        self.next_sample = self.sample_every;
    }

    fn apply_opening_policy(&mut self) {
        if self.options.policy != PolicyKind::CriticalAssetProtection {
            return;
        }
        for index in 0..self.state.len() {
            if self.model.criticality[index] >= 0.8 && self.state[index] == EpiState::Susceptible {
                self.state[index] = EpiState::Protected;
            }
        }
    }

    fn recount(&mut self) {
        self.counts = [0; 7];
        for state in &self.state {
            self.counts[state.as_index()] += 1;
        }
    }

    fn rebuild_rates(&mut self) {
        for index in 0..self.state.len() {
            let rate = self.total_rate(index);
            self.cached[index] = rate;
            self.tree.set(index, rate);
        }
    }

    fn rebuild_transmitters(&mut self) {
        self.transmitters.clear();
        self.tx_pos.fill(NONE);
        for (index, state) in self.state.iter().enumerate() {
            if self.transmits(*state) {
                self.tx_pos[index] = self.transmitters.len() as u32;
                self.transmitters.push(index as u32);
            }
        }
    }

    fn transmits(&self, state: EpiState) -> bool {
        match state {
            EpiState::Infectious => true,
            EpiState::Detected => self.model.detection_enabled,
            _ => false,
        }
    }

    fn seed_infections(&mut self) {
        let susceptible: Vec<usize> = self
            .state
            .iter()
            .enumerate()
            .filter_map(|(index, state)| (*state == EpiState::Susceptible).then_some(index))
            .collect();
        let identity_like: Vec<usize> = susceptible
            .iter()
            .copied()
            .filter(|index| self.model.types[*index].is_identity_like())
            .collect();
        let mut candidates = if identity_like.len() >= self.options.initial_infected as usize {
            identity_like
        } else if !susceptible.is_empty() {
            susceptible
        } else {
            return;
        };
        let want = (self.options.initial_infected as usize).min(candidates.len());
        match self.options.strategy {
            SeedStrategy::Random => {
                for index in 0..want {
                    let swap = index + self.rng.random_range(0..(candidates.len() - index));
                    candidates.swap(index, swap);
                }
            }
            SeedStrategy::Targeted => {
                let scores = &self.model.reproduction.expected_secondary;
                candidates.sort_by(|left, right| {
                    scores[*right]
                        .total_cmp(&scores[*left])
                        .then(left.cmp(right))
                });
            }
        }
        for &index in candidates.iter().take(want) {
            self.apply_state(index, EpiState::Infectious, None, true);
            self.seeds += 1;
        }
    }

    fn run_gillespie(&mut self) {
        let mut guard = 0u64;
        let limit = (self.state.len() as u64).saturating_mul(200).max(10_000);
        while self.time < self.duration && guard < limit {
            guard += 1;
            if guard == limit {
                self.fill_samples_until(self.duration);
                self.time = self.duration;
                break;
            }
            let total = self.tree.total();
            if !total.is_finite() || total <= 1e-12 {
                self.time = self.duration;
                self.fill_samples_until(self.duration);
                break;
            }
            let unit = (1.0 - self.rng.random::<f64>()).clamp(1e-12, 1.0);
            let dt = -unit.ln() / total;
            if !dt.is_finite() || self.time + dt >= self.duration {
                self.fill_samples_until(self.duration);
                self.time = self.duration;
                break;
            }
            self.fill_samples_until(self.time + dt);
            self.time += dt;
            let pick = self.rng.random::<f64>() * total;
            let index = self.tree.find(pick);
            self.fire(index);
        }
        self.fill_samples_until(self.duration);
    }

    fn run_discrete(&mut self) {
        let dt = self.options.dt.as_days().max(1.0 / 1_440.0);
        while self.time < self.duration - 1e-12 {
            let step = dt.min(self.duration - self.time);
            let snapshot = self.state.clone();
            let transmitters = self.transmitters.clone();
            let mut plans = Vec::new();
            for index in 0..snapshot.len() {
                if let Some(action) = self.discrete_progression(index, &snapshot, step) {
                    plans.push(action);
                }
            }
            for src in transmitters {
                let src = src as usize;
                if !self.transmits(snapshot[src]) || self.segmented[src] {
                    continue;
                }
                for &edge in &self.model.outgoing[src] {
                    let edge = edge as usize;
                    let dst = self.model.dst[edge];
                    if snapshot[dst] != EpiState::Susceptible {
                        continue;
                    }
                    let hazard = self.model.edge_hazard[edge];
                    if hazard <= 0.0 {
                        continue;
                    }
                    let probability = 1.0 - (-hazard * step).exp();
                    if self.rng.random::<f64>() < probability {
                        plans.push(Action::Transmit { src, dst });
                    }
                }
            }
            self.fill_samples_until(self.time + step);
            self.time += step;
            let mut claimed = vec![false; snapshot.len()];
            for action in plans {
                match action {
                    Action::Transmit { src, dst } => {
                        if !claimed[dst] && self.state[dst] == EpiState::Susceptible {
                            claimed[dst] = true;
                            self.perform(Action::Transmit { src, dst });
                        }
                    }
                    Action::State { index, state } => {
                        if !claimed[index] && self.state[index] == snapshot[index] {
                            claimed[index] = true;
                            self.perform(Action::State { index, state });
                        }
                    }
                }
            }
        }
        self.fill_samples_until(self.duration);
    }

    fn discrete_progression(
        &mut self,
        index: usize,
        snapshot: &[EpiState],
        step: f64,
    ) -> Option<Action> {
        let mut chosen: Option<Action> = None;
        let mut total = 0.0;
        self.for_each_event_in(index, snapshot, |rate, action| {
            if !matches!(action, Action::Transmit { .. }) {
                total += rate;
                if chosen.is_none() {
                    chosen = Some(action);
                }
            }
            true
        });
        // The walk above kept only the first action. Re-roll properly.
        if total <= 0.0 {
            return None;
        }
        let probability = 1.0 - (-total * step).exp();
        if self.rng.random::<f64>() >= probability {
            return None;
        }
        let target = self.rng.random::<f64>() * total;
        let mut acc = 0.0;
        let mut picked = None;
        self.for_each_event_in(index, snapshot, |rate, action| {
            if matches!(action, Action::Transmit { .. }) {
                return true;
            }
            acc += rate;
            if acc >= target {
                picked = Some(action);
                return false;
            }
            true
        });
        let _ = chosen;
        picked
    }

    fn fire(&mut self, index: usize) {
        let threshold = self.rng.random::<f64>() * self.cached[index].max(0.0);
        let mut acc = 0.0;
        let mut picked = None;
        self.for_each_event(index, |rate, action| {
            acc += rate;
            if acc >= threshold {
                picked = Some(action);
                return false;
            }
            true
        });
        if let Some(action) = picked {
            self.perform(action);
        } else {
            self.recompute(index);
        }
    }

    fn perform(&mut self, action: Action) {
        match action {
            Action::State { index, state } => self.transition(index, state, None),
            Action::Transmit { src, dst } => {
                if self.state[dst] != EpiState::Susceptible {
                    self.recompute(src);
                    return;
                }
                let next = self.model.entry_on_transmit;
                self.secondary[src] = self.secondary[src].saturating_add(1);
                self.transition(dst, next, Some(src as u32));
            }
        }
    }

    fn transition(&mut self, index: usize, new_state: EpiState, source: Option<u32>) {
        if self.state[index] == new_state {
            return;
        }
        let seeded = false;
        self.apply_state(index, new_state, source, seeded);
        if self.policy_depth > 0 {
            return;
        }
        let decision = self.observe(index);
        self.policy_depth = self.policy_depth.saturating_add(1);
        self.apply_decision(index, decision);
        self.policy_depth = self.policy_depth.saturating_sub(1);
    }

    fn observe(&self, index: usize) -> Decision {
        let counts = self.count_struct();
        let observation = Observation {
            time_days: self.time,
            entity: index as u32,
            entity_type: self.model.types[index],
            state: self.state[index],
            criticality: self.model.criticality[index],
            privilege: self.model.privilege[index],
            counts: &counts,
        };
        PolicyKindImpl(self.options.policy).observe(&observation)
    }

    fn apply_decision(&mut self, index: usize, decision: Decision) {
        match decision {
            Decision::DoNothing | Decision::InvestigateCluster => {
                if decision == Decision::InvestigateCluster {
                    self.note(EventKind::Policy, index, None);
                }
            }
            Decision::Isolate => {
                if !matches!(
                    self.state[index],
                    EpiState::Quarantined | EpiState::Recovered | EpiState::Protected
                ) {
                    self.note(EventKind::Policy, index, None);
                    self.apply_state(index, EpiState::Quarantined, None, false);
                }
            }
            Decision::RevokeIdentity => {
                if self.model.types[index].is_identity_like() {
                    self.note(EventKind::Policy, index, None);
                    self.apply_state(index, EpiState::Quarantined, None, false);
                }
            }
            Decision::SegmentGraph => {
                self.segmented[index] = true;
                self.note(EventKind::Segmented, index, None);
                self.recompute(index);
            }
            Decision::IncreaseMonitoring => {
                self.monitoring[index] = true;
                self.note(EventKind::Monitoring, index, None);
                self.recompute(index);
            }
            Decision::PrioritisePatching => {
                if matches!(
                    self.state[index],
                    EpiState::Susceptible | EpiState::Recovered
                ) {
                    self.apply_state(index, EpiState::Protected, None, false);
                }
            }
        }
    }

    fn apply_state(
        &mut self,
        index: usize,
        new_state: EpiState,
        source: Option<u32>,
        seeded: bool,
    ) {
        let old = self.state[index];
        if old == new_state {
            return;
        }
        let old_sus = old == EpiState::Susceptible;
        let new_sus = new_state == EpiState::Susceptible;
        self.note_timing(index, old, new_state);
        if matches!(new_state, EpiState::Exposed | EpiState::Infectious)
            && matches!(
                old,
                EpiState::Susceptible | EpiState::Recovered | EpiState::Protected
            )
        {
            self.cumulative = self.cumulative.saturating_add(1);
            if self.compromise_at[index] < 0.0 {
                self.compromise_at[index] = self.time;
            }
            if let Some(source) = source {
                self.parent[index] = source;
            }
        }
        if new_state == EpiState::Infectious
            && old == EpiState::Exposed
            && self.compromise_at[index] < 0.0
        {
            self.compromise_at[index] = self.time;
        }
        self.counts[old.as_index()] = self.counts[old.as_index()].saturating_sub(1);
        self.counts[new_state.as_index()] = self.counts[new_state.as_index()].saturating_add(1);
        self.state[index] = new_state;
        self.entered_at[index] = self.time;
        self.update_transmitter(index, old, new_state);
        self.recompute(index);
        if old_sus != new_sus {
            let incoming = self.model.incoming[index].clone();
            for edge in incoming {
                let src = self.model.src[edge as usize];
                if src != index {
                    self.recompute(src);
                }
            }
        }
        let active = self.counts[EpiState::Infectious.as_index()]
            + self.counts[EpiState::Detected.as_index()];
        if active >= self.peak {
            self.peak = active;
            self.peak_time = self.time;
        }
        self.note_containment();
        if self.record {
            self.note(kind_for(new_state, seeded), index, source);
        } else if seeded {
            self.note(EventKind::Seeded, index, source);
        }
        let _ = seeded;
    }

    fn note_timing(&mut self, index: usize, old: EpiState, new_state: EpiState) {
        let start = self.compromise_at[index];
        if start < 0.0 {
            return;
        }
        let delta = (self.time - start).max(0.0) * 86_400.0;
        if new_state == EpiState::Detected && old == EpiState::Infectious {
            self.sum_ttd += delta;
            self.n_ttd += 1;
        }
        if new_state == EpiState::Quarantined {
            self.sum_tti += (self.time - self.entered_at[index]).max(0.0) * 86_400.0;
            self.n_tti += 1;
        }
        if new_state == EpiState::Recovered {
            self.sum_ttr += delta;
            self.n_ttr += 1;
        }
    }

    fn note_containment(&mut self) {
        let active = self.counts[EpiState::Exposed.as_index()]
            + self.counts[EpiState::Infectious.as_index()]
            + self.counts[EpiState::Detected.as_index()];
        let lingering = active + self.counts[EpiState::Quarantined.as_index()];
        if self.time > 0.0 && active == 0 && self.contained_at.is_none() && self.cumulative > 0 {
            self.contained_at = Some(self.time);
        }
        if self.time > 0.0 && lingering == 0 && self.outbreak_end.is_none() && self.cumulative > 0 {
            self.outbreak_end = Some(self.time);
        }
        if active > 0 {
            self.contained_at = None;
            self.outbreak_end = None;
        }
    }

    fn update_transmitter(&mut self, index: usize, old: EpiState, new_state: EpiState) {
        let was = self.transmits(old);
        let now = self.transmits(new_state);
        if was && !now {
            self.remove_transmitter(index);
        } else if !was && now {
            self.tx_pos[index] = self.transmitters.len() as u32;
            self.transmitters.push(index as u32);
        }
    }

    fn remove_transmitter(&mut self, index: usize) {
        let pos = self.tx_pos[index];
        if pos == NONE {
            return;
        }
        let pos = pos as usize;
        let last = self.transmitters.len() - 1;
        let moved = self.transmitters[last];
        self.transmitters.swap(pos, last);
        self.transmitters.pop();
        if pos < self.transmitters.len() {
            self.tx_pos[moved as usize] = pos as u32;
        }
        self.tx_pos[index] = NONE;
    }

    fn recompute(&mut self, index: usize) {
        let rate = self.total_rate(index);
        self.cached[index] = rate;
        self.tree.set(index, rate);
    }

    fn total_rate(&self, index: usize) -> f64 {
        let mut total = 0.0;
        self.for_each_event(index, |rate, _| {
            total += rate;
            true
        });
        total
    }

    fn for_each_event(&self, index: usize, visit: impl FnMut(f64, Action) -> bool) {
        self.for_each_event_in(index, &self.state, visit);
    }

    fn for_each_event_in(
        &self,
        index: usize,
        states: &[EpiState],
        mut visit: impl FnMut(f64, Action) -> bool,
    ) {
        let node = &self.model.nodes[index];
        let step =
            |rate: f64, action: Action, visit: &mut dyn FnMut(f64, Action) -> bool| -> bool {
                if rate > 1e-15 {
                    visit(rate, action)
                } else {
                    true
                }
            };
        let continue_walk = match states[index] {
            EpiState::Susceptible => step(
                node.protect,
                Action::State {
                    index,
                    state: EpiState::Protected,
                },
                &mut visit,
            ),
            EpiState::Exposed => step(
                node.latent,
                Action::State {
                    index,
                    state: EpiState::Infectious,
                },
                &mut visit,
            ),
            EpiState::Infectious => {
                let detect = if self.monitoring[index] {
                    node.detect * 2.0
                } else {
                    node.detect
                };
                let mut ok = true;
                if self.model.detection_enabled {
                    ok = step(
                        detect,
                        Action::State {
                            index,
                            state: EpiState::Detected,
                        },
                        &mut visit,
                    );
                }
                if ok {
                    ok = step(
                        node.recover_i,
                        Action::State {
                            index,
                            state: self.model.infectious_recovery_target,
                        },
                        &mut visit,
                    );
                }
                ok
            }
            EpiState::Detected => {
                let mut ok = true;
                if self.model.quarantine_enabled {
                    ok = step(
                        node.isolate,
                        Action::State {
                            index,
                            state: EpiState::Quarantined,
                        },
                        &mut visit,
                    );
                }
                if ok {
                    ok = step(
                        node.recover_d,
                        Action::State {
                            index,
                            state: self.model.infectious_recovery_target,
                        },
                        &mut visit,
                    );
                }
                ok
            }
            EpiState::Quarantined => step(
                node.recover_q,
                Action::State {
                    index,
                    state: EpiState::Recovered,
                },
                &mut visit,
            ),
            EpiState::Recovered => step(
                node.wane,
                Action::State {
                    index,
                    state: EpiState::Susceptible,
                },
                &mut visit,
            ),
            EpiState::Protected => step(
                node.lose_protection,
                Action::State {
                    index,
                    state: EpiState::Susceptible,
                },
                &mut visit,
            ),
        };
        if !continue_walk || self.segmented[index] || !self.transmits(states[index]) {
            return;
        }
        for &edge in &self.model.outgoing[index] {
            let edge = edge as usize;
            let dst = self.model.dst[edge];
            if states[dst] != EpiState::Susceptible {
                continue;
            }
            let hazard = self.model.edge_hazard[edge];
            if hazard <= 1e-15 {
                continue;
            }
            if !visit(hazard, Action::Transmit { src: index, dst }) {
                return;
            }
        }
    }

    fn fill_samples_until(&mut self, t_end: f64) {
        while self.next_sample <= t_end + 1e-9 && self.next_sample <= self.duration + 1e-9 {
            self.capture_sample(self.next_sample.min(self.duration));
            self.next_sample += self.sample_every;
        }
    }

    fn capture_sample(&mut self, time_days: f64) {
        let rt = instantaneous_rt(&self.model, &self.state, &self.transmitters);
        if rt >= 1.0 {
            self.rt_above = true;
            self.rt_below = None;
        } else if self.rt_above && self.rt_below.is_none() {
            self.rt_below = Some(time_days);
        }
        self.samples.push(CurveSample {
            t_seconds: time_days * 86_400.0,
            susceptible: f64::from(self.counts[0]),
            exposed: f64::from(self.counts[1]),
            infectious: f64::from(self.counts[2]),
            detected: f64::from(self.counts[3]),
            quarantined: f64::from(self.counts[4]),
            recovered: f64::from(self.counts[5]),
            protected: f64::from(self.counts[6]),
            rt,
            cumulative: f64::from(self.cumulative),
        });
        self.last_cumulative_sample = self.cumulative;
    }

    fn count_struct(&self) -> StateCounts {
        StateCounts::from_array(self.counts)
    }

    fn note(&mut self, kind: EventKind, index: usize, source: Option<u32>) {
        if self.events.len() >= MAX_EVENTS {
            return;
        }
        if !self.record && kind != EventKind::Seeded {
            return;
        }
        self.events.push(SimEvent {
            t_seconds: self.time * 86_400.0,
            kind,
            entity: index as u32,
            source,
        });
    }

    fn finish(&mut self, seed: u64) -> RunSummary {
        let population = self.state.len().max(1) as f64;
        let threshold = ((self.options.extinction_fraction * population).ceil() as u32)
            .max(self.options.extinction_minimum);
        let extinct = self.cumulative < threshold;
        let horizon = self.duration * 86_400.0;
        RunSummary {
            seed,
            r0: self.model.reproduction.r0_mean,
            invasion_threshold: self.model.reproduction.invasion_threshold,
            attack_rate: self.cumulative as f64 / population,
            outbreak_size: self.cumulative,
            peak_prevalence: self.peak,
            time_to_peak_seconds: self.peak_time * 86_400.0,
            outbreak_duration_seconds: self
                .outbreak_end
                .map(|days| days * 86_400.0)
                .unwrap_or(horizon),
            containment_seconds: Some(
                self.contained_at
                    .map(|days| days * 86_400.0)
                    .unwrap_or(horizon),
            ),
            rt_below_one_seconds: self.rt_below.map(|days| days * 86_400.0),
            extinct,
            final_counts: self.count_struct(),
            mean_time_to_detection_seconds: mean_of(self.sum_ttd, self.n_ttd),
            mean_time_to_isolation_seconds: mean_of(self.sum_tti, self.n_tti),
            mean_time_to_recovery_seconds: mean_of(self.sum_ttr, self.n_ttr),
            secondary: std::mem::take(&mut self.secondary),
            parent: std::mem::take(&mut self.parent),
            samples: std::mem::take(&mut self.samples),
            events: std::mem::take(&mut self.events),
        }
    }
}

fn mean_of(sum: f64, count: u32) -> Option<f64> {
    if count == 0 {
        None
    } else {
        Some(sum / f64::from(count))
    }
}

fn kind_for(state: EpiState, seeded: bool) -> EventKind {
    if seeded {
        return EventKind::Seeded;
    }
    match state {
        EpiState::Exposed => EventKind::Exposed,
        EpiState::Infectious => EventKind::Compromised,
        EpiState::Detected => EventKind::Detected,
        EpiState::Quarantined => EventKind::Quarantined,
        EpiState::Recovered => EventKind::Recovered,
        EpiState::Protected => EventKind::Protected,
        EpiState::Susceptible => EventKind::Susceptible,
    }
}

pub fn run_once(model: Arc<CompiledModel>, options: RunOptions, seed: u64) -> RunSummary {
    Engine::new(model, options).run(seed)
}

pub fn run_monte_carlo(
    model: Arc<CompiledModel>,
    options: RunOptions,
    runs: u32,
    base_seed: u64,
    threads: usize,
    progress: &(impl Fn(u32) + Sync),
) -> Vec<RunSummary> {
    if runs == 0 {
        return Vec::new();
    }
    let threads = threads.max(1).min(runs as usize);
    let done = AtomicU32::new(0);
    let mut chunks: Vec<Vec<(u32, RunSummary)>> = (0..threads)
        .into_par_iter()
        .map(|worker| {
            let mut engine = Engine::new(Arc::clone(&model), options.clone());
            let mut local = Vec::new();
            let mut run = worker as u32;
            while run < runs {
                if options.event_mode == EventMode::Sampled {
                    engine.options.event_mode = if run.is_multiple_of(20) {
                        EventMode::Full
                    } else {
                        EventMode::Summary
                    };
                }
                local.push((run, engine.run(base_seed.wrapping_add(u64::from(run)))));
                let finished = done.fetch_add(1, Ordering::Relaxed) + 1;
                if finished == runs || finished.is_multiple_of(25) {
                    progress(finished);
                }
                run += threads as u32;
            }
            local
        })
        .collect();
    let mut ordered = Vec::new();
    for chunk in &mut chunks {
        ordered.append(chunk);
    }
    ordered.sort_by_key(|(index, _)| *index);
    ordered.into_iter().map(|(_, summary)| summary).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use epi_controls::ControlSet;
    use epi_core::{Channel, EntityType, ModelPreset, RelationKind, TransmissionLaw};
    use epi_graph::GraphBuilder;
    use epi_models::{compile, CompileInputs, TimingOverrides};
    use epi_pathogens::Pathogen;

    fn tiny_pathogen(latent_hours: u64) -> Pathogen {
        let mut pathogen = epi_pathogens::builtin("IdentityStealer").unwrap();
        pathogen.latent_period = SimDuration::hours(latent_hours);
        pathogen.detection_probability = 1.0;
        pathogen.recovery_probability = 0.2;
        pathogen.infectiousness = 0.9;
        pathogen.reinfection_probability = 0.0;
        pathogen
    }

    fn two_node() -> (Arc<CompiledModel>,) {
        let mut builder = GraphBuilder::new();
        let a = builder.add_typed("a", EntityType::User);
        let b = builder.add_typed("b", EntityType::Server);
        builder.entity_mut(a).infectiousness = 1.0;
        builder.entity_mut(b).susceptibility = 1.0;
        builder.entity_mut(a).detection_probability = 0.2;
        builder.entity_mut(b).detection_probability = 0.2;
        builder.connect(
            a,
            b,
            RelationKind::AuthenticatesTo,
            Channel::Identity,
            2.0,
            0.8,
            0.7,
            0.0,
            1.0,
        );
        let graph = builder.finish();
        let pathogen = tiny_pathogen(1);
        let timing = TimingOverrides {
            detection_delay: Some(SimDuration::hours(6)),
            isolation_delay: Some(SimDuration::hours(2)),
            recovery_time: Some(SimDuration::hours(4)),
        };
        let model = compile(&CompileInputs {
            graph: &graph,
            pathogen: &pathogen,
            controls: &ControlSet::default(),
            preset: ModelPreset::Seidqrp,
            law: TransmissionLaw::ClampedProduct,
            timing: &timing,
            coverage_seed: 3,
        })
        .unwrap();
        (model,)
    }

    #[test]
    fn same_seed_reproduces_the_run() {
        let (model,) = two_node();
        let options = RunOptions {
            duration: SimDuration::days(3),
            event_mode: EventMode::Full,
            ..RunOptions::default()
        };
        let left = run_once(Arc::clone(&model), options.clone(), 99);
        let right = run_once(model, options, 99);
        assert_eq!(left.outbreak_size, right.outbreak_size);
        assert_eq!(left.events, right.events);
        assert_eq!(left.samples.len(), right.samples.len());
        for sample in &left.samples {
            let total = sample.susceptible
                + sample.exposed
                + sample.infectious
                + sample.detected
                + sample.quarantined
                + sample.recovered
                + sample.protected;
            assert!((total - 2.0).abs() < 1e-6);
        }
    }

    #[test]
    fn counts_never_go_negative_and_sum_to_population() {
        let (model,) = two_node();
        let summary = run_once(
            model,
            RunOptions {
                duration: SimDuration::days(2),
                ..RunOptions::default()
            },
            5,
        );
        assert_eq!(summary.final_counts.total(), 2);
        assert!(summary.attack_rate <= 1.0 && summary.attack_rate >= 0.0);
    }

    #[test]
    fn isolated_node_is_never_reached() {
        let mut builder = GraphBuilder::new();
        let a = builder.add_typed("a", EntityType::User);
        let b = builder.add_typed("b", EntityType::Server);
        let isolated = builder.add_typed("solo", EntityType::Database);
        builder.connect(
            a,
            b,
            RelationKind::ConnectsTo,
            Channel::Network,
            3.0,
            0.9,
            0.9,
            0.0,
            1.0,
        );
        let graph = builder.finish();
        let pathogen = tiny_pathogen(1);
        let timing = TimingOverrides::default();
        let model = compile(&CompileInputs {
            graph: &graph,
            pathogen: &pathogen,
            controls: &ControlSet::default(),
            preset: ModelPreset::Sir,
            law: TransmissionLaw::ExponentialSaturation,
            timing: &timing,
            coverage_seed: 1,
        })
        .unwrap();
        let summary = run_once(
            model,
            RunOptions {
                duration: SimDuration::days(4),
                algorithm: Algorithm::Gillespie,
                strategy: SeedStrategy::Targeted,
                ..RunOptions::default()
            },
            8,
        );
        assert_eq!(summary.parent[isolated.index()], NONE);
        let _ = (a, b);
    }

    #[test]
    fn latent_clock_matches_exponential_mean() {
        let mut builder = GraphBuilder::new();
        builder.add_typed("only", EntityType::User);
        let graph = builder.finish();
        let mut pathogen = tiny_pathogen(12);
        pathogen.detection_probability = 0.0;
        pathogen.recovery_probability = 1.0;
        let timing = TimingOverrides {
            detection_delay: Some(SimDuration::days(5)),
            isolation_delay: Some(SimDuration::days(5)),
            recovery_time: Some(SimDuration::days(5)),
        };
        let model = compile(&CompileInputs {
            graph: &graph,
            pathogen: &pathogen,
            controls: &ControlSet::default(),
            preset: ModelPreset::Seir,
            law: TransmissionLaw::ClampedProduct,
            timing: &timing,
            coverage_seed: 1,
        })
        .unwrap();
        // Start the only node in Exposed by using a zero-infection run and then
        // measuring the latent rate directly through many full runs is awkward
        // because seeding jumps to Infectious. Check the compiled latent rate.
        let latent = model.nodes[0].latent;
        let expected = 1.0 / (12.0 / 24.0);
        assert!((latent - expected).abs() < 1e-6, "{latent} vs {expected}");
    }

    #[test]
    fn immediate_isolation_clears_detected_state() {
        let (model,) = two_node();
        let summary = run_once(
            model,
            RunOptions {
                policy: PolicyKind::ImmediateIsolation,
                duration: SimDuration::days(5),
                event_mode: EventMode::Full,
                strategy: SeedStrategy::Targeted,
                ..RunOptions::default()
            },
            21,
        );
        assert_eq!(summary.final_counts.total(), 2);
        assert!(
            summary.final_counts.detected == 0
                || summary
                    .events
                    .iter()
                    .any(|event| event.kind == EventKind::Quarantined)
        );
    }
}
