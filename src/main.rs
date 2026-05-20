use anyhow::{bail, Result};
use stat_rain::cli::{Cli, Command, RunArgs, SendArgs, StressCpuArgs};
use stat_rain::config::AppConfig;
use stat_rain::dev_tools;
use stat_rain::effect::{
    apply_message_overlay, EffectSmoother, GlyphSet, MessageOverlay, MessageTiming,
};
use stat_rain::health::HealthState;
use stat_rain::metrics::{MetricProvider, MetricRegistry, MetricStatus, MetricValue};
use stat_rain::protocol::{self, ProtocolMessage};
use stat_rain::system_metrics::BuiltinSystemProvider;
use stat_rain::terminal::{self, FrameRenderer, TerminalCapabilities, TerminalSize};
use stat_rain::text::{frames_from_ms, MessageQueue, QueuedMessage};
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
        self.metrics.insert(name.clone(), value.clone());
        if name == "cpu" {
            self.metrics.insert("cpu.total".to_string(), value);
        }
    }

    fn mark_stale(&mut self, name: String, reason: Option<String>) {
        let value = MetricValue {
            raw: None,
            normalized: Some(1.0),
            stale: true,
            status: MetricStatus::Stale { reason },
        };
        self.set(name, value);
    }

    fn mark_error(&mut self, name: String, reason: Option<String>) {
        let value = MetricValue {
            raw: None,
            normalized: Some(1.0),
            stale: true,
            status: MetricStatus::Error { reason },
        };
        self.set(name, value);
    }

    fn clear_status(&mut self, name: String) {
        clear_override_status(&mut self.metrics, &name);
        if name == "cpu" {
            clear_override_status(&mut self.metrics, "cpu.total");
        }
    }

    fn apply_to(&self, metrics: &mut MetricRegistry) {
        for (name, value) in &self.metrics {
            metrics.set(name.clone(), value.clone());
        }
    }
}

fn clear_override_status(metrics: &mut BTreeMap<String, MetricValue>, name: &str) {
    let remove = metrics.get(name).is_some_and(|value| {
        value.raw.is_none() && value.normalized == Some(1.0) && value.status != MetricStatus::Normal
    });
    if remove {
        metrics.remove(name);
    } else if let Some(value) = metrics.get_mut(name) {
        value.stale = false;
        value.status = MetricStatus::Normal;
    }
}

fn run(args: RunArgs) -> Result<()> {
    run_with_writer(args, &mut io::stdout())
}

