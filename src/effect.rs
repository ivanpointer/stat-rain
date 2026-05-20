#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EffectState {
    pub speed: f64,
    pub density: f64,
    pub color_hotness: f64,
    pub brightness: f64,
    pub fade_length: f64,
    pub glyph_churn: f64,
    pub message_reveal_intensity: f64,
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
            fade_length: 8.0,
            glyph_churn: 0.25,
            message_reveal_intensity: 0.0,
            glyph_set: GlyphSet::Unicode,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderCell {
    pub glyph: char,
    pub color_hotness_bucket: u8,
    pub brightness_bucket: u8,
    pub head_brightness_bucket: u8,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Frame {
    pub width: usize,
    pub height: usize,
    pub cells: Vec<RenderCell>,
}

#[derive(Debug, Clone)]
pub struct RainEngine {
    width: usize,
    height: usize,
    tick: u64,
    columns: Vec<ColumnState>,
}

impl RainEngine {
    pub fn new(width: usize, height: usize, seed: u64) -> Self {
        let mut rng = Lcg::new(seed);
        let columns = (0..width)
            .map(|_| ColumnState {
                phase: rng.next_usize(height.max(1)) as f64,
                speed_scale: 0.65 + rng.next_usize(70) as f64 / 100.0,
                seed: rng.next(),
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
                let head = column.phase as usize % self.height.max(1);
                let distance = if y <= head {
                    head - y
                } else {
                    self.height + head - y
                };
                let trail = (1.0 - distance as f64 / fade_length).clamp(0.0, 1.0);
                let column_enabled = density >= self.column_noise(x);
                let rain_brightness = if column_enabled { trail } else { 0.0 } * state.brightness;
                let ambient_brightness = if rain_brightness == 0.0 && density > 0.0 {
                    self.ambient_brightness(x, y, state.brightness)
                } else {
                    0.0
                };
                let brightness = rain_brightness.max(ambient_brightness);
                let brightness_bucket = bucket(brightness);
                let head_brightness_bucket = if column_enabled && distance == 0 {
                    bucket(state.brightness)
                } else {
                    0
                };
                let glyph = if brightness_bucket == 0 {
                    ' '
                } else {
                    let local_churn =
                        (glyph_churn + trail * 0.75 + ambient_brightness).clamp(0.0, 1.0);
                    self.glyph_for(x, y, local_churn, state.glyph_set)
                };

                cells.push(RenderCell {
                    glyph,
                    color_hotness_bucket: hotness,
                    brightness_bucket,
                    head_brightness_bucket,
                });
            }
        }

        self.tick = self.tick.wrapping_add(1);
        for column in &mut self.columns {
            column.phase = (column.phase + speed * column.speed_scale) % self.height.max(1) as f64;
        }

        Frame {
            width: self.width,
            height: self.height,
            cells,
        }
    }

    fn column_noise(&self, x: usize) -> f64 {
        (((x as u64 * 1_103_515_245 + self.columns[x].seed) >> 16) & 0xff) as f64 / 255.0
    }

    fn glyph_for(&self, x: usize, y: usize, glyph_churn: f64, glyph_set: GlyphSet) -> char {
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
        let index = (x as u64 * 17 + y as u64 * 31 + self.tick * churn + self.columns[x].seed)
            as usize
            % glyphs.len();
        glyphs[index]
    }

    fn ambient_brightness(&self, x: usize, y: usize, brightness: f64) -> f64 {
        let hash = self.cell_hash(x, y);
        let phase = (self.tick.wrapping_add(hash & 0x3f)) & 0x3f;
        let intensity = match phase {
            0 => 0.34,
            1 => 0.22,
            2 => 0.12,
            _ => 0.0,
        };
        intensity * brightness
    }

    fn cell_hash(&self, x: usize, y: usize) -> u64 {
        self.columns[x].seed
            ^ (x as u64).wrapping_mul(0x9e37_79b9_7f4a_7c15)
            ^ (y as u64).wrapping_mul(0xbf58_476d_1ce4_e5b9)
    }
}

#[derive(Debug, Clone)]
struct ColumnState {
    phase: f64,
    speed_scale: f64,
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
            fade_length: 1.0,
            brightness: 1.0,
            speed: 0.0,
            ..EffectState::default()
        };

        let frame = engine.step(state);
        let pop_glyphs = frame
            .cells
            .iter()
            .filter(|cell| cell.brightness_bucket > 0 && cell.head_brightness_bucket == 0)
            .count();

        assert!(pop_glyphs > 0);
        assert!(pop_glyphs < frame.cells.len() / 5);
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

        assert_eq!(head_cells, 1);
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
}
