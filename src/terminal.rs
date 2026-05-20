#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorMode {
    TrueColor,
    Ansi256,
    Ansi16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GlyphMode {
    Unicode,
    Ascii,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TerminalCapabilities {
    pub color_mode: ColorMode,
    pub glyph_mode: GlyphMode,
    pub alternate_screen: bool,
    pub tmux: bool,
}

impl TerminalCapabilities {
    pub fn detect_from_env(
        term: Option<&str>,
        colorterm: Option<&str>,
        tmux: Option<&str>,
    ) -> Self {
        let color_mode = match (colorterm, term) {
            (Some(value), _)
                if value.eq_ignore_ascii_case("truecolor")
                    || value.eq_ignore_ascii_case("24bit") =>
            {
                ColorMode::TrueColor
            }
            (_, Some(value)) if value.contains("256color") => ColorMode::Ansi256,
            _ => ColorMode::Ansi16,
        };

        Self {
            color_mode,
            glyph_mode: GlyphMode::Unicode,
            alternate_screen: true,
            tmux: tmux.is_some(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_truecolor() {
        let caps =
            TerminalCapabilities::detect_from_env(Some("xterm-256color"), Some("truecolor"), None);

        assert_eq!(caps.color_mode, ColorMode::TrueColor);
        assert!(!caps.tmux);
    }

    #[test]
    fn detects_tmux_and_256_color() {
        let caps =
            TerminalCapabilities::detect_from_env(Some("screen-256color"), None, Some("/tmp/tmux"));

        assert_eq!(caps.color_mode, ColorMode::Ansi256);
        assert!(caps.tmux);
    }
}
