#![allow(clippy::needless_range_loop, clippy::explicit_counter_loop)]
//! Aggregation, centrality, and CyberEpi scores.
//!
//! Centrality here is a structural property of the contact graph. It is not an
//! instruction to attack a node.

mod aggregate;
mod centrality;
mod scores;

pub use aggregate::{percentile, summarize_runs, Histogram, NumericSummary};
pub use centrality::{centrality, Centrality};
pub use scores::{rank_superspreaders, SuperspreaderScore};
