//! Compile a population, pathogen, and control set into daily hazards.
//!
//! Mapping from user-facing delays to exponential rates (illustrative):
//!
//! * Latent sojourn mean = pathogen latent period. Rate = 1 / mean.
//! * Infectious sojourn mean = detection delay. The exit splits between
//!   detection and undetected recovery in proportion to
//!   `pathogen.detection_probability * sensor` and
//!   `pathogen.recovery_probability`.
//! * Detected sojourn mean = isolation delay, split by isolation probability.
//! * Quarantine sojourn mean = recovery time.
//! * Waning rate = reinfection probability / immunity period.
//!
//! Presets other than SEIDQRP collapse this chain. See `docs/modelling/state-models.md`.

use crate::reproduction::{reproduction, Reproduction};
use crate::transmission::{
    edge_modifier, environmental_modifier, privilege_modifier, ClampedProduct,
    ExponentialSaturation, TransmissionContext, TransmissionStrategy,
};
use epi_controls::{ControlSet, ResolvedControl};
use epi_core::{Channel, EpiState, ModelPreset, SimDuration, TransmissionLaw, MAX_HAZARD_PER_DAY};
use epi_graph::PopulationGraph;
use epi_pathogens::Pathogen;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use thiserror::Error;

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct TimingOverrides {
    pub detection_delay: Option<SimDuration>,
    pub isolation_delay: Option<SimDuration>,
    pub recovery_time: Option<SimDuration>,
}

#[derive(Clone, Copy, Debug)]
pub struct NodeRates {
    pub latent: f64,
    pub detect: f64,
    pub recover_i: f64,
    pub isolate: f64,
    pub recover_d: f64,
    pub recover_q: f64,
    pub wane: f64,
    pub protect: f64,
    pub lose_protection: f64,
}

#[derive(Clone, Debug)]
pub struct CompiledModel {
    pub graph_name: String,
    pub preset: ModelPreset,
    pub law: TransmissionLaw,
    pub nodes: Vec<NodeRates>,
    pub initial_state: Vec<EpiState>,
    pub edge_hazard: Vec<f64>,
    pub outgoing: Vec<Vec<u32>>,
    pub incoming: Vec<Vec<u32>>,
    pub dst: Vec<usize>,
    pub src: Vec<usize>,
    pub names: Vec<String>,
    pub types: Vec<epi_core::EntityType>,
    pub criticality: Vec<f64>,
    pub privilege: Vec<f64>,
    pub susceptibility: Vec<f64>,
    pub segmented_seed: Vec<bool>,
    pub entry_on_transmit: EpiState,
    pub infectious_recovery_target: EpiState,
    pub detection_enabled: bool,
    pub quarantine_enabled: bool,
    pub protection_enabled: bool,
    pub reproduction: Reproduction,
}

impl CompiledModel {
    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }
}

#[derive(Debug, Error)]
pub enum CompileError {
    #[error("population is empty")]
    Empty,
    #[error(transparent)]
    Controls(#[from] epi_controls::ControlError),
}

struct Profile {
    entry: EpiState,
    use_latent: bool,
    use_detection: bool,
    use_quarantine: bool,
    use_wane: bool,
    use_protection: bool,
    recover_to: EpiState,
}

fn profile(preset: ModelPreset) -> Profile {
    match preset {
        ModelPreset::Sis => Profile {
            entry: EpiState::Infectious,
            use_latent: false,
            use_detection: false,
            use_quarantine: false,
            use_wane: false,
            use_protection: false,
            recover_to: EpiState::Susceptible,
        },
        ModelPreset::Sir => Profile {
            entry: EpiState::Infectious,
            use_latent: false,
            use_detection: false,
            use_quarantine: false,
            use_wane: false,
            use_protection: false,
            recover_to: EpiState::Recovered,
        },
        ModelPreset::Seir => Profile {
            entry: EpiState::Exposed,
            use_latent: true,
            use_detection: false,
            use_quarantine: false,
            use_wane: false,
            use_protection: false,
            recover_to: EpiState::Recovered,
        },
        ModelPreset::Seirs => Profile {
            entry: EpiState::Exposed,
            use_latent: true,
            use_detection: false,
            use_quarantine: false,
            use_wane: true,
            use_protection: false,
            recover_to: EpiState::Recovered,
        },
        ModelPreset::Seidqrp => Profile {
            entry: EpiState::Exposed,
            use_latent: true,
            use_detection: true,
            use_quarantine: true,
            use_wane: true,
            use_protection: true,
            recover_to: EpiState::Recovered,
        },
    }
}

pub struct CompileInputs<'a> {
    pub graph: &'a PopulationGraph,
    pub pathogen: &'a Pathogen,
    pub controls: &'a ControlSet,
    pub preset: ModelPreset,
    pub law: TransmissionLaw,
    pub timing: &'a TimingOverrides,
    pub coverage_seed: u64,
}

