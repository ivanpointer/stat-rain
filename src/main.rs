use anyhow::{bail, Result};
use stat_rain::cli::{Cli, Command, RunArgs, SendArgs, StressCpuArgs};
use stat_rain::config::AppConfig;
use stat_rain::dev_tools;
use stat_rain::effect::{apply_message_overlay, EffectSmoother, GlyphSet, MessageOverlay};
use stat_rain::metrics::{MetricProvider, MetricRegistry, MetricValue};
use stat_rain::protocol::{self, ProtocolMessage};
use stat_rain::system_metrics::BuiltinSystemProvider;
use stat_rain::terminal::{self, FrameRenderer, TerminalCapabilities, TerminalSize};
use std::collections::BTreeMap;
use std::fs;
use std::io::{self, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

fn main() -> Result<()> {
    let cli = Cli::parse_args();

    match cli.command {
        Command::Run(args) => run(args)?,
        Command::Init(_) => {
            println!("stat-rain init scaffold");
        }
        Command::Send(args) => send(args)?,
        Command::StressCpu(args) => stress_cpu(args)?,
    }

    Ok(())
}

#[derive(Debug, Default)]
struct ExternalMetricOverrides {
    metrics: BTreeMap<String, MetricValue>,
}

impl ExternalMetricOverrides {
    fn set(&mut self, name: String, value: MetricValue) {
        self.metrics.insert(name.clone(), value);
        if name == "cpu" {
            self.metrics.insert("cpu.total".to_string(), value);
        }
    }

    fn mark_stale(&mut self, name: String) {
        let value = MetricValue {
            raw: None,
            normalized: Some(1.0),
            stale: true,
        };
        self.set(name, value);
    }

    fn apply_to(&self, metrics: &mut MetricRegistry) {
        for (name, value) in &self.metrics {
            metrics.set(name.clone(), *value);
        }
    }
}

fn run(args: RunArgs) -> Result<()> {
    run_with_writer(args, &mut io::stdout())
}

pub fn run_with_writer(args: RunArgs, output: &mut impl Write) -> Result<()> {
    let config = load_config(&args)?;
    let simulated_metrics = dev_tools::parse_simulated_metrics(&args.simulated_metrics)?;
    let mut metrics = initial_metric_registry();
    let mut external_overrides = ExternalMetricOverrides::default();
    let mut active_message: Option<MessageOverlay> = None;
    let mut provider = BuiltinSystemProvider::new();
    sample_builtin_metrics(&mut provider, &mut metrics);
    dev_tools::apply_simulated_metrics(&mut metrics, &simulated_metrics);
    let metric_interval = Duration::from_millis(args.metric_sample_ms);
    let mut last_metric_sample = Instant::now();
    let mut last_frame = Instant::now();
    let mut smoother = EffectSmoother::new(Duration::from_millis(args.effect_smoothing_ms));
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
    let socket_input = match &args.socket {
        Some(path) => Some(start_socket_listener(path, Arc::clone(&interrupted))?),
        None => None,
    };

    terminal::write_enter(&mut *output, alternate_screen)?;
    for _ in 0..frames {
        if interrupted.load(Ordering::Relaxed) {
            break;
        }
        drain_socket_messages(
            &socket_input,
            &mut metrics,
            &mut external_overrides,
            &mut active_message,
        );
        if last_metric_sample.elapsed() >= metric_interval {
            sample_builtin_metrics(&mut provider, &mut metrics);
            dev_tools::apply_simulated_metrics(&mut metrics, &simulated_metrics);
            external_overrides.apply_to(&mut metrics);
            last_metric_sample = Instant::now();
        }
        let now = Instant::now();
        let frame_elapsed = now.duration_since(last_frame);
        last_frame = now;
        let mut target_state = config.evaluate_effect_state(&metrics)?;
        if args.ascii {
            target_state.glyph_set = GlyphSet::Ascii;
        }
        let state = smoother.update(target_state, frame_elapsed);
        let new_size = current_size(&args);
        if should_rebuild_engine(size, new_size) {
            size = new_size;
            engine = stat_rain::effect::RainEngine::new(size.width, size.height, 0x5154_5241_494e);
            renderer.clear();
            terminal::write_clear(&mut *output)?;
        }
        let mut frame = engine.step(state);
        if let Some(message) = active_message.as_mut() {
            apply_message_overlay(&mut frame, message, state.message_reveal_intensity);
            message.advance();
            if message.is_expired() {
                active_message = None;
            }
        }
        renderer.write_frame(&mut *output, &frame)?;
        if !delay.is_zero() {
            thread::sleep(delay);
        }
    }
    terminal::write_exit(&mut *output, alternate_screen)?;
    interrupted.store(true, Ordering::Relaxed);

    Ok(())
}

fn send(args: SendArgs) -> Result<()> {
    let message = message_from_send_args(&args)?;
    send_protocol_message(&args.socket, &message)
}

fn send_protocol_message(path: &Path, message: &ProtocolMessage) -> Result<()> {
    let mut stream = UnixStream::connect(path)?;
    protocol::write_framed_message(&mut stream, message)
}

struct SocketInput {
    path: PathBuf,
    receiver: Receiver<ProtocolMessage>,
}

impl Drop for SocketInput {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

fn start_socket_listener(path: &Path, interrupted: Arc<AtomicBool>) -> Result<SocketInput> {
    if path.exists() {
        fs::remove_file(path)?;
    }
    let listener = UnixListener::bind(path)?;
    listener.set_nonblocking(true)?;
    let (sender, receiver) = mpsc::channel();

    thread::spawn(move || {
        while !interrupted.load(Ordering::Relaxed) {
            match listener.accept() {
                Ok((stream, _)) => {
                    if let Ok(message) = protocol::read_framed_message(stream) {
                        let _ = sender.send(message);
                    }
                }
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                    thread::sleep(Duration::from_millis(5));
                }
                Err(_) => {
                    thread::sleep(Duration::from_millis(25));
                }
            }
        }
    });

    Ok(SocketInput {
        path: path.to_path_buf(),
        receiver,
    })
}

fn drain_socket_messages(
    socket_input: &Option<SocketInput>,
    metrics: &mut MetricRegistry,
    overrides: &mut ExternalMetricOverrides,
    active_message: &mut Option<MessageOverlay>,
) {
    let Some(socket_input) = socket_input else {
        return;
    };
    while let Ok(message) = socket_input.receiver.try_recv() {
        apply_protocol_message(metrics, overrides, active_message, message);
    }
}

fn message_from_send_args(args: &SendArgs) -> Result<ProtocolMessage> {
    match (&args.metric, args.value, &args.message) {
        (Some(name), Some(value), None) => {
            if !(0.0..=1.0).contains(&value) || !value.is_finite() {
                bail!("--value must be a finite normalized value between 0.0 and 1.0");
            }
            Ok(ProtocolMessage::MetricUpdate {
                name: name.clone(),
                raw: None,
                normalized: Some(value),
            })
        }
        (None, None, Some(text)) => Ok(ProtocolMessage::TextInjection { text: text.clone() }),
        (Some(_), None, None) => bail!("--metric requires --value"),
        _ => bail!("provide either --metric with --value, or --message"),
    }
}

fn apply_protocol_message(
    metrics: &mut MetricRegistry,
    overrides: &mut ExternalMetricOverrides,
    active_message: &mut Option<MessageOverlay>,
    message: ProtocolMessage,
) {
    match message {
        ProtocolMessage::MetricUpdate {
            name,
            raw,
            normalized,
        } => {
            let value = MetricValue::new(raw, normalized);
            overrides.set(name, value);
            overrides.apply_to(metrics);
        }
        ProtocolMessage::MetricStale { name } => {
            overrides.mark_stale(name);
            overrides.apply_to(metrics);
        }
        ProtocolMessage::TextInjection { text } => {
            *active_message = Some(MessageOverlay::new(text, 180, 0x5445_5854));
        }
    }
}

fn stress_cpu(args: StressCpuArgs) -> Result<()> {
    let duration = Duration::from_secs(args.duration_seconds);
    let started = Instant::now();
    eprintln!(
        "stressing CPU with {} Fibonacci worker(s) for {}s; press Ctrl-C to stop the process",
        args.threads.max(1),
        args.duration_seconds
    );
    let iterations = dev_tools::run_cpu_stress(args.threads, duration, args.fib_n);
    eprintln!(
        "completed {iterations} Fibonacci iteration(s) in {:.2}s",
        started.elapsed().as_secs_f64()
    );
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

    #[test]
    fn send_metric_args_create_metric_update_message() {
        let args = stat_rain::cli::SendArgs {
            socket: "/tmp/stat-rain.sock".into(),
            metric: Some("cpu".to_string()),
            value: Some(0.99),
            message: None,
        };

        assert_eq!(
            message_from_send_args(&args).unwrap(),
            stat_rain::protocol::ProtocolMessage::MetricUpdate {
                name: "cpu".to_string(),
                raw: None,
                normalized: Some(0.99)
            }
        );
    }

    #[test]
    fn external_cpu_metric_update_persists_as_total_override() {
        let mut metrics = initial_metric_registry();
        let mut overrides = ExternalMetricOverrides::default();
        let mut active_message = None;
        let message = stat_rain::protocol::ProtocolMessage::MetricUpdate {
            name: "cpu".to_string(),
            raw: None,
            normalized: Some(0.99),
        };

        apply_protocol_message(&mut metrics, &mut overrides, &mut active_message, message);
        metrics.set("cpu", MetricValue::new(None, Some(0.01)));
        metrics.set("cpu.total", MetricValue::new(None, Some(0.01)));
        overrides.apply_to(&mut metrics);

        assert_eq!(metrics.get("cpu").unwrap().normalized, Some(0.99));
        assert_eq!(metrics.get("cpu.total").unwrap().normalized, Some(0.99));
    }

    #[test]
    fn text_injection_sets_active_message_overlay() {
        let mut metrics = initial_metric_registry();
        let mut overrides = ExternalMetricOverrides::default();
        let mut active_message = None;
        let message = stat_rain::protocol::ProtocolMessage::TextInjection {
            text: "BUILD OK".to_string(),
        };

        apply_protocol_message(&mut metrics, &mut overrides, &mut active_message, message);

        assert_eq!(active_message.unwrap().text, "BUILD OK");
    }
}

fn sample_builtin_metrics(provider: &mut impl MetricProvider, metrics: &mut MetricRegistry) {
    match provider.sample() {
        Ok(sample) => metrics.apply_sample(sample),
        Err(_) => {
            metrics.mark_stale("cpu");
            metrics.mark_stale("cpu.total");
            metrics.mark_stale("memory");
        }
    }
}

fn initial_metric_registry() -> MetricRegistry {
    let mut metrics = MetricRegistry::default();
    metrics.set("cpu", MetricValue::new(Some(0.0), Some(0.0)));
    metrics.set("cpu.total", MetricValue::new(Some(0.0), Some(0.0)));
    metrics.set("memory", MetricValue::new(Some(0.0), Some(0.0)));
    metrics.set("thermal_zone", MetricValue::new(Some(45.0), Some(0.45)));
    metrics.set(
        "external.message_pressure",
        MetricValue::new(Some(0.0), Some(0.0)),
    );
    metrics
}

fn load_config(args: &RunArgs) -> Result<AppConfig> {
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

    Ok(config)
}
