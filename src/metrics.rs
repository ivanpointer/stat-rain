use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq)]
pub struct MetricValue {
    pub raw: Option<f64>,
    pub normalized: Option<f64>,
    pub stale: bool,
    pub status: MetricStatus,
}

impl MetricValue {
    pub fn new(raw: Option<f64>, normalized: Option<f64>) -> Self {
        Self {
            raw,
            normalized,
            stale: false,
            status: MetricStatus::Normal,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MetricStatus {
    Normal,
    Stale { reason: Option<String> },
    Error { reason: Option<String> },
}

#[derive(Debug, Default, Clone)]
pub struct MetricRegistry {
    values: BTreeMap<String, MetricValue>,
}

impl MetricRegistry {
    pub fn set(&mut self, name: impl Into<String>, value: MetricValue) {
        self.values.insert(name.into(), value);
    }

    pub fn mark_stale(&mut self, name: impl Into<String>, reason: Option<String>) {
        self.values.insert(
            name.into(),
            MetricValue {
                raw: None,
                normalized: Some(1.0),
                stale: true,
                status: MetricStatus::Stale { reason },
            },
        );
    }

    pub fn mark_error(&mut self, name: impl Into<String>, reason: Option<String>) {
        self.values.insert(
            name.into(),
            MetricValue {
                raw: None,
                normalized: Some(1.0),
                stale: true,
                status: MetricStatus::Error { reason },
            },
        );
    }

    pub fn clear_status(&mut self, name: impl Into<String>) {
        let name = name.into();
        if let Some(value) = self.values.get_mut(&name) {
            value.stale = false;
            value.status = MetricStatus::Normal;
        }
    }

    pub fn apply_sample(&mut self, sample: MetricSample) {
        for (name, value) in sample.values {
            self.set(name, value);
        }
    }

    pub fn get(&self, name: &str) -> Option<MetricValue> {
        self.values.get(name).cloned()
    }

    pub fn len(&self) -> usize {
        self.values.len()
    }

    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }
}

#[derive(Debug, Default, Clone)]
pub struct MetricSample {
    values: BTreeMap<String, MetricValue>,
}

impl MetricSample {
    pub fn set(&mut self, name: impl Into<String>, value: MetricValue) {
        self.values.insert(name.into(), value);
    }

    pub fn get(&self, name: &str) -> Option<MetricValue> {
        self.values.get(name).cloned()
    }
}

pub trait MetricProvider {
    fn sample(&mut self) -> anyhow::Result<MetricSample>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CpuTicks {
    pub user: u64,
    pub nice: u64,
    pub system: u64,
    pub idle: u64,
}

impl CpuTicks {
    pub fn total(self) -> u64 {
        self.user + self.nice + self.system + self.idle
    }
}

pub fn normalized_cpu_usage(previous: CpuTicks, current: CpuTicks) -> Option<f64> {
    let total_delta = current.total().checked_sub(previous.total())?;
    if total_delta == 0 {
        return None;
    }
    let idle_delta = current.idle.checked_sub(previous.idle)?;
    Some((1.0 - idle_delta as f64 / total_delta as f64).clamp(0.0, 1.0))
}

pub fn normalized_memory_usage(total: u64, available: u64) -> Option<f64> {
    if total == 0 || available > total {
        return None;
    }
    Some((1.0 - available as f64 / total as f64).clamp(0.0, 1.0))
}

pub fn normalized_io_rate(
    previous: u64,
    current: u64,
    elapsed_secs: f64,
    max_rate: f64,
) -> Option<(f64, f64)> {
    if elapsed_secs <= 0.0 || max_rate <= 0.0 || !elapsed_secs.is_finite() || !max_rate.is_finite()
    {
        return None;
    }
    let delta = current.checked_sub(previous)? as f64;
    let rate = delta / elapsed_secs;
    Some((rate, (rate / max_rate).clamp(0.0, 1.0)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn calculates_cpu_usage_from_tick_delta() {
        let previous = CpuTicks {
            user: 100,
            nice: 0,
            system: 100,
            idle: 800,
        };
        let current = CpuTicks {
            user: 150,
            nice: 0,
            system: 150,
            idle: 900,
        };

        let usage = normalized_cpu_usage(previous, current).unwrap();

        assert_eq!(usage, 0.5);
    }

    #[test]
    fn calculates_memory_usage_from_available_memory() {
        let usage = normalized_memory_usage(1_000, 250).unwrap();

        assert_eq!(usage, 0.75);
    }

    #[test]
    fn normalizes_io_rate_from_counter_delta() {
        let (raw, normalized) = normalized_io_rate(1_000, 3_000, 2.0, 4_000.0).unwrap();

        assert_eq!(raw, 1_000.0);
        assert_eq!(normalized, 0.25);
    }

    #[test]
    fn applies_metric_sample_to_registry() {
        let mut sample = MetricSample::default();
        sample.set("cpu", MetricValue::new(Some(50.0), Some(0.5)));
        let mut registry = MetricRegistry::default();

        registry.apply_sample(sample);

        assert_eq!(registry.get("cpu").unwrap().normalized, Some(0.5));
    }

    #[test]
    fn marks_metric_stale() {
        let mut registry = MetricRegistry::default();

        registry.mark_stale("cpu", Some("timeout".to_string()));

        let value = registry.get("cpu").unwrap();
        assert!(value.stale);
        assert_eq!(value.normalized, Some(1.0));
        assert_eq!(
            value.status,
            MetricStatus::Stale {
                reason: Some("timeout".to_string())
            }
        );
    }

    #[test]
    fn marks_metric_error() {
        let mut registry = MetricRegistry::default();

        registry.mark_error("cpu", Some("read failed".to_string()));

        let value = registry.get("cpu").unwrap();
        assert!(value.stale);
        assert_eq!(
            value.status,
            MetricStatus::Error {
                reason: Some("read failed".to_string())
            }
        );
    }

    #[test]
    fn normal_update_clears_metric_status() {
        let mut registry = MetricRegistry::default();
        registry.mark_error("cpu", None);

        registry.set("cpu", MetricValue::new(Some(42.0), Some(0.42)));

        let value = registry.get("cpu").unwrap();
        assert!(!value.stale);
        assert_eq!(value.status, MetricStatus::Normal);
    }

    #[test]
    fn clears_metric_status_without_replacing_value() {
        let mut registry = MetricRegistry::default();
        registry.set("cpu", MetricValue::new(Some(42.0), Some(0.42)));
        registry.mark_stale("cpu", None);

        registry.clear_status("cpu");

        let value = registry.get("cpu").unwrap();
        assert!(!value.stale);
        assert_eq!(value.status, MetricStatus::Normal);
    }
}