pub fn compile(inputs: &CompileInputs<'_>) -> Result<Arc<CompiledModel>, CompileError> {
    if inputs.graph.entities.is_empty() {
        return Err(CompileError::Empty);
    }
    let controls = inputs.controls.resolved()?;
    let shape = profile(inputs.preset);
    let n = inputs.graph.entities.len();
    let mut nodes = Vec::with_capacity(n);
    let mut initial_state = Vec::with_capacity(n);
    let mut names = Vec::with_capacity(n);
    let mut types = Vec::with_capacity(n);
    let mut criticality = Vec::with_capacity(n);
    let mut privilege = Vec::with_capacity(n);
    let mut susceptibility = Vec::with_capacity(n);

    for entity in &inputs.graph.entities {
        let applied = entity_controls(&controls, inputs.coverage_seed, entity.id.0);
        let mut detection_delay = inputs
            .timing
            .detection_delay
            .unwrap_or(entity.detection_delay);
        let mut isolation_delay = inputs
            .timing
            .isolation_delay
            .unwrap_or(entity.isolation_delay);
        let mut recovery_time = inputs.timing.recovery_time.unwrap_or(entity.recovery_time);
        let mut detection_multiplier = 1.0;
        let mut isolation_multiplier = 1.0;
        let mut susceptibility_multiplier = 1.0;
        let mut protected = false;
        for control in &applied {
            detection_multiplier *= control.detection_multiplier.max(0.0);
            isolation_multiplier *= control.isolation_delay_multiplier.max(0.0);
            susceptibility_multiplier *= control.susceptibility_multiplier.max(0.0);
            if let Some(value) = control.detection_delay {
                detection_delay = value;
            }
            if let Some(value) = control.isolation_delay {
                isolation_delay = value;
            }
            if let Some(value) = control.recovery_time {
                recovery_time = value;
            }
            if control.protects && shape.use_protection {
                protected = true;
            }
        }
        let sensor = epi_core::clamp01(entity.detection_probability);
        let detect_weight = epi_core::nonneg(inputs.pathogen.detection_probability)
            * (0.45 + 0.55 * sensor)
            * detection_multiplier;
        let recover_weight = epi_core::nonneg(inputs.pathogen.recovery_probability).max(0.05);
        let detect_share = if shape.use_detection {
            detect_weight / (detect_weight + recover_weight)
        } else {
            0.0
        };
        let infectious_days = days_floor(detection_delay);
        let isolation_days = days_floor(isolation_delay) * isolation_multiplier.max(1.0 / 1_440.0);
        let recovery_days = days_floor(recovery_time);
        let latent = if shape.use_latent {
            1.0 / inputs.pathogen.latent_days()
        } else {
            0.0
        };
        let (detect, recover_i) = if shape.use_detection {
            (
                detect_share / infectious_days,
                (1.0 - detect_share) / infectious_days,
            )
        } else {
            let rate = (recover_weight.max(0.2)) / recovery_days;
            (0.0, rate)
        };
        let isolate_p = epi_core::clamp01(entity.isolation_probability);
        let (isolate, recover_d) = if shape.use_detection {
            (
                isolate_p / isolation_days,
                (1.0 - isolate_p) / isolation_days,
            )
        } else {
            (0.0, 0.0)
        };
        let recover_q = if shape.use_quarantine {
            1.0 / recovery_days
        } else {
            0.0
        };
        let wane = if shape.use_wane {
            epi_core::clamp01(inputs.pathogen.reinfection_probability)
                / inputs.pathogen.immunity_days()
        } else {
            0.0
        };
        let lose_protection = if protected || shape.use_protection {
            (1.0 - applied.iter().map(|c| c.strength).sum::<f64>().min(0.95)
                / applied.len().max(1) as f64)
                / 90.0
        } else {
            0.0
        };
        nodes.push(NodeRates {
            latent,
            detect,
            recover_i,
            isolate,
            recover_d,
            recover_q,
            wane,
            protect: 0.0,
            lose_protection: if protected {
                lose_protection.max(1.0 / 180.0)
            } else {
                0.0
            },
        });
        let state = if protected {
            EpiState::Protected
        } else {
            EpiState::Susceptible
        };
        initial_state.push(state);
        names.push(entity.name.clone());
        types.push(entity.entity_type);
        criticality.push(epi_core::clamp01(entity.criticality));
        privilege.push(epi_core::clamp01(entity.privilege));
        susceptibility.push(
            (epi_core::clamp01(entity.susceptibility) * susceptibility_multiplier).clamp(0.0, 1.0),
        );
        let _ = shape.recover_to;
    }

    let mut edge_hazard = Vec::with_capacity(inputs.graph.edges.len());
    let mut dst = Vec::with_capacity(inputs.graph.edges.len());
    let mut src = Vec::with_capacity(inputs.graph.edges.len());
    for edge in &inputs.graph.edges {
        let source = &inputs.graph.entities[edge.src.index()];
        let target_index = edge.dst.index();
        let target_controls = entity_controls(&controls, inputs.coverage_seed, edge.dst.0);
        let source_controls = entity_controls(&controls, inputs.coverage_seed, edge.src.0);
        let mut control_modifier = 1.0;
        let mut privilege_scale = 1.0;
        for control in &target_controls {
            if !matches!(control.kind, epi_controls::ControlKind::Segmentation) {
                control_modifier *= control.channel_multiplier(edge.channel);
            }
        }
        for control in &source_controls {
            privilege_scale *= control.privilege_multiplier.max(0.0);
        }
        // Segmentation is a property of the edge's network channel, applied once.
        for control in &controls {
            if matches!(control.kind, epi_controls::ControlKind::Segmentation)
                && matches!(edge.channel, Channel::Network)
            {
                control_modifier *= control.channel_multiplier(edge.channel);
            }
        }
        let ctx = TransmissionContext {
            pathogen_infectiousness: inputs.pathogen.infectiousness,
            channel_weight: inputs.pathogen.channels.weight(edge.channel),
            source_infectiousness: source.infectiousness,
            target_susceptibility: susceptibility[target_index],
            edge_modifier: edge_modifier(edge.transmission_modifier, edge.trust_level),
            contact_frequency: edge.contact_frequency,
            privilege_modifier: privilege_modifier(source.privilege, edge.access_strength)
                * privilege_scale.clamp(0.0, 2.0),
            environmental_modifier: environmental_modifier(
                edge.segmentation_strength,
                inputs.graph.entities[target_index].patch_level,
                inputs.graph.entities[target_index].exposure,
            ),
            control_modifier: control_modifier.max(0.0),
        };
        let hazard = match inputs.law {
            TransmissionLaw::ClampedProduct => ClampedProduct.hazard_per_day(&ctx),
            TransmissionLaw::ExponentialSaturation => ExponentialSaturation.hazard_per_day(&ctx),
        };
        edge_hazard.push(hazard.min(MAX_HAZARD_PER_DAY));
        dst.push(edge.dst.index());
        src.push(edge.src.index());
    }

    let mut model = CompiledModel {
        graph_name: inputs
            .graph
            .entities
            .first()
            .map(|entity| entity.name.clone())
            .unwrap_or_default(),
        preset: inputs.preset,
        law: inputs.law,
        nodes,
        initial_state,
        edge_hazard,
        outgoing: inputs.graph.outgoing.clone(),
        incoming: inputs.graph.incoming.clone(),
        dst,
        src,
        names,
        types,
        criticality,
        privilege,
        susceptibility,
        segmented_seed: vec![false; n],
        entry_on_transmit: shape.entry,
        infectious_recovery_target: shape.recover_to,
        detection_enabled: shape.use_detection,
        quarantine_enabled: shape.use_quarantine,
        protection_enabled: shape.use_protection,
        reproduction: Reproduction {
            r0_mean: 0.0,
            invasion_threshold: 0.0,
            expected_secondary: Vec::new(),
        },
    };
    model.reproduction = reproduction(&model);
    Ok(Arc::new(model))
}

