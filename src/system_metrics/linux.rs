use crate::metrics::{
    normalized_cpu_usage, normalized_io_rate, normalized_memory_usage, CpuTicks, MetricProvider,
    MetricSample, MetricValue,
};
use anyhow::{Context, Result};
use std::fs;
use std::time::Instant;

const DISK_IO_MAX_BYTES_PER_SEC: f64 = 250_000_000.0;
const NETWORK_IO_MAX_BYTES_PER_SEC: f64 = 125_000_000.0;

#[derive(Debug, Default)]
pub struct LinuxSystemProvider {
    previous_cpu: Option<CpuTicks>,
    previous_io: Option<IoCounters>,
    previous_io_sample: Option<Instant>,
}

impl LinuxSystemProvider {
    pub fn new() -> Self {
        Self::default()
    }

    fn sample_from_strings(
        &mut self,
        stat: &str,
        meminfo: &str,
        diskstats: &str,
        netdev: &str,
    ) -> Result<MetricSample> {
        let now = Instant::now();
        let elapsed_secs = self
            .previous_io_sample
            .map(|previous| now.duration_since(previous).as_secs_f64())
            .unwrap_or(0.0);
        self.sample_from_strings_with_elapsed(stat, meminfo, diskstats, netdev, elapsed_secs)
    }

    fn sample_from_strings_with_elapsed(
        &mut self,
        stat: &str,
        meminfo: &str,
        diskstats: &str,
        netdev: &str,
        elapsed_secs: f64,
    ) -> Result<MetricSample> {
        let current_cpu = parse_proc_stat(stat).context("failed to parse /proc/stat")?;
        let memory = parse_proc_meminfo(meminfo).context("failed to parse /proc/meminfo")?;
        let current_io = IoCounters {
            disk_bytes: parse_proc_diskstats(diskstats)
                .context("failed to parse /proc/diskstats")?,
            network_bytes: parse_proc_net_dev(netdev).context("failed to parse /proc/net/dev")?,
        };
        let mut sample = MetricSample::default();

        if let Some(previous_cpu) = self.previous_cpu {
            if let Some(normalized) = normalized_cpu_usage(previous_cpu, current_cpu) {
                let value = MetricValue::new(Some(normalized * 100.0), Some(normalized));
                sample.set("cpu", value.clone());
                sample.set("cpu.total", value);
            }
        }
        self.previous_cpu = Some(current_cpu);

        if let Some(normalized) = normalized_memory_usage(memory.total_kib, memory.available_kib) {
            sample.set(
                "memory",
                MetricValue::new(Some(normalized * 100.0), Some(normalized)),
            );
        }

        if let Some(previous_io) = self.previous_io {
            if let Some((raw, normalized)) = normalized_io_rate(
                previous_io.disk_bytes,
                current_io.disk_bytes,
                elapsed_secs,
                DISK_IO_MAX_BYTES_PER_SEC,
            ) {
                sample.set("disk_io", MetricValue::new(Some(raw), Some(normalized)));
            }
            if let Some((raw, normalized)) = normalized_io_rate(
                previous_io.network_bytes,
                current_io.network_bytes,
                elapsed_secs,
                NETWORK_IO_MAX_BYTES_PER_SEC,
            ) {
                sample.set("network_io", MetricValue::new(Some(raw), Some(normalized)));
            }
        }
        self.previous_io = Some(current_io);
        self.previous_io_sample = Some(Instant::now());

        Ok(sample)
    }
}

