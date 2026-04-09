use clap::ValueEnum;
use serde::{Deserialize, Serialize};

use crate::graph::TransformPath;

/// Controls how the pathfinding engine scores and selects transformation paths.
///
/// * `Speed`    – minimizes total cost; fastest but potentially lower quality.
/// * `Quality`  – maximizes total quality; best output but potentially slower.
/// * `Balanced` – weighted combination of cost and quality (default).
/// * `Pareto`   – returns the full Pareto-optimal frontier of non-dominated paths.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize, Serialize, ValueEnum)]
#[serde(rename_all = "lowercase")]
pub enum OptimizationMode {
    /// Minimise total transformation cost.
    Speed,
    /// Maximise total output quality.
    Quality,
    /// Weighted combination of cost and quality (default).
    #[default]
    Balanced,
    /// Return the Pareto-optimal frontier of non-dominated paths (multi-objective).
    Pareto,
}

impl OptimizationMode {
    /// Compute a score for `path` under this optimization mode.
    ///
    /// Higher values indicate a more desirable path.
    ///
    /// * `Speed`    – score = −total_cost  (lower cost ⇒ higher score)
    /// * `Quality`  – score = total_quality (higher quality ⇒ higher score)
    /// * `Balanced` – score = −0.5 × total_cost + 0.5 × total_quality
    /// * `Pareto`   – delegates to `Balanced` (single-path scoring not applicable)
    pub fn score(&self, path: &TransformPath) -> f32 {
        match self {
            Self::Speed => -path.total_cost,
            Self::Quality => path.total_quality,
            Self::Balanced | Self::Pareto => -0.5 * path.total_cost + 0.5 * path.total_quality,
        }
    }

    /// Compute an additive edge weight suitable for use with Dijkstra / A\*.
    ///
    /// Lower weight means the edge is more desirable for this mode:
    ///
    /// * `Speed`    – weight = cost
    /// * `Quality`  – weight = 1 − quality  (low quality ⇒ high weight)
    /// * `Balanced` – weight = 0.5 × cost + 0.5 × (1 − quality)
    /// * `Pareto`   – delegates to `Balanced` (single-path weighting not applicable)
    pub fn edge_weight(&self, cost: f32, quality: f32) -> f32 {
        match self {
            Self::Speed => cost,
            Self::Quality => 1.0 - quality,
            Self::Balanced | Self::Pareto => 0.5 * cost + 0.5 * (1.0 - quality),
        }
    }
}

