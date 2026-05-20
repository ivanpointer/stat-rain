use crate::metrics::{
    normalized_cpu_usage, normalized_io_rate, normalized_memory_usage, CpuTicks, MetricProvider,
    MetricSample, MetricValue,
};
use anyhow::{Context, Result};
use std::ffi::CString;
use std::mem::{size_of, MaybeUninit};
use std::os::raw::{c_char, c_void};
use std::ptr;
use std::time::Instant;

const DISK_IO_MAX_BYTES_PER_SEC: f64 = 250_000_000.0;
const NETWORK_IO_MAX_BYTES_PER_SEC: f64 = 125_000_000.0;

#[derive(Debug, Default)]
pub struct MacosSystemProvider {
    previous_cpu: Option<CpuTicks>,
    previous_disk_bytes: Option<u64>,
    previous_disk_sample: Option<Instant>,
    previous_network_bytes: Option<u64>,
    previous_network_sample: Option<Instant>,
}

impl MacosSystemProvider {
    pub fn new() -> Self {
        Self::default()
    }

    fn sample_from_values(&mut self, cpu: CpuTicks, memory: MacosMemory) -> MetricSample {
        let now = Instant::now();
        let disk_elapsed_secs = self
            .previous_disk_sample
            .map(|previous| now.duration_since(previous).as_secs_f64())
            .unwrap_or(0.0);
        let network_elapsed_secs = self
            .previous_network_sample
            .map(|previous| now.duration_since(previous).as_secs_f64())
            .unwrap_or(0.0);
        let disk_bytes = read_disk_bytes().ok();
        let network_bytes = read_network_bytes().ok();
        self.sample_from_values_with_io(
            cpu,
            memory,
            disk_bytes,
            disk_elapsed_secs,
            network_bytes,
            network_elapsed_secs,
        )
    }

