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

impl Cli {
    pub fn parse_args() -> Self {
        Self::parse()
    }
}
