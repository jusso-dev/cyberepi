use epi_controls::ControlSet;
use epi_core::{
    parse_duration, Algorithm, EventMode, ModelPreset, PolicyKind, SeedStrategy, SimDuration,
    TransmissionLaw,
};
use epi_generator::OrgTemplate;
use epi_pathogens::builtin;
use serde::de::{self, Deserializer, Visitor};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::fmt;
use thiserror::Error;

const SCHEMA: &str = include_str!("../../../schemas/scenario.schema.json");

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Scenario {
    #[serde(rename = "apiVersion")]
    pub api_version: String,
    pub kind: String,
    pub metadata: Metadata,
    pub population: PopulationSpec,
    pub pathogen: PathogenRef,
    #[serde(default)]
    pub initial_conditions: InitialConditions,
    #[serde(default = "default_model")]
    pub model: ModelPreset,
    #[serde(default)]
    pub controls: ControlSet,
    #[serde(default)]
    pub defender: DefenderSpec,
    pub simulation: SimulationSpec,
    #[serde(default)]
    pub variants: Vec<VariantSpec>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Metadata {
    pub name: String,
    #[serde(default)]
    pub description: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PopulationSpec {
    pub template: OrgTemplate,
    pub size: u32,
    #[serde(default = "default_seed")]
    pub seed: u64,
    #[serde(default)]
    pub name: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PathogenRef {
    #[serde(rename = "type")]
    pub kind: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct InitialConditions {
    #[serde(default = "default_infected")]
    pub infected_entities: u32,
    #[serde(default)]
    pub strategy: SeedStrategy,
}

impl Default for InitialConditions {
    fn default() -> Self {
        Self {
            infected_entities: 1,
            strategy: SeedStrategy::Random,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DefenderSpec {
    #[serde(default = "default_policy")]
    pub policy: PolicyKind,
}

impl Default for DefenderSpec {
    fn default() -> Self {
        Self {
            policy: PolicyKind::NoResponse,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SimulationSpec {
    #[serde(default = "default_algorithm")]
    pub algorithm: Algorithm,
    #[serde(default = "default_law")]
    pub transmission: TransmissionLaw,
    #[serde(deserialize_with = "de_duration")]
    pub duration: SimDuration,
    pub runs: u32,
    pub seed: u64,
    #[serde(default = "default_event_mode")]
    pub event_mode: EventMode,
    #[serde(default = "default_sample", deserialize_with = "de_duration")]
    pub sample_interval: SimDuration,
    #[serde(default = "default_dt", deserialize_with = "de_duration")]
    pub dt: SimDuration,
    #[serde(default)]
    pub threads: Option<usize>,
    #[serde(default = "default_extinction")]
    pub extinction_threshold_fraction: f64,
    #[serde(default, deserialize_with = "epi_controls::de_opt_duration")]
    pub detection_delay: Option<SimDuration>,
    #[serde(default, deserialize_with = "epi_controls::de_opt_duration")]
    pub isolation_delay: Option<SimDuration>,
    #[serde(default, deserialize_with = "epi_controls::de_opt_duration")]
    pub recovery_time: Option<SimDuration>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct VariantSpec {
    pub name: String,
    #[serde(default)]
    pub controls: ControlSet,
    #[serde(default)]
    pub policy: Option<PolicyKind>,
}

fn default_model() -> ModelPreset {
    ModelPreset::Seidqrp
}
fn default_seed() -> u64 {
    1
}
fn default_infected() -> u32 {
    1
}
fn default_policy() -> PolicyKind {
    PolicyKind::NoResponse
}
fn default_algorithm() -> Algorithm {
    Algorithm::Gillespie
}
fn default_law() -> TransmissionLaw {
    TransmissionLaw::ClampedProduct
}
fn default_event_mode() -> EventMode {
    EventMode::Summary
}
fn default_sample() -> SimDuration {
    SimDuration::hours(1)
}
fn default_dt() -> SimDuration {
    SimDuration::minutes(15)
}
fn default_extinction() -> f64 {
    0.005
}

pub fn de_duration<'de, D>(deserializer: D) -> Result<SimDuration, D::Error>
where
    D: Deserializer<'de>,
{
    struct DurationVisitor;
    impl Visitor<'_> for DurationVisitor {
        type Value = SimDuration;

        fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
            formatter.write_str("a duration such as 7d or 15m")
        }

        fn visit_str<E: de::Error>(self, value: &str) -> Result<Self::Value, E> {
            parse_duration(value).map_err(E::custom)
        }

        fn visit_u64<E: de::Error>(self, value: u64) -> Result<Self::Value, E> {
            Ok(SimDuration::seconds(value))
        }
    }
    deserializer.deserialize_any(DurationVisitor)
}

#[derive(Debug, Error)]
pub enum ScenarioError {
    #[error("scenario YAML could not be parsed: {0}")]
    Parse(String),
    #[error("scenario is invalid:\n{0}")]
    Invalid(String),
    #[error("scenario JSON schema rejected the document: {0}")]
    Schema(String),
    #[error(transparent)]
    Generate(#[from] epi_generator::GenerateError),
    #[error(transparent)]
    Compile(#[from] epi_models::CompileError),
    #[error(transparent)]
    Controls(#[from] epi_controls::ControlError),
    #[error("{0}")]
    Message(String),
}

pub fn load_scenario_str(yaml: &str) -> Result<Scenario, ScenarioError> {
    let value: Value =
        serde_yaml::from_str(yaml).map_err(|err| ScenarioError::Parse(err.to_string()))?;
    validate_value(&value)?;
    let scenario: Scenario =
        serde_yaml::from_str(yaml).map_err(|err| ScenarioError::Parse(err.to_string()))?;
    validate_scenario(&scenario)?;
    Ok(scenario)
}

pub fn load_scenario(path: &std::path::Path) -> Result<Scenario, ScenarioError> {
    let text = std::fs::read_to_string(path).map_err(|err| {
        ScenarioError::Message(format!("failed to read {}: {err}", path.display()))
    })?;
    load_scenario_str(&text)
}

pub fn scenario_hash(scenario: &Scenario) -> String {
    let json = serde_json::to_vec(scenario).unwrap_or_default();
    hex::encode(Sha256::digest(json))
}

pub fn validate_scenario(scenario: &Scenario) -> Result<(), ScenarioError> {
    let mut errors = Vec::new();
    if scenario.api_version != "cyberepi.io/v1" {
        errors.push(format!(
            "apiVersion must be cyberepi.io/v1, found {}",
            scenario.api_version
        ));
    }
    if scenario.kind != "Scenario" {
        errors.push("kind must be Scenario".to_string());
    }
    if scenario.metadata.name.trim().is_empty() {
        errors.push("metadata.name is required".to_string());
    }
    if !(32..=1_000_000).contains(&scenario.population.size) {
        errors.push("population.size must be between 32 and 1000000".to_string());
    }
    if builtin(&scenario.pathogen.kind).is_err() {
        errors.push(format!(
            "unknown pathogen `{}`. Built-ins: {}",
            scenario.pathogen.kind,
            epi_pathogens::builtins()
                .iter()
                .map(|item| item.name.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }
    if scenario.initial_conditions.infected_entities == 0 {
        errors.push("initial_conditions.infected_entities must be at least 1".to_string());
    }
    if scenario.initial_conditions.infected_entities > scenario.population.size {
        errors.push("initial infections cannot exceed the population".to_string());
    }
    if scenario.simulation.runs == 0 || scenario.simulation.runs > 1_000_000 {
        errors.push("simulation.runs must be between 1 and 1000000".to_string());
    }
    if scenario.simulation.duration.as_secs() == 0 {
        errors.push("simulation.duration must be greater than zero".to_string());
    }
    if !(0.0..=1.0).contains(&scenario.simulation.extinction_threshold_fraction) {
        errors.push("extinction_threshold_fraction must be between 0 and 1".to_string());
    }
    if let Err(err) = scenario.controls.resolved() {
        errors.push(err.to_string());
    }
    for variant in &scenario.variants {
        if variant.name.trim().is_empty() {
            errors.push("variant name is required".to_string());
        }
        if let Err(err) = variant.controls.resolved() {
            errors.push(format!("variant `{}`: {err}", variant.name));
        }
    }
    if errors.is_empty() {
        Ok(())
    } else {
        Err(ScenarioError::Invalid(errors.join("\n")))
    }
}

fn validate_value(value: &Value) -> Result<(), ScenarioError> {
    let schema =
        serde_json::from_str(SCHEMA).map_err(|err| ScenarioError::Schema(err.to_string()))?;
    let validator =
        jsonschema::validator_for(&schema).map_err(|err| ScenarioError::Schema(err.to_string()))?;
    if let Err(error) = validator.validate(value) {
        return Err(ScenarioError::Schema(error.to_string()));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn demo_scenario_validates() {
        let yaml = include_str!("../../../scenarios/demos/smoke.yaml");
        let scenario = load_scenario_str(yaml).expect("smoke scenario");
        assert_eq!(scenario.metadata.name, "smoke-outbreak");
        assert_eq!(scenario_hash(&scenario).len(), 64);
    }
}
