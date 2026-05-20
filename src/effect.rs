#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EffectState {
    pub speed: f64,
    pub density: f64,
    pub color_hotness: f64,
    pub brightness: f64,
    pub fade_length: f64,
    pub glyph_churn: f64,
    pub message_reveal_intensity: f64,
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
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderCell {
    pub glyph: char,
    pub color_hotness_bucket: u8,
    pub brightness_bucket: u8,
}
