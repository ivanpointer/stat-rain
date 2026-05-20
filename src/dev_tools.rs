use crate::metrics::{MetricRegistry, MetricValue};
use anyhow::{bail, Result};
use std::thread;
use std::time::{Duration, Instant};

#[derive(Debug, Clone, PartialEq)]
pub struct SimulatedMetric {
    pub name: String,
    pub raw: Option<f64>,
    pub normalized: Option<f64>,
}

impl SimulatedMetric {
    pub fn parse(input: &str) -> Result<Self> {
        let Some((name, value)) = input.split_once('=') else {
            bail!("expected simulated metric in name=normalized or name=raw:normalized form");
        };
        let name = name.trim();
        if name.is_empty() {
            bail!("simulated metric name cannot be empty");
        }

        let (raw, normalized) = if let Some((raw, normalized)) = value.split_once(':') {
            (
                Some(parse_f64(raw, "raw")?),
                Some(parse_normalized(normalized)?),
            )
        } else {
            (None, Some(parse_normalized(value)?))
        };

        Ok(Self {
            name: name.to_string(),
            raw,
            normalized,
        })
    }
}

pub fn parse_simulated_metrics(values: &[String]) -> Result<Vec<SimulatedMetric>> {
    values
        .iter()
        .map(|value| SimulatedMetric::parse(value))
        .collect()
}

pub fn apply_simulated_metrics(metrics: &mut MetricRegistry, simulated: &[SimulatedMetric]) {
    for metric in simulated {
        let value = MetricValue::new(metric.raw, metric.normalized);
        metrics.set(metric.name.clone(), value.clone());
        if metric.name == "cpu" {
            metrics.set("cpu.total", value);
        }
    }
}

fn parse_normalized(input: &str) -> Result<f64> {
    let value = parse_f64(input, "normalized")?;
    if !(0.0..=1.0).contains(&value) {
        bail!("normalized simulated metric value must be between 0.0 and 1.0");
    }
    Ok(value)
}

fn parse_f64(input: &str, label: &str) -> Result<f64> {
    let value = input
        .trim()
        .parse::<f64>()
        .map_err(|error| anyhow::anyhow!("invalid {label} simulated metric value: {error}"))?;
    if !value.is_finite() {
        bail!("{label} simulated metric value must be finite");
    }
    Ok(value)
}

pub fn run_cpu_stress(threads: usize, duration: Duration, fibonacci_n: u32) -> u64 {
    let worker_count = threads.max(1);
    let deadline = Instant::now() + duration;
    let mut workers = Vec::with_capacity(worker_count);

    for _ in 0..worker_count {
        workers.push(thread::spawn(move || {
            let mut iterations = 0_u64;
            while Instant::now() < deadline {
                std::hint::black_box(fibonacci(fibonacci_n));
                iterations = iterations.wrapping_add(1);
            }
            iterations
        }));
    }

    workers
        .into_iter()
        .map(|worker| worker.join().unwrap_or(0))
        .sum()
}

fn fibonacci(n: u32) -> u64 {
    match n {
        0 => 0,
        1 => 1,
        _ => fibonacci(n - 1).wrapping_add(fibonacci(n - 2)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_normalized_simulated_metric() {
        let metric = SimulatedMetric::parse("cpu=0.75").unwrap();

        assert_eq!(
            metric,
            SimulatedMetric {
                name: "cpu".to_string(),
                raw: None,
                normalized: Some(0.75)
            }
        );
    }

    #[test]
    fn parses_raw_and_normalized_simulated_metric() {
        let metric = SimulatedMetric::parse("thermal_zone=92:0.92").unwrap();

        assert_eq!(metric.raw, Some(92.0));
        assert_eq!(metric.normalized, Some(0.92));
    }

    #[test]
    fn rejects_out_of_range_normalized_metric() {
        assert!(SimulatedMetric::parse("cpu=1.25").is_err());
    }

    #[test]
    fn applying_cpu_simulation_sets_total_alias() {
        let mut registry = MetricRegistry::default();
        let simulated = vec![SimulatedMetric::parse("cpu=0.9").unwrap()];

        apply_simulated_metrics(&mut registry, &simulated);

        assert_eq!(registry.get("cpu").unwrap().normalized, Some(0.9));
        assert_eq!(registry.get("cpu.total").unwrap().normalized, Some(0.9));
    }
}
