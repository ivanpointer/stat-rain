use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VisualAttribute {
    Speed,
    Density,
    ColorHotness,
    Brightness,
    FadeLength,
    GlyphChurn,
    MessageRevealIntensity,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MappingExpression(String);

impl MappingExpression {
    pub fn new(value: impl Into<String>) -> Result<Self, MappingError> {
        let value = value.into();
        if value.trim().is_empty() {
            return Err(MappingError::EmptyExpression);
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MappingError {
    EmptyExpression,
}

impl fmt::Display for MappingError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyExpression => write!(f, "mapping expression cannot be empty"),
        }
    }
}

impl std::error::Error for MappingError {}

impl FromStr for VisualAttribute {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "speed" => Ok(Self::Speed),
            "density" => Ok(Self::Density),
            "color_hotness" => Ok(Self::ColorHotness),
            "brightness" => Ok(Self::Brightness),
            "fade_length" => Ok(Self::FadeLength),
            "glyph_churn" => Ok(Self::GlyphChurn),
            "message_reveal_intensity" => Ok(Self::MessageRevealIntensity),
            _ => Err(format!("unknown visual attribute: {value}")),
        }
    }
}
