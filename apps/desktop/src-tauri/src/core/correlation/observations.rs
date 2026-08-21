//! Fix-feedback observations and dynamic confidence calibration.
//! With at least five observations, strong co-resolution raises confidence and weak
//! co-resolution lowers it.

use std::collections::HashMap;

use crate::core::correlation::causal_graph::Confidence;
use crate::db::Database;

#[derive(Debug, Default)]
pub struct ObservationIndex {
    counts: HashMap<(String, String), (u32, u32)>,
}

impl ObservationIndex {
    pub fn load(db: &Database, project_id: i64) -> Result<Self, String> {
        let counts = db
            .get_causal_link_observation_counts(project_id)
            .map_err(|e| e.to_string())?;
        Ok(Self { counts })
    }

    pub fn for_link(&self, cause: &str, effect: &str) -> (u32, u32) {
        self.counts
            .get(&(cause.to_string(), effect.to_string()))
            .copied()
            .unwrap_or((0, 0))
    }
}

pub fn dynamic_confidence(base: Confidence, resolved: u32, active: u32) -> Confidence {
    if active < 5 {
        return base;
    }
    let ratio = resolved as f32 / active as f32;
    let base_val = base.as_f32();
    let adjusted = if ratio < 0.2 {
        base_val - 0.4
    } else if ratio > 0.7 {
        base_val + 0.2
    } else {
        base_val
    };
    Confidence::from_f32(adjusted.clamp(0.2, 1.0))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn low_sample_size_returns_base_unchanged() {
        assert_eq!(dynamic_confidence(Confidence::High, 1, 1), Confidence::High);
        assert_eq!(
            dynamic_confidence(Confidence::Medium, 4, 4),
            Confidence::Medium
        );
        assert_eq!(dynamic_confidence(Confidence::Low, 0, 4), Confidence::Low);
    }

    #[test]
    fn high_resolution_ratio_raises_tier() {
        // Medium (0.7) + 0.2 = 0.9 = High
        assert_eq!(
            dynamic_confidence(Confidence::Medium, 9, 10),
            Confidence::High
        );
        // Low (0.3) + 0.2 = 0.5 = Medium
        assert_eq!(
            dynamic_confidence(Confidence::Low, 9, 10),
            Confidence::Medium
        );
        // High (1.0) + 0.2 = clamped at 1.0 = stays High
        assert_eq!(
            dynamic_confidence(Confidence::High, 9, 10),
            Confidence::High
        );
    }

    #[test]
    fn low_resolution_ratio_drops_tier() {
        // High (1.0) - 0.4 = 0.6 = Medium
        assert_eq!(
            dynamic_confidence(Confidence::High, 1, 10),
            Confidence::Medium
        );
        // Medium (0.7) - 0.4 = 0.3 = Low
        assert_eq!(
            dynamic_confidence(Confidence::Medium, 1, 10),
            Confidence::Low
        );
        // Low (0.3) - 0.4 = clamped at 0.2 = stays Low
        assert_eq!(dynamic_confidence(Confidence::Low, 1, 10), Confidence::Low);
    }

    #[test]
    fn mid_ratio_leaves_base_unchanged() {
        // ratio = 0.5 is between 0.2 and 0.7 thresholds, so base stays
        assert_eq!(
            dynamic_confidence(Confidence::Medium, 5, 10),
            Confidence::Medium
        );
        assert_eq!(
            dynamic_confidence(Confidence::High, 5, 10),
            Confidence::High
        );
    }
}
