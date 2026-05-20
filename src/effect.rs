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
    phase: f64,
    rng: Lcg,
    column_offsets: Vec<usize>,
}

impl RainEngine {
    pub fn new(width: usize, height: usize, seed: u64) -> Self {
        let mut rng = Lcg::new(seed);
        let column_offsets = (0..width)
            .map(|_| rng.next_usize(height.max(1)))
            .collect::<Vec<_>>();

        Self {
            width,
            height,
            tick: 0,
            phase: 0.0,
            rng,
            column_offsets,
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
                let head = (self.phase as usize + self.column_offsets[x]) % self.height.max(1);
                let distance = if y <= head {
                    head - y
                } else {
                    self.height + head - y
                };
                let trail = (1.0 - distance as f64 / fade_length).clamp(0.0, 1.0);
                let column_enabled = density >= self.column_noise(x);
                let brightness = if column_enabled { trail } else { 0.0 } * state.brightness;
                let brightness_bucket = bucket(brightness);
                let glyph = if brightness_bucket == 0 {
                    ' '
                } else {
                    self.glyph_for(x, y, glyph_churn, state.glyph_set)
                };

                cells.push(RenderCell {
                    glyph,
                    color_hotness_bucket: hotness,
                    brightness_bucket,
                });
            }
        }

        self.tick = self.tick.wrapping_add(1);
        self.phase = (self.phase + speed) % self.height.max(1) as f64;

        Frame {
            width: self.width,
            height: self.height,
            cells,
        }
    }

    fn column_noise(&self, x: usize) -> f64 {
        (((x as u64 * 1_103_515_245 + self.rng.seed) >> 16) & 0xff) as f64 / 255.0
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
        let index = (x as u64 * 17 + y as u64 * 31 + self.tick * churn + self.rng.seed) as usize
            % glyphs.len();
        glyphs[index]
    }
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

    fn brightest_row(frame: &Frame) -> usize {
        frame
            .cells
            .iter()
            .enumerate()
            .max_by_key(|(_, cell)| cell.brightness_bucket)
            .map(|(index, _)| index / frame.width)
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
