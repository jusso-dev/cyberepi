use epi_graph::PopulationGraph;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Centrality {
    pub degree: Vec<f64>,
    pub pagerank: Vec<f64>,
    pub eigenvector: Vec<f64>,
    /// Approximate betweenness from a sample of sources. Empty when skipped.
    pub betweenness: Vec<f64>,
}

pub fn centrality(graph: &PopulationGraph) -> Centrality {
    let n = graph.node_count();
    let mut degree = vec![0.0; n];
    for (index, edges) in graph.outgoing.iter().enumerate() {
        degree[index] = edges.len() as f64;
    }
    let max_degree = degree.iter().copied().fold(0.0_f64, f64::max).max(1.0);
    for value in &mut degree {
        *value /= max_degree;
    }
    Centrality {
        degree,
        pagerank: pagerank(graph, 20, 0.85),
        eigenvector: eigenvector(graph, 24),
        betweenness: if n <= 2_500 {
            approximate_betweenness(graph, 24.min(n.max(1)))
        } else {
            Vec::new()
        },
    }
}

fn pagerank(graph: &PopulationGraph, iterations: usize, damping: f64) -> Vec<f64> {
    let n = graph.node_count();
    if n == 0 {
        return Vec::new();
    }
    let mut rank = vec![1.0 / n as f64; n];
    let mut next = vec![0.0; n];
    let mut out_degree = vec![0.0; n];
    for (index, edges) in graph.outgoing.iter().enumerate() {
        out_degree[index] = edges.len() as f64;
    }
    for _ in 0..iterations {
        let leak = rank
            .iter()
            .enumerate()
            .filter(|(index, _)| out_degree[*index] == 0.0)
            .map(|(_, value)| *value)
            .sum::<f64>();
        let base = (1.0 - damping) / n as f64 + damping * leak / n as f64;
        next.fill(base);
        for (src, edges) in graph.outgoing.iter().enumerate() {
            if out_degree[src] == 0.0 {
                continue;
            }
            let share = damping * rank[src] / out_degree[src];
            for &edge_index in edges {
                let dst = graph.edges[edge_index as usize].dst.index();
                next[dst] += share;
            }
        }
        rank.clone_from(&next);
    }
    normalize_max(&mut rank);
    rank
}

fn eigenvector(graph: &PopulationGraph, iterations: usize) -> Vec<f64> {
    let n = graph.node_count();
    let mut v = vec![1.0 / (n as f64).sqrt().max(1.0); n];
    let mut next = vec![0.0; n];
    for _ in 0..iterations {
        next.fill(0.0);
        for (src, edges) in graph.outgoing.iter().enumerate() {
            for &edge_index in edges {
                let dst = graph.edges[edge_index as usize].dst.index();
                next[dst] += v[src];
                next[src] += v[dst] * 0.15;
            }
        }
        let norm = next.iter().map(|value| value * value).sum::<f64>().sqrt();
        if norm <= 1e-15 {
            break;
        }
        for (slot, value) in v.iter_mut().zip(next.iter()) {
            *slot = value / norm;
        }
    }
    normalize_max(&mut v);
    v
}

fn approximate_betweenness(graph: &PopulationGraph, sources: usize) -> Vec<f64> {
    let n = graph.node_count();
    let mut score = vec![0.0; n];
    if n <= 2 {
        return score;
    }
    let step = (n / sources.max(1)).max(1);
    let mut used = 0;
    for mut source in (0..n).step_by(step) {
        if used >= sources {
            break;
        }
        source = source.min(n - 1);
        brandes_one(graph, source, &mut score);
        used += 1;
    }
    normalize_max(&mut score);
    score
}

fn brandes_one(graph: &PopulationGraph, source: usize, score: &mut [f64]) {
    let n = graph.node_count();
    let mut stack = Vec::new();
    let mut predecessors: Vec<Vec<usize>> = vec![Vec::new(); n];
    let mut sigma = vec![0.0; n];
    let mut dist = vec![-1i32; n];
    let mut queue = std::collections::VecDeque::new();
    sigma[source] = 1.0;
    dist[source] = 0;
    queue.push_back(source);
    while let Some(v) = queue.pop_front() {
        stack.push(v);
        for &edge_index in &graph.outgoing[v] {
            let w = graph.edges[edge_index as usize].dst.index();
            if dist[w] < 0 {
                dist[w] = dist[v] + 1;
                queue.push_back(w);
            }
            if dist[w] == dist[v] + 1 {
                sigma[w] += sigma[v];
                predecessors[w].push(v);
            }
        }
    }
    let mut delta = vec![0.0; n];
    while let Some(w) = stack.pop() {
        for &v in &predecessors[w] {
            if sigma[w] > 0.0 {
                delta[v] += (sigma[v] / sigma[w]) * (1.0 + delta[w]);
            }
        }
        if w != source {
            score[w] += delta[w];
        }
    }
}

fn normalize_max(values: &mut [f64]) {
    let max = values.iter().copied().fold(0.0_f64, f64::max);
    if max > 0.0 {
        for value in values {
            *value /= max;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use epi_core::{Channel, EntityType, RelationKind};
    use epi_graph::GraphBuilder;

    #[test]
    fn hub_has_highest_degree_centrality() {
        let mut builder = GraphBuilder::new();
        let hub = builder.add_typed("hub", EntityType::ServiceAccount);
        for index in 0..6 {
            let leaf = builder.add_typed(format!("leaf-{index}"), EntityType::Server);
            builder.connect(
                hub,
                leaf,
                RelationKind::Administers,
                Channel::Identity,
                1.0,
                0.8,
                0.5,
                0.1,
                1.0,
            );
        }
        let graph = builder.finish();
        let scores = centrality(&graph);
        let hub_degree = scores.degree[0];
        assert!(scores
            .degree
            .iter()
            .all(|value| *value <= hub_degree + 1e-9));
        assert!(scores.pagerank[0] > 0.5);
    }
}
