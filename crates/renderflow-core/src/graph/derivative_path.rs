//! Bounded, deterministic route selection for a derivative forest.
//!
//! Legacy targeted conversion keeps its existing optimization policy. Forest
//! routes always rank cumulative fidelity first, then source proximity (hop
//! count), optimization cost, and a stable full edge identity. A target is a
//! terminal: a rejected terminal selector cannot be bypassed by visiting the
//! target, leaving it, and returning through a cycle.

use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashMap};
use std::fmt;

use serde::{Deserialize, Serialize};

use super::{Format, TransformEdge, TransformGraph, TransformPath};
use crate::optimization::OptimizationMode;

/// Constraints apply to the final edge, not merely its output extension.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DerivativePathConstraints {
    pub provider_id: Option<String>,
    pub capability_id: Option<String>,
    pub transform_id: Option<String>,
    pub variant_id: Option<String>,
}

impl DerivativePathConstraints {
    fn matches(&self, edge: &TransformEdge) -> bool {
        self.provider_id
            .as_ref()
            .is_none_or(|id| edge.provider_id.as_ref() == Some(id))
            && self.capability_id
                .as_ref()
                .is_none_or(|id| edge.capability_id.as_ref() == Some(id))
            && self.transform_id
                .as_ref()
                .is_none_or(|id| edge.evidence.get("transform_id") == Some(id))
            && self.variant_id
                .as_ref()
                .is_none_or(|id| edge.variant_id.as_ref() == Some(id))
    }

    fn is_empty(&self) -> bool {
        self.provider_id.is_none()
            && self.capability_id.is_none()
            && self.transform_id.is_none()
            && self.variant_id.is_none()
    }
}

/// The hop limit defines eligible paths; the candidate limit bounds search work.
/// Exhausting search work is an error, never an approximate successful plan.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DerivativePathPolicy {
    pub max_hops: usize,
    pub max_candidates: usize,
    /// Maximum `1 - product(edge.quality)` permitted for a selected path.
    pub max_loss: Option<f64>,
    pub optimization: OptimizationMode,
}

impl Default for DerivativePathPolicy {
    fn default() -> Self {
        Self {
            max_hops: 8,
            max_candidates: 10_000,
            max_loss: None,
            optimization: OptimizationMode::default(),
        }
    }
}

/// Evidence for the proven best path inside the declared hop/loss policy.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DerivativePathEvidence {
    pub ranking_policy: String,
    pub total_quality: f64,
    pub cumulative_loss: f64,
    pub hops: usize,
    pub optimization_cost: f64,
    pub tie_break_identity: String,
    /// Outgoing edge candidates inspected, including rejected cycles/selectors.
    pub explored_candidates: usize,
    /// Eligible terminal candidates discovered before the winner was proven.
    pub candidate_count: usize,
    pub max_hops: usize,
    pub max_candidates: usize,
    pub max_loss: Option<f64>,
    pub optimization: OptimizationMode,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SelectedDerivativePath {
    pub path: TransformPath,
    pub evidence: DerivativePathEvidence,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DerivativePathError {
    InvalidPolicy(String),
    InvalidEdge { identity: String, reason: String },
    CandidateLimitExceeded { limit: usize },
}

impl fmt::Display for DerivativePathError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidPolicy(reason) => write!(f, "invalid derivative path policy: {reason}"),
            Self::InvalidEdge { identity, reason } => write!(f, "invalid derivative edge {identity}: {reason}"),
            Self::CandidateLimitExceeded { limit } => write!(f, "derivative path search exhausted its {limit} candidate budget; no route was selected"),
        }
    }
}

impl std::error::Error for DerivativePathError {}

/// Covers provider, capability, variant, options evidence, input kind and metrics.
/// Used for deterministic ranking and for detecting unsafe prefix substitutions.
pub(super) fn derivative_edge_identity(edge: &TransformEdge) -> String {
    let mut required = edge.required_provider_ids.clone();
    required.sort();
    required.dedup();
    serde_json::to_string(&(
        edge.from.to_string(),
        edge.to.to_string(),
        edge.input_kind.to_string(),
        &edge.provider_id,
        &edge.capability_id,
        &edge.variant_id,
        required,
        &edge.evidence,
        edge.cost.to_bits(),
        edge.quality.to_bits(),
    ))
    .expect("edge identity consists only of JSON-serializable values")
}

