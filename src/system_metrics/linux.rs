use crate::metrics::{
    normalized_cpu_usage, normalized_memory_usage, CpuTicks, MetricProvider, MetricSample,
    MetricValue,
};
use anyhow::{Context, Result};
use std::fs;

#[derive(Debug, Default)]
pub struct LinuxSystemProvider {
    previous_cpu: Option<CpuTicks>,
}

impl LinuxSystemProvider {
    pub fn new() -> Self {
        Self::default()
    }

    fn sample_from_strings(&mut self, stat: &str, meminfo: &str) -> Result<MetricSample> {
        let current_cpu = parse_proc_stat(stat).context("failed to parse /proc/stat")?;
        let memory = parse_proc_meminfo(meminfo).context("failed to parse /proc/meminfo")?;
        let mut sample = MetricSample::default();

        if let Some(previous_cpu) = self.previous_cpu {
            if let Some(normalized) = normalized_cpu_usage(previous_cpu, current_cpu) {
                sample.set("cpu", MetricValue::new(Some(normalized * 100.0), Some(normalized)));
            }
        }
        self.previous_cpu = Some(current_cpu);

        if let Some(normalized) = normalized_memory_usage(memory.total_kib, memory.available_kib) {
            sample.set("memory", MetricValue::new(Some(normalized * 100.0), Some(normalized)));
        }

        Ok(sample)
    }
}

impl MetricProvider for LinuxSystemProvider {
    fn sample(&mut self) -> Result<MetricSample> {
        let stat = fs::read_to_string("/proc/stat").context("failed to read /proc/stat")?;
        let meminfo =
            fs::read_to_string("/proc/meminfo").context("failed to read /proc/meminfo")?;
        self.sample_from_strings(&stat, &meminfo)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct LinuxMemory {
    total_kib: u64,
    available_kib: u64,
}

fn parse_proc_stat(input: &str) -> Result<CpuTicks> {
    let Some(line) = input.lines().find(|line| line.starts_with("cpu ")) else {
        anyhow::bail!("missing aggregate cpu line");
    };
    let mut fields = line.split_whitespace();
    let _cpu = fields.next();
    let user = parse_next(&mut fields, "user")?;
    let nice = parse_next(&mut fields, "nice")?;
    let system = parse_next(&mut fields, "system")?;
    let idle = parse_next(&mut fields, "idle")?;
    Ok(CpuTicks {
        user,
        nice,
        system,
        idle,
    })
}

fn parse_proc_meminfo(input: &str) -> Result<LinuxMemory> {
    let mut total = None;
    let mut available = None;

    for line in input.lines() {
        if let Some(value) = line.strip_prefix("MemTotal:") {
            total = Some(parse_kib(value)?);
        } else if let Some(value) = line.strip_prefix("MemAvailable:") {
            available = Some(parse_kib(value)?);
        }
    }

    Ok(LinuxMemory {
        total_kib: total.context("missing MemTotal")?,
        available_kib: available.context("missing MemAvailable")?,
    })
}

fn parse_next<'a>(fields: &mut impl Iterator<Item = &'a str>, name: &str) -> Result<u64> {
    fields
        .next()
        .context(format!("missing {name} field"))?
        .parse::<u64>()
        .context(format!("invalid {name} field"))
}

fn parse_kib(value: &str) -> Result<u64> {
    value
        .split_whitespace()
        .next()
        .context("missing KiB value")?
        .parse::<u64>()
        .context("invalid KiB value")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_proc_stat_cpu_ticks() {
        let ticks = parse_proc_stat("cpu  100 2 30 400 5 6 7 8 9 10\n").unwrap();

        assert_eq!(
            ticks,
            CpuTicks {
                user: 100,
                nice: 2,
                system: 30,
                idle: 400
            }
        );
    }

    #[test]
    fn parses_proc_meminfo_memory_totals() {
        let memory = parse_proc_meminfo(
            "MemTotal:       1000000 kB\nMemFree:         100000 kB\nMemAvailable:    250000 kB\n",
        )
        .unwrap();

        assert_eq!(
            memory,
            LinuxMemory {
                total_kib: 1_000_000,
                available_kib: 250_000
            }
        );
    }

    #[test]
    fn samples_memory_and_second_cpu_reading() {
        let mut provider = LinuxSystemProvider::new();
        let first = provider
            .sample_from_strings(
                "cpu 100 0 100 800\n",
                "MemTotal: 1000 kB\nMemAvailable: 500 kB\n",
            )
            .unwrap();
        let second = provider
            .sample_from_strings(
                "cpu 150 0 150 900\n",
                "MemTotal: 1000 kB\nMemAvailable: 250 kB\n",
            )
            .unwrap();

        assert!(first.get("cpu").is_none());
        assert_eq!(first.get("memory").unwrap().normalized, Some(0.5));
        assert_eq!(second.get("cpu").unwrap().normalized, Some(0.5));
        assert_eq!(second.get("memory").unwrap().normalized, Some(0.75));
    }
}
