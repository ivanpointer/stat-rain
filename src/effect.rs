use crate::message::MessageClass;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EffectState {
    pub speed: f64,
    pub density: f64,
    pub color_hotness: f64,
    pub brightness: f64,
    pub fade_length: f64,
    pub glyph_churn: f64,
    pub message_reveal_intensity: f64,
    pub ember_density: f64,
    pub ember_brightness: f64,
    pub ember_color_hotness: f64,
    pub ember_fade_length: f64,
    pub glyph_set: GlyphSet,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GlyphSet {
    Unicode,
    Ascii,
}

#[derive(Debug, Clone)]
pub struct EffectSmoother {
    current: Option<EffectState>,
    transition_start: Option<EffectState>,
    target: Option<EffectState>,
    elapsed: std::time::Duration,
    window: std::time::Duration,
}

impl EffectSmoother {
    pub fn new(window: std::time::Duration) -> Self {
        Self {
            current: None,
            transition_start: None,
            target: None,
            elapsed: std::time::Duration::ZERO,
            window,
        }
    }

    pub fn update(&mut self, target: EffectState, elapsed: std::time::Duration) -> EffectState {
        let Some(current) = self.current else {
            self.current = Some(target);
            self.transition_start = Some(target);
            self.target = Some(target);
            return target;
        };

        if self.window.is_zero() {
            self.current = Some(target);
            self.transition_start = Some(target);
            self.target = Some(target);
            self.elapsed = std::time::Duration::ZERO;
            return target;
        }

        if self.target != Some(target) {
            self.transition_start = Some(current);
            self.target = Some(target);
            self.elapsed = std::time::Duration::ZERO;
        }

        self.elapsed = self.elapsed.saturating_add(elapsed);
        let factor = (self.elapsed.as_secs_f64() / self.window.as_secs_f64()).clamp(0.0, 1.0);
        let start = self.transition_start.unwrap_or(current);
        let smoothed = EffectState {
            speed: lerp(start.speed, target.speed, factor),
            density: lerp(start.density, target.density, factor),
            color_hotness: lerp(start.color_hotness, target.color_hotness, factor),
            brightness: lerp(start.brightness, target.brightness, factor),
            fade_length: lerp(start.fade_length, target.fade_length, factor),
            glyph_churn: lerp(start.glyph_churn, target.glyph_churn, factor),
            message_reveal_intensity: lerp(
                start.message_reveal_intensity,
                target.message_reveal_intensity,
                factor,
            ),
            ember_density: lerp(start.ember_density, target.ember_density, factor),
            ember_brightness: lerp(start.ember_brightness, target.ember_brightness, factor),
            ember_color_hotness: lerp(
                start.ember_color_hotness,
                target.ember_color_hotness,
                factor,
            ),
            ember_fade_length: lerp(start.ember_fade_length, target.ember_fade_length, factor),
            glyph_set: target.glyph_set,
        };
        self.current = Some(smoothed);
        smoothed
    }
}

fn lerp(current: f64, target: f64, factor: f64) -> f64 {
    current + (target - current) * factor
}

impl Default for EffectState {
    fn default() -> Self {
        Self {
            speed: 1.0,
            density: 0.35,
            color_hotness: 0.0,
            brightness: 0.8,
            fade_length: 14.0,
            glyph_churn: 0.25,
            message_reveal_intensity: 0.0,
            ember_density: 0.0015,
            ember_brightness: 0.9,
            ember_color_hotness: 0.0,
            ember_fade_length: 80.0,
            glyph_set: GlyphSet::Unicode,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderCell {
    pub glyph: char,
    pub color_hotness_bucket: u8,
    pub message_color_bucket: u8,
    pub brightness_bucket: u8,
    pub head_brightness_bucket: u8,
    pub ember_brightness_bucket: u8,
    pub ember_color_hotness_bucket: u8,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Frame {
    pub width: usize,
    pub height: usize,
    pub cells: Vec<RenderCell>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MessageOverlay {
    pub text: String,
    pub age: u64,
    pub fade_in: u64,
    pub stay: u64,
    pub fade_out: u64,
    pub class: MessageClass,
    seed: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MessageTiming {
    pub fade_in: u64,
    pub stay: u64,
    pub fade_out: u64,
}

impl MessageOverlay {
    pub fn new(text: String, fade_in: u64, stay: u64, fade_out: u64, seed: u64) -> Self {
        Self {
            text,
            age: 0,
            fade_in,
            stay,
            fade_out,
            class: MessageClass::Info,
            seed,
        }
    }

    pub fn advance(&mut self) {
        self.age = self.age.saturating_add(1);
    }

    pub fn is_expired(&self) -> bool {
        self.age >= self.lifetime()
    }

    fn lifetime(&self) -> u64 {
        self.fade_in + self.stay + self.fade_out.max(1) + self.fade_out / 2 + self.fade_out / 5
    }
}

pub fn apply_message_overlay(frame: &mut Frame, message: &MessageOverlay, intensity: f64) {
    if frame.width == 0 || frame.height == 0 || message.is_expired() {
        return;
    }

    let visible_chars = message.text.chars().take(frame.width).collect::<Vec<_>>();
    if visible_chars.is_empty() {
        return;
    }

    let row = frame.height / 2;
    let start_x = frame.width.saturating_sub(visible_chars.len()) / 2;

    for (offset, glyph) in visible_chars.into_iter().enumerate() {
        if glyph == ' ' {
            continue;
        }
        let char_hash = mix_hash(message.seed ^ (offset as u64).wrapping_mul(0x9e37_79b9));
        let Some((glyph, brightness_bucket)) =
            message_glyph_and_brightness(message, offset, glyph, char_hash, intensity)
        else {
            continue;
        };
        let index = row * frame.width + start_x + offset;
        let cell = &mut frame.cells[index];
        cell.glyph = glyph;
        cell.brightness_bucket = cell.brightness_bucket.max(brightness_bucket);
        cell.head_brightness_bucket = cell.head_brightness_bucket.max(brightness_bucket);
        cell.color_hotness_bucket = cell
            .color_hotness_bucket
            .max(message.class.color_hotness_bucket());
        cell.message_color_bucket = message.class.color_bucket();
        cell.ember_brightness_bucket = 0;
    }
}

fn message_glyph_and_brightness(
    message: &MessageOverlay,
    offset: usize,
    target: char,
    char_hash: u64,
    intensity: f64,
) -> Option<(char, u8)> {
    let reveal_at = if message.fade_in == 0 {
        0
    } else {
        char_hash % message.fade_in
    };
    let max_brightness = message.class.brightness_floor() + intensity.clamp(0.0, 1.0) * 0.28;
    let max_brightness = max_brightness.min(1.0);

    if message.age < message.fade_in {
        let brightness =
            ((message.age + 1) as f64 / message.fade_in.max(1) as f64) * max_brightness;
        let glyph = if message.age >= reveal_at {
            target
        } else {
            message_noise_glyph(offset, message.age, char_hash)
        };
        return Some((glyph, bucket(brightness)));
    }

    let stay_end = message.fade_in + message.stay;
    if message.age < stay_end {
        return Some((target, bucket(max_brightness)));
    }

    let fade_delay = message_fade_delay(message.fade_out, char_hash);
    let fade_age = message.age.saturating_sub(stay_end);
    if fade_age < fade_delay {
        return Some((target, bucket(max_brightness)));
    }
    let fade_age = fade_age - fade_delay;
    let fade_length = jittered_message_fade(message.fade_out, char_hash);
    if fade_age >= fade_length {
        return None;
    }
    let brightness = (1.0 - fade_age as f64 / fade_length as f64) * max_brightness;
    let progress = fade_age as f64 / fade_length as f64;
    let glyph = if message_should_glitch(
        offset,
        message.age,
        char_hash,
        progress,
        message.class.glitch_boost(),
    ) {
        message_noise_glyph(offset, message.age, char_hash)
    } else {
        target
    };
    Some((glyph, bucket(brightness)))
}

fn jittered_message_fade(base: u64, hash: u64) -> u64 {
    let base = base.max(1);
    base + (mix_hash(hash) % (base / 2 + 1))
}

fn message_fade_delay(base: u64, hash: u64) -> u64 {
    if base <= 1 {
        return 0;
    }
    mix_hash(hash ^ 0xfeed_fade) % (base / 5 + 1)
}

fn message_should_glitch(offset: usize, age: u64, hash: u64, progress: f64, boost: f64) -> bool {
    if progress < 0.15 {
        return false;
    }
    let roll = mix_hash(hash ^ age.wrapping_mul(17) ^ (offset as u64).wrapping_mul(31)) % 100;
    let threshold = ((18.0 + progress.clamp(0.0, 1.0) * 55.0) * boost).min(96.0) as u64;
    roll < threshold
}

fn message_noise_glyph(offset: usize, age: u64, hash: u64) -> char {
    const GLYPHS: &[char] = &[
        '0', '1', '3', '7', '9', 'a', 'b', 'x', 'z', '+', '-', '*', '/', '|', ':', '.', '=', '#',
    ];
    GLYPHS[(hash.wrapping_add(age * 13).wrapping_add(offset as u64 * 7) as usize) % GLYPHS.len()]
}

#[derive(Debug, Clone)]
pub struct RainEngine {
    width: usize,
    height: usize,
    tick: u64,
    columns: Vec<ColumnState>,
}

const RAIN_TRACES_PER_COLUMN: usize = 3;

impl RainEngine {
    pub fn new(width: usize, height: usize, seed: u64) -> Self {
        let mut rng = Lcg::new(seed);
        let columns = (0..width)
            .map(|_| {
                let seed = rng.next();
                ColumnState {
                    seed,
                    traces: [
                        TraceState {
                            phase: rng.next_usize(height.max(1)) as f64,
                            speed_scale: 0.65 + rng.next_usize(70) as f64 / 100.0,
                            density_scale: 1.0,
                            seed: rng.next(),
                        },
                        TraceState {
                            phase: rng.next_usize(height.max(1)) as f64,
                            speed_scale: 0.95 + rng.next_usize(90) as f64 / 100.0,
                            density_scale: 0.35,
                            seed: rng.next(),
                        },
                        TraceState {
                            phase: rng.next_usize(height.max(1)) as f64,
                            speed_scale: 0.45 + rng.next_usize(120) as f64 / 100.0,
                            density_scale: 0.18,
                            seed: rng.next(),
                        },
                    ],
                }
            })
            .collect::<Vec<_>>();

        Self {
            width,
            height,
            tick: 0,
            columns,
        }
    }

    pub fn step(&mut self, state: EffectState) -> Frame {
        let mut cells = Vec::with_capacity(self.width * self.height);
        let speed = state.speed.max(0.0);
        let density = state.density.clamp(0.0, 1.0);
        let fade_length = state.fade_length.max(1.0);
        let hotness = bucket(state.color_hotness);
        let glyph_churn = state.glyph_churn.clamp(0.0, 1.0);

        for y in 0..self.height {
            for x in 0..self.width {
                let column = &self.columns[x];
                let mut best_trail = 0.0;
                let mut best_trace_seed = column.seed;
                let mut best_trace_speed = 1.0;
                let mut rain_head = false;
                for trace in &column.traces {
                    if density * trace.density_scale < self.trace_noise(x, trace) {
                        continue;
                    }
                    let head = trace.phase as usize % self.height.max(1);
                    let distance = if y <= head {
                        head - y
                    } else {
                        self.height + head - y
                    };
                    let trace_fade_length = trace_fade_length(fade_length, trace.speed_scale);
                    let trail = (1.0 - distance as f64 / trace_fade_length).clamp(0.0, 1.0);
                    if distance == 0 {
                        rain_head = true;
                    }
                    if trail > best_trail {
                        best_trail = trail;
                        best_trace_seed = trace.seed;
                        best_trace_speed = trace.speed_scale;
                    }
                }
                let rain_brightness = best_trail * state.brightness;
                let ember_brightness = if rain_brightness == 0.0 && state.ember_density > 0.0 {
                    self.ember_brightness(
                        x,
                        y,
                        state.ember_density,
                        state.ember_brightness,
                        state.ember_fade_length,
                    )
                } else {
                    0.0
                };
                let brightness = rain_brightness.max(ember_brightness);
                let brightness_bucket = bucket(brightness);
                let head_brightness_bucket = if rain_head {
                    bucket(state.brightness)
                } else {
                    0
                };
                let glyph = if brightness_bucket == 0 {
                    ' '
                } else if ember_brightness > 0.0 && rain_brightness == 0.0 {
                    self.ember_glyph_for(x, y, state.ember_density, state.glyph_set)
                } else {
                    let local_churn = tail_glyph_churn(glyph_churn, best_trace_speed, best_trail);
                    self.glyph_for(x, y, local_churn, state.glyph_set, best_trace_seed)
                };

                cells.push(RenderCell {
                    glyph,
                    color_hotness_bucket: hotness,
                    message_color_bucket: 0,
                    brightness_bucket,
                    head_brightness_bucket,
                    ember_brightness_bucket: bucket(ember_brightness),
                    ember_color_hotness_bucket: bucket(state.ember_color_hotness),
                });
            }
        }

        self.tick = self.tick.wrapping_add(1);
        for column in &mut self.columns {
            for trace in &mut column.traces {
                let height = self.height.max(1) as f64;
                let next_phase = trace.phase + speed * trace.speed_scale;
                if next_phase >= height {
                    trace.seed = mix_hash(trace.seed ^ self.tick ^ self.height as u64);
                    trace.speed_scale = trace_speed_scale(trace.seed);
                }
                trace.phase = next_phase % height;
            }
        }

        Frame {
            width: self.width,
            height: self.height,
            cells,
        }
    }

    fn trace_noise(&self, x: usize, trace: &TraceState) -> f64 {
        (((x as u64 * 1_103_515_245 + trace.seed) >> 16) & 0xff) as f64 / 255.0
    }

    fn glyph_for(
        &self,
        x: usize,
        y: usize,
        glyph_churn: f64,
        glyph_set: GlyphSet,
        seed: u64,
    ) -> char {
        const UNICODE_GLYPHS: &[char] = &[
            '0', '1', '3', '7', '9', 'a', 'b', 'x', 'z', 'ﾊ', 'ﾐ', 'ﾋ', 'ｰ', 'ｳ', 'ｼ', 'ﾅ', 'ﾓ',
            'ﾆ',
        ];
        const ASCII_GLYPHS: &[char] = &[
            '0', '1', '3', '7', '9', 'a', 'b', 'x', 'z', '+', '-', '*', '/', '|', ':', '.', '=',
            '#',
        ];
        let glyphs = match glyph_set {
            GlyphSet::Unicode => UNICODE_GLYPHS,
            GlyphSet::Ascii => ASCII_GLYPHS,
        };

        let churn = (glyph_churn * 32.0) as u64;
        let index =
            (x as u64 * 17 + y as u64 * 31 + self.tick * churn + seed) as usize % glyphs.len();
        glyphs[index]
    }

    fn ember_glyph_for(&self, x: usize, y: usize, density: f64, glyph_set: GlyphSet) -> char {
        const UNICODE_GLYPHS: &[char] = &[
            '0', '1', '3', '7', '9', 'a', 'b', 'x', 'z', 'ﾊ', 'ﾐ', 'ﾋ', 'ｰ', 'ｳ', 'ｼ', 'ﾅ', 'ﾓ',
            'ﾆ',
        ];
        const ASCII_GLYPHS: &[char] = &[
            '0', '1', '3', '7', '9', 'a', 'b', 'x', 'z', '+', '-', '*', '/', '|', ':', '.', '=',
            '#',
        ];
        let glyphs = match glyph_set {
            GlyphSet::Unicode => UNICODE_GLYPHS,
            GlyphSet::Ascii => ASCII_GLYPHS,
        };

        let hash = self.cell_hash(x, y);
        let age = self.ember_age(x, y, density).unwrap_or(0);
        let period = ember_period(density);
        let phase = if mix_hash(hash / period) & 1 == 0 {
            0
        } else {
            age / 24
        };
        glyphs[hash.wrapping_add(phase.wrapping_mul(7)) as usize % glyphs.len()]
    }

    fn ember_brightness(
        &self,
        x: usize,
        y: usize,
        density: f64,
        brightness: f64,
        fade_length: f64,
    ) -> f64 {
        let age = self.ember_age(x, y, density).unwrap_or(0);
        let fade_length = self.ember_fade_length(x, y, fade_length);
        if age as f64 >= fade_length {
            return 0.0;
        }
        let fade = 1.0 - age as f64 / fade_length;
        fade * brightness.clamp(0.0, 1.0)
    }

    fn ember_fade_length(&self, x: usize, y: usize, fade_length: f64) -> f64 {
        let jitter = (mix_hash(self.cell_hash(x, y)) % 1024) as f64 / 3072.0;
        (fade_length * (1.0 + jitter)).max(1.0)
    }

    fn ember_age(&self, x: usize, y: usize, density: f64) -> Option<u64> {
        if density <= 0.0 {
            return None;
        }
        let hash = self.cell_hash(x, y);
        let period = ember_period(density);
        let event_tick = hash % period;
        Some(self.tick.wrapping_sub(event_tick) % period)
    }

    fn cell_hash(&self, x: usize, y: usize) -> u64 {
        self.columns[x].seed
            ^ (x as u64).wrapping_mul(0x9e37_79b9_7f4a_7c15)
            ^ (y as u64).wrapping_mul(0xbf58_476d_1ce4_e5b9)
    }
}

fn ember_period(density: f64) -> u64 {
    let density = density.clamp(0.0, 1.0);
    (16384.0 - density * 16256.0).round().max(128.0) as u64
}

fn mix_hash(value: u64) -> u64 {
    let mut mixed = value;
    mixed ^= mixed >> 30;
    mixed = mixed.wrapping_mul(0xbf58_476d_1ce4_e5b9);
    mixed ^= mixed >> 27;
    mixed = mixed.wrapping_mul(0x94d0_49bb_1331_11eb);
    mixed ^ (mixed >> 31)
}

fn trace_speed_scale(seed: u64) -> f64 {
    0.45 + (mix_hash(seed) % 140) as f64 / 100.0
}

fn trace_fade_length(base_fade_length: f64, speed_scale: f64) -> f64 {
    let speed_factor = ((speed_scale - 0.45) / 1.4).clamp(0.0, 1.0);
    base_fade_length.max(1.0) * (0.6 + speed_factor * 0.55)
}

fn tail_glyph_churn(base_churn: f64, speed_scale: f64, trail: f64) -> f64 {
    let speed_factor = ((speed_scale - 0.45) / 1.4).clamp(0.0, 1.0);
    let freshness = trail.clamp(0.0, 1.0);
    (base_churn * 0.35 + speed_factor * 0.35 + freshness * 0.55).clamp(0.0, 1.0)
}

#[derive(Debug, Clone)]
struct ColumnState {
    seed: u64,
    traces: [TraceState; RAIN_TRACES_PER_COLUMN],
}

#[derive(Debug, Clone)]
struct TraceState {
    phase: f64,
    speed_scale: f64,
    density_scale: f64,
    seed: u64,
}

#[derive(Debug, Clone)]
struct Lcg {
    seed: u64,
}

impl Lcg {
    fn new(seed: u64) -> Self {
        Self { seed }
    }

    fn next(&mut self) -> u64 {
        self.seed = self
            .seed
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1);
        self.seed
    }

    fn next_usize(&mut self, max: usize) -> usize {
        if max == 0 {
            0
        } else {
            (self.next() as usize) % max
        }
    }
}

fn bucket(value: f64) -> u8 {
    (value.clamp(0.0, 1.0) * 255.0) as u8
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::message::MessageClass;
    use std::time::Duration;

    #[test]
    fn engine_generates_one_cell_per_position() {
        let mut engine = RainEngine::new(8, 4, 7);

        let frame = engine.step(EffectState::default());

        assert_eq!(frame.width, 8);
        assert_eq!(frame.height, 4);
        assert_eq!(frame.cells.len(), 32);
    }

    #[test]
    fn engine_is_deterministic_for_same_seed() {
        let mut first = RainEngine::new(10, 5, 42);
        let mut second = RainEngine::new(10, 5, 42);

        let first_frame = first.step(EffectState::default());
        let second_frame = second.step(EffectState::default());

        assert_eq!(first_frame, second_frame);
    }

    #[test]
    fn inactive_background_cells_are_spaces() {
        let mut engine = RainEngine::new(8, 4, 7);
        let state = EffectState {
            density: 0.0,
            ember_density: 0.0,
            ..EffectState::default()
        };

        let frame = engine.step(state);

        assert!(frame.cells.iter().all(|cell| cell.glyph == ' '));
    }

    #[test]
    fn active_rain_includes_stationary_pop_glyphs_outside_trails() {
        let mut engine = RainEngine::new(24, 12, 7);
        let state = EffectState {
            density: 1.0,
            ember_density: 1.0,
            ember_fade_length: 4.0,
            fade_length: 1.0,
            brightness: 1.0,
            speed: 0.0,
            ..EffectState::default()
        };

        let frame = engine.step(state);
        let pop_glyphs = frame
            .cells
            .iter()
            .filter(|cell| cell.ember_brightness_bucket > 0)
            .count();

        assert!(pop_glyphs > 0);
        assert!(pop_glyphs < frame.cells.len() / 12);
    }

    #[test]
    fn default_embers_are_rare_and_slow_fading() {
        let state = EffectState::default();

        assert_eq!(state.ember_density, 0.0015);
        assert_eq!(state.ember_fade_length, 80.0);
    }

    #[test]
    fn ember_lifetime_has_per_cell_jitter() {
        let mut engine = RainEngine::new(160, 40, 7);
        let state = EffectState {
            density: 0.0,
            ember_density: 1.0,
            ember_fade_length: 80.0,
            ember_brightness: 1.0,
            speed: 0.0,
            ..EffectState::default()
        };

        let first_frame = engine.step(state);
        let ember_indices = first_frame
            .cells
            .iter()
            .enumerate()
            .filter_map(|(index, cell)| (cell.ember_brightness_bucket == 255).then_some(index))
            .collect::<Vec<_>>();
        assert!(ember_indices.len() >= 12);

        let mut lifetimes = vec![1; ember_indices.len()];
        for _ in 0..90 {
            let frame = engine.step(state);
            for (ember_index, cell_index) in ember_indices.iter().enumerate() {
                if frame.cells[*cell_index].ember_brightness_bucket > 0 {
                    lifetimes[ember_index] += 1;
                }
            }
        }

        assert!(lifetimes
            .iter()
            .all(|lifetime| (80..=107).contains(lifetime)));
        assert!(
            lifetimes.iter().min() != lifetimes.iter().max(),
            "expected jittered lifetimes, got {lifetimes:?}"
        );
    }

    #[test]
    fn default_rain_trail_fades_slowly_without_filling_column() {
        let mut engine = RainEngine::new(1, 24, 7);
        let state = EffectState {
            density: 1.0,
            ember_density: 0.0,
            speed: 0.0,
            ..EffectState::default()
        };

        let frame = engine.step(state);
        let visible_cells = frame
            .cells
            .iter()
            .filter(|cell| cell.brightness_bucket > 0)
            .count();

        assert!(
            (8..20).contains(&visible_cells),
            "visible cells: {visible_cells}"
        );
    }

    #[test]
    fn rain_allows_multiple_drops_in_one_column() {
        let mut engine = RainEngine::new(1, 24, 7);
        let state = EffectState {
            density: 1.0,
            ember_density: 0.0,
            speed: 0.0,
            ..EffectState::default()
        };

        let frame = engine.step(state);
        let head_cells = frame
            .cells
            .iter()
            .filter(|cell| cell.head_brightness_bucket > 0)
            .count();

        assert!(head_cells > 1);
    }

    #[test]
    fn rain_traces_reseed_after_wrapping() {
        let mut engine = RainEngine::new(1, 12, 7);
        let first_seeds = engine.columns[0]
            .traces
            .iter()
            .map(|trace| trace.seed)
            .collect::<Vec<_>>();
        let state = EffectState {
            density: 1.0,
            ember_density: 0.0,
            speed: 6.0,
            ..EffectState::default()
        };

        for _ in 0..8 {
            engine.step(state);
        }

        let later_seeds = engine.columns[0]
            .traces
            .iter()
            .map(|trace| trace.seed)
            .collect::<Vec<_>>();
        assert_ne!(first_seeds, later_seeds);
    }

    #[test]
    fn faster_traces_have_longer_tails() {
        let base_fade = 14.0;

        assert!(trace_fade_length(base_fade, 1.6) > trace_fade_length(base_fade, 0.5));
        assert!(trace_fade_length(base_fade, 1.6) <= base_fade * 1.5);
        assert!(trace_fade_length(base_fade, 0.5) >= base_fade * 0.6);
    }

    #[test]
    fn tail_glyph_churn_falls_as_tail_fades() {
        let base_churn = 0.25;

        let head_churn = tail_glyph_churn(base_churn, 1.5, 1.0);
        let dim_tail_churn = tail_glyph_churn(base_churn, 1.5, 0.15);
        let slow_head_churn = tail_glyph_churn(base_churn, 0.5, 1.0);

        assert!(head_churn > dim_tail_churn);
        assert!(head_churn > slow_head_churn);
    }

    #[test]
    fn about_half_of_ember_glyphs_never_change_during_full_fade_window() {
        let mut engine = RainEngine::new(120, 40, 7);
        let state = EffectState {
            density: 0.0,
            ember_density: 1.0,
            ember_fade_length: 60.0,
            ember_brightness: 1.0,
            glyph_churn: 1.0,
            speed: 0.0,
            glyph_set: GlyphSet::Ascii,
            ..EffectState::default()
        };

        let first_frame = engine.step(state);
        let ember_indices = first_frame
            .cells
            .iter()
            .enumerate()
            .filter_map(|(index, cell)| (cell.ember_brightness_bucket == 255).then_some(index))
            .collect::<Vec<_>>();
        assert!(ember_indices.len() >= 12);
        let mut previous_glyphs = ember_indices
            .iter()
            .map(|index| first_frame.cells[*index].glyph)
            .collect::<Vec<_>>();
        let mut changes = vec![0; ember_indices.len()];

        for _ in 0..39 {
            let frame = engine.step(state);
            for (ember_index, cell_index) in ember_indices.iter().enumerate() {
                let cell = &frame.cells[*cell_index];
                if cell.ember_brightness_bucket == 0 {
                    continue;
                }
                if cell.glyph != previous_glyphs[ember_index] {
                    changes[ember_index] += 1;
                    previous_glyphs[ember_index] = cell.glyph;
                }
            }
        }

        let unchanged = changes.iter().filter(|count| **count == 0).count();
        let changed = changes.iter().filter(|count| **count > 0).count();

        assert!(unchanged.abs_diff(changed) <= ember_indices.len() / 8);
        assert!(
            changes.iter().all(|count| *count <= 1),
            "ember glyph changes were {changes:?}"
        );
    }

    #[test]
    fn stationary_pop_glyphs_do_not_form_row_bands() {
        let mut engine = RainEngine::new(40, 12, 7);
        let state = EffectState {
            density: 1.0,
            fade_length: 1.0,
            brightness: 1.0,
            speed: 0.0,
            ..EffectState::default()
        };

        let frame = engine.step(state);
        let max_row_pop_glyphs = frame
            .cells
            .chunks(frame.width)
            .map(|row| {
                row.iter()
                    .filter(|cell| cell.ember_brightness_bucket > 0)
                    .count()
            })
            .max()
            .unwrap();

        assert!(max_row_pop_glyphs <= 4);
    }

    #[test]
    fn ascii_mode_emits_only_ascii_visible_glyphs() {
        let mut engine = RainEngine::new(8, 4, 7);
        let state = EffectState {
            density: 1.0,
            glyph_set: GlyphSet::Ascii,
            ..EffectState::default()
        };

        let frame = engine.step(state);

        assert!(frame
            .cells
            .iter()
            .filter(|cell| cell.glyph != ' ')
            .all(|cell| cell.glyph.is_ascii()));
    }

    #[test]
    fn rain_head_is_marked_separately_from_trail() {
        let mut engine = RainEngine::new(1, 4, 7);
        let state = EffectState {
            density: 1.0,
            fade_length: 4.0,
            brightness: 1.0,
            speed: 0.0,
            ..EffectState::default()
        };

        let frame = engine.step(state);
        let head_cells = frame
            .cells
            .iter()
            .filter(|cell| cell.head_brightness_bucket > 0)
            .count();
        let trail_cells = frame
            .cells
            .iter()
            .filter(|cell| cell.brightness_bucket > 0 && cell.head_brightness_bucket == 0)
            .count();

        assert!(head_cells >= 1);
        assert!(trail_cells > 0);
    }

    #[test]
    fn rain_head_glyph_flickers_even_with_low_global_churn() {
        let mut engine = RainEngine::new(1, 4, 7);
        let state = EffectState {
            density: 1.0,
            fade_length: 1.0,
            brightness: 1.0,
            speed: 0.0,
            glyph_churn: 0.0,
            ..EffectState::default()
        };

        let first = engine.step(state);
        let second = engine.step(state);

        assert_ne!(first.cells[0].glyph, second.cells[0].glyph);
    }

    #[test]
    fn message_overlay_writes_text_into_frame() {
        let mut frame = Frame {
            width: 12,
            height: 5,
            cells: vec![blank_cell(); 60],
        };
        let message = MessageOverlay::new("ALERT".to_string(), 0, 120, 0, 7);

        apply_message_overlay(&mut frame, &message, 0.0);

        let rendered = frame
            .cells
            .iter()
            .filter(|cell| cell.head_brightness_bucket > 0)
            .map(|cell| cell.glyph)
            .collect::<String>();
        assert_eq!(rendered, "ALERT");
    }

    #[test]
    fn message_overlay_centers_text_vertically() {
        let mut frame = Frame {
            width: 13,
            height: 5,
            cells: vec![blank_cell(); 65],
        };
        let message = MessageOverlay::new("HEY".to_string(), 0, 120, 0, 7);

        apply_message_overlay(&mut frame, &message, 0.0);

        let message_indices = frame
            .cells
            .iter()
            .enumerate()
            .filter_map(|(index, cell)| (cell.head_brightness_bucket > 0).then_some(index))
            .collect::<Vec<_>>();
        assert_eq!(message_indices, vec![31, 32, 33]);
    }

    #[test]
    fn message_overlay_reveals_letters_in_random_order() {
        let mut early_frame = Frame {
            width: 12,
            height: 5,
            cells: vec![blank_cell(); 60],
        };
        let mut stay_frame = early_frame.clone();
        let early = MessageOverlay {
            age: 8,
            ..MessageOverlay::new("ALERT".to_string(), 30, 60, 30, 7)
        };
        let stay = MessageOverlay {
            age: 31,
            ..early.clone()
        };

        apply_message_overlay(&mut early_frame, &early, 0.0);
        apply_message_overlay(&mut stay_frame, &stay, 0.0);

        let early_rendered = rendered_message_chars(&early_frame);
        let stay_rendered = rendered_message_chars(&stay_frame);
        assert_ne!(early_rendered, "ALERT");
        assert_eq!(stay_rendered, "ALERT");
        assert!(early_rendered.chars().any(|glyph| !"ALERT".contains(glyph)));
    }

    #[test]
    fn message_overlay_fades_out_with_per_character_jitter() {
        let mut frame = Frame {
            width: 12,
            height: 5,
            cells: vec![blank_cell(); 60],
        };
        let message = MessageOverlay {
            age: 105,
            ..MessageOverlay::new("ALERT".to_string(), 0, 60, 60, 7)
        };

        apply_message_overlay(&mut frame, &message, 0.0);

        let brightnesses = frame
            .cells
            .iter()
            .filter(|cell| cell.head_brightness_bucket > 0)
            .map(|cell| cell.head_brightness_bucket)
            .collect::<Vec<_>>();
        assert!(brightnesses.len() > 1);
        assert!(brightnesses.iter().min() != brightnesses.iter().max());
    }

    #[test]
    fn message_overlay_staggers_fade_out_start_by_character() {
        let mut frame = Frame {
            width: 12,
            height: 5,
            cells: vec![blank_cell(); 60],
        };
        let message = MessageOverlay {
            age: 68,
            ..MessageOverlay::new("ALERT".to_string(), 0, 60, 60, 7)
        };

        apply_message_overlay(&mut frame, &message, 0.0);

        let brightnesses = frame
            .cells
            .iter()
            .filter(|cell| cell.head_brightness_bucket > 0)
            .map(|cell| cell.head_brightness_bucket)
            .collect::<Vec<_>>();
        assert!(brightnesses
            .iter()
            .any(|brightness| *brightness == bucket(0.72)));
        assert!(brightnesses
            .iter()
            .any(|brightness| *brightness < bucket(0.72)));
    }

    #[test]
    fn message_overlay_glitches_some_letters_during_fade_out() {
        let mut frame = Frame {
            width: 12,
            height: 5,
            cells: vec![blank_cell(); 60],
        };
        let message = MessageOverlay {
            age: 115,
            ..MessageOverlay::new("ALERTALERT".to_string(), 0, 60, 60, 7)
        };

        apply_message_overlay(&mut frame, &message, 0.0);

        let rendered = rendered_message_chars(&frame);
        assert_ne!(rendered, "ALERTALERT");
    }

    #[test]
    fn message_overlay_applies_class_hotness() {
        let mut frame = Frame {
            width: 12,
            height: 5,
            cells: vec![blank_cell(); 60],
        };
        let message = MessageOverlay {
            class: MessageClass::Error,
            ..MessageOverlay::new("ALERT".to_string(), 0, 120, 0, 7)
        };

        apply_message_overlay(&mut frame, &message, 0.0);

        let hotnesses = frame
            .cells
            .iter()
            .filter(|cell| cell.head_brightness_bucket > 0)
            .map(|cell| cell.color_hotness_bucket)
            .collect::<Vec<_>>();
        assert_eq!(hotnesses, vec![255, 255, 255, 255, 255]);
        let message_colors = frame
            .cells
            .iter()
            .filter(|cell| cell.head_brightness_bucket > 0)
            .map(|cell| cell.message_color_bucket)
            .collect::<Vec<_>>();
        assert_eq!(message_colors, vec![4, 4, 4, 4, 4]);
    }

    #[test]
    fn error_message_glitches_more_than_info_during_fade_out() {
        let info = MessageOverlay {
            age: 95,
            class: MessageClass::Info,
            ..MessageOverlay::new("ALERTALERTALERT".to_string(), 0, 60, 60, 7)
        };
        let error = MessageOverlay {
            class: MessageClass::Error,
            ..info.clone()
        };

        assert!(
            fade_out_glitch_count(&error) > fade_out_glitch_count(&info),
            "error messages should glitch more aggressively than info messages"
        );
    }

    #[test]
    fn column_heads_drift_independently_over_time() {
        let mut engine = RainEngine::new(2, 32, 7);
        let state = EffectState {
            density: 1.0,
            fade_length: 1.0,
            brightness: 1.0,
            speed: 1.0,
            ..EffectState::default()
        };

        let first = engine.step(state);
        for _ in 0..20 {
            engine.step(state);
        }
        let later = engine.step(state);

        assert_ne!(
            head_distance_between_columns(&first, 0, 1),
            head_distance_between_columns(&later, 0, 1)
        );
    }

    #[test]
    fn speed_changes_advance_from_current_position() {
        let mut engine = RainEngine::new(1, 100, 7);
        let slow = EffectState {
            density: 1.0,
            fade_length: 1.0,
            brightness: 1.0,
            speed: 1.0,
            ..EffectState::default()
        };
        let faster = EffectState { speed: 2.0, ..slow };

        for _ in 0..50 {
            engine.step(slow);
        }
        let before = brightest_row(&engine.step(slow));
        let after = brightest_row(&engine.step(faster));

        assert!(forward_distance(before, after, 100) <= 2);
    }

    #[test]
    fn smoother_returns_first_target_immediately() {
        let mut smoother = EffectSmoother::new(Duration::from_secs(2));
        let target = EffectState {
            speed: 5.0,
            density: 0.8,
            ..EffectState::default()
        };

        assert_eq!(smoother.update(target, Duration::from_millis(500)), target);
    }

    #[test]
    fn smoother_moves_numeric_fields_toward_target_over_window() {
        let mut smoother = EffectSmoother::new(Duration::from_secs(2));
        smoother.update(
            EffectState {
                speed: 1.0,
                density: 0.2,
                ..EffectState::default()
            },
            Duration::ZERO,
        );

        let smoothed = smoother.update(
            EffectState {
                speed: 9.0,
                density: 0.8,
                ..EffectState::default()
            },
            Duration::from_secs(1),
        );

        assert_eq!(smoothed.speed, 5.0);
        assert_eq!(smoothed.density, 0.5);
    }

    #[test]
    fn smoother_reaches_target_after_full_window() {
        let mut smoother = EffectSmoother::new(Duration::from_secs(2));
        smoother.update(EffectState::default(), Duration::ZERO);
        let target = EffectState {
            speed: 9.0,
            density: 0.8,
            ..EffectState::default()
        };

        assert_eq!(smoother.update(target, Duration::from_secs(2)), target);
    }

    #[test]
    fn smoother_reaches_target_after_incremental_window() {
        let mut smoother = EffectSmoother::new(Duration::from_secs(2));
        smoother.update(
            EffectState {
                speed: 1.0,
                ..EffectState::default()
            },
            Duration::ZERO,
        );
        let target = EffectState {
            speed: 9.0,
            ..EffectState::default()
        };

        for _ in 0..3 {
            let smoothed = smoother.update(target, Duration::from_millis(500));
            assert_ne!(smoothed.speed, 9.0);
        }

        assert_eq!(
            smoother.update(target, Duration::from_millis(500)).speed,
            9.0
        );
    }

    #[test]
    fn smoother_switches_glyph_set_immediately() {
        let mut smoother = EffectSmoother::new(Duration::from_secs(2));
        smoother.update(EffectState::default(), Duration::ZERO);

        let smoothed = smoother.update(
            EffectState {
                speed: 9.0,
                glyph_set: GlyphSet::Ascii,
                ..EffectState::default()
            },
            Duration::from_millis(1),
        );

        assert_eq!(smoothed.glyph_set, GlyphSet::Ascii);
        assert_ne!(smoothed.speed, 9.0);
    }

    fn brightest_row(frame: &Frame) -> usize {
        frame
            .cells
            .iter()
            .enumerate()
            .max_by_key(|(_, cell)| cell.brightness_bucket)
            .map(|(index, _)| index / frame.width)
            .unwrap()
    }

    fn head_distance_between_columns(frame: &Frame, left: usize, right: usize) -> usize {
        let left_head = head_row(frame, left);
        let right_head = head_row(frame, right);
        forward_distance(left_head, right_head, frame.height)
    }

    fn head_row(frame: &Frame, x: usize) -> usize {
        frame
            .cells
            .iter()
            .enumerate()
            .filter(|(index, cell)| index % frame.width == x && cell.head_brightness_bucket > 0)
            .map(|(index, _)| index / frame.width)
            .next()
            .unwrap()
    }

    fn forward_distance(previous: usize, current: usize, height: usize) -> usize {
        if current >= previous {
            current - previous
        } else {
            height - previous + current
        }
    }

    fn blank_cell() -> RenderCell {
        RenderCell {
            glyph: ' ',
            color_hotness_bucket: 0,
            message_color_bucket: 0,
            brightness_bucket: 0,
            head_brightness_bucket: 0,
            ember_brightness_bucket: 0,
            ember_color_hotness_bucket: 0,
        }
    }

    fn rendered_message_chars(frame: &Frame) -> String {
        frame
            .cells
            .iter()
            .filter(|cell| cell.head_brightness_bucket > 0)
            .map(|cell| cell.glyph)
            .collect()
    }

    fn fade_out_glitch_count(message: &MessageOverlay) -> usize {
        message
            .text
            .chars()
            .enumerate()
            .filter(|(offset, target)| {
                let char_hash = mix_hash(message.seed ^ (*offset as u64).wrapping_mul(0x9e37_79b9));
                let Some((glyph, _)) =
                    message_glyph_and_brightness(message, *offset, *target, char_hash, 0.0)
                else {
                    return false;
                };
                glyph != *target
            })
            .count()
    }
}