#[derive(Clone)]
struct Candidate {
    steps: Vec<TransformEdge>,
    formats: Vec<Format>,
    quality: f64,
    optimization_cost: f64,
    identities: Vec<String>,
    optimization_first: bool,
}

impl Candidate {
    fn source(source: Format, optimization_first: bool) -> Self {
        Self {
            steps: Vec::new(),
            formats: vec![source],
            quality: 1.0,
            optimization_cost: 0.0,
            identities: Vec::new(),
            optimization_first,
        }
    }

    /// A retained prefix may replace this one only when it leaves at least the
    /// same unvisited formats available. Component-wise ranking also remains
    /// sound when a subsequent zero-quality edge makes both qualities zero.
    fn dominates(&self, other: &Self) -> bool {
        self.quality >= other.quality
            && self.steps.len() <= other.steps.len()
            && (!self.optimization_first || self.optimization_cost <= other.optimization_cost)
            && self.formats.iter().all(|format| other.formats.contains(format))
            && (self.steps.len() < other.steps.len()
                || self.optimization_cost < other.optimization_cost
                || (self.optimization_cost == other.optimization_cost
                    && self.identities <= other.identities))
    }

    fn selected(
        self,
        policy: &DerivativePathPolicy,
        explored: usize,
        terminals: usize,
    ) -> SelectedDerivativePath {
        let evidence = DerivativePathEvidence {
            ranking_policy: if self.optimization_first {
                "optimization_then_cumulative_fidelity_then_hops_then_identity_v1"
            } else {
                "cumulative_fidelity_then_hops_then_optimization_then_identity_v1"
            }.to_string(),
            total_quality: self.quality,
            cumulative_loss: 1.0 - self.quality,
            hops: self.steps.len(),
            optimization_cost: self.optimization_cost,
            tie_break_identity: serde_json::to_string(&self.identities)
                .expect("path identities are strings"),
            explored_candidates: explored,
            candidate_count: terminals,
            max_hops: policy.max_hops,
            max_candidates: policy.max_candidates,
            max_loss: policy.max_loss,
            optimization: policy.optimization,
        };
        SelectedDerivativePath {
            path: TransformPath::from_steps(self.steps),
            evidence,
        }
    }
}

impl PartialEq for Candidate {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}
impl Eq for Candidate {}
impl PartialOrd for Candidate {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for Candidate {
    fn cmp(&self, other: &Self) -> Ordering {
        // A search never mixes ranking modes. Comparing the flag first still
        // provides a total order for the private frontier value itself.
        self.optimization_first.cmp(&other.optimization_first).then_with(|| {
            if self.optimization_first {
                other.optimization_cost.total_cmp(&self.optimization_cost)
                    .then_with(|| self.quality.total_cmp(&other.quality))
                    .then_with(|| other.steps.len().cmp(&self.steps.len()))
                    .then_with(|| other.identities.cmp(&self.identities))
            } else {
                self.quality.total_cmp(&other.quality)
                    .then_with(|| other.steps.len().cmp(&self.steps.len()))
                    .then_with(|| other.optimization_cost.total_cmp(&self.optimization_cost))
                    .then_with(|| other.identities.cmp(&self.identities))
            }
        })
    }
}

impl TransformGraph {
    /// Select one simple original-source path without mutating or merging it.
    ///
    /// The frontier is ordered by the same rank as final paths. Every extension
    /// can only reduce quality and increase hop count, so the first terminal
    /// popped is the best eligible route. Parallel edges retain their provider
    /// identity. Prefix dominance requires a subset of visited formats, which
    /// prevents pruning a route that needs a format visited by another prefix.
    ///
    /// An unconstrained source identity has an empty path; callers decide
    /// whether an explicit source-copy target is permitted. A constrained source
    /// identity is unreachable because returning to it repeats a format node.
    pub fn select_derivative_path(
        &self,
        source: Format,
        target: Format,
        constraints: &DerivativePathConstraints,
        policy: &DerivativePathPolicy,
    ) -> Result<Option<SelectedDerivativePath>, DerivativePathError> {
        self.select_bounded_path(source, target, constraints, policy, false)
    }

