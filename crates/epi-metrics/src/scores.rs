use epi_core::EntityType;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SuperspreaderScore {
    pub entity: u32,
    pub name: String,
    pub entity_type: EntityType,
    pub score: f64,
    pub expected_secondary: f64,
    pub mean_secondary: f64,
    pub degree: f64,
    pub pagerank: f64,
    pub privilege: f64,
    pub criticality: f64,
    pub transmission_contribution: f64,
    pub systemic_exposure: f64,
    pub containment_importance: f64,
    pub reasons: Vec<String>,
}

#[allow(clippy::too_many_arguments)]
pub fn rank_superspreaders(
    names: &[String],
    types: &[EntityType],
    expected: &[f64],
    mean_secondary: &[f64],
    degree: &[f64],
    pagerank: &[f64],
    privilege: &[f64],
    criticality: &[f64],
    betweenness: &[f64],
    limit: usize,
) -> Vec<SuperspreaderScore> {
    let n = names.len();
    let max_expected = expected.iter().copied().fold(0.0_f64, f64::max).max(1e-9);
    let max_secondary = mean_secondary
        .iter()
        .copied()
        .fold(0.0_f64, f64::max)
        .max(1e-9);
    let total_secondary = mean_secondary.iter().sum::<f64>().max(1e-9);
    let mut rows = Vec::with_capacity(n);
    for index in 0..n {
        let expected_n = expected.get(index).copied().unwrap_or(0.0) / max_expected;
        let secondary_n = mean_secondary.get(index).copied().unwrap_or(0.0) / max_secondary;
        let degree_n = degree.get(index).copied().unwrap_or(0.0);
        let page_n = pagerank.get(index).copied().unwrap_or(0.0);
        let privilege_n = privilege.get(index).copied().unwrap_or(0.0);
        let criticality_n = criticality.get(index).copied().unwrap_or(0.0);
        let between = betweenness.get(index).copied().unwrap_or(0.0);
        let transmission_contribution =
            mean_secondary.get(index).copied().unwrap_or(0.0) / total_secondary;
        let systemic_exposure =
            (0.45 * page_n + 0.35 * criticality_n + 0.20 * privilege_n).clamp(0.0, 1.0);
        let containment_importance =
            (0.5 * between + 0.3 * expected_n + 0.2 * criticality_n).clamp(0.0, 1.0);
        let score = 0.42 * expected_n
            + 0.23 * secondary_n
            + 0.15 * page_n
            + 0.10 * degree_n
            + 0.10 * privilege_n;
        let mut reasons = Vec::new();
        if expected_n > 0.55 {
            reasons.push(format!(
                "Expected secondary compromises ({:.1}) are high if this entity is the index case in a susceptible population.",
                expected.get(index).copied().unwrap_or(0.0)
            ));
        }
        if secondary_n > 0.4 {
            reasons.push(format!(
                "Across the Monte Carlo runs it produced {:.1} secondary compromises on average.",
                mean_secondary.get(index).copied().unwrap_or(0.0)
            ));
        }
        if degree_n > 0.7 {
            reasons
                .push("Contact degree is among the highest in the organisation graph.".to_string());
        }
        if page_n > 0.6 {
            reasons.push(
                "PageRank is high, so paths through this entity reach many others.".to_string(),
            );
        }
        if privilege_n > 0.75 {
            reasons.push(
                "Privilege is high, which raises the transmission modifier on administrative relationships."
                    .to_string(),
            );
        }
        if criticality_n > 0.8 {
            reasons.push(
                "Criticality is high, so compromise here has outsized operational impact in the score."
                    .to_string(),
            );
        }
        if reasons.is_empty() {
            reasons.push(
                "Ranked from a blend of expected offspring, observed transmission, and graph position."
                    .to_string(),
            );
        }
        rows.push(SuperspreaderScore {
            entity: index as u32,
            name: names[index].clone(),
            entity_type: types.get(index).copied().unwrap_or(EntityType::Server),
            score,
            expected_secondary: expected.get(index).copied().unwrap_or(0.0),
            mean_secondary: mean_secondary.get(index).copied().unwrap_or(0.0),
            degree: degree_n,
            pagerank: page_n,
            privilege: privilege_n,
            criticality: criticality_n,
            transmission_contribution,
            systemic_exposure,
            containment_importance,
            reasons,
        });
    }
    rows.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    rows.truncate(limit);
    rows
}
