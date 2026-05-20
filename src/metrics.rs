use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MetricValue {
    pub raw: Option<f64>,
    pub normalized: Option<f64>,
}

impl MetricValue {
    pub fn new(raw: Option<f64>, normalized: Option<f64>) -> Self {
        Self { raw, normalized }
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