pub fn run_with_writer(args: RunArgs, output: &mut impl Write) -> Result<()> {
    let config = load_config(&args)?;
    let mapped_metrics = config.referenced_metrics()?;
    let simulated_metrics = dev_tools::parse_simulated_metrics(&args.simulated_metrics)?;
    let message_timing = message_timing_from_args(&args);
    let mut metrics = initial_metric_registry();
    let mut external_overrides = ExternalMetricOverrides::default();
    let mut message_queue = MessageQueue::default();
    let mut active_message: Option<MessageOverlay> = None;
    let mut health_message: Option<MessageOverlay> = None;
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
            &mut message_queue,
            &mut active_message,
            message_timing,
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
        let health = HealthState::from_mapped_metrics(&metrics, &mapped_metrics);
        let mut frame = engine.step_with_health(state, &health);
        if active_message.is_none() {
            if let Some(queued) = message_queue.pop_next() {
                active_message = Some(message_overlay_from_queued(queued, message_timing));
            }
        }
        if let Some(message) = active_message.as_mut() {
            apply_message_overlay(&mut frame, message, state.message_reveal_intensity);
            message.advance();
            if message.is_expired() {
                active_message = None;
            }
        } else {
            apply_health_message(
                &mut frame,
                &mut health_message,
                &health,
                message_timing,
                state.message_reveal_intensity,
            );
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

fn apply_health_message(
    frame: &mut stat_rain::effect::Frame,
    health_message: &mut Option<MessageOverlay>,
    health: &HealthState,
    timing: MessageTiming,
    intensity: f64,
) {
    let Some(text) = health.message_text() else {
        *health_message = None;
        return;
    };

    let reset = health_message
        .as_ref()
        .map_or(true, |message| message.text != text);
    if reset {
        let mut overlay = MessageOverlay::new(text, timing.fade_in, u64::MAX / 4, 0, 0x4845_414c);
        overlay.class = if health.has_error() {
            stat_rain::message::MessageClass::Error
        } else {
            stat_rain::message::MessageClass::Warning
        };
        *health_message = Some(overlay);
    }

    if let Some(message) = health_message.as_mut() {
        apply_message_overlay(frame, message, intensity);
        if message.age < message.fade_in + 1 {
            message.advance();
        }
    }
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
    message_queue: &mut MessageQueue,
    active_message: &mut Option<MessageOverlay>,
    message_timing: MessageTiming,
) {
    let Some(socket_input) = socket_input else {
        return;
    };
    while let Ok(message) = socket_input.receiver.try_recv() {
        apply_protocol_message(
            metrics,
            overrides,
            message_queue,
            active_message,
            message_timing,
            message,
        );
    }
}

fn message_from_send_args(args: &SendArgs) -> Result<ProtocolMessage> {
    match (&args.metric, args.value, &args.message) {
        (Some(name), Some(value), None)
            if !args.stale && !args.error && !args.clear_status && args.reason.is_none() =>
        {
            if !(0.0..=1.0).contains(&value) || !value.is_finite() {
                bail!("--value must be a finite normalized value between 0.0 and 1.0");
            }
            Ok(ProtocolMessage::MetricUpdate {
                name: name.clone(),
                raw: None,
                normalized: Some(value),
            })
        }
        (Some(name), None, None) if args.stale && !args.error && !args.clear_status => {
            Ok(ProtocolMessage::MetricStale {
                name: name.clone(),
                reason: args.reason.clone(),
            })
        }
        (Some(name), None, None) if args.error && !args.stale && !args.clear_status => {
            Ok(ProtocolMessage::MetricError {
                name: name.clone(),
                reason: args.reason.clone(),
            })
        }
        (Some(name), None, None) if args.clear_status && !args.stale && !args.error => {
            Ok(ProtocolMessage::MetricStatusClear { name: name.clone() })
        }
        (None, None, Some(text)) => Ok(ProtocolMessage::TextInjection {
            text: text.clone(),
            class: args.class,
            ttl_ms: args.ttl_ms,
        }),
        (Some(_), None, None) => bail!("--metric requires --value"),
        _ => bail!("provide either --metric with --value, or --message"),
    }
}

fn apply_protocol_message(
    metrics: &mut MetricRegistry,
    overrides: &mut ExternalMetricOverrides,
    message_queue: &mut MessageQueue,
    active_message: &mut Option<MessageOverlay>,
    message_timing: MessageTiming,
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
        ProtocolMessage::MetricStale { name, reason } => {
            overrides.mark_stale(name, reason);
            overrides.apply_to(metrics);
        }
        ProtocolMessage::MetricError { name, reason } => {
            overrides.mark_error(name, reason);
            overrides.apply_to(metrics);
        }
        ProtocolMessage::MetricStatusClear { name } => {
            metrics.clear_status(name.clone());
            if name == "cpu" {
                metrics.clear_status("cpu.total");
            }
            overrides.clear_status(name);
            overrides.apply_to(metrics);
        }
        ProtocolMessage::TextInjection {
            text,
            class,
            ttl_ms,
        } => {
            let ttl_frames =
                ttl_ms.map(|ttl_ms| frames_from_ms(ttl_ms, message_timing.frame_delay));
            message_queue.enqueue_or_refresh(
                active_message,
                QueuedMessage::new(text, class, ttl_frames),
                message_timing,
            );
        }
    }
}

fn message_timing_from_args(args: &RunArgs) -> MessageTiming {
    MessageTiming {
        fade_in: frames_from_ms(args.message_fade_in_ms, args.frame_delay_ms),
        stay: frames_from_ms(args.message_stay_ms, args.frame_delay_ms),
        fade_out: frames_from_ms(args.message_wash_ms, args.frame_delay_ms),
        frame_delay: args.frame_delay_ms,
    }
}

fn message_overlay_from_queued(message: QueuedMessage, timing: MessageTiming) -> MessageOverlay {
    let mut overlay = MessageOverlay::new(
        message.text,
        timing.fade_in,
        message.ttl_frames.unwrap_or(timing.stay),
        timing.fade_out,
        0x5445_5854,
    );
    overlay.class = message.class;
    overlay
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
    use stat_rain::message::MessageClass;

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
            stale: false,
            error: false,
            reason: None,
            clear_status: false,
            message: None,
            class: MessageClass::Info,
            ttl_ms: None,
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
    fn send_message_args_default_to_info_class() {
        let args = stat_rain::cli::SendArgs {
            socket: "/tmp/stat-rain.sock".into(),
            metric: None,
            value: None,
            stale: false,
            error: false,
            reason: None,
            clear_status: false,
            message: Some("BUILD OK".to_string()),
            class: MessageClass::Info,
            ttl_ms: Some(10_000),
        };

        assert_eq!(
            message_from_send_args(&args).unwrap(),
            stat_rain::protocol::ProtocolMessage::TextInjection {
                text: "BUILD OK".to_string(),
                class: MessageClass::Info,
                ttl_ms: Some(10_000),
            }
        );
    }

    #[test]
    fn send_message_args_use_requested_error_class() {
        let args = stat_rain::cli::SendArgs {
            socket: "/tmp/stat-rain.sock".into(),
            metric: None,
            value: None,
            stale: false,
            error: false,
            reason: None,
            clear_status: false,
            message: Some("BUILD FAILED".to_string()),
            class: MessageClass::Error,
            ttl_ms: None,
        };

        assert_eq!(
            message_from_send_args(&args).unwrap(),
            stat_rain::protocol::ProtocolMessage::TextInjection {
                text: "BUILD FAILED".to_string(),
                class: MessageClass::Error,
                ttl_ms: None,
            }
        );
    }

    #[test]
    fn send_stale_args_create_metric_stale_message() {
        let args = stat_rain::cli::SendArgs {
            socket: "/tmp/stat-rain.sock".into(),
            metric: Some("thermal_zone".to_string()),
            value: None,
            stale: true,
            error: false,
            reason: Some("sensor timeout".to_string()),
            clear_status: false,
            message: None,
            class: MessageClass::Info,
            ttl_ms: None,
        };

        assert_eq!(
            message_from_send_args(&args).unwrap(),
            stat_rain::protocol::ProtocolMessage::MetricStale {
                name: "thermal_zone".to_string(),
                reason: Some("sensor timeout".to_string())
            }
        );
    }

    #[test]
    fn send_error_args_create_metric_error_message() {
        let args = stat_rain::cli::SendArgs {
            socket: "/tmp/stat-rain.sock".into(),
            metric: Some("thermal_zone".to_string()),
            value: None,
            stale: false,
            error: true,
            reason: Some("read failed".to_string()),
            clear_status: false,
            message: None,
            class: MessageClass::Info,
            ttl_ms: None,
        };

        assert_eq!(
            message_from_send_args(&args).unwrap(),
            stat_rain::protocol::ProtocolMessage::MetricError {
                name: "thermal_zone".to_string(),
                reason: Some("read failed".to_string())
            }
        );
    }

    #[test]
    fn send_clear_status_args_create_metric_clear_message() {
        let args = stat_rain::cli::SendArgs {
            socket: "/tmp/stat-rain.sock".into(),
            metric: Some("thermal_zone".to_string()),
            value: None,
            stale: false,
            error: false,
            reason: None,
            clear_status: true,
            message: None,
            class: MessageClass::Info,
            ttl_ms: None,
        };

        assert_eq!(
            message_from_send_args(&args).unwrap(),
            stat_rain::protocol::ProtocolMessage::MetricStatusClear {
                name: "thermal_zone".to_string()
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
        let timing = MessageTiming {
            fade_in: 1,
            stay: 1,
            fade_out: 1,
            frame_delay: 33,
        };

        apply_protocol_message(
            &mut metrics,
            &mut overrides,
            &mut MessageQueue::default(),
            &mut active_message,
            timing,
            message,
        );
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
            class: MessageClass::Warning,
            ttl_ms: None,
        };
        let timing = MessageTiming {
            fade_in: 12,
            stay: 34,
            fade_out: 56,
            frame_delay: 33,
        };
        let mut queue = MessageQueue::default();

        apply_protocol_message(
            &mut metrics,
            &mut overrides,
            &mut queue,
            &mut active_message,
            timing,
            message,
        );

        let active_message = message_overlay_from_queued(queue.pop_next().unwrap(), timing);
        assert_eq!(active_message.text, "BUILD OK");
        assert_eq!(active_message.class, MessageClass::Warning);
        assert_eq!(active_message.fade_in, 12);
        assert_eq!(active_message.stay, 34);
        assert_eq!(active_message.fade_out, 56);
    }

    #[test]
    fn message_timing_uses_milliseconds_from_run_args() {
        let args = stat_rain::cli::RunArgs {
            config: None,
            profile: None,
            config_inline: None,
            mappings: Vec::new(),
            simulated_metrics: Vec::new(),
            socket: None,
            frames: None,
            frame_delay_ms: 33,
            metric_sample_ms: 1_000,
            effect_smoothing_ms: 4_000,
            message_fade_in_ms: 100,
            message_stay_ms: 300,
            message_wash_ms: 500,
            width: None,
            height: None,
            no_alt_screen: false,
            ascii: false,
        };

        let timing = message_timing_from_args(&args);

        assert_eq!(timing.fade_in, 4);
        assert_eq!(timing.stay, 10);
        assert_eq!(timing.fade_out, 16);
        assert_eq!(timing.frame_delay, 33);
    }

    #[test]
    fn health_message_persists_and_uses_error_class() {
        let mut frame = stat_rain::effect::Frame {
            width: 24,
            height: 5,
            cells: vec![
                stat_rain::effect::RenderCell {
                    glyph: ' ',
                    color_hotness_bucket: 0,
                    message_color_bucket: 0,
                    brightness_bucket: 0,
                    head_brightness_bucket: 0,
                    ember_brightness_bucket: 0,
                    ember_color_hotness_bucket: 0,
                    health_degraded: false,
                    error_tint_bucket: 0,
                };
                120
            ],
        };
        let mut health_message = None;
        let health = HealthState {
            stale_metrics: vec!["cpu".to_string()],
            error_metrics: vec!["thermal_zone".to_string()],
        };
        let timing = MessageTiming {
            fade_in: 0,
            stay: 1,
            fade_out: 1,
            frame_delay: 33,
        };

        apply_health_message(&mut frame, &mut health_message, &health, timing, 0.0);

        let message = health_message.unwrap();
        assert_eq!(message.text, "ERROR: thermal_zone  STALE: cpu");
        assert_eq!(message.class, MessageClass::Error);
        assert!(!message.is_expired());
    }
}

fn sample_builtin_metrics(provider: &mut impl MetricProvider, metrics: &mut MetricRegistry) {
    match provider.sample() {
        Ok(sample) => metrics.apply_sample(sample),
        Err(_) => {
            metrics.mark_stale("cpu", None);
            metrics.mark_stale("cpu.total", None);
            metrics.mark_stale("memory", None);
        }
    }
}

fn initial_metric_registry() -> MetricRegistry {
    let mut metrics = MetricRegistry::default();
    metrics.set("cpu", MetricValue::new(Some(0.0), Some(0.0)));
    metrics.set("cpu.total", MetricValue::new(Some(0.0), Some(0.0)));
    metrics.set("memory", MetricValue::new(Some(0.0), Some(0.0)));
    metrics.set("disk_io", MetricValue::new(Some(0.0), Some(0.0)));
    metrics.set("network_io", MetricValue::new(Some(0.0), Some(0.0)));
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
