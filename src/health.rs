use crate::metrics::{MetricRegistry, MetricStatus};
use std::collections::BTreeSet;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct HealthState {
    pub stale_metrics: Vec<String>,
    pub error_metrics: Vec<String>,
}

impl HealthState {
    pub fn from_mapped_metrics(
        metrics: &MetricRegistry,
        mapped_metrics: &BTreeSet<String>,
    ) -> Self {
        let mut stale_metrics = Vec::new();
        let mut error_metrics = Vec::new();

        for name in mapped_metrics {
            let Some(value) = metrics.get(name) else {
                continue;
            };
            match value.status {
                MetricStatus::Normal => {}
                MetricStatus::Stale { .. } => stale_metrics.push(name.clone()),
                MetricStatus::Error { .. } => error_metrics.push(name.clone()),
            }
        }

        Self {
            stale_metrics,
            error_metrics,
        }
    }

    pub fn is_degraded(&self) -> bool {
        !self.stale_metrics.is_empty() || !self.error_metrics.is_empty()
    }

    pub fn has_error(&self) -> bool {
        !self.error_metrics.is_empty()
    }

    pub fn message_text(&self) -> Option<String> {
        if !self.is_degraded() {
            return None;
        }

        let mut parts = Vec::new();
        if !self.error_metrics.is_empty() {
            parts.push(format!("ERROR: {}", self.error_metrics.join(", ")));
        }
        if !self.stale_metrics.is_empty() {
            parts.push(format!("STALE: {}", self.stale_metrics.join(", ")));
        }

        Some(parts.join("  "))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::metrics::MetricValue;

    #[test]
    fn ignores_unmapped_stale_metrics() {
        let mut metrics = MetricRegistry::default();
        metrics.mark_stale("thermal_zone", Some("timeout".to_string()));
        let mapped_metrics = BTreeSet::from(["cpu".to_string()]);

        let health = HealthState::from_mapped_metrics(&metrics, &mapped_metrics);

        assert!(!health.is_degraded());
    }

    #[test]
    fn includes_only_mapped_stale_and_error_metrics() {
        let mut metrics = MetricRegistry::default();
        metrics.mark_stale("cpu", None);
        metrics.mark_error("thermal_zone", Some("read failed".to_string()));
        metrics.mark_error("disk", None);
        let mapped_metrics = BTreeSet::from(["cpu".to_string(), "thermal_zone".to_string()]);

        let health = HealthState::from_mapped_metrics(&metrics, &mapped_metrics);

        assert_eq!(health.stale_metrics, vec!["cpu"]);
        assert_eq!(health.error_metrics, vec!["thermal_zone"]);
        assert!(health.has_error());
    }

    #[test]
    fn formats_error_before_stale_metrics() {
        let health = HealthState {
            stale_metrics: vec!["cpu".to_string(), "memory".to_string()],
            error_metrics: vec!["thermal_zone".to_string()],
        };

        assert_eq!(
            health.message_text().as_deref(),
            Some("ERROR: thermal_zone  STALE: cpu, memory")
        );
    }

    #[test]
    fn normal_update_clears_metric_status_before_health_build() {
        let mut metrics = MetricRegistry::default();
        metrics.mark_error("cpu", None);
        metrics.set("cpu", MetricValue::new(None, Some(0.5)));
        let mapped_metrics = BTreeSet::from(["cpu".to_string()]);

        let health = HealthState::from_mapped_metrics(&metrics, &mapped_metrics);

        assert!(!health.is_degraded());
    }
}