/// Filter a slice of paths down to the Pareto-optimal (non-dominated) subset.
///
/// A path A **dominates** path B when A has a lower-or-equal cost *and* a
/// higher-or-equal quality than B, with at least one of those comparisons being
/// strict.  Any dominated path is excluded from the returned set.
///
/// The returned paths are sorted by `total_cost` ascending so the cheapest
/// option appears first.
///
/// An optional `cap` limits the number of returned paths; when `None` all
/// non-dominated paths are returned.
///
/// # Example
///
/// ```rust
/// use renderflow::graph::{Format, TransformEdge, TransformPath};
/// use renderflow::optimization::pareto_frontier;
///
/// fn make_path(cost: f32, quality: f32) -> TransformPath {
///     TransformPath {
///         steps: vec![],
///         total_cost: cost,
///         total_quality: quality,
///     }
/// }
///
/// // Path A (cost=1, quality=0.9) dominates path B (cost=2, quality=0.8).
/// let paths = vec![make_path(1.0, 0.9), make_path(2.0, 0.8)];
/// let frontier = pareto_frontier(&paths, None);
/// assert_eq!(frontier.len(), 1);
/// assert!((frontier[0].total_cost - 1.0).abs() < 1e-5);
/// ```
pub fn pareto_frontier(paths: &[TransformPath], cap: Option<usize>) -> Vec<TransformPath> {
    let mut frontier: Vec<TransformPath> = Vec::new();

    'outer: for candidate in paths {
        // Drop candidate if it is dominated by any path already in the frontier.
        for existing in &frontier {
            if dominates(existing, candidate) {
                continue 'outer;
            }
        }
        // Remove any existing paths that are dominated by the new candidate.
        frontier.retain(|existing| !dominates(candidate, existing));
        frontier.push(candidate.clone());
    }

    // Sort by total_cost ascending (cheapest first).
    // Paths with NaN metrics sort as equal to each other due to partial_cmp fallback.
    frontier.sort_by(|a, b| {
        a.total_cost
            .partial_cmp(&b.total_cost)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    // Deduplicate paths with identical objectives: two paths that share both
    // total_cost and total_quality represent the same trade-off point and only
    // one needs to be kept in the frontier.
    let mut seen = std::collections::HashSet::new();
    frontier.retain(|p| seen.insert((p.total_cost.to_bits(), p.total_quality.to_bits())));

    if let Some(limit) = cap {
        frontier.truncate(limit);
    }

    frontier
}

/// Return `true` when path `a` dominates path `b`.
///
/// `a` dominates `b` when it is at least as good in every objective and
/// strictly better in at least one:
/// - `a.total_cost <= b.total_cost`
/// - `a.total_quality >= b.total_quality`
/// - at least one of those comparisons is strict.
fn dominates(a: &TransformPath, b: &TransformPath) -> bool {
    a.total_cost <= b.total_cost
        && a.total_quality >= b.total_quality
        && (a.total_cost < b.total_cost || a.total_quality > b.total_quality)
}

impl std::fmt::Display for OptimizationMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Speed => write!(f, "speed"),
            Self::Quality => write!(f, "quality"),
            Self::Balanced => write!(f, "balanced"),
            Self::Pareto => write!(f, "pareto"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::TransformPath;

    fn make_path(cost: f32, quality: f32) -> TransformPath {
        TransformPath {
            steps: vec![],
            total_cost: cost,
            total_quality: quality,
        }
    }

    // ── score ─────────────────────────────────────────────────────────────────

    #[test]
    fn test_speed_score_is_negative_cost() {
        let path = make_path(2.0, 0.9);
        let score = OptimizationMode::Speed.score(&path);
        assert!((score - (-2.0_f32)).abs() < 1e-5, "expected -2.0, got {score}");
    }

    #[test]
    fn test_quality_score_equals_total_quality() {
        let path = make_path(1.0, 0.75);
        let score = OptimizationMode::Quality.score(&path);
        assert!((score - 0.75_f32).abs() < 1e-5, "expected 0.75, got {score}");
    }

    #[test]
    fn test_balanced_score_is_weighted_combination() {
        let path = make_path(2.0, 0.8);
        // -0.5 * 2.0 + 0.5 * 0.8 = -1.0 + 0.4 = -0.6
        let score = OptimizationMode::Balanced.score(&path);
        assert!((score - (-0.6_f32)).abs() < 1e-5, "expected -0.6, got {score}");
    }

    #[test]
    fn test_speed_prefers_lower_cost() {
        let cheap = make_path(0.5, 0.5);
        let expensive = make_path(2.0, 0.9);
        assert!(
            OptimizationMode::Speed.score(&cheap) > OptimizationMode::Speed.score(&expensive),
            "speed mode should prefer cheaper paths"
        );
    }

    #[test]
    fn test_quality_prefers_higher_quality() {
        let high_q = make_path(2.0, 0.95);
        let low_q = make_path(0.5, 0.50);
        assert!(
            OptimizationMode::Quality.score(&high_q) > OptimizationMode::Quality.score(&low_q),
            "quality mode should prefer higher-quality paths"
        );
    }

    // ── edge_weight ───────────────────────────────────────────────────────────

    #[test]
    fn test_speed_edge_weight_equals_cost() {
        assert!((OptimizationMode::Speed.edge_weight(1.5, 0.9) - 1.5).abs() < 1e-5);
    }

    #[test]
    fn test_quality_edge_weight_is_one_minus_quality() {
        assert!((OptimizationMode::Quality.edge_weight(1.0, 0.9) - 0.1).abs() < 1e-5);
    }

    #[test]
    fn test_balanced_edge_weight_is_half_cost_plus_half_inverse_quality() {
        // 0.5 * 1.0 + 0.5 * (1 - 0.8) = 0.5 + 0.1 = 0.6
        assert!((OptimizationMode::Balanced.edge_weight(1.0, 0.8) - 0.6).abs() < 1e-5);
    }

    // ── default ───────────────────────────────────────────────────────────────

    #[test]
    fn test_default_is_balanced() {
        assert_eq!(OptimizationMode::default(), OptimizationMode::Balanced);
    }

    // ── display ───────────────────────────────────────────────────────────────

    #[test]
    fn test_display_speed() {
        assert_eq!(OptimizationMode::Speed.to_string(), "speed");
    }

    #[test]
    fn test_display_quality() {
        assert_eq!(OptimizationMode::Quality.to_string(), "quality");
    }

    #[test]
    fn test_display_balanced() {
        assert_eq!(OptimizationMode::Balanced.to_string(), "balanced");
    }

    #[test]
    fn test_display_pareto() {
        assert_eq!(OptimizationMode::Pareto.to_string(), "pareto");
    }

    // ── Pareto score / edge_weight delegate to Balanced ───────────────────────

    #[test]
    fn test_pareto_score_same_as_balanced() {
        let path = make_path(2.0, 0.8);
        assert!(
            (OptimizationMode::Pareto.score(&path)
                - OptimizationMode::Balanced.score(&path))
            .abs()
                < 1e-5
        );
    }

    #[test]
    fn test_pareto_edge_weight_same_as_balanced() {
        assert!(
            (OptimizationMode::Pareto.edge_weight(1.0, 0.8)
                - OptimizationMode::Balanced.edge_weight(1.0, 0.8))
            .abs()
                < 1e-5
        );
    }

    // ── pareto_frontier ───────────────────────────────────────────────────────

    #[test]
    fn test_pareto_frontier_single_path_is_optimal() {
        let paths = vec![make_path(1.0, 0.9)];
        let frontier = pareto_frontier(&paths, None);
        assert_eq!(frontier.len(), 1);
    }

    #[test]
    fn test_pareto_frontier_dominated_path_removed() {
        // A (cost=1, q=0.9) dominates B (cost=2, q=0.8).
        let paths = vec![make_path(1.0, 0.9), make_path(2.0, 0.8)];
        let frontier = pareto_frontier(&paths, None);
        assert_eq!(frontier.len(), 1);
        assert!((frontier[0].total_cost - 1.0).abs() < 1e-5);
    }

    #[test]
    fn test_pareto_frontier_two_non_dominated_paths_kept() {
        // A (cost=1, q=0.5) and B (cost=3, q=0.9) — neither dominates the other.
        let paths = vec![make_path(1.0, 0.5), make_path(3.0, 0.9)];
        let frontier = pareto_frontier(&paths, None);
        assert_eq!(frontier.len(), 2);
    }

    #[test]
    fn test_pareto_frontier_sorted_by_cost_ascending() {
        let paths = vec![make_path(3.0, 0.9), make_path(1.0, 0.5)];
        let frontier = pareto_frontier(&paths, None);
        assert_eq!(frontier.len(), 2);
        assert!(frontier[0].total_cost <= frontier[1].total_cost);
    }

    #[test]
    fn test_pareto_frontier_cap_limits_results() {
        // Three non-dominated paths; cap at 2.
        let paths = vec![
            make_path(1.0, 0.3),
            make_path(2.0, 0.6),
            make_path(3.0, 0.9),
        ];
        let frontier = pareto_frontier(&paths, Some(2));
        assert_eq!(frontier.len(), 2);
        // Cheapest two are returned (sorted by cost).
        assert!((frontier[0].total_cost - 1.0).abs() < 1e-5);
        assert!((frontier[1].total_cost - 2.0).abs() < 1e-5);
    }

    #[test]
    fn test_pareto_frontier_empty_input_returns_empty() {
        let frontier = pareto_frontier(&[], None);
        assert!(frontier.is_empty());
    }

    #[test]
    fn test_pareto_frontier_all_dominated_except_one() {
        // Each successive path is strictly better in both dimensions.
        let paths = vec![
            make_path(3.0, 0.5),
            make_path(2.0, 0.7),
            make_path(1.0, 0.9),
        ];
        let frontier = pareto_frontier(&paths, None);
        assert_eq!(frontier.len(), 1);
        assert!((frontier[0].total_cost - 1.0).abs() < 1e-5);
        assert!((frontier[0].total_quality - 0.9).abs() < 1e-5);
    }

    #[test]
    fn test_pareto_frontier_equal_paths_deduplicated() {
        // Two paths with identical objectives: only one should be kept.
        let paths = vec![make_path(1.0, 0.9), make_path(1.0, 0.9)];
        let frontier = pareto_frontier(&paths, None);
        assert_eq!(frontier.len(), 1);
    }

    #[test]
    fn test_pareto_frontier_cap_zero_returns_empty() {
        let paths = vec![make_path(1.0, 0.9), make_path(2.0, 0.5)];
        let frontier = pareto_frontier(&paths, Some(0));
        assert!(frontier.is_empty());
    }
}