impl MetricProvider for LinuxSystemProvider {
    fn sample(&mut self) -> Result<MetricSample> {
        let stat = fs::read_to_string("/proc/stat").context("failed to read /proc/stat")?;
        let meminfo =
            fs::read_to_string("/proc/meminfo").context("failed to read /proc/meminfo")?;
        let diskstats =
            fs::read_to_string("/proc/diskstats").context("failed to read /proc/diskstats")?;
        let netdev = fs::read_to_string("/proc/net/dev").context("failed to read /proc/net/dev")?;
        self.sample_from_strings(&stat, &meminfo, &diskstats, &netdev)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct IoCounters {
    disk_bytes: u64,
    network_bytes: u64,
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

fn parse_proc_diskstats(input: &str) -> Result<u64> {
    let mut total_sectors = 0_u64;

    for line in input.lines() {
        let fields = line.split_whitespace().collect::<Vec<_>>();
        if fields.len() < 10 {
            continue;
        }
        let name = fields[2];
        if is_ignored_block_device(name) {
            continue;
        }
        let sectors_read = fields[5]
            .parse::<u64>()
            .context("invalid sectors read field")?;
        let sectors_written = fields[9]
            .parse::<u64>()
            .context("invalid sectors written field")?;
        total_sectors = total_sectors.saturating_add(sectors_read.saturating_add(sectors_written));
    }

    Ok(total_sectors.saturating_mul(512))
}

fn is_ignored_block_device(name: &str) -> bool {
    name.starts_with("loop")
        || name.starts_with("ram")
        || name.starts_with("fd")
        || is_partition_name(name)
}

fn is_partition_name(name: &str) -> bool {
    if name.starts_with("nvme") || name.starts_with("mmcblk") {
        return name
            .rsplit_once('p')
            .is_some_and(|(_, partition)| partition.chars().all(|c| c.is_ascii_digit()));
    }

    (name.starts_with("sd") || name.starts_with("vd") || name.starts_with("xvd"))
        && name
            .chars()
            .last()
            .is_some_and(|last| last.is_ascii_digit())
}

fn parse_proc_net_dev(input: &str) -> Result<u64> {
    let mut total_bytes = 0_u64;

    for line in input.lines() {
        let Some((name, values)) = line.split_once(':') else {
            continue;
        };
        if name.trim() == "lo" {
            continue;
        }
        let fields = values.split_whitespace().collect::<Vec<_>>();
        if fields.len() < 16 {
            continue;
        }
        let rx_bytes = fields[0].parse::<u64>().context("invalid rx bytes field")?;
        let tx_bytes = fields[8].parse::<u64>().context("invalid tx bytes field")?;
        total_bytes = total_bytes.saturating_add(rx_bytes.saturating_add(tx_bytes));
    }

    Ok(total_bytes)
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
                "8 0 sda 10 0 100 0 20 0 200 0 0 0 0 0 0 0 0 0\n",
                "Inter-| Receive | Transmit\neth0: 1000 0 0 0 0 0 0 0 2000 0 0 0 0 0 0 0\n",
            )
            .unwrap();
        let second = provider
            .sample_from_strings_with_elapsed(
                "cpu 150 0 150 900\n",
                "MemTotal: 1000 kB\nMemAvailable: 250 kB\n",
                "8 0 sda 10 0 1100 0 20 0 1200 0 0 0 0 0 0 0 0 0\n",
                "Inter-| Receive | Transmit\neth0: 2000 0 0 0 0 0 0 0 5000 0 0 0 0 0 0 0\n",
                1.0,
            )
            .unwrap();

        assert!(first.get("cpu").is_none());
        assert_eq!(first.get("memory").unwrap().normalized, Some(0.5));
        assert_eq!(second.get("cpu").unwrap().normalized, Some(0.5));
        assert_eq!(second.get("cpu.total").unwrap().normalized, Some(0.5));
        assert_eq!(second.get("memory").unwrap().normalized, Some(0.75));
        assert_eq!(second.get("disk_io").unwrap().raw, Some(1_024_000.0));
        assert_eq!(second.get("network_io").unwrap().raw, Some(4_000.0));
    }

    #[test]
    fn parses_proc_diskstats_total_read_and_written_bytes() {
        let bytes = parse_proc_diskstats(
            "7 0 loop0 1 0 999 0 1 0 999 0 0 0 0 0 0 0 0 0\n8 0 sda 10 0 100 0 20 0 200 0 0 0 0 0 0 0 0 0\n8 1 sda1 10 0 100 0 20 0 200 0 0 0 0 0 0 0 0 0\n259 0 nvme0n1 10 0 50 0 20 0 50 0 0 0 0 0 0 0 0 0\n259 1 nvme0n1p1 10 0 50 0 20 0 50 0 0 0 0 0 0 0 0 0\n",
        )
        .unwrap();

        assert_eq!(bytes, 400 * 512);
    }

    #[test]
    fn parses_proc_net_dev_total_non_loopback_bytes() {
        let bytes = parse_proc_net_dev(
            "Inter-| Receive | Transmit\nlo: 10 0 0 0 0 0 0 0 20 0 0 0 0 0 0 0\neth0: 100 0 0 0 0 0 0 0 250 0 0 0 0 0 0 0\n",
        )
        .unwrap();

        assert_eq!(bytes, 350);
    }
}
