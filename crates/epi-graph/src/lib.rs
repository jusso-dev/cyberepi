//! Multiplex contact graph.
//!
//! Edges are relationships with transmission attributes. They are not sessions,
//! exploits, or protocol implementations.

use epi_core::{Channel, Entity, EntityId, EntityType, EpiState, RelationKind};
use petgraph::unionfind::UnionFind;
use petgraph::Graph;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Edge {
    pub src: EntityId,
    pub dst: EntityId,
    pub kind: RelationKind,
    pub channel: Channel,
    /// Expected contact opportunities per day. May be greater than 1.
    pub contact_frequency: f64,
    pub access_strength: f64,
    pub trust_level: f64,
    pub segmentation_strength: f64,
    pub transmission_modifier: f64,
}

impl Edge {
    pub fn spec(
        kind: RelationKind,
        channel: Channel,
        contact_frequency: f64,
        access_strength: f64,
        trust_level: f64,
        segmentation_strength: f64,
        transmission_modifier: f64,
    ) -> Self {
        Self {
            src: EntityId(0),
            dst: EntityId(0),
            kind,
            channel,
            contact_frequency,
            access_strength,
            trust_level,
            segmentation_strength,
            transmission_modifier,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PopulationGraph {
    pub entities: Vec<Entity>,
    pub edges: Vec<Edge>,
    pub outgoing: Vec<Vec<u32>>,
    pub incoming: Vec<Vec<u32>>,
}

impl PopulationGraph {
    pub fn node_count(&self) -> usize {
        self.entities.len()
    }

    pub fn edge_count(&self) -> usize {
        self.edges.len()
    }

    pub fn entity(&self, id: EntityId) -> &Entity {
        &self.entities[id.index()]
    }

    /// Undirected connectivity via petgraph's union-find.
    pub fn component_count(&self) -> usize {
        let mut uf = UnionFind::new(self.entities.len());
        for edge in &self.edges {
            uf.union(edge.src.index(), edge.dst.index());
        }
        let mut roots = std::collections::BTreeSet::new();
        for index in 0..self.entities.len() {
            roots.insert(uf.find(index));
        }
        roots.len()
    }

    pub fn largest_component_fraction(&self) -> f64 {
        if self.entities.is_empty() {
            return 0.0;
        }
        let mut uf = UnionFind::new(self.entities.len());
        for edge in &self.edges {
            uf.union(edge.src.index(), edge.dst.index());
        }
        let mut sizes = vec![0u32; self.entities.len()];
        for index in 0..self.entities.len() {
            sizes[uf.find(index)] += 1;
        }
        let largest = sizes.into_iter().max().unwrap_or(0) as f64;
        largest / self.entities.len() as f64
    }

    /// Weighted digraph for callers that want petgraph algorithms.
    /// Edge weight is the stored transmission modifier.
    pub fn to_petgraph(&self) -> Graph<u32, f64> {
        let mut graph = Graph::with_capacity(self.entities.len(), self.edges.len());
        let nodes: Vec<_> = (0..self.entities.len())
            .map(|index| graph.add_node(index as u32))
            .collect();
        for edge in &self.edges {
            graph.add_edge(
                nodes[edge.src.index()],
                nodes[edge.dst.index()],
                edge.transmission_modifier,
            );
        }
        graph
    }
}

#[derive(Clone, Debug)]
pub struct GraphBuilder {
    entities: Vec<Entity>,
    edges: Vec<Edge>,
}

impl Default for GraphBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl GraphBuilder {
    pub fn new() -> Self {
        Self {
            entities: Vec::new(),
            edges: Vec::new(),
        }
    }

    pub fn add_entity(&mut self, mut entity: Entity) -> EntityId {
        let id = EntityId(self.entities.len() as u32);
        entity.id = id;
        if entity.state != EpiState::Susceptible
            && entity.state != EpiState::Protected
            && entity.state != EpiState::Infectious
        {
            entity.state = EpiState::Susceptible;
        }
        self.entities.push(entity);
        id
    }

    pub fn add_typed(&mut self, name: impl Into<String>, entity_type: EntityType) -> EntityId {
        let id = EntityId(self.entities.len() as u32);
        self.add_entity(Entity::baseline(id, name, entity_type))
    }

    pub fn entity_mut(&mut self, id: EntityId) -> &mut Entity {
        &mut self.entities[id.index()]
    }

    pub fn add_edge(&mut self, src: EntityId, dst: EntityId, mut edge: Edge) {
        if src == dst {
            return;
        }
        edge.src = src;
        edge.dst = dst;
        self.edges.push(edge);
    }

    #[allow(clippy::too_many_arguments)]
    pub fn connect(
        &mut self,
        src: EntityId,
        dst: EntityId,
        kind: RelationKind,
        channel: Channel,
        contact_frequency: f64,
        access_strength: f64,
        trust_level: f64,
        segmentation_strength: f64,
        transmission_modifier: f64,
    ) {
        self.add_edge(
            src,
            dst,
            Edge::spec(
                kind,
                channel,
                contact_frequency,
                access_strength,
                trust_level,
                segmentation_strength,
                transmission_modifier,
            ),
        );
    }

    pub fn len(&self) -> usize {
        self.entities.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entities.is_empty()
    }

    pub fn finish(self) -> PopulationGraph {
        let n = self.entities.len();
        let mut outgoing = vec![Vec::new(); n];
        let mut incoming = vec![Vec::new(); n];
        for (index, edge) in self.edges.iter().enumerate() {
            let index = index as u32;
            outgoing[edge.src.index()].push(index);
            incoming[edge.dst.index()].push(index);
        }
        PopulationGraph {
            entities: self.entities,
            edges: self.edges,
            outgoing,
            incoming,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use epi_core::EntityType;
    use petgraph::algo::dijkstra;

    #[test]
    fn builder_links_both_directions_and_petgraph_reaches() {
        let mut builder = GraphBuilder::new();
        let a = builder.add_typed("a", EntityType::User);
        let b = builder.add_typed("b", EntityType::Endpoint);
        builder.connect(
            a,
            b,
            RelationKind::AuthenticatesTo,
            Channel::Identity,
            1.0,
            0.4,
            0.5,
            0.1,
            0.8,
        );
        let graph = builder.finish();
        assert_eq!(graph.edge_count(), 1);
        assert_eq!(graph.component_count(), 1);
        let pet = graph.to_petgraph();
        let distances = dijkstra(&pet, pet.node_indices().next().unwrap(), None, |edge| {
            *edge.weight()
        });
        assert_eq!(distances.len(), 2);
    }
}