    fn sample_from_values_with_io(
        &mut self,
        cpu: CpuTicks,
        memory: MacosMemory,
        disk_bytes: Option<u64>,
        disk_elapsed_secs: f64,
        network_bytes: Option<u64>,
        network_elapsed_secs: f64,
    ) -> MetricSample {
        let mut sample = MetricSample::default();

        if let Some(previous_cpu) = self.previous_cpu {
            if let Some(normalized) = normalized_cpu_usage(previous_cpu, cpu) {
                let value = MetricValue::new(Some(normalized * 100.0), Some(normalized));
                sample.set("cpu", value.clone());
                sample.set("cpu.total", value);
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

        if let Some(disk_bytes) = disk_bytes {
            if let Some(previous_disk_bytes) = self.previous_disk_bytes {
                if let Some((raw, normalized)) = normalized_io_rate(
                    previous_disk_bytes,
                    disk_bytes,
                    disk_elapsed_secs,
                    DISK_IO_MAX_BYTES_PER_SEC,
                ) {
                    sample.set("disk_io", MetricValue::new(Some(raw), Some(normalized)));
                }
            }
            self.previous_disk_bytes = Some(disk_bytes);
            self.previous_disk_sample = Some(Instant::now());
        }

        if let Some(network_bytes) = network_bytes {
            if let Some(previous_network_bytes) = self.previous_network_bytes {
                if let Some((raw, normalized)) = normalized_io_rate(
                    previous_network_bytes,
                    network_bytes,
                    network_elapsed_secs,
                    NETWORK_IO_MAX_BYTES_PER_SEC,
                ) {
                    sample.set("network_io", MetricValue::new(Some(raw), Some(normalized)));
                }
            }
            self.previous_network_bytes = Some(network_bytes);
            self.previous_network_sample = Some(Instant::now());
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

type CFAllocatorRef = *const c_void;
type CFDictionaryRef = *const c_void;
type CFMutableDictionaryRef = *mut c_void;
type CFNumberRef = *const c_void;
type CFStringRef = *const c_void;
type CFTypeRef = *const c_void;
type IoObject = u32;
type IoIterator = IoObject;
type IoRegistryEntry = IoObject;

const K_CF_STRING_ENCODING_UTF8: u32 = 0x0800_0100;
const K_CF_NUMBER_SINT64_TYPE: i32 = 4;
const K_IO_RETURN_SUCCESS: i32 = 0;
const K_IO_MAIN_PORT_DEFAULT: libc::mach_port_t = 0;

#[link(name = "CoreFoundation", kind = "framework")]
extern "C" {
    fn CFDictionaryGetValue(dict: CFDictionaryRef, key: *const c_void) -> *const c_void;
    fn CFNumberGetValue(number: CFNumberRef, the_type: i32, value: *mut c_void) -> bool;
    fn CFRelease(value: CFTypeRef);
    fn CFStringCreateWithCString(
        alloc: CFAllocatorRef,
        c_str: *const c_char,
        encoding: u32,
    ) -> CFStringRef;
}

#[link(name = "IOKit", kind = "framework")]
extern "C" {
    fn IOIteratorNext(iterator: IoIterator) -> IoObject;
    fn IOObjectRelease(object: IoObject) -> i32;
    fn IORegistryEntryCreateCFProperty(
        entry: IoRegistryEntry,
        key: CFStringRef,
        allocator: CFAllocatorRef,
        options: u32,
    ) -> CFTypeRef;
    fn IOServiceGetMatchingServices(
        main_port: libc::mach_port_t,
        matching: CFDictionaryRef,
        existing: *mut IoIterator,
    ) -> i32;
    fn IOServiceMatching(name: *const c_char) -> CFMutableDictionaryRef;
}

struct CfRef<T>
where
    T: Copy + Into<CFTypeRef>,
{
    value: T,
}

impl<T> CfRef<T>
where
    T: Copy + Into<CFTypeRef>,
{
    fn new(value: T) -> Self {
        Self { value }
    }

    fn get(&self) -> T {
        self.value
    }
}

impl<T> Drop for CfRef<T>
where
    T: Copy + Into<CFTypeRef>,
{
    fn drop(&mut self) {
        let value = self.value.into();
        if !value.is_null() {
            unsafe { CFRelease(value) };
        }
    }
}

struct IoRef {
    value: IoObject,
}

impl IoRef {
    fn new(value: IoObject) -> Self {
        Self { value }
    }

    fn get(&self) -> IoObject {
        self.value
    }
}

impl Drop for IoRef {
    fn drop(&mut self) {
        if self.value != 0 {
            unsafe { IOObjectRelease(self.value) };
        }
    }
}

fn cf_string(value: &str) -> Result<CfRef<CFStringRef>> {
    let value = CString::new(value)?;
    let cf_value = unsafe {
        CFStringCreateWithCString(ptr::null(), value.as_ptr(), K_CF_STRING_ENCODING_UTF8)
    };
    if cf_value.is_null() {
        anyhow::bail!("failed to create CFString");
    }
    Ok(CfRef::new(cf_value))
}

fn cf_number_u64(value: CFNumberRef) -> Option<u64> {
    if value.is_null() {
        return None;
    }
    let mut number = 0_i64;
    let ok = unsafe {
        CFNumberGetValue(
            value,
            K_CF_NUMBER_SINT64_TYPE,
            &mut number as *mut _ as *mut c_void,
        )
    };
    if ok && number >= 0 {
        Some(number as u64)
    } else {
        None
    }
}

fn read_disk_bytes() -> Result<u64> {
    let class_name = CString::new("IOBlockStorageDriver")?;
    let matching = unsafe { IOServiceMatching(class_name.as_ptr()) };
    if matching.is_null() {
        anyhow::bail!("IOServiceMatching failed for IOBlockStorageDriver");
    }

    let mut iterator = 0;
    let result =
        unsafe { IOServiceGetMatchingServices(K_IO_MAIN_PORT_DEFAULT, matching, &mut iterator) };
    if result != K_IO_RETURN_SUCCESS {
        anyhow::bail!("IOServiceGetMatchingServices failed: {result}");
    }
    let iterator = IoRef::new(iterator);
    let statistics_key = cf_string("Statistics")?;
    let bytes_read_key = cf_string("Bytes (Read)")?;
    let bytes_written_key = cf_string("Bytes (Write)")?;

    let mut total = 0_u64;
    loop {
        let service = unsafe { IOIteratorNext(iterator.get()) };
        if service == 0 {
            break;
        }
        let service = IoRef::new(service);
        let statistics = unsafe {
            IORegistryEntryCreateCFProperty(service.get(), statistics_key.get(), ptr::null(), 0)
        };
        if statistics.is_null() {
            continue;
        }
        let statistics = CfRef::new(statistics as CFDictionaryRef);
        let bytes_read =
            unsafe { CFDictionaryGetValue(statistics.get(), bytes_read_key.get()) as CFNumberRef };
        let bytes_written = unsafe {
            CFDictionaryGetValue(statistics.get(), bytes_written_key.get()) as CFNumberRef
        };
        total = total
            .saturating_add(cf_number_u64(bytes_read).unwrap_or(0))
            .saturating_add(cf_number_u64(bytes_written).unwrap_or(0));
    }

    Ok(total)
}

fn read_network_bytes() -> Result<u64> {
    let mut addrs: *mut libc::ifaddrs = ptr::null_mut();
    if unsafe { libc::getifaddrs(&mut addrs) } != 0 {
        anyhow::bail!("getifaddrs failed");
    }

    let mut total = 0_u64;
    let mut current = addrs;
    while !current.is_null() {
        let ifaddr = unsafe { &*current };
        if !ifaddr.ifa_addr.is_null()
            && !ifaddr.ifa_data.is_null()
            && unsafe { (*ifaddr.ifa_addr).sa_family as i32 } == libc::AF_LINK
            && ifaddr.ifa_flags & (libc::IFF_LOOPBACK as u32) == 0
        {
            let data = unsafe { &*(ifaddr.ifa_data as *const libc::if_data) };
            total = total.saturating_add(data.ifi_ibytes as u64 + data.ifi_obytes as u64);
        }
        current = ifaddr.ifa_next;
    }

    unsafe { libc::freeifaddrs(addrs) };
    Ok(total)
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
        let first = provider.sample_from_values_with_io(
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
            Some(10_000),
            0.0,
            Some(1_000),
            0.0,
        );
        let second = provider.sample_from_values_with_io(
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
            Some(16_000),
            2.0,
            Some(3_000),
            2.0,
        );

        assert!(first.get("cpu").is_none());
        assert_eq!(first.get("memory").unwrap().normalized, Some(0.5));
        assert_eq!(second.get("cpu").unwrap().normalized, Some(0.5));
        assert_eq!(second.get("cpu.total").unwrap().normalized, Some(0.5));
        assert_eq!(second.get("memory").unwrap().normalized, Some(0.75));
        assert_eq!(second.get("disk_io").unwrap().raw, Some(3_000.0));
        assert_eq!(second.get("network_io").unwrap().raw, Some(1_000.0));
    }
}
