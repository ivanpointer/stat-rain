use anyhow::Result;
use stat_rain::cli::{Cli, Command, RunArgs};
use stat_rain::config::AppConfig;
use stat_rain::effect::{EffectState, GlyphSet};
use stat_rain::metrics::{MetricRegistry, MetricValue};
use stat_rain::terminal::{self, TerminalCapabilities};
use std::fs;
use std::io::{self, Write};
use std::thread;
use std::time::Duration;

fn main() -> Result<()> {
    let cli = Cli::parse_args();

    match cli.command {
        Command::Run(args) => run(args)?,
        Command::Init(_) => {
            println!("stat-rain init scaffold");
        }
        Command::Send(_) => {
            println!("stat-rain send scaffold");
        }
    }

    Ok(())
}

fn run(args: RunArgs) -> Result<()> {
    run_with_writer(args, &mut io::stdout())
}

pub fn run_with_writer(args: RunArgs, output: &mut impl Write) -> Result<()> {
    let mut state = load_effect_state(&args)?;
    if args.ascii {
        state.glyph_set = GlyphSet::Ascii;
    }
    let width = args.width.unwrap_or(80);
    let height = args.height.unwrap_or(24);
    let frames = args.frames.unwrap_or(u64::MAX);
    let delay = Duration::from_millis(args.frame_delay_ms);
    let caps = TerminalCapabilities::detect_from_env(
        std::env::var("TERM").ok().as_deref(),
        std::env::var("COLORTERM").ok().as_deref(),
        std::env::var("TMUX").ok().as_deref(),
    );
    let alternate_screen = caps.alternate_screen && !args.no_alt_screen;
    let color_mode = caps.color_mode;
    let mut engine = stat_rain::effect::RainEngine::new(width, height, 0x5154_5241_494e);

    terminal::write_enter(&mut *output, alternate_screen)?;
    for _ in 0..frames {
        let frame = engine.step(state);
        terminal::write_frame(&mut *output, &frame, color_mode)?;
        if !delay.is_zero() {
            thread::sleep(delay);
        }
    }
    terminal::write_exit(&mut *output, alternate_screen)?;

    Ok(())
}

fn load_effect_state(args: &RunArgs) -> Result<EffectState> {
    let profile = args.profile.as_deref().unwrap_or("default");
    let mut config = if let Some(path) = &args.config {
        let input = fs::read_to_string(path)?;
        AppConfig::from_toml_profile(&input, profile)?
    } else if let Some(input) = &args.config_inline {
        AppConfig::from_toml_profile(input, profile)?
    } else {
        AppConfig::default()
    };

    for mapping in &args.mappings {
        config.apply_mapping_override(mapping)?;
    }

    let mut metrics = MetricRegistry::default();
    metrics.set("cpu", MetricValue::new(Some(0.2), Some(0.2)));
    metrics.set("memory", MetricValue::new(Some(0.35), Some(0.35)));
    metrics.set("thermal_zone", MetricValue::new(Some(45.0), Some(0.45)));
    metrics.set(
        "external.message_pressure",
        MetricValue::new(Some(0.0), Some(0.0)),
    );

    config.evaluate_effect_state(&metrics)
}
