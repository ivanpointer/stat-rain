use anyhow::Result;
use stat_rain::cli::{Cli, Command, RunArgs};
use stat_rain::config::AppConfig;
use stat_rain::effect::{EffectState, GlyphSet};
use stat_rain::metrics::{MetricRegistry, MetricValue};
use stat_rain::terminal::{self, FrameRenderer, TerminalCapabilities, TerminalSize};
use std::fs;
use std::io::{self, Write};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
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
    let frames = args.frames.unwrap_or(u64::MAX);
    let delay = Duration::from_millis(args.frame_delay_ms);
    let caps = TerminalCapabilities::detect_from_env(
        std::env::var("TERM").ok().as_deref(),
        std::env::var("COLORTERM").ok().as_deref(),
        std::env::var("TMUX").ok().as_deref(),
    );
    let alternate_screen = caps.alternate_screen && !args.no_alt_screen;
    let color_mode = caps.color_mode;
    let mut size = current_size(&args);
    let mut engine = stat_rain::effect::RainEngine::new(size.width, size.height, 0x5154_5241_494e);
    let mut renderer = FrameRenderer::new(color_mode);
    let interrupted = install_interrupt_flag()?;

    terminal::write_enter(&mut *output, alternate_screen)?;
    for _ in 0..frames {
        if interrupted.load(Ordering::Relaxed) {
            break;
        }
        let new_size = current_size(&args);
        if should_rebuild_engine(size, new_size) {
            size = new_size;
            engine = stat_rain::effect::RainEngine::new(size.width, size.height, 0x5154_5241_494e);
            renderer.clear();
            terminal::write_clear(&mut *output)?;
        }
        let frame = engine.step(state);
        renderer.write_frame(&mut *output, &frame)?;
        if !delay.is_zero() {
            thread::sleep(delay);
        }
    }
    terminal::write_exit(&mut *output, alternate_screen)?;

    Ok(())
}

fn current_size(args: &RunArgs) -> TerminalSize {
    terminal::resolve_terminal_size(
        args.width,
        args.height,
        terminal::detect_terminal_size().unwrap_or(TerminalSize::DEFAULT),
    )
}

pub fn should_rebuild_engine(previous: TerminalSize, current: TerminalSize) -> bool {
    previous != current
}

fn install_interrupt_flag() -> Result<Arc<AtomicBool>> {
    let interrupted = Arc::new(AtomicBool::new(false));
    signal_hook::flag::register(signal_hook::consts::SIGINT, Arc::clone(&interrupted))?;
    signal_hook::flag::register(signal_hook::consts::SIGTERM, Arc::clone(&interrupted))?;
    Ok(interrupted)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rebuilds_engine_when_terminal_size_changes() {
        assert!(should_rebuild_engine(
            TerminalSize {
                width: 80,
                height: 24
            },
            TerminalSize {
                width: 100,
                height: 40
            }
        ));
    }

    #[test]
    fn keeps_engine_when_terminal_size_is_unchanged() {
        assert!(!should_rebuild_engine(
            TerminalSize {
                width: 80,
                height: 24
            },
            TerminalSize {
                width: 80,
                height: 24
            }
        ));
    }
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