    /// Preserve explicit targeted conversion's optimization-first policy while
    /// retaining terminal constraints, simple paths, deterministic ties, and
    /// bounded search. The primary weight is the same additive edge weight used
    /// by `find_path_with_mode`; fidelity and hop count only resolve equal costs.
    pub fn select_target_path(
        &self,
        source: Format,
        target: Format,
        constraints: &DerivativePathConstraints,
        policy: &DerivativePathPolicy,
    ) -> Result<Option<SelectedDerivativePath>, DerivativePathError> {
        self.select_bounded_path(source, target, constraints, policy, true)
    }

    fn select_bounded_path(
        &self,
        source: Format,
        target: Format,
        constraints: &DerivativePathConstraints,
        policy: &DerivativePathPolicy,
        optimization_first: bool,
    ) -> Result<Option<SelectedDerivativePath>, DerivativePathError> {
        if policy.max_hops == 0 || policy.max_candidates == 0 {
            return Err(DerivativePathError::InvalidPolicy(
                "hop and candidate limits must be positive".to_string(),
            ));
        }
        if policy.max_loss
            .is_some_and(|loss| !loss.is_finite() || !(0.0..=1.0).contains(&loss))
        {
            return Err(DerivativePathError::InvalidPolicy(
                "maximum loss must be finite and between zero and one".to_string(),
            ));
        }
        if source == target {
            return Ok(constraints.is_empty()
                .then(|| Candidate::source(source, optimization_first).selected(policy, 0, 1)));
        }

        let mut adjacency: HashMap<Format, Vec<(String, &TransformEdge)>> = HashMap::new();
        for edge in self.all_edges() {
            let identity = derivative_edge_identity(edge);
            if !edge.cost.is_finite()
                || edge.cost < 0.0
                || !edge.quality.is_finite()
                || !(0.0..=1.0).contains(&edge.quality)
            {
                return Err(DerivativePathError::InvalidEdge {
                    identity,
                    reason: "cost must be finite/nonnegative and quality must be finite within [0, 1]".to_string(),
                });
            }
            adjacency.entry(edge.from).or_default().push((identity, edge));
        }
        for outgoing in adjacency.values_mut() {
            outgoing.sort_by(|left, right| left.0.cmp(&right.0));
            outgoing.dedup_by(|left, right| left.0 == right.0);
        }

        let initial = Candidate::source(source, optimization_first);
        let mut frontier = BinaryHeap::from([initial.clone()]);
        let mut prefixes = HashMap::from([(source, vec![initial])]);
        let mut explored = 0;
        let mut terminals = 0;
        while let Some(candidate) = frontier.pop() {
            let current = *candidate.formats.last()
                .expect("each candidate contains the source");
            if current == target {
                return Ok(Some(candidate.selected(policy, explored, terminals)));
            }
            if candidate.steps.len() == policy.max_hops {
                continue;
            }
            let Some(outgoing) = adjacency.get(&current) else {
                continue;
            };
            for (identity, edge) in outgoing {
                if explored == policy.max_candidates {
                    return Err(DerivativePathError::CandidateLimitExceeded {
                        limit: policy.max_candidates,
                    });
                }
                explored += 1;
                if candidate.formats.contains(&edge.to)
                    || (edge.to == target && !constraints.matches(edge))
                {
                    continue;
                }
                let quality = candidate.quality * f64::from(edge.quality);
                if policy.max_loss.is_some_and(|loss| 1.0 - quality > loss) {
                    continue;
                }
                let mut next = candidate.clone();
                next.steps.push((**edge).clone());
                next.formats.push(edge.to);
                next.quality = quality;
                next.optimization_cost += f64::from(
                    policy.optimization.edge_weight(edge.cost, edge.quality),
                );
                next.identities.push(identity.clone());
                let retained = prefixes.entry(edge.to).or_default();
                if retained.iter().any(|prefix| prefix.dominates(&next)) {
                    continue;
                }
                retained.retain(|prefix| !next.dominates(prefix));
                retained.push(next.clone());
                if edge.to == target {
                    terminals += 1;
                }
                frontier.push(next);
            }
        }
        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn select(graph: &TransformGraph, target: Format) -> SelectedDerivativePath {
        graph.select_derivative_path(Format::Markdown, target, &DerivativePathConstraints::default(), &DerivativePathPolicy::default()).unwrap().unwrap()
    }

    #[test]
    fn reversible_edges_never_repeat_source_or_an_intermediate() {
        let mut graph = TransformGraph::new();
        graph.add_transform(TransformEdge::new(Format::Markdown, Format::Html, 1.0, 1.0));
        graph.add_transform(TransformEdge::new(Format::Html, Format::Markdown, 0.0, 1.0));
        graph.add_transform(TransformEdge::new(Format::Html, Format::Pdf, 1.0, 0.9));
        let selected = select(&graph, Format::Pdf);
        assert_eq!(selected.path.steps.len(), 2);
        assert!(selected.path.steps.iter().all(|edge| edge.to != Format::Markdown));
    }

    #[test]
    fn fidelity_precedes_directness_and_cost() {
        let mut graph = TransformGraph::new();
        graph.add_transform(TransformEdge::new(Format::Markdown, Format::Pdf, 0.1, 0.7));
        graph.add_transform(TransformEdge::new(Format::Markdown, Format::Html, 4.0, 0.99));
        graph.add_transform(TransformEdge::new(Format::Html, Format::Pdf, 4.0, 0.99));
        let selected = select(&graph, Format::Pdf);
        assert_eq!(selected.path.steps.len(), 2);
        assert!(selected.evidence.total_quality > 0.98);
    }

    #[test]
    fn equal_fidelity_prefers_original_source_even_if_more_expensive() {
        let mut graph = TransformGraph::new();
        graph.add_transform(TransformEdge::new(Format::Markdown, Format::Pdf, 100.0, 1.0));
        graph.add_transform(TransformEdge::new(Format::Markdown, Format::Html, 0.1, 1.0));
        graph.add_transform(TransformEdge::new(Format::Html, Format::Pdf, 0.1, 1.0));
        assert_eq!(select(&graph, Format::Pdf).path.steps.len(), 1);
    }

    #[test]
    fn targeted_speed_preserves_the_cheaper_lossy_route() {
        let mut graph = TransformGraph::new();
        graph.add_transform(TransformEdge::new(Format::Markdown, Format::Pdf, 0.1, 0.7));
        graph.add_transform(TransformEdge::new(Format::Markdown, Format::Html, 1.0, 1.0));
        graph.add_transform(TransformEdge::new(Format::Html, Format::Pdf, 1.0, 1.0));
        let policy = DerivativePathPolicy {
            optimization: OptimizationMode::Speed,
            ..DerivativePathPolicy::default()
        };
        let selected = graph.select_target_path(
            Format::Markdown,
            Format::Pdf,
            &DerivativePathConstraints::default(),
            &policy,
        ).unwrap().unwrap();
        assert_eq!(selected.path.steps.len(), 1);
        assert_eq!(selected.path.total_cost, 0.1);
        assert_eq!(selected.evidence.ranking_policy,
            "optimization_then_cumulative_fidelity_then_hops_then_identity_v1");
    }

    #[test]
    fn exhaustive_forest_keeps_fidelity_ahead_of_speed() {
        let mut graph = TransformGraph::new();
        graph.add_transform(TransformEdge::new(Format::Markdown, Format::Pdf, 0.1, 0.7));
        graph.add_transform(TransformEdge::new(Format::Markdown, Format::Html, 1.0, 1.0));
        graph.add_transform(TransformEdge::new(Format::Html, Format::Pdf, 1.0, 1.0));
        let policy = DerivativePathPolicy {
            optimization: OptimizationMode::Speed,
            ..DerivativePathPolicy::default()
        };
        let selected = graph.select_derivative_path(
            Format::Markdown,
            Format::Pdf,
            &DerivativePathConstraints::default(),
            &policy,
        ).unwrap().unwrap();
        assert_eq!(selected.path.steps.len(), 2);
        assert_eq!(selected.evidence.total_quality, 1.0);
        assert_eq!(selected.evidence.ranking_policy,
            "cumulative_fidelity_then_hops_then_optimization_then_identity_v1");
    }

    #[test]
    fn terminal_selectors_preserve_provider_variant_and_transform() {
        let mut graph = TransformGraph::new();
        graph.add_transform(TransformEdge::new(Format::Markdown, Format::Pdf, 0.1, 1.0).with_provider("fast", "pdf.fast"));
        graph.add_transform(TransformEdge::new(Format::Markdown, Format::Pdf, 2.0, 0.9).with_provider("print", "pdf.print").with_variant("a4").with_evidence("transform_id", "print-a4"));
        let constraints = DerivativePathConstraints { provider_id: Some("print".to_string()), capability_id: Some("pdf.print".to_string()), transform_id: Some("print-a4".to_string()), variant_id: Some("a4".to_string()) };
        let selected = graph.select_derivative_path(Format::Markdown, Format::Pdf, &constraints, &DerivativePathPolicy::default()).unwrap().unwrap();
        assert_eq!(selected.path.steps[0].provider_id.as_deref(), Some("print"));
    }

    #[test]
    fn loss_budget_is_cumulative_and_not_per_edge() {
        let mut graph = TransformGraph::new();
        graph.add_transform(TransformEdge::new(Format::Markdown, Format::Html, 1.0, 0.9));
        graph.add_transform(TransformEdge::new(Format::Html, Format::Pdf, 1.0, 0.9));
        let policy = DerivativePathPolicy { max_loss: Some(0.15), ..DerivativePathPolicy::default() };
        assert!(graph.select_derivative_path(Format::Markdown, Format::Pdf, &DerivativePathConstraints::default(), &policy).unwrap().is_none());
    }

    #[test]
    fn candidate_exhaustion_never_returns_a_provisional_route() {
        let mut graph = TransformGraph::new();
        graph.add_transform(TransformEdge::new(Format::Markdown, Format::Html, 1.0, 1.0));
        graph.add_transform(TransformEdge::new(Format::Markdown, Format::Pdf, 1.0, 1.0));
        let policy = DerivativePathPolicy { max_candidates: 1, ..DerivativePathPolicy::default() };
        assert!(matches!(graph.select_derivative_path(Format::Markdown, Format::Pdf, &DerivativePathConstraints::default(), &policy), Err(DerivativePathError::CandidateLimitExceeded { limit: 1 })));
    }

    #[test]
    fn ties_ignore_registration_order_and_keep_parallel_edge_identity() {
        let a = TransformEdge::new(Format::Markdown, Format::Pdf, 1.0, 1.0).with_provider("a", "pdf");
        let b = TransformEdge::new(Format::Markdown, Format::Pdf, 1.0, 1.0).with_provider("b", "pdf");
        let mut graph = TransformGraph::new();
        graph.add_transform(b.clone()); graph.add_transform(a.clone());
        let first = select(&graph, Format::Pdf);
        let mut reversed = TransformGraph::new();
        reversed.add_transform(a); reversed.add_transform(b);
        assert_eq!(first, select(&reversed, Format::Pdf));
    }

    #[test]
    fn explicit_source_identity_is_zero_hop_and_never_a_roundtrip() {
        let mut graph = TransformGraph::new();
        graph.add_transform(TransformEdge::new(Format::Markdown, Format::Html, 1.0, 1.0));
        graph.add_transform(TransformEdge::new(Format::Html, Format::Markdown, 1.0, 1.0));
        assert!(select(&graph, Format::Markdown).path.steps.is_empty());
        let constraints = DerivativePathConstraints { transform_id: Some("roundtrip".to_string()), ..DerivativePathConstraints::default() };
        assert!(graph.select_derivative_path(Format::Markdown, Format::Markdown, &constraints, &DerivativePathPolicy::default()).unwrap().is_none());
    }

    #[test]
    fn hop_limit_defines_eligible_paths() {
        let mut graph = TransformGraph::new();
        graph.add_transform(TransformEdge::new(Format::Markdown, Format::Html, 1.0, 1.0));
        graph.add_transform(TransformEdge::new(Format::Html, Format::Pdf, 1.0, 1.0));
        let policy = DerivativePathPolicy { max_hops: 1, ..DerivativePathPolicy::default() };
        assert!(graph.select_derivative_path(Format::Markdown, Format::Pdf, &DerivativePathConstraints::default(), &policy).unwrap().is_none());
    }

    #[test]
    fn invalid_numbers_are_typed_errors() {
        let mut graph = TransformGraph::new();
        graph.add_transform(TransformEdge::new(Format::Markdown, Format::Pdf, f32::NAN, 1.0));
        assert!(matches!(graph.select_derivative_path(Format::Markdown, Format::Pdf, &DerivativePathConstraints::default(), &DerivativePathPolicy::default()), Err(DerivativePathError::InvalidEdge { .. })));
    }

    #[test]
    fn unavailable_provider_cannot_win_fidelity_ranking() {
        let mut graph = TransformGraph::new();
        graph.add_transform(
            TransformEdge::new(Format::Markdown, Format::Pdf, 0.1, 1.0)
                .with_provider("missing", "pdf"),
        );
        graph.add_transform(
            TransformEdge::new(Format::Markdown, Format::Pdf, 1.0, 0.9)
                .with_provider("installed", "pdf"),
        );
        let available = std::collections::HashSet::from(["installed".to_string()]);
        let selected = select(&graph.filtered_by_available_providers(&available), Format::Pdf);
        assert_eq!(selected.path.steps[0].provider_id.as_deref(), Some("installed"));
    }

    #[test]
    fn dense_reversible_graph_prunes_redundant_lossless_prefixes() {
        let formats = [
            Format::Markdown, Format::Html, Format::Pdf, Format::Wav,
            Format::Aiff, Format::Bwf, Format::Pcm, Format::Flac,
            Format::M4aAlac, Format::Wv, Format::Ape, Format::Tta,
            Format::Dsf, Format::Dff, Format::Shn, Format::Mp3,
            Format::M4aAac, Format::Aac, Format::Ogg, Format::Opus,
            Format::Wma, Format::Amr, Format::Mp2, Format::Ra,
            Format::Oma, Format::Ac3, Format::Ec3, Format::Thd,
            Format::Dts, Format::DtsHd,
        ];
        let mut graph = TransformGraph::new();
        for from in formats {
            for to in formats {
                if from != to {
                    let quality = if to == Format::Pdf { 0.9 } else { 1.0 };
                    graph.add_transform(TransformEdge::new(from, to, 1.0, quality));
                }
            }
        }
        let selected = select(&graph, Format::Pdf);
        assert_eq!(selected.path.steps.len(), 1);
        assert!(selected.evidence.explored_candidates <= formats.len() * formats.len());
    }

    #[test]
    fn terminal_constraint_does_not_allow_revisiting_target() {
        let mut graph = TransformGraph::new();
        graph.add_transform(TransformEdge::new(Format::Markdown, Format::Html, 1.0, 1.0));
        graph.add_transform(TransformEdge::new(Format::Html, Format::Pdf, 1.0, 1.0));
        graph.add_transform(
            TransformEdge::new(Format::Pdf, Format::Html, 1.0, 1.0)
                .with_evidence("transform_id", "required"),
        );
        let constraints = DerivativePathConstraints {
            transform_id: Some("required".to_string()),
            ..DerivativePathConstraints::default()
        };
        assert!(graph.select_derivative_path(
            Format::Markdown,
            Format::Html,
            &constraints,
            &DerivativePathPolicy::default(),
        ).unwrap().is_none());
    }
}
