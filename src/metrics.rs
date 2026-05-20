use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MetricValue {
    pub raw: Option<f64>,
    pub normalized: Option<f64>,
    pub stale: bool,
}

impl MetricValue {
    pub fn new(raw: Option<f64>, normalized: Option<f64>) -> Self {
        Self {
            raw,
            normalized,
            stale: false,
        }
    }
}

#[derive(Debug, Default, Clone)]
pub struct MetricRegistry {
    values: BTreeMap<String, MetricValue>,
}

impl MetricRegistry {
    pub fn set(&mut self, name: impl Into<String>, value: MetricValue) {
        self.values.insert(name.into(), value);
    }

    pub fn mark_stale(&mut self, name: impl Into<String>) {
        self.values.insert(
            name.into(),
            MetricValue {
                raw: None,
                normalized: Some(1.0),
                stale: true,
            },
        );
    }

    pub fn apply_sample(&mut self, sample: MetricSample) {
        for (name, value) in sample.values {
            self.set(name, value);
        }
    }

    pub fn get(&self, name: &str) -> Option<MetricValue> {
        self.values.get(name).copied()
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
        self.values.get(name).copied()
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

        registry.mark_stale("cpu");

        let value = registry.get("cpu").unwrap();
        assert!(value.stale);
        assert_eq!(value.normalized, Some(1.0));
    }
}
