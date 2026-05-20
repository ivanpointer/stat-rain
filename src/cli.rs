use clap::{Args, Parser, Subcommand};
use std::path::PathBuf;

#[derive(Debug, Parser)]
#[command(name = "stat-rain")]
#[command(about = "Low-overhead Matrix-style terminal status pane")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    Run(RunArgs),
    Init(InitArgs),
    Send(SendArgs),
    StressCpu(StressCpuArgs),
}

#[derive(Debug, Args)]
pub struct RunArgs {
    #[arg(long)]
    pub config: Option<PathBuf>,

    #[arg(long)]
    pub profile: Option<String>,

    #[arg(long = "config-inline")]
    pub config_inline: Option<String>,

    #[arg(long = "map")]
    pub mappings: Vec<String>,

    #[arg(long = "simulate-metric")]
    pub simulated_metrics: Vec<String>,

    #[arg(long)]
    pub frames: Option<u64>,

    #[arg(long = "frame-delay-ms", default_value_t = 33)]
    pub frame_delay_ms: u64,

    #[arg(long = "metric-sample-ms", default_value_t = 1_000)]
    pub metric_sample_ms: u64,

    #[arg(long = "effect-smoothing-ms", default_value_t = 10_000)]
    pub effect_smoothing_ms: u64,

    #[arg(long)]
    pub width: Option<usize>,

    #[arg(long)]
    pub height: Option<usize>,

    #[arg(long = "no-alt-screen")]
    pub no_alt_screen: bool,

    #[arg(long)]
    pub ascii: bool,
}

#[derive(Debug, Args)]
pub struct InitArgs {
    #[arg(long)]
    pub examples: bool,

    #[arg(long, default_value = "stat-rain.toml")]
    pub output: PathBuf,
}

#[derive(Debug, Args)]
pub struct SendArgs {
    #[arg(long)]
    pub socket: PathBuf,

    #[arg(long)]
    pub metric: Option<String>,

    #[arg(long)]
    pub value: Option<f64>,

    #[arg(long)]
    pub message: Option<String>,
}

#[derive(Debug, Args)]
pub struct StressCpuArgs {
    #[arg(long, default_value_t = default_stress_threads())]
    pub threads: usize,

    #[arg(long = "duration-seconds", default_value_t = 30)]
    pub duration_seconds: u64,

    #[arg(long = "fib-n", default_value_t = 35)]
    pub fib_n: u32,
}

fn default_stress_threads() -> usize {
    std::thread::available_parallelism()
        .map(usize::from)
        .unwrap_or(1)
}

impl Cli {
    pub fn parse_args() -> Self {
        Self::parse()
    }
}
