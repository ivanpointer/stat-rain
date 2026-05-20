use crate::effect::{Frame, RenderCell};
use std::io::{Result, Write};
use std::mem::MaybeUninit;
use std::os::fd::RawFd;

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TerminalSize {
    pub width: usize,
    pub height: usize,
}

impl TerminalSize {
    pub const DEFAULT: Self = Self {
        width: 80,
        height: 24,
    };
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

pub fn write_enter(mut output: impl Write, alternate_screen: bool) -> Result<()> {
    if alternate_screen {
        write!(output, "\x1b[?1049h")?;
    }
    write!(output, "\x1b[?25l")?;
    output.flush()
}

pub fn write_exit(mut output: impl Write, alternate_screen: bool) -> Result<()> {
    write!(output, "\x1b[?25h")?;
    if alternate_screen {
        write!(output, "\x1b[?1049l")?;
    }
    output.flush()
}

pub fn write_clear(mut output: impl Write) -> Result<()> {
    write!(output, "\x1b[2J\x1b[H")?;
    output.flush()
}

pub fn write_frame(mut output: impl Write, frame: &Frame, color_mode: ColorMode) -> Result<()> {
    let mut renderer = FrameRenderer::new(color_mode);
    renderer.write_frame(&mut output, frame)
}

#[derive(Debug, Clone)]
pub struct FrameRenderer {
    color_mode: ColorMode,
    previous: Option<Frame>,
}

impl FrameRenderer {
    pub fn new(color_mode: ColorMode) -> Self {
        Self {
            color_mode,
            previous: None,
        }
    }

    pub fn clear(&mut self) {
        self.previous = None;
    }

    pub fn write_frame(&mut self, mut output: impl Write, frame: &Frame) -> Result<()> {
        let previous = self.previous.as_ref();

        for y in 0..frame.height {
            for x in 0..frame.width {
                let index = y * frame.width + x;
                let cell = &frame.cells[index];
                if previous
                    .and_then(|previous| previous.cells.get(index))
                    .is_some_and(|previous_cell| previous_cell == cell)
                {
                    continue;
                }
                write_cell(&mut output, x, y, cell, self.color_mode)?;
            }
        }

        self.previous = Some(frame.clone());
        write!(output, "\x1b[0m")?;
        output.flush()
    }
}

fn write_cell(
    mut output: impl Write,
    x: usize,
    y: usize,
    cell: &RenderCell,
    color_mode: ColorMode,
) -> Result<()> {
    write!(output, "\x1b[{};{}H", y + 1, x + 1)?;
    write_color(&mut output, cell, color_mode)?;
    write!(output, "{}", cell.glyph)
}

pub fn write_full_frame(
    mut output: impl Write,
    frame: &Frame,
    color_mode: ColorMode,
) -> Result<()> {
    for y in 0..frame.height {
        for x in 0..frame.width {
            let index = y * frame.width + x;
            let cell = &frame.cells[index];
            write_cell(&mut output, x, y, cell, color_mode)?;
        }
    }
    write!(output, "\x1b[0m")?;
    output.flush()
}

pub fn resolve_terminal_size(
    width: Option<usize>,
    height: Option<usize>,
    fallback: TerminalSize,
) -> TerminalSize {
    TerminalSize {
        width: width.unwrap_or(fallback.width).max(1),
        height: height.unwrap_or(fallback.height).max(1),
    }
}

pub fn detect_terminal_size() -> Option<TerminalSize> {
    detect_terminal_size_from_fd(libc::STDOUT_FILENO)
}

pub fn detect_terminal_size_from_fd(fd: RawFd) -> Option<TerminalSize> {
    let mut winsize = MaybeUninit::<libc::winsize>::zeroed();
    let result = unsafe { libc::ioctl(fd, libc::TIOCGWINSZ, winsize.as_mut_ptr()) };
    if result != 0 {
        return None;
    }

    let winsize = unsafe { winsize.assume_init() };
    if winsize.ws_col == 0 || winsize.ws_row == 0 {
        return None;
    }

    Some(TerminalSize {
        width: winsize.ws_col as usize,
        height: winsize.ws_row as usize,
    })
}

fn write_color(mut output: impl Write, cell: &RenderCell, color_mode: ColorMode) -> Result<()> {
    match color_mode {
        ColorMode::TrueColor => {
            let hot = cell.color_hotness_bucket;
            let green = cell.brightness_bucket;
            let red = hot.max(green / 3);
            let blue = green / 5;
            write!(output, "\x1b[38;2;{red};{green};{blue}m")
        }
        ColorMode::Ansi256 => {
            let color = if cell.color_hotness_bucket > 170 {
                196
            } else if cell.color_hotness_bucket > 84 {
                226
            } else {
                46
            };
            write!(output, "\x1b[38;5;{color}m")
        }
        ColorMode::Ansi16 => {
            let color = if cell.color_hotness_bucket > 170 {
                31
            } else if cell.color_hotness_bucket > 84 {
                33
            } else {
                32
            };
            write!(output, "\x1b[{color}m")
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

    #[test]
    fn lifecycle_writes_alternate_screen_and_cursor_sequences() {
        let mut output = Vec::new();

        write_enter(&mut output, true).unwrap();
        write_exit(&mut output, true).unwrap();

        assert_eq!(
            String::from_utf8(output).unwrap(),
            "\x1b[?1049h\x1b[?25l\x1b[?25h\x1b[?1049l"
        );
    }

    #[test]
    fn frame_renderer_writes_positioned_cells() {
        let frame = Frame {
            width: 2,
            height: 1,
            cells: vec![
                RenderCell {
                    glyph: '0',
                    color_hotness_bucket: 0,
                    brightness_bucket: 255,
                },
                RenderCell {
                    glyph: '1',
                    color_hotness_bucket: 255,
                    brightness_bucket: 128,
                },
            ],
        };
        let mut output = Vec::new();

        write_frame(&mut output, &frame, ColorMode::Ansi16).unwrap();

        let rendered = String::from_utf8(output).unwrap();
        assert!(rendered.contains("\x1b[1;1H"));
        assert!(rendered.contains('0'));
        assert!(rendered.contains("\x1b[1;2H"));
        assert!(rendered.contains('1'));
    }

    #[test]
    fn frame_renderer_skips_unchanged_cells_after_first_frame() {
        let frame = Frame {
            width: 2,
            height: 1,
            cells: vec![
                RenderCell {
                    glyph: '0',
                    color_hotness_bucket: 0,
                    brightness_bucket: 255,
                },
                RenderCell {
                    glyph: '1',
                    color_hotness_bucket: 0,
                    brightness_bucket: 255,
                },
            ],
        };
        let mut renderer = FrameRenderer::new(ColorMode::Ansi16);
        let mut output = Vec::new();

        renderer.write_frame(&mut output, &frame).unwrap();
        output.clear();
        renderer.write_frame(&mut output, &frame).unwrap();

        assert_eq!(String::from_utf8(output).unwrap(), "\x1b[0m");
    }

    #[test]
    fn resolves_terminal_size_from_overrides() {
        let size = resolve_terminal_size(
            Some(100),
            Some(40),
            TerminalSize {
                width: 80,
                height: 24,
            },
        );

        assert_eq!(
            size,
            TerminalSize {
                width: 100,
                height: 40
            }
        );
    }

    #[test]
    fn resolves_terminal_size_from_fallback() {
        let size = resolve_terminal_size(
            None,
            None,
            TerminalSize {
                width: 80,
                height: 24,
            },
        );

        assert_eq!(
            size,
            TerminalSize {
                width: 80,
                height: 24
            }
        );
    }
}
