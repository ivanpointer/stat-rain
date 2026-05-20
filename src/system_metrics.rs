use crate::metrics::{MetricProvider, MetricSample};

#[cfg(target_os = "linux")]
pub mod linux;

#[cfg(target_os = "macos")]
pub mod macos;

#[cfg(target_os = "linux")]
pub type BuiltinSystemProvider = linux::LinuxSystemProvider;

#[cfg(target_os = "macos")]
pub type BuiltinSystemProvider = macos::MacosSystemProvider;

pub fn sample_provider(provider: &mut impl MetricProvider) -> anyhow::Result<MetricSample> {
    provider.sample()
}
