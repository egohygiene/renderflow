use clap::ValueEnum;
use serde::{Deserialize, Serialize};

use crate::graph::TransformPath;

/// Controls how the pathfinding engine scores and selects transformation paths.
///
/// * `Speed`    – minimizes total cost; fastest but potentially lower quality.
/// * `Quality`  – maximizes total quality; best output but potentially slower.
/// * `Balanced` – weighted combination of cost and quality (default).
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
}

impl OptimizationMode {
    /// Compute a score for `path` under this optimization mode.
    ///
    /// Higher values indicate a more desirable path.
    ///
    /// * `Speed`    – score = −total_cost  (lower cost ⇒ higher score)
    /// * `Quality`  – score = total_quality (higher quality ⇒ higher score)
    /// * `Balanced` – score = −0.5 × total_cost + 0.5 × total_quality
    pub fn score(&self, path: &TransformPath) -> f32 {
        match self {
            Self::Speed => -path.total_cost,
            Self::Quality => path.total_quality,
            Self::Balanced => -0.5 * path.total_cost + 0.5 * path.total_quality,
        }
    }

    /// Compute an additive edge weight suitable for use with Dijkstra / A\*.
    ///
    /// Lower weight means the edge is more desirable for this mode:
    ///
    /// * `Speed`    – weight = cost
    /// * `Quality`  – weight = 1 − quality  (low quality ⇒ high weight)
    /// * `Balanced` – weight = 0.5 × cost + 0.5 × (1 − quality)
    pub fn edge_weight(&self, cost: f32, quality: f32) -> f32 {
        match self {
            Self::Speed => cost,
            Self::Quality => 1.0 - quality,
            Self::Balanced => 0.5 * cost + 0.5 * (1.0 - quality),
        }
    }
}

impl std::fmt::Display for OptimizationMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Speed => write!(f, "speed"),
            Self::Quality => write!(f, "quality"),
            Self::Balanced => write!(f, "balanced"),
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
}
