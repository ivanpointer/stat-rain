use crate::metrics::{
    normalized_cpu_usage, normalized_memory_usage, CpuTicks, MetricProvider, MetricSample,
    MetricValue,
};
use anyhow::{Context, Result};
use std::ffi::CString;
use std::mem::{size_of, MaybeUninit};
use std::ptr;

#[derive(Debug, Default)]
pub struct MacosSystemProvider {
    previous_cpu: Option<CpuTicks>,
}

impl MacosSystemProvider {
    pub fn new() -> Self {
        Self::default()
    }

    fn sample_from_values(&mut self, cpu: CpuTicks, memory: MacosMemory) -> MetricSample {
        let mut sample = MetricSample::default();

        if let Some(previous_cpu) = self.previous_cpu {
            if let Some(normalized) = normalized_cpu_usage(previous_cpu, cpu) {
                sample.set(
                    "cpu",
                    MetricValue::new(Some(normalized * 100.0), Some(normalized)),
                );
            }
        }
        self.previous_cpu = Some(cpu);

        if let Some(normalized) =
            normalized_memory_usage(memory.total_bytes, memory.available_bytes)
        {
            sample.set(
                "memory",
                MetricValue::new(Some(normalized * 100.0), Some(normalized)),
            );
        }

        sample
    }
}

impl MetricProvider for MacosSystemProvider {
    fn sample(&mut self) -> Result<MetricSample> {
        let cpu = read_cpu_ticks().context("failed to read macOS CPU ticks")?;
        let memory = read_memory().context("failed to read macOS memory")?;
        Ok(self.sample_from_values(cpu, memory))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct MacosMemory {
    total_bytes: u64,
    available_bytes: u64,
}

fn ticks_from_host_cpu(info: libc::host_cpu_load_info) -> CpuTicks {
    CpuTicks {
        user: info.cpu_ticks[libc::CPU_STATE_USER as usize] as u64,
        nice: info.cpu_ticks[libc::CPU_STATE_NICE as usize] as u64,
        system: info.cpu_ticks[libc::CPU_STATE_SYSTEM as usize] as u64,
        idle: info.cpu_ticks[libc::CPU_STATE_IDLE as usize] as u64,
    }
}

fn available_memory_bytes(stats: &libc::vm_statistics64, page_size: u64) -> u64 {
    let available_pages =
        stats.free_count as u64 + stats.inactive_count as u64 + stats.speculative_count as u64;
    available_pages.saturating_mul(page_size)
}

fn read_cpu_ticks() -> Result<CpuTicks> {
    let mut info = MaybeUninit::<libc::host_cpu_load_info>::zeroed();
    let mut count = libc::HOST_CPU_LOAD_INFO_COUNT;
    #[allow(deprecated)]
    let host = unsafe { libc::mach_host_self() };
    let result = unsafe {
        libc::host_statistics(
            host,
            libc::HOST_CPU_LOAD_INFO,
            info.as_mut_ptr() as libc::host_info_t,
            &mut count,
        )
    };
    if result != libc::KERN_SUCCESS {
        anyhow::bail!("host_statistics HOST_CPU_LOAD_INFO failed: {result}");
    }
    Ok(ticks_from_host_cpu(unsafe { info.assume_init() }))
}

fn read_memory() -> Result<MacosMemory> {
    let total_bytes = sysctl_u64("hw.memsize")?;
    let page_size = unsafe { libc::sysconf(libc::_SC_PAGESIZE) };
    if page_size <= 0 {
        anyhow::bail!("sysconf _SC_PAGESIZE failed");
    }

    let mut stats = MaybeUninit::<libc::vm_statistics64>::zeroed();
    let mut count = libc::HOST_VM_INFO64_COUNT;
    #[allow(deprecated)]
    let host = unsafe { libc::mach_host_self() };
    let result = unsafe {
        libc::host_statistics64(
            host,
            libc::HOST_VM_INFO64,
            stats.as_mut_ptr() as libc::host_info64_t,
            &mut count,
        )
    };
    if result != libc::KERN_SUCCESS {
        anyhow::bail!("host_statistics64 HOST_VM_INFO64 failed: {result}");
    }

    let stats = unsafe { stats.assume_init() };
    Ok(MacosMemory {
        total_bytes,
        available_bytes: available_memory_bytes(&stats, page_size as u64),
    })
}

fn sysctl_u64(name: &str) -> Result<u64> {
    let name = CString::new(name)?;
    let mut value = 0_u64;
    let mut len = size_of::<u64>();
    let result = unsafe {
        libc::sysctlbyname(
            name.as_ptr(),
            &mut value as *mut _ as *mut libc::c_void,
            &mut len,
            ptr::null_mut(),
            0,
        )
    };
    if result != 0 {
        anyhow::bail!("sysctlbyname failed");
    }
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converts_host_cpu_ticks_to_common_ticks() {
        let mut info = libc::host_cpu_load_info { cpu_ticks: [0; 4] };
        info.cpu_ticks[libc::CPU_STATE_USER as usize] = 10;
        info.cpu_ticks[libc::CPU_STATE_NICE as usize] = 1;
        info.cpu_ticks[libc::CPU_STATE_SYSTEM as usize] = 20;
        info.cpu_ticks[libc::CPU_STATE_IDLE as usize] = 100;

        assert_eq!(
            ticks_from_host_cpu(info),
            CpuTicks {
                user: 10,
                nice: 1,
                system: 20,
                idle: 100
            }
        );
    }

    #[test]
    fn calculates_available_memory_from_vm_stats() {
        let mut stats = unsafe { MaybeUninit::<libc::vm_statistics64>::zeroed().assume_init() };
        stats.free_count = 10;
        stats.inactive_count = 20;
        stats.speculative_count = 5;

        assert_eq!(available_memory_bytes(&stats, 4096), 35 * 4096);
    }

    #[test]
    fn samples_memory_and_second_cpu_reading() {
        let mut provider = MacosSystemProvider::new();
        let first = provider.sample_from_values(
            CpuTicks {
                user: 100,
                nice: 0,
                system: 100,
                idle: 800,
            },
            MacosMemory {
                total_bytes: 1_000,
                available_bytes: 500,
            },
        );
        let second = provider.sample_from_values(
            CpuTicks {
                user: 150,
                nice: 0,
                system: 150,
                idle: 900,
            },
            MacosMemory {
                total_bytes: 1_000,
                available_bytes: 250,
            },
        );

        assert!(first.get("cpu").is_none());
        assert_eq!(first.get("memory").unwrap().normalized, Some(0.5));
        assert_eq!(second.get("cpu").unwrap().normalized, Some(0.5));
        assert_eq!(second.get("memory").unwrap().normalized, Some(0.75));
    }
}