fn entity_controls(controls: &[ResolvedControl], seed: u64, entity: u32) -> Vec<ResolvedControl> {
    controls
        .iter()
        .filter(|control| control.applies_to(seed, entity))
        .cloned()
        .collect()
}

fn days_floor(duration: SimDuration) -> f64 {
    duration.as_days().max(1.0 / 1_440.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use epi_core::{Entity, EntityId, EntityType};
    use epi_graph::GraphBuilder;
    use epi_pathogens::builtin;
    use std::collections::BTreeMap;

    #[test]
    fn mfa_lowers_identity_hazard_and_r0() {
        let mut builder = GraphBuilder::new();
        let a = builder.add_typed("a", EntityType::User);
        let b = builder.add_typed("b", EntityType::Identity);
        builder.entity_mut(a).infectiousness = 1.0;
        builder.entity_mut(b).susceptibility = 1.0;
        builder.connect(
            a,
            b,
            epi_core::RelationKind::AuthenticatesTo,
            Channel::Identity,
            1.5,
            0.8,
            0.7,
            0.05,
            1.0,
        );
        builder.connect(
            b,
            a,
            epi_core::RelationKind::UsesIdentity,
            Channel::Identity,
            1.2,
            0.6,
            0.6,
            0.05,
            1.0,
        );
        let graph = builder.finish();
        let pathogen = builtin("IdentityStealer").unwrap();
        let timing = TimingOverrides::default();
        let bare = compile(&CompileInputs {
            graph: &graph,
            pathogen: &pathogen,
            controls: &ControlSet::default(),
            preset: ModelPreset::Seidqrp,
            law: TransmissionLaw::ClampedProduct,
            timing: &timing,
            coverage_seed: 1,
        })
        .unwrap();
        let protected = ControlSet {
            controls: BTreeMap::from([(
                "mfa".to_string(),
                epi_controls::ControlConfig {
                    coverage: Some(1.0),
                    ..epi_controls::ControlConfig::default()
                },
            )]),
        };
        let with_mfa = compile(&CompileInputs {
            graph: &graph,
            pathogen: &pathogen,
            controls: &protected,
            preset: ModelPreset::Seidqrp,
            law: TransmissionLaw::ClampedProduct,
            timing: &timing,
            coverage_seed: 1,
        })
        .unwrap();
        assert!(with_mfa.edge_hazard[0] < bare.edge_hazard[0]);
        assert!(with_mfa.reproduction.r0_mean < bare.reproduction.r0_mean);
        let _ = Entity::baseline(EntityId(0), "unused", EntityType::User);
    }
}
